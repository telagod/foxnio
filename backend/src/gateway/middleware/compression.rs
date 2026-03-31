//! 压缩中间件 - 请求/响应压缩支持
//!
//! 功能:
//! - 自动内容协商 (Accept-Encoding)
//! - 支持 gzip 和 brotli 压缩
//! - 流式压缩支持
//! - 请求解压缩
//! - 压缩统计
//!
//! 注意：部分统计和扩展功能正在开发中，暂未完全使用

#![allow(dead_code)]

use axum::{
    body::Body,
    http::{
        header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE},
        HeaderMap, HeaderValue, Request, Response,
    },
    middleware::Next,
};
use brotli::{CompressorReader, Decompressor};
use bytes::{Bytes, BytesMut};
use flate2::{read::GzDecoder, write::GzEncoder, Compression as GzCompression};
use std::sync::Arc;
use std::{
    io::{Read, Write},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

/// 压缩级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionLevel {
    /// 快速压缩 - 低 CPU 使用率，较低压缩率
    Fast,
    /// 默认压缩 - 平衡 CPU 和压缩率
    #[default]
    Default,
    /// 最佳压缩 - 高 CPU 使用率，最高压缩率
    Best,
}

impl From<CompressionLevel> for GzCompression {
    fn from(level: CompressionLevel) -> Self {
        match level {
            CompressionLevel::Fast => GzCompression::fast(),
            CompressionLevel::Default => GzCompression::default(),
            CompressionLevel::Best => GzCompression::best(),
        }
    }
}

impl From<CompressionLevel> for u32 {
    /// 转换为 brotli 压缩级别 (0-11)
    fn from(level: CompressionLevel) -> Self {
        match level {
            CompressionLevel::Fast => 1,
            CompressionLevel::Default => 6,
            CompressionLevel::Best => 11,
        }
    }
}

/// 内容编码类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ContentEncoding {
    /// 无压缩
    #[default]
    Identity,
    /// Gzip 压缩
    Gzip,
    /// Brotli 压缩
    Brotli,
}

impl std::fmt::Display for ContentEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentEncoding::Identity => write!(f, "identity"),
            ContentEncoding::Gzip => write!(f, "gzip"),
            ContentEncoding::Brotli => write!(f, "br"),
        }
    }
}

impl std::str::FromStr for ContentEncoding {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "gzip" | "x-gzip" => Ok(Self::Gzip),
            "br" | "brotli" => Ok(Self::Brotli),
            "identity" | "" => Ok(Self::Identity),
            _ => Err(anyhow::anyhow!("Unknown content encoding: {}", s)),
        }
    }
}

/// 压缩响应包装器
#[derive(Debug, Clone)]
pub struct CompressedResponse {
    /// 压缩后的响应体
    pub body: Bytes,
    /// 使用的编码
    pub encoding: ContentEncoding,
    /// 原始大小
    pub original_size: usize,
    /// 压缩后大小
    pub compressed_size: usize,
    /// 压缩耗时 (毫秒)
    pub compression_time_ms: u64,
}

impl CompressedResponse {
    /// 计算压缩率
    pub fn compression_ratio(&self) -> f64 {
        if self.original_size == 0 {
            return 0.0;
        }
        let ratio = 1.0 - (self.compressed_size as f64 / self.original_size as f64);
        (ratio * 100.0).round() / 100.0
    }

    /// 节省的字节数
    pub fn bytes_saved(&self) -> usize {
        self.original_size.saturating_sub(self.compressed_size)
    }
}

/// 压缩层配置
#[derive(Debug, Clone)]
pub struct CompressionLayer {
    /// 是否启用 gzip
    pub gzip_enabled: bool,
    /// 是否启用 brotli
    pub brotli_enabled: bool,
    /// 最小压缩大小 (字节)
    pub min_size: usize,
    /// 压缩级别
    pub level: CompressionLevel,
    /// 压缩统计
    stats: Arc<CompressionStats>,
}

impl Default for CompressionLayer {
    fn default() -> Self {
        Self {
            gzip_enabled: true,
            brotli_enabled: true,
            min_size: 1024, // 1KB
            level: CompressionLevel::Default,
            stats: Arc::new(CompressionStats::new()),
        }
    }
}

impl CompressionLayer {
    /// 创建新的压缩层
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 gzip 启用状态
    pub fn gzip(mut self, enabled: bool) -> Self {
        self.gzip_enabled = enabled;
        self
    }

    /// 设置 brotli 启用状态
    pub fn brotli(mut self, enabled: bool) -> Self {
        self.brotli_enabled = enabled;
        self
    }

    /// 设置最小压缩大小
    pub fn min_size(mut self, size: usize) -> Self {
        self.min_size = size;
        self
    }

    /// 设置压缩级别
    pub fn level(mut self, level: CompressionLevel) -> Self {
        self.level = level;
        self
    }

    /// 获取压缩统计
    pub fn stats(&self) -> &Arc<CompressionStats> {
        &self.stats
    }

    /// 根据客户端 Accept-Encoding 选择最佳编码
    pub fn select_encoding(&self, accept_encoding: Option<&str>) -> ContentEncoding {
        let header = match accept_encoding {
            Some(h) => h,
            None => return ContentEncoding::Identity,
        };

        // 解析 Accept-Encoding 头
        // 格式: gzip, deflate, br;q=0.9
        let mut best_encoding = ContentEncoding::Identity;
        let mut best_quality: f32 = 0.0;

        for part in header.split(',') {
            let part = part.trim();
            let (encoding, quality) = Self::parse_encoding_part(part);

            // 优先级: brotli > gzip > identity
            let can_use = match encoding {
                ContentEncoding::Brotli => self.brotli_enabled,
                ContentEncoding::Gzip => self.gzip_enabled,
                ContentEncoding::Identity => true,
            };

            // 当质量相同时，优先选择 brotli
            if can_use {
                let quality_matches = (quality - best_quality).abs() < f32::EPSILON;
                let should_select =
                    quality > best_quality || (quality_matches && encoding > best_encoding);
                if should_select {
                    best_quality = quality;
                    best_encoding = encoding;
                }
            }
        }

        // 如果客户端没有明确偏好，按优先级选择
        if best_encoding == ContentEncoding::Identity && best_quality == 0.0 {
            if self.brotli_enabled {
                return ContentEncoding::Brotli;
            }
            if self.gzip_enabled {
                return ContentEncoding::Gzip;
            }
        }

        best_encoding
    }

    /// 解析编码部分
    fn parse_encoding_part(part: &str) -> (ContentEncoding, f32) {
        let parts: Vec<&str> = part.split(';').collect();
        let encoding = parts[0].trim();

        let quality = if parts.len() > 1 {
            parts[1]
                .trim()
                .strip_prefix("q=")
                .and_then(|q| q.parse::<f32>().ok())
                .unwrap_or(1.0)
        } else {
            1.0
        };

        let encoding = match encoding.to_lowercase().as_str() {
            "gzip" | "x-gzip" => ContentEncoding::Gzip,
            "br" | "brotli" => ContentEncoding::Brotli,
            "*" => {
                // 通配符，默认返回最高优先级
                if quality > 0.0 {
                    return (ContentEncoding::Brotli, quality);
                }
                ContentEncoding::Identity
            }
            _ => ContentEncoding::Identity,
        };

        (encoding, quality)
    }

    /// 压缩数据
    pub fn compress(
        &self,
        data: &[u8],
        encoding: ContentEncoding,
    ) -> anyhow::Result<CompressedResponse> {
        let original_size = data.len();

        // 检查是否需要压缩
        if original_size < self.min_size || encoding == ContentEncoding::Identity {
            return Ok(CompressedResponse {
                body: Bytes::copy_from_slice(data),
                encoding: ContentEncoding::Identity,
                original_size,
                compressed_size: original_size,
                compression_time_ms: 0,
            });
        }

        let start = Instant::now();

        let (body, encoding) = match encoding {
            ContentEncoding::Gzip => {
                let compressed = self.compress_gzip(data)?;
                (compressed, ContentEncoding::Gzip)
            }
            ContentEncoding::Brotli => {
                let compressed = self.compress_brotli(data)?;
                (compressed, ContentEncoding::Brotli)
            }
            ContentEncoding::Identity => (Bytes::copy_from_slice(data), ContentEncoding::Identity),
        };

        let compressed_size = body.len();
        let compression_time_ms = start.elapsed().as_millis() as u64;

        // 更新统计
        self.stats
            .record_compression(original_size, compressed_size, compression_time_ms);

        Ok(CompressedResponse {
            body,
            encoding,
            original_size,
            compressed_size,
            compression_time_ms,
        })
    }

    /// Gzip 压缩
    fn compress_gzip(&self, data: &[u8]) -> anyhow::Result<Bytes> {
        let mut encoder = GzEncoder::new(Vec::new(), self.level.into());
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;
        Ok(Bytes::from(compressed))
    }

    /// Brotli 压缩
    fn compress_brotli(&self, data: &[u8]) -> anyhow::Result<Bytes> {
        let mut compressed = Vec::new();
        {
            let mut encoder = CompressorReader::new(
                data,
                4096,
                self.level.into(),
                22, // lgwin (window size)
            );
            std::io::copy(&mut encoder, &mut compressed)?;
        }
        Ok(Bytes::from(compressed))
    }

    /// 解压缩请求体
    pub fn decompress(&self, data: &[u8], encoding: ContentEncoding) -> anyhow::Result<Bytes> {
        let start = Instant::now();
        let original_size = data.len();

        let decompressed = match encoding {
            ContentEncoding::Gzip => self.decompress_gzip(data)?,
            ContentEncoding::Brotli => self.decompress_brotli(data)?,
            ContentEncoding::Identity => Bytes::copy_from_slice(data),
        };

        let decompression_time_ms = start.elapsed().as_millis() as u64;

        // 更新统计
        self.stats
            .record_decompression(original_size, decompressed.len(), decompression_time_ms);

        Ok(decompressed)
    }

    /// Gzip 解压缩
    fn decompress_gzip(&self, data: &[u8]) -> anyhow::Result<Bytes> {
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(Bytes::from(decompressed))
    }

    /// Brotli 解压缩
    fn decompress_brotli(&self, data: &[u8]) -> anyhow::Result<Bytes> {
        let mut decoder = Decompressor::new(data, 4096);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(Bytes::from(decompressed))
    }
}

/// 压缩统计
#[derive(Debug, Default)]
pub struct CompressionStats {
    /// 压缩请求总数
    compress_count: AtomicU64,
    /// 解压缩请求总数
    decompress_count: AtomicU64,
    /// 原始数据总大小
    total_original_size: AtomicU64,
    /// 压缩后总大小
    total_compressed_size: AtomicU64,
    /// 总压缩时间 (毫秒)
    total_compression_time_ms: AtomicU64,
    /// 总解压缩时间 (毫秒)
    total_decompression_time_ms: AtomicU64,
}

impl CompressionStats {
    fn new() -> Self {
        Self::default()
    }

    /// 记录压缩操作
    fn record_compression(&self, original_size: usize, compressed_size: usize, time_ms: u64) {
        self.compress_count.fetch_add(1, Ordering::Relaxed);
        self.total_original_size
            .fetch_add(original_size as u64, Ordering::Relaxed);
        self.total_compressed_size
            .fetch_add(compressed_size as u64, Ordering::Relaxed);
        self.total_compression_time_ms
            .fetch_add(time_ms, Ordering::Relaxed);
    }

    /// 记录解压缩操作
    fn record_decompression(&self, compressed_size: usize, decompressed_size: usize, time_ms: u64) {
        self.decompress_count.fetch_add(1, Ordering::Relaxed);
        self.total_compressed_size
            .fetch_add(compressed_size as u64, Ordering::Relaxed);
        self.total_original_size
            .fetch_add(decompressed_size as u64, Ordering::Relaxed);
        self.total_decompression_time_ms
            .fetch_add(time_ms, Ordering::Relaxed);
    }

    /// 获取统计快照
    pub fn snapshot(&self) -> StatsSnapshot {
        let compress_count = self.compress_count.load(Ordering::Relaxed);
        let decompress_count = self.decompress_count.load(Ordering::Relaxed);
        let total_original = self.total_original_size.load(Ordering::Relaxed);
        let total_compressed = self.total_compressed_size.load(Ordering::Relaxed);
        let compression_time = self.total_compression_time_ms.load(Ordering::Relaxed);
        let decompression_time = self.total_decompression_time_ms.load(Ordering::Relaxed);

        let compression_ratio = if total_original > 0 {
            1.0 - (total_compressed as f64 / total_original as f64)
        } else {
            0.0
        };

        let avg_compression_time = if compress_count > 0 {
            compression_time as f64 / compress_count as f64
        } else {
            0.0
        };

        let avg_decompression_time = if decompress_count > 0 {
            decompression_time as f64 / decompress_count as f64
        } else {
            0.0
        };

        StatsSnapshot {
            compress_count,
            decompress_count,
            total_original_size: total_original,
            total_compressed_size: total_compressed,
            bytes_saved: total_original.saturating_sub(total_compressed),
            compression_ratio,
            avg_compression_time_ms: avg_compression_time,
            avg_decompression_time_ms: avg_decompression_time,
        }
    }

    /// 重置统计
    pub fn reset(&self) {
        self.compress_count.store(0, Ordering::Relaxed);
        self.decompress_count.store(0, Ordering::Relaxed);
        self.total_original_size.store(0, Ordering::Relaxed);
        self.total_compressed_size.store(0, Ordering::Relaxed);
        self.total_compression_time_ms.store(0, Ordering::Relaxed);
        self.total_decompression_time_ms.store(0, Ordering::Relaxed);
    }
}

/// 统计快照
#[derive(Debug, Clone)]
pub struct StatsSnapshot {
    /// 压缩次数
    pub compress_count: u64,
    /// 解压缩次数
    pub decompress_count: u64,
    /// 原始数据总大小
    pub total_original_size: u64,
    /// 压缩数据总大小
    pub total_compressed_size: u64,
    /// 节省的字节数
    pub bytes_saved: u64,
    /// 平均压缩率
    pub compression_ratio: f64,
    /// 平均压缩时间 (毫秒)
    pub avg_compression_time_ms: f64,
    /// 平均解压缩时间 (毫秒)
    pub avg_decompression_time_ms: f64,
}

impl std::fmt::Display for StatsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Compression Statistics:")?;
        writeln!(f, "  Compress count: {}", self.compress_count)?;
        writeln!(f, "  Decompress count: {}", self.decompress_count)?;
        writeln!(
            f,
            "  Total original size: {} bytes ({:.2} MB)",
            self.total_original_size,
            self.total_original_size as f64 / 1_048_576.0
        )?;
        writeln!(
            f,
            "  Total compressed size: {} bytes ({:.2} MB)",
            self.total_compressed_size,
            self.total_compressed_size as f64 / 1_048_576.0
        )?;
        writeln!(
            f,
            "  Bytes saved: {} bytes ({:.2} MB)",
            self.bytes_saved,
            self.bytes_saved as f64 / 1_048_576.0
        )?;
        writeln!(
            f,
            "  Compression ratio: {:.2}%",
            self.compression_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Avg compression time: {:.2} ms",
            self.avg_compression_time_ms
        )?;
        writeln!(
            f,
            "  Avg decompression time: {:.2} ms",
            self.avg_decompression_time_ms
        )
    }
}

/// 流式压缩读取器
pub struct StreamingCompressor {
    encoding: ContentEncoding,
    level: CompressionLevel,
    buffer: BytesMut,
}

impl StreamingCompressor {
    pub fn new(encoding: ContentEncoding, level: CompressionLevel) -> Self {
        Self {
            encoding,
            level,
            buffer: BytesMut::with_capacity(8192),
        }
    }

    /// 压缩数据块
    pub fn compress_chunk(&mut self, chunk: &[u8]) -> anyhow::Result<Bytes> {
        match self.encoding {
            ContentEncoding::Gzip => {
                let mut encoder = GzEncoder::new(Vec::new(), self.level.into());
                encoder.write_all(chunk)?;
                let compressed = encoder.finish()?;
                Ok(Bytes::from(compressed))
            }
            ContentEncoding::Brotli => {
                let mut compressed = Vec::new();
                {
                    let mut encoder = CompressorReader::new(chunk, 4096, self.level.into(), 22);
                    std::io::copy(&mut encoder, &mut compressed)?;
                }
                Ok(Bytes::from(compressed))
            }
            ContentEncoding::Identity => Ok(Bytes::copy_from_slice(chunk)),
        }
    }
}

/// 从请求头获取 Content-Encoding
pub fn get_content_encoding(headers: &HeaderMap) -> ContentEncoding {
    headers
        .get(CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(ContentEncoding::Identity)
}

/// 从请求头获取 Accept-Encoding
pub fn get_accept_encoding(headers: &HeaderMap) -> Option<&str> {
    headers.get(ACCEPT_ENCODING).and_then(|v| v.to_str().ok())
}

/// 检查内容类型是否适合压缩
pub fn should_compress(headers: &HeaderMap) -> bool {
    // 检查 Content-Type
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // 可压缩的类型
    let compressible_types = [
        "text/",
        "application/json",
        "application/javascript",
        "application/xml",
        "application/x-javascript",
        "application/xhtml+xml",
    ];

    // 不应压缩的类型 (已压缩格式)
    let non_compressible = [
        "image/",
        "video/",
        "audio/",
        "application/zip",
        "application/gzip",
        "application/x-gzip",
        "application/x-brotli",
        "application/pdf",
    ];

    // 检查是否为已压缩格式
    if non_compressible.iter().any(|t| content_type.starts_with(t)) {
        return false;
    }

    // 检查是否为可压缩类型
    compressible_types
        .iter()
        .any(|t| content_type.starts_with(t))
}

/// 响应压缩中间件
pub async fn compression_middleware(req: Request<Body>, next: Next) -> Response<Body> {
    let compression_layer = CompressionLayer::new();

    // 获取 Accept-Encoding
    let accept_encoding = get_accept_encoding(req.headers());
    let encoding = compression_layer.select_encoding(accept_encoding);

    // 执行请求
    let response = next.run(req).await;

    // 检查是否需要压缩响应
    if encoding == ContentEncoding::Identity || !should_compress(response.headers()) {
        return response;
    }

    // 获取响应体
    let (parts, body) = response.into_parts();
    let body_bytes = match axum::body::to_bytes(body, 1024 * 1024 * 10).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };

    // 检查大小
    if body_bytes.len() < compression_layer.min_size {
        let builder = Response::from_parts(parts, Body::from(body_bytes));
        return builder;
    }

    // 压缩响应
    let compressed = match compression_layer.compress(&body_bytes, encoding) {
        Ok(c) => c,
        Err(_) => return Response::from_parts(parts, Body::from(body_bytes)),
    };

    // 构建新响应
    let mut builder = Response::builder()
        .status(parts.status)
        .version(parts.version);

    // 复制头部
    if let Some(headers) = builder.headers_mut() {
        for (name, value) in parts.headers.iter() {
            // 跳过 Content-Length 和 Content-Encoding
            if name != CONTENT_LENGTH && name != CONTENT_ENCODING {
                headers.insert(name.clone(), value.clone());
            }
        }

        // 添加 Content-Encoding
        headers.insert(
            CONTENT_ENCODING,
            HeaderValue::from_str(&compressed.encoding.to_string()).unwrap(),
        );

        // 更新 Content-Length
        headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from(compressed.compressed_size),
        );
    }

    // 添加压缩统计扩展
    if let Some(extensions) = builder.extensions_mut() {
        extensions.insert(compressed.clone());
    }

    builder.body(Body::from(compressed.body)).unwrap()
}

/// 请求解压缩中间件
pub async fn decompression_middleware(req: Request<Body>, next: Next) -> Response<Body> {
    let compression_layer = CompressionLayer::new();

    // 获取 Content-Encoding
    let encoding = get_content_encoding(req.headers());

    if encoding == ContentEncoding::Identity {
        return next.run(req).await;
    }

    // 解压缩请求体
    let (parts, body) = req.into_parts();
    let body_bytes = match axum::body::to_bytes(body, 1024 * 1024 * 10).await {
        Ok(b) => b,
        Err(_) => {
            return Response::builder()
                .status(400)
                .body(Body::from("Failed to read request body"))
                .unwrap()
        }
    };

    let decompressed = match compression_layer.decompress(&body_bytes, encoding) {
        Ok(d) => d,
        Err(e) => {
            return Response::builder()
                .status(400)
                .body(Body::from(format!("Decompression failed: {e}")))
                .unwrap()
        }
    };

    // 更新请求
    let mut parts = parts;
    parts.headers.remove(CONTENT_ENCODING);
    parts
        .headers
        .insert(CONTENT_LENGTH, HeaderValue::from(decompressed.len()));

    let new_req = Request::from_parts(parts, Body::from(decompressed));
    next.run(new_req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_compression_level_default() {
        let layer = CompressionLayer::new();
        assert_eq!(layer.level, CompressionLevel::Default);
        assert!(layer.gzip_enabled);
        assert!(layer.brotli_enabled);
        assert_eq!(layer.min_size, 1024);
    }

    #[test]
    fn test_select_encoding_with_accept_header() {
        let layer = CompressionLayer::new();

        // 只支持 gzip
        assert_eq!(layer.select_encoding(Some("gzip")), ContentEncoding::Gzip);

        // 只支持 brotli
        assert_eq!(layer.select_encoding(Some("br")), ContentEncoding::Brotli);

        // 两者都支持，优先 brotli
        assert_eq!(
            layer.select_encoding(Some("gzip, br")),
            ContentEncoding::Brotli
        );

        // 带 quality 值
        assert_eq!(
            layer.select_encoding(Some("gzip, br;q=0.9")),
            ContentEncoding::Gzip
        );

        // 不支持的编码
        assert_eq!(
            layer.select_encoding(Some("deflate")),
            ContentEncoding::Identity
        );
    }

    #[test]
    fn test_compress_gzip() {
        let layer = CompressionLayer::new().brotli(false).min_size(100);

        let data = b"Hello, World! This is a test string that should compress well. ".repeat(10);

        let result = layer.compress(&data, ContentEncoding::Gzip).unwrap();

        assert_eq!(result.encoding, ContentEncoding::Gzip);
        assert!(result.compressed_size < result.original_size);
        assert!(result.compression_ratio() > 0.0);
    }

    #[test]
    fn test_compress_brotli() {
        let layer = CompressionLayer::new().gzip(false).min_size(100);

        let data = b"Hello, World! This is a test string that should compress well. ".repeat(10);

        let result = layer.compress(&data, ContentEncoding::Brotli).unwrap();

        assert_eq!(result.encoding, ContentEncoding::Brotli);
        assert!(result.compressed_size < result.original_size);
        assert!(result.compression_ratio() > 0.0);
    }

    #[test]
    fn test_compress_small_data() {
        let layer = CompressionLayer::new().min_size(1024);

        let data = b"small";

        let result = layer.compress(data, ContentEncoding::Gzip).unwrap();

        // 小于最小大小，不压缩
        assert_eq!(result.encoding, ContentEncoding::Identity);
        assert_eq!(result.original_size, result.compressed_size);
    }

    #[test]
    fn test_decompress_gzip() {
        let layer = CompressionLayer::new().min_size(10);

        let original = b"Hello, World! This is a test string that is long enough.";
        let compressed = layer.compress(original, ContentEncoding::Gzip).unwrap();

        // 确保数据被压缩了
        assert_eq!(compressed.encoding, ContentEncoding::Gzip);

        let decompressed = layer
            .decompress(&compressed.body, ContentEncoding::Gzip)
            .unwrap();

        assert_eq!(decompressed.as_ref(), original);
    }

    #[test]
    fn test_decompress_brotli() {
        let layer = CompressionLayer::new().min_size(10);

        let original = b"Hello, World! This is a test string that is long enough.";
        let compressed = layer.compress(original, ContentEncoding::Brotli).unwrap();

        // 确保数据被压缩了
        assert_eq!(compressed.encoding, ContentEncoding::Brotli);

        let decompressed = layer
            .decompress(&compressed.body, ContentEncoding::Brotli)
            .unwrap();

        assert_eq!(decompressed.as_ref(), original);
    }

    #[test]
    fn test_stats() {
        let layer = CompressionLayer::new().min_size(50);

        // 压缩一些数据 - 确保足够长以触发压缩
        let data = b"Test data for compression statistics that is long enough to trigger compression. We need more data here.";
        layer.compress(data, ContentEncoding::Gzip).unwrap();
        layer.compress(data, ContentEncoding::Brotli).unwrap();

        let stats = layer.stats().snapshot();

        assert_eq!(stats.compress_count, 2);
        assert!(stats.total_original_size > 0);
        assert!(stats.total_compressed_size > 0);
    }

    #[test]
    fn test_compression_ratio() {
        let layer = CompressionLayer::new().min_size(100);

        // 重复数据压缩率高
        let data = b"a".repeat(1000);
        let result = layer.compress(&data, ContentEncoding::Gzip).unwrap();

        // gzip 应该能显著压缩重复数据
        assert!(result.compression_ratio() > 0.5);
    }

    #[test]
    fn test_content_encoding_parse() {
        assert_eq!(
            ContentEncoding::from_str("gzip").unwrap(),
            ContentEncoding::Gzip
        );
        assert_eq!(
            ContentEncoding::from_str("br").unwrap(),
            ContentEncoding::Brotli
        );
        assert_eq!(
            ContentEncoding::from_str("identity").unwrap(),
            ContentEncoding::Identity
        );
        assert!(ContentEncoding::from_str("unknown").is_err());
    }

    #[test]
    fn test_compression_level_conversion() {
        // Gzip 级别
        let _: GzCompression = CompressionLevel::Fast.into();
        let _: GzCompression = CompressionLevel::Default.into();
        let _: GzCompression = CompressionLevel::Best.into();

        // Brotli 级别
        let level: u32 = CompressionLevel::Fast.into();
        assert_eq!(level, 1);

        let level: u32 = CompressionLevel::Default.into();
        assert_eq!(level, 6);

        let level: u32 = CompressionLevel::Best.into();
        assert_eq!(level, 11);
    }
}

/// 性能基准测试
#[cfg(test)]
mod benches {
    use super::*;
    use std::time::Instant;

    #[test]
    fn bench_gzip_compression() {
        let layer = CompressionLayer::new()
            .brotli(false)
            .level(CompressionLevel::Default);

        let data = b"x".repeat(1_000_000); // 1MB

        let start = Instant::now();
        let result = layer.compress(&data, ContentEncoding::Gzip).unwrap();
        let elapsed = start.elapsed();

        println!(
            "Gzip: {} bytes -> {} bytes in {:?} ({:.2}x compression)",
            result.original_size,
            result.compressed_size,
            elapsed,
            result.original_size as f64 / result.compressed_size as f64
        );
    }

    #[test]
    fn bench_brotli_compression() {
        let layer = CompressionLayer::new()
            .gzip(false)
            .level(CompressionLevel::Default);

        let data = b"x".repeat(1_000_000); // 1MB

        let start = Instant::now();
        let result = layer.compress(&data, ContentEncoding::Brotli).unwrap();
        let elapsed = start.elapsed();

        println!(
            "Brotli: {} bytes -> {} bytes in {:?} ({:.2}x compression)",
            result.original_size,
            result.compressed_size,
            elapsed,
            result.original_size as f64 / result.compressed_size as f64
        );
    }

    #[test]
    fn bench_mixed_content() {
        let layer = CompressionLayer::new();

        // JSON-like content
        let json = r#"{"key": "value", "nested": {"a": 1, "b": 2}}"#.repeat(1000);

        let start = Instant::now();
        let gzip_result = layer
            .compress(json.as_bytes(), ContentEncoding::Gzip)
            .unwrap();
        let gzip_time = start.elapsed();

        let start = Instant::now();
        let brotli_result = layer
            .compress(json.as_bytes(), ContentEncoding::Brotli)
            .unwrap();
        let brotli_time = start.elapsed();

        println!("JSON content compression:");
        println!(
            "  Gzip: {} -> {} in {:?}",
            json.len(),
            gzip_result.compressed_size,
            gzip_time
        );
        println!(
            "  Brotli: {} -> {} in {:?}",
            json.len(),
            brotli_result.compressed_size,
            brotli_time
        );
    }
}

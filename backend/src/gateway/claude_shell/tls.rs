// TLS 配置 - 匹配 Node.js 24.x 指纹
// 注：这是可选功能，默认使用系统 TLS

/// TLS 配置
/// 
/// 此模块提供 TLS 指纹配置，使请求看起来像来自 Node.js 24.x
/// 注意：这是一个高级功能，可能需要特殊的 TLS 库支持

#[cfg(feature = "tls-custom")]
pub fn build_tls_config() -> anyhow::Result<reqwest::tls::Config> {
    // TODO: 实现 TLS 指纹配置
    // 这需要使用自定义 TLS 库，如：
    // - rustls 配置
    // - 或使用 native-tls 配置
    
    // 当前返回默认配置
    Ok(reqwest::tls::Config::default())
}

/// Node.js 24.x TLS 密码套件
#[cfg(feature = "tls-custom")]
pub const NODEJS_24_CIPHER_SUITES: &[&str] = &[
    // TLS 1.3
    "TLS_AES_128_GCM_SHA256",
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
    
    // TLS 1.2
    "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
    "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
    "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
    "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
    "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
    "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
    "TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA",
    "TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA",
    "TLS_RSA_WITH_AES_128_GCM_SHA256",
    "TLS_RSA_WITH_AES_256_GCM_SHA384",
    "TLS_RSA_WITH_AES_128_CBC_SHA",
    "TLS_RSA_WITH_AES_256_CBC_SHA",
    "TLS_RSA_WITH_3DES_EDE_CBC_SHA",
];

/// Node.js 24.x 支持的曲线
#[cfg(feature = "tls-custom")]
pub const NODEJS_24_CURVES: &[&str] = &[
    "X25519",
    "P-256",
    "P-384",
];

/// Node.js 24.x TLS 扩展顺序
#[cfg(feature = "tls-custom")]
pub const NODEJS_24_EXTENSIONS: &[&str] = &[
    "server_name",
    "extended_master_secret",
    "max_fragment_length",
    "session_ticket",
    "application_layer_protocol_negotiation",
    "status_request",
    "signed_certificate_timestamp",
    "key_share",
    "psk_key_exchange_modes",
    "pre_shared_key",
    "renegotiation_info",
    "supported_versions",
    "compress_certificate",
    "ec_point_formats",
    "supported_groups",
    "signature_algorithms_cert",
    "signature_algorithms",
    "record_size_limit",
    "padding",
];

#[cfg(test)]
mod tests {
    #[test]
    fn test_cipher_suites_count() {
        #[cfg(feature = "tls-custom")]
        {
            assert_eq!(super::NODEJS_24_CIPHER_SUITES.len(), 15);
        }
    }

    #[test]
    fn test_curves_count() {
        #[cfg(feature = "tls-custom")]
        {
            assert_eq!(super::NODEJS_24_CURVES.len(), 3);
        }
    }
}

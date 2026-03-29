use bytes::Bytes;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Response size limit checker
pub struct UpstreamResponseLimit {
    max_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseLimitConfig {
    pub max_size_mb: u64,
    pub warn_threshold_mb: u64,
}

impl Default for ResponseLimitConfig {
    fn default() -> Self {
        Self {
            max_size_mb: 100,
            warn_threshold_mb: 80,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResponseLimitError {
    #[error("Response size {actual} exceeds limit {limit}")]
    ExceedsLimit { actual: u64, limit: u64 },
}

impl UpstreamResponseLimit {
    pub fn new(max_size_bytes: u64) -> Self {
        Self { max_size_bytes }
    }

    /// Check if size is within limit
    pub fn check(&self, size: u64) -> Result<(), ResponseLimitError> {
        if size > self.max_size_bytes {
            return Err(ResponseLimitError::ExceedsLimit {
                actual: size,
                limit: self.max_size_bytes,
            });
        }
        Ok(())
    }

    /// Create limited stream
    pub fn limit_stream(
        &self,
        stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    ) -> LimitedStream {
        LimitedStream {
            inner: stream,
            max_size: self.max_size_bytes,
            current_size: 0,
        }
    }
}

/// Stream wrapper with size limit
pub struct LimitedStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    max_size: u64,
    current_size: u64,
}

impl Stream for LimitedStream {
    type Item = Result<Bytes, ResponseLimitError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use futures::StreamExt;

        match self.inner.as_mut().poll_next_unpin(cx) {
            std::task::Poll::Ready(Some(Ok(bytes))) => {
                self.current_size += bytes.len() as u64;

                if self.current_size > self.max_size {
                    std::task::Poll::Ready(Some(Err(ResponseLimitError::ExceedsLimit {
                        actual: self.current_size,
                        limit: self.max_size,
                    })))
                } else {
                    std::task::Poll::Ready(Some(Ok(bytes)))
                }
            }
            std::task::Poll::Ready(Some(Err(_e))) => {
                std::task::Poll::Ready(Some(Err(ResponseLimitError::ExceedsLimit {
                    actual: 0,
                    limit: self.max_size,
                })))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_limit() {
        let limit = UpstreamResponseLimit::new(1000);

        assert!(limit.check(500).is_ok());
        assert!(limit.check(1500).is_err());
    }
}

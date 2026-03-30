// HTTP 客户端配置

use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

/// 构建配置好的 HTTP 客户端
pub fn build_client() -> Result<Client> {
    let client = Client::builder()
        // 超时配置
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(10))
        
        // 连接池配置
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(90))
        
        // HTTP/2 支持
        .http2_prior_knowledge()
        .http2_keep_alive_interval(Duration::from_secs(30))
        .http2_keep_alive_timeout(Duration::from_secs(10))
        
        // User-Agent
        .user_agent(super::headers::get_user_agent())
        
        // 其他配置
        .tcp_nodelay(true)
        .tcp_keepalive(Some(Duration::from_secs(60)))
        
        .build()?;

    Ok(client)
}

/// 构建 TLS 配置的客户端（匹配 Node.js 24.x）
#[cfg(feature = "tls-custom")]
pub fn build_client_with_tls() -> Result<Client> {
    use super::tls::build_tls_config;
    
    let tls_config = build_tls_config()?;
    
    let client = Client::builder()
        .use_preconfigured_tls(tls_config)
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(90))
        .http2_prior_knowledge()
        .user_agent(super::headers::get_user_agent())
        .tcp_nodelay(true)
        .tcp_keepalive(Some(Duration::from_secs(60)))
        .build()?;

    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_client() {
        let client = build_client();
        assert!(client.is_ok());
    }
}

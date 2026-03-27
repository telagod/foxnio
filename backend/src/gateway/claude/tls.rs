//! TLS 指纹配置

/// TLS 指纹配置
/// 
/// 基于 Node.js 24.x (Claude Code 客户端) 的 TLS 握手特征
/// JA3 Hash: 44f88fca027f27bab4bb08d4af15f23e
/// JA4: t13d1714h1_5b57614c22b0_7baf387fc6ff
#[derive(Debug, Clone)]
pub struct TLSFingerprint {
    /// 配置名称
    pub name: String,
    /// 密码套件（17 个）
    pub cipher_suites: Vec<u16>,
    /// 曲线（3 个）
    pub curves: Vec<u16>,
    /// 点格式
    pub point_formats: Vec<u16>,
    /// 是否启用 GREASE
    pub enable_grease: bool,
    /// 签名算法（9 个）
    pub signature_algorithms: Vec<u16>,
    /// ALPN 协议
    pub alpn_protocols: Vec<String>,
    /// 支持的版本
    pub supported_versions: Vec<u16>,
    /// Key Share 组
    pub key_share_groups: Vec<u16>,
    /// PSK 模式
    pub psk_modes: Vec<u16>,
    /// 扩展顺序（19 个）
    pub extensions: Vec<u16>,
}

impl Default for TLSFingerprint {
    fn default() -> Self {
        Self::nodejs_24x()
    }
}

impl TLSFingerprint {
    /// Node.js 24.x 默认指纹（Claude Code 客户端）
    pub fn nodejs_24x() -> Self {
        Self {
            name: "nodejs-24x".to_string(),
            cipher_suites: DEFAULT_CIPHER_SUITES.to_vec(),
            curves: DEFAULT_CURVES.to_vec(),
            point_formats: DEFAULT_POINT_FORMATS.to_vec(),
            enable_grease: true,
            signature_algorithms: DEFAULT_SIGNATURE_ALGORITHMS.to_vec(),
            alpn_protocols: vec!["http/1.1".to_string()],
            supported_versions: vec![
                TLS_VERSION_TLS13,
                TLS_VERSION_TLS12,
            ],
            key_share_groups: vec![CURVE_X25519],
            psk_modes: vec![PSK_MODE_DHE_KE],
            extensions: DEFAULT_EXTENSION_ORDER.to_vec(),
        }
    }
    
    /// 自定义指纹
    pub fn custom(
        cipher_suites: Vec<u16>,
        curves: Vec<u16>,
        extensions: Vec<u16>,
    ) -> Self {
        Self {
            name: "custom".to_string(),
            cipher_suites,
            curves,
            extensions,
            ..Default::default()
        }
    }
}

// ============ TLS 常量 ============

/// TLS 版本
pub const TLS_VERSION_TLS13: u16 = 0x0304;
pub const TLS_VERSION_TLS12: u16 = 0x0303;

/// 曲线 ID
pub const CURVE_X25519: u16 = 0x001d;
pub const CURVE_P256: u16 = 0x0017;
pub const CURVE_P384: u16 = 0x0018;

/// PSK 模式
pub const PSK_MODE_DHE_KE: u16 = 0x0001;

/// 默认密码套件（17 个，来自 Node.js 24.x）
/// 
/// 注意：顺序对于 JA3 指纹非常重要
pub const DEFAULT_CIPHER_SUITES: &[u16] = &[
    // TLS 1.3 cipher suites (3)
    0x1301, // TLS_AES_128_GCM_SHA256
    0x1302, // TLS_AES_256_GCM_SHA384
    0x1303, // TLS_CHACHA20_POLY1305_SHA256
    
    // ECDHE + AES-GCM (4)
    0xc02b, // TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
    0xc02f, // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
    0xc02c, // TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
    0xc030, // TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
    
    // ECDHE + ChaCha20-Poly1305 (2)
    0xcca9, // TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
    0xcca8, // TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
    
    // ECDHE + AES-CBC-SHA (4)
    0xc009, // TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA
    0xc013, // TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA
    0xc00a, // TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA
    0xc014, // TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA
    
    // RSA + AES-GCM (2)
    0x009c, // TLS_RSA_WITH_AES_128_GCM_SHA256
    0x009d, // TLS_RSA_WITH_AES_256_GCM_SHA384
    
    // RSA + AES-CBC-SHA (2)
    0x002f, // TLS_RSA_WITH_AES_128_CBC_SHA
    0x0035, // TLS_RSA_WITH_AES_256_CBC_SHA
];

/// 默认曲线（3 个）
pub const DEFAULT_CURVES: &[u16] = &[
    CURVE_X25519,    // 0x001d
    CURVE_P256,      // 0x0017 (secp256r1)
    CURVE_P384,      // 0x0018 (secp384r1)
];

/// 默认点格式
pub const DEFAULT_POINT_FORMATS: &[u16] = &[
    0, // uncompressed
];

/// 默认签名算法（9 个）
pub const DEFAULT_SIGNATURE_ALGORITHMS: &[u16] = &[
    0x0403, // ecdsa_secp256r1_sha256
    0x0804, // rsa_pss_rsae_sha256
    0x0401, // rsa_pkcs1_sha256
    0x0503, // ecdsa_secp384r1_sha384
    0x0805, // rsa_pss_rsae_sha384
    0x0501, // rsa_pkcs1_sha384
    0x0806, // rsa_pss_rsae_sha512
    0x0601, // rsa_pkcs1_sha512
    0x0201, // rsa_pkcs1_sha1
];

/// 默认扩展顺序（19 个）
/// 
/// 注意：顺序对于 JA3 指纹非常重要
pub const DEFAULT_EXTENSION_ORDER: &[u16] = &[
    0,     // server_name
    65037, // encrypted_client_hello (ECH)
    23,    // extended_master_secret
    65281, // renegotiation_info
    10,    // supported_groups
    11,    // ec_point_formats
    35,    // session_ticket
    16,    // alpn
    5,     // status_request (OCSP)
    13,    // signature_algorithms
    18,    // signed_certificate_timestamp
    51,    // key_share
    45,    // psk_key_exchange_modes
    43,    // supported_versions
];

/// 扩展类型常量
pub mod extensions {
    pub const SERVER_NAME: u16 = 0;
    pub const STATUS_REQUEST: u16 = 5;
    pub const SUPPORTED_GROUPS: u16 = 10;
    pub const EC_POINT_FORMATS: u16 = 11;
    pub const SIGNATURE_ALGORITHMS: u16 = 13;
    pub const ALPN: u16 = 16;
    pub const SCT: u16 = 18;
    pub const EXTENDED_MASTER_SECRET: u16 = 23;
    pub const SESSION_TICKET: u16 = 35;
    pub const SUPPORTED_VERSIONS: u16 = 43;
    pub const PSK_KEY_EXCHANGE_MODES: u16 = 45;
    pub const KEY_SHARE: u16 = 51;
    pub const ENCRYPTED_CLIENT_HELLO: u16 = 65037;
    pub const RENEGOTIATION_INFO: u16 = 65281;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tls_fingerprint_default() {
        let fp = TLSFingerprint::default();
        
        assert_eq!(fp.name, "nodejs-24x");
        assert_eq!(fp.cipher_suites.len(), 17);
        assert_eq!(fp.curves.len(), 3);
        assert_eq!(fp.extensions.len(), 14);
    }
    
    #[test]
    fn test_cipher_suites_count() {
        // 必须正好 17 个密码套件
        assert_eq!(DEFAULT_CIPHER_SUITES.len(), 17);
    }
    
    #[test]
    fn test_curves_count() {
        // 必须正好 3 个曲线
        assert_eq!(DEFAULT_CURVES.len(), 3);
    }
    
    #[test]
    fn test_signature_algorithms_count() {
        // 必须正好 9 个签名算法
        assert_eq!(DEFAULT_SIGNATURE_ALGORITHMS.len(), 9);
    }
    
    #[test]
    fn test_extension_order() {
        // 第一个必须是 server_name (0)
        assert_eq!(DEFAULT_EXTENSION_ORDER[0], 0);
        
        // 第二个必须是 ECH (65037)
        assert_eq!(DEFAULT_EXTENSION_ORDER[1], 65037);
    }
    
    #[test]
    fn test_tls_version() {
        assert_eq!(TLS_VERSION_TLS13, 0x0304);
        assert_eq!(TLS_VERSION_TLS12, 0x0303);
    }
    
    #[test]
    fn test_curve_ids() {
        assert_eq!(CURVE_X25519, 0x001d);
        assert_eq!(CURVE_P256, 0x0017);
        assert_eq!(CURVE_P384, 0x0018);
    }
}

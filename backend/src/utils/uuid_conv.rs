// 临时文件：UUID 和 i64 转换辅助函数

use uuid::Uuid;

/// 将 UUID 转换为 i64（取前 8 字节）
/// 注意：这是一个有损转换，仅用于兼容性目的
#[inline]
pub fn uuid_to_i64(id: Uuid) -> i64 {
    i64::from_ne_bytes(id.as_bytes()[..8].try_into().unwrap())
}

/// 尝试将 i64 转换为 UUID
/// 注意：这是一个不完整的转换，仅用于兼容性目的
#[inline]
pub fn i64_to_uuid(id: i64) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&id.to_ne_bytes());
    Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_i64_conversion() {
        let uuid = Uuid::new_v4();
        let id = uuid_to_i64(uuid);
        let back = i64_to_uuid(id);

        // 前 8 字节应该匹配
        assert_eq!(uuid.as_bytes()[..8], back.as_bytes()[..8]);
    }
}

//! 时间工具

use chrono::{DateTime, Utc, TimeZone};
use std::time::{SystemTime, UNIX_EPOCH};

/// 获取当前时间戳（毫秒）
pub fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// 获取当前时间戳（秒）
pub fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// 时间戳转 DateTime
pub fn timestamp_to_datetime(timestamp: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(timestamp).unwrap()
}

/// DateTime 转时间戳
pub fn datetime_to_timestamp(dt: DateTime<Utc>) -> i64 {
    dt.timestamp_millis()
}

/// 格式化时间
pub fn format_datetime(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// 解析时间
pub fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| Utc.from_utc_datetime(&dt))
}

/// 计算时间差（秒）
pub fn duration_seconds(start: DateTime<Utc>, end: DateTime<Utc>) -> i64 {
    (end - start).num_seconds()
}

/// 计算时间差（毫秒）
pub fn duration_millis(start: DateTime<Utc>, end: DateTime<Utc>) -> i64 {
    (end - start).num_milliseconds()
}

/// 检查是否过期
pub fn is_expired(expires_at: DateTime<Utc>) -> bool {
    Utc::now() > expires_at
}

/// 获取今天开始时间
pub fn today_start() -> DateTime<Utc> {
    Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc()
}

/// 获取今天结束时间
pub fn today_end() -> DateTime<Utc> {
    Utc::now().date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        assert!(ts > 0);
    }
    
    #[test]
    fn test_timestamp_conversion() {
        let now = Utc::now();
        let ts = datetime_to_timestamp(now);
        let dt = timestamp_to_datetime(ts);
        
        assert_eq!(now.timestamp_millis(), dt.timestamp_millis());
    }
    
    #[test]
    fn test_format_datetime() {
        let dt = Utc::now();
        let formatted = format_datetime(dt);
        
        assert!(formatted.contains("UTC"));
    }
    
    #[test]
    fn test_is_expired() {
        let past = Utc::now() - chrono::Duration::hours(1);
        let future = Utc::now() + chrono::Duration::hours(1);
        
        assert!(is_expired(past));
        assert!(!is_expired(future));
    }
    
    #[test]
    fn test_duration() {
        let start = Utc::now();
        let end = start + chrono::Duration::seconds(60);
        
        let secs = duration_seconds(start, end);
        assert_eq!(secs, 60);
    }
}

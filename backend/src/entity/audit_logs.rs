//! Audit Log Entity - 审计日志实体

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// 审计事件类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    // 用户认证
    UserLogin,
    UserLogout,
    UserRegister,
    PasswordChange,
    PasswordReset,
    
    // API Key 管理
    ApiKeyCreate,
    ApiKeyDelete,
    ApiKeyUpdate,
    
    // 账户操作
    AccountUpdate,
    AccountCreate,
    AccountDelete,
    
    // 管理员操作
    AdminAction,
    
    // 余额操作
    BalanceUpdate,
    BalanceRecharge,
    
    // 其他
    ApiRequest,
    RateLimitExceeded,
    SecurityAlert,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserLogin => "USER_LOGIN",
            Self::UserLogout => "USER_LOGOUT",
            Self::UserRegister => "USER_REGISTER",
            Self::PasswordChange => "PASSWORD_CHANGE",
            Self::PasswordReset => "PASSWORD_RESET",
            Self::ApiKeyCreate => "API_KEY_CREATE",
            Self::ApiKeyDelete => "API_KEY_DELETE",
            Self::ApiKeyUpdate => "API_KEY_UPDATE",
            Self::AccountUpdate => "ACCOUNT_UPDATE",
            Self::AccountCreate => "ACCOUNT_CREATE",
            Self::AccountDelete => "ACCOUNT_DELETE",
            Self::AdminAction => "ADMIN_ACTION",
            Self::BalanceUpdate => "BALANCE_UPDATE",
            Self::BalanceRecharge => "BALANCE_RECHARGE",
            Self::ApiRequest => "API_REQUEST",
            Self::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
            Self::SecurityAlert => "SECURITY_ALERT",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "USER_LOGIN" => Some(Self::UserLogin),
            "USER_LOGOUT" => Some(Self::UserLogout),
            "USER_REGISTER" => Some(Self::UserRegister),
            "PASSWORD_CHANGE" => Some(Self::PasswordChange),
            "PASSWORD_RESET" => Some(Self::PasswordReset),
            "API_KEY_CREATE" => Some(Self::ApiKeyCreate),
            "API_KEY_DELETE" => Some(Self::ApiKeyDelete),
            "API_KEY_UPDATE" => Some(Self::ApiKeyUpdate),
            "ACCOUNT_UPDATE" => Some(Self::AccountUpdate),
            "ACCOUNT_CREATE" => Some(Self::AccountCreate),
            "ACCOUNT_DELETE" => Some(Self::AccountDelete),
            "ADMIN_ACTION" => Some(Self::AdminAction),
            "BALANCE_UPDATE" => Some(Self::BalanceUpdate),
            "BALANCE_RECHARGE" => Some(Self::BalanceRecharge),
            "API_REQUEST" => Some(Self::ApiRequest),
            "RATE_LIMIT_EXCEEDED" => Some(Self::RateLimitExceeded),
            "SECURITY_ALERT" => Some(Self::SecurityAlert),
            _ => None,
        }
    }
    
    /// 判断是否为敏感操作
    pub fn is_sensitive(&self) -> bool {
        matches!(
            self,
            Self::UserLogin
                | Self::PasswordChange
                | Self::PasswordReset
                | Self::ApiKeyCreate
                | Self::ApiKeyDelete
                | Self::AdminAction
                | Self::SecurityAlert
        )
    }
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_data: Option<JsonValue>,
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id"
    )]
    User,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 获取审计动作类型
    pub fn get_action(&self) -> Option<AuditAction> {
        AuditAction::from_str(&self.action)
    }
    
    /// 检查是否为敏感操作
    pub fn is_sensitive(&self) -> bool {
        self.get_action().map(|a| a.is_sensitive()).unwrap_or(false)
    }
    
    /// 脱敏显示
    pub fn sanitized(&self) -> SanitizedAuditLog {
        SanitizedAuditLog {
            id: self.id,
            user_id: self.user_id,
            action: self.action.clone(),
            resource_type: self.resource_type.clone(),
            resource_id: self.resource_id.clone(),
            ip_address: self.ip_address.as_ref().map(|ip| mask_ip(ip)),
            user_agent: self.user_agent.as_ref().map(|ua| mask_user_agent(ua)),
            created_at: self.created_at,
            response_status: self.response_status,
            // 不返回 request_data，敏感数据
        }
    }
}

/// 脱敏后的审计日志（用于 API 返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizedAuditLog {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// IP 地址脱敏
fn mask_ip(ip: &str) -> String {
    if ip.contains(':') {
        // IPv6: 保留前两段
        let parts: Vec<&str> = ip.split(':').collect();
        if parts.len() >= 2 {
            format!("{}:{}::xxxx", parts[0], parts[1])
        } else {
            "x::x".to_string()
        }
    } else {
        // IPv4: 保留前两段
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            format!("{}.{}.x.x", parts[0], parts[1])
        } else {
            "x.x.x.x".to_string()
        }
    }
}

/// User Agent 脱敏（只保留浏览器/客户端名称）
fn mask_user_agent(ua: &str) -> String {
    // 只保留前 50 个字符
    if ua.len() > 50 {
        format!("{}...", &ua[..50])
    } else {
        ua.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_action() {
        assert_eq!(AuditAction::UserLogin.as_str(), "USER_LOGIN");
        assert_eq!(AuditAction::from_str("USER_LOGIN"), Some(AuditAction::UserLogin));
        assert!(AuditAction::UserLogin.is_sensitive());
        assert!(!AuditAction::ApiRequest.is_sensitive());
    }

    #[test]
    fn test_mask_ip() {
        assert_eq!(mask_ip("192.168.1.100"), "192.168.x.x");
        assert_eq!(mask_ip("2001:db8:85a3::8a2e:370:7334"), "2001:db8::xxxx");
    }

    #[test]
    fn test_mask_user_agent() {
        let short_ua = "Mozilla/5.0";
        assert_eq!(mask_user_agent(short_ua), short_ua);

        let long_ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";
        let masked = mask_user_agent(long_ua);
        assert!(masked.ends_with("..."));
        assert!(masked.len() < long_ua.len());
    }
}

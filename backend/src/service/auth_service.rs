use crate::model::user::User;
use crate::service::SettingService;
use argon2::{
    password_hash::{
        rand_core::OsRng, Error as HashError, PasswordHash, PasswordHasher, PasswordVerifier,
        SaltString,
    },
    Algorithm, Argon2, Params,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, PgPool};
use std::sync::Arc;

/// Authentication service for user login and token management
pub struct AuthService {
    pool: PgPool,
    setting_service: Arc<SettingService>,
    jwt_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id as UUID string
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("User not found")]
    UserNotFound,
    #[error("Token expired")]
    TokenExpired,
    #[error("Invalid token")]
    InvalidToken,
    #[error("Password hash error: {0}")]
    PasswordHash(String),
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<HashError> for AuthError {
    fn from(e: HashError) -> Self {
        AuthError::PasswordHash(e.to_string())
    }
}

impl From<argon2::Error> for AuthError {
    fn from(e: argon2::Error) -> Self {
        AuthError::PasswordHash(e.to_string())
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: uuid::Uuid,
    pub email: String,
    pub username: String,
    pub role: String,
}

impl AuthService {
    pub fn new(pool: PgPool, setting_service: Arc<SettingService>, jwt_secret: String) -> Self {
        Self {
            pool,
            setting_service,
            jwt_secret,
        }
    }

    /// Login user with email and password
    pub async fn login(&self, req: LoginRequest) -> Result<LoginResponse, AuthError> {
        // Find user by email
        let user =
            query_as::<_, User>("SELECT * FROM users WHERE email = $1 AND deleted_at IS NULL")
                .bind(&req.email)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(AuthError::InvalidCredentials)?;

        // Verify password
        self.verify_password(&req.password, &user.password_hash)?;

        // Generate tokens
        let access_token = self.generate_access_token(&user)?;
        let refresh_token = self.generate_refresh_token(&user)?;

        Ok(LoginResponse {
            access_token,
            refresh_token,
            expires_in: 3600, // 1 hour
            user: UserInfo {
                id: user.id,
                email: user.email.clone(),
                username: user.email.clone(), // Use email as username
                role: user.role.to_string(),
            },
        })
    }

    /// Verify JWT token
    pub async fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )?;

        // Check if token is expired
        if token_data.claims.exp < Utc::now().timestamp() {
            return Err(AuthError::TokenExpired);
        }

        Ok(token_data.claims)
    }

    /// Hash password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let params = Params::new(65536, 3, 4, Some(32))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, argon2::Version::V0x13, params);
        let hash = argon2.hash_password(password.as_bytes(), &salt)?;
        Ok(hash.to_string())
    }

    /// Verify password against hash
    fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AuthError> {
        let parsed_hash = PasswordHash::new(hash)?;
        let argon2 = Argon2::default();
        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(_) => Ok(true),
            Err(_) => Err(AuthError::InvalidCredentials),
        }
    }

    /// Generate access token (short-lived)
    fn generate_access_token(&self, user: &User) -> Result<String, AuthError> {
        let expiration = Utc::now() + Duration::hours(1);
        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.to_string(),
            exp: expiration.timestamp(),
            iat: Utc::now().timestamp(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(AuthError::from)
    }

    /// Generate refresh token (long-lived)
    fn generate_refresh_token(&self, user: &User) -> Result<String, AuthError> {
        let expiration = Utc::now() + Duration::days(7);
        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.to_string(),
            exp: expiration.timestamp(),
            iat: Utc::now().timestamp(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(AuthError::from)
    }

    /// Refresh access token using refresh token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<String, AuthError> {
        let claims = self.verify_token(refresh_token).await?;

        // Get user
        let user = query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(&claims.sub)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        // Generate new access token
        self.generate_access_token(&user)
    }

    /// Logout user (invalidate tokens)
    pub async fn logout(&self, user_id: i64) -> Result<(), AuthError> {
        // In a real implementation, you'd add the token to a blacklist
        // For now, we just update the user's last_logout_at
        query("UPDATE users SET last_logout_at = NOW() WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::password_hash::{PasswordHasher, SaltString};
    use argon2::{Algorithm, Argon2, Params};

    #[test]
    fn test_password_hashing() {
        let password = "test_password";
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            argon2::Version::V0x13,
            Params::default(),
        );
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .to_string();

        // Verify the hash can be parsed and validated
        let parsed_hash = PasswordHash::new(&hash).unwrap();
        assert!(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok());
        assert!(Argon2::default()
            .verify_password("wrong_password".as_bytes(), &parsed_hash)
            .is_err());
    }

    #[test]
    fn test_password_hash_uniqueness() {
        let password = "same_password";
        let salt1 = SaltString::generate(&mut OsRng);
        let salt2 = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            argon2::Version::V0x13,
            Params::default(),
        );

        let hash1 = argon2
            .hash_password(password.as_bytes(), &salt1)
            .unwrap()
            .to_string();
        let hash2 = argon2
            .hash_password(password.as_bytes(), &salt2)
            .unwrap()
            .to_string();

        // Same password should produce different hashes (due to salt)
        assert_ne!(hash1, hash2);
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// User roles for authorization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    /// Full admin access - can approve users, manage everything
    Admin,
    /// Regular approved user
    User,
    /// Newly registered, awaiting admin approval
    Pending,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::User => "user",
            UserRole::Pending => "pending",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "admin" => UserRole::Admin,
            "user" => UserRole::User,
            _ => UserRole::Pending,
        }
    }
}

/// A user in the system, created from OIDC identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
    pub role: UserRole,
    pub oidc_subject: String,
    pub oidc_issuer: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
    pub role: String,
    pub oidc_subject: String,
    pub oidc_issuer: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            email: row.email,
            name: row.name,
            picture: row.picture,
            role: UserRole::from_str(&row.role),
            oidc_subject: row.oidc_subject,
            oidc_issuer: row.oidc_issuer,
            created_at: DateTime::parse_from_rfc3339(&row.created_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            updated_at: DateTime::parse_from_rfc3339(&row.updated_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        }
    }
}

/// A user session, stored server-side and referenced by cookie token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SessionRow {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: String,
    pub created_at: String,
}

impl From<SessionRow> for Session {
    fn from(row: SessionRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            token_hash: row.token_hash,
            expires_at: DateTime::parse_from_rfc3339(&row.expires_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            created_at: DateTime::parse_from_rfc3339(&row.created_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        }
    }
}

/// Response sent to the frontend for current user info
#[derive(Debug, Serialize)]
pub struct AuthUserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
    pub role: UserRole,
}

impl From<User> for AuthUserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            name: user.name,
            picture: user.picture,
            role: user.role,
        }
    }
}

/// Admin endpoint: list of users for management
#[derive(Debug, Serialize)]
pub struct AdminUserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
}

impl From<User> for AdminUserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            name: user.name,
            picture: user.picture,
            role: user.role,
            created_at: user.created_at,
        }
    }
}

/// Request body for updating a user's role
#[derive(Debug, Deserialize)]
pub struct UpdateUserRoleRequest {
    pub role: UserRole,
}

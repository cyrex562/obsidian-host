use crate::error::{AppError, AppResult};
use crate::models::{
    AdminUser, ApiKeyInfo, AuditLogEntry, EditorMode, GroupInfo, GroupMember, MlUndoReceipt,
    ReverseAction, SessionInfo, UserPreferences, Vault, VaultRole, VaultRow, VaultShareEntry,
    VaultShareList,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use uuid::Uuid;

fn parse_vault_role(role: &str) -> Option<VaultRole> {
    match role {
        "owner" => Some(VaultRole::Owner),
        "editor" => Some(VaultRole::Editor),
        "viewer" => Some(VaultRole::Viewer),
        _ => None,
    }
}

fn format_vault_role(role: &VaultRole) -> &'static str {
    match role {
        VaultRole::Owner => "owner",
        VaultRole::Editor => "editor",
        VaultRole::Viewer => "viewer",
    }
}

fn parse_rfc3339_utc(value: &str) -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

fn editor_mode_from_str(value: &str) -> EditorMode {
    match value {
        "raw" => EditorMode::Raw,
        "formatted_raw" => EditorMode::FormattedRaw,
        "fully_rendered" | "wysiwyg" => EditorMode::FullyRendered,
        _ => EditorMode::SideBySide,
    }
}

fn editor_mode_to_str(value: &EditorMode) -> &'static str {
    match value {
        EditorMode::Raw => "raw",
        EditorMode::SideBySide => "side_by_side",
        EditorMode::FormattedRaw => "formatted_raw",
        EditorMode::FullyRendered => "fully_rendered",
    }
}

#[derive(sqlx::FromRow)]
struct FileChangeLogRow {
    path: String,
    event_type: String,
    old_path: Option<String>,
    timestamp: i64,
}

#[derive(sqlx::FromRow)]
struct MlUndoReceiptRow {
    receipt_id: String,
    vault_id: String,
    file_path: String,
    description: String,
    reverse_action: String,
    applied_at: String,
}

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> AppResult<Self> {
        let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let db = Database { pool };
        db.run_migrations().await?;

        Ok(db)
    }

    async fn run_migrations(&self) -> AppResult<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS vaults (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create preferences table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS preferences (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                theme TEXT NOT NULL DEFAULT 'dark',
                editor_mode TEXT NOT NULL DEFAULT 'side_by_side',
                font_size INTEGER NOT NULL DEFAULT 14,
                window_layout TEXT,
                icon_map TEXT,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Initialize default preferences if not exists
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO preferences (id, theme, editor_mode, font_size, updated_at)
            VALUES (1, 'dark', 'side_by_side', 14, ?)
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        // Add window_layout column if it doesn't exist (for existing DBs)
        let _ = sqlx::query("ALTER TABLE preferences ADD COLUMN window_layout TEXT")
            .execute(&self.pool)
            .await;

        let _ = sqlx::query("ALTER TABLE preferences ADD COLUMN icon_map TEXT")
            .execute(&self.pool)
            .await;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS recent_files (
                vault_id TEXT NOT NULL,
                path TEXT NOT NULL,
                last_accessed TEXT NOT NULL,
                PRIMARY KEY (vault_id, path),
                FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                is_admin INTEGER NOT NULL DEFAULT 0,
                must_change_password INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        let _ = sqlx::query("ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0")
            .execute(&self.pool)
            .await;

        let _ = sqlx::query(
            "ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;

        sqlx::query(
            r#"
            UPDATE users
            SET is_admin = 1
            WHERE id = (
                SELECT id FROM users ORDER BY created_at ASC LIMIT 1
            )
            AND NOT EXISTS (
                SELECT 1 FROM users WHERE is_admin = 1
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_preferences (
                user_id TEXT PRIMARY KEY,
                theme TEXT NOT NULL DEFAULT 'dark',
                editor_mode TEXT NOT NULL DEFAULT 'side_by_side',
                font_size INTEGER NOT NULL DEFAULT 14,
                window_layout TEXT,
                icon_map TEXT,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        let _ = sqlx::query("ALTER TABLE user_preferences ADD COLUMN icon_map TEXT")
            .execute(&self.pool)
            .await;

        let _ =
            sqlx::query("ALTER TABLE vaults ADD COLUMN owner_user_id TEXT REFERENCES users(id)")
                .execute(&self.pool)
                .await;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                created_by_user_id TEXT NOT NULL,
                FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS group_members (
                group_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                added_at TEXT NOT NULL,
                PRIMARY KEY (group_id, user_id),
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS vault_user_shares (
                vault_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                role TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (vault_id, user_id),
                FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS vault_group_shares (
                vault_id TEXT NOT NULL,
                group_id TEXT NOT NULL,
                role TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (vault_id, group_id),
                FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE,
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_group_members_user_id ON group_members(user_id)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_vault_user_shares_user_id ON vault_user_shares(user_id)"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_vault_group_shares_group_id ON vault_group_shares(group_id)"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS file_change_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                vault_id TEXT NOT NULL,
                path TEXT NOT NULL,
                event_type TEXT NOT NULL,
                etag TEXT,
                old_path TEXT,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_file_change_log_vault_timestamp
            ON file_change_log(vault_id, timestamp)
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Bookmarks table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY NOT NULL,
                vault_id TEXT NOT NULL,
                path TEXT NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE,
                UNIQUE(vault_id, path)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_bookmarks_vault_id ON bookmarks(vault_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ml_undo_receipts (
                receipt_id TEXT PRIMARY KEY NOT NULL,
                vault_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                description TEXT NOT NULL,
                reverse_action TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_ml_undo_receipts_vault_id ON ml_undo_receipts(vault_id)",
        )
        .execute(&self.pool)
        .await?;

        // ── Multi-user hardening migrations ─────────────────────────────────

        // Add is_active flag for user deactivation (default true = active).
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1")
            .execute(&self.pool)
            .await;

        // Failed-login tracking columns for account lockout.
        let _ = sqlx::query(
            "ALTER TABLE users ADD COLUMN failed_login_attempts INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;

        let _ = sqlx::query("ALTER TABLE users ADD COLUMN locked_until TEXT")
            .execute(&self.pool)
            .await;

        // Audit log table for security-relevant events.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                user_id TEXT,
                username TEXT,
                event_type TEXT NOT NULL,
                detail TEXT,
                ip_address TEXT,
                success INTEGER NOT NULL DEFAULT 1
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_user_id ON audit_log(user_id)",
        )
        .execute(&self.pool)
        .await?;

        // Active sessions table for token revocation support.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                token_id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                revoked INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)",
        )
        .execute(&self.pool)
        .await?;

        // API keys table for programmatic access.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                prefix TEXT NOT NULL,
                key_hash TEXT NOT NULL,
                user_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT,
                revoked INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_api_keys_prefix ON api_keys(prefix)")
            .execute(&self.pool)
            .await?;

        // Vault visibility: 'private' (default) or 'public'.
        let _ = sqlx::query(
            "ALTER TABLE vaults ADD COLUMN visibility TEXT NOT NULL DEFAULT 'private'",
        )
        .execute(&self.pool)
        .await;

        // ── Phase 4b: TOTP 2FA ──────────────────────────────────────────
        let _ = sqlx::query(
            "ALTER TABLE users ADD COLUMN totp_secret TEXT",
        )
        .execute(&self.pool)
        .await;

        let _ = sqlx::query(
            "ALTER TABLE users ADD COLUMN totp_enabled INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;

        let _ = sqlx::query(
            "ALTER TABLE users ADD COLUMN totp_backup_codes TEXT",
        )
        .execute(&self.pool)
        .await;

        // ── Phase 4b: Invitations ───────────────────────────────────────
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS invitations (
                id TEXT PRIMARY KEY,
                token TEXT NOT NULL UNIQUE,
                role TEXT NOT NULL DEFAULT 'viewer',
                vault_id TEXT,
                created_by_user_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                accepted INTEGER NOT NULL DEFAULT 0,
                accepted_by_user_id TEXT,
                FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE,
                FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_invitations_token ON invitations(token)",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn user_count(&self) -> AppResult<i64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(count.0)
    }

    pub async fn save_ml_undo_receipt(&self, receipt: &MlUndoReceipt) -> AppResult<()> {
        let reverse_action = serde_json::to_string(&receipt.reverse_action).map_err(|e| {
            AppError::InternalError(format!("Failed to serialize reverse action: {e}"))
        })?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO ml_undo_receipts
            (receipt_id, vault_id, file_path, description, reverse_action, applied_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&receipt.receipt_id)
        .bind(&receipt.vault_id)
        .bind(&receipt.file_path)
        .bind(&receipt.description)
        .bind(reverse_action)
        .bind(receipt.applied_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(())
    }

    pub async fn consume_ml_undo_receipt(
        &self,
        vault_id: &str,
        receipt_id: &str,
    ) -> AppResult<MlUndoReceipt> {
        let row = sqlx::query_as::<_, MlUndoReceiptRow>(
            r#"
            SELECT receipt_id, vault_id, file_path, description, reverse_action, applied_at
            FROM ml_undo_receipts
            WHERE vault_id = ? AND receipt_id = ?
            "#,
        )
        .bind(vault_id)
        .bind(receipt_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Undo receipt '{}' was not found (it may be expired or already used)",
                receipt_id
            ))
        })?;

        let delete_result =
            sqlx::query("DELETE FROM ml_undo_receipts WHERE vault_id = ? AND receipt_id = ?")
                .bind(vault_id)
                .bind(receipt_id)
                .execute(&self.pool)
                .await
                .map_err(AppError::from)?;

        if delete_result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Undo receipt '{}' was not found (it may be expired or already used)",
                receipt_id
            )));
        }

        let reverse_action: ReverseAction = serde_json::from_str(&row.reverse_action)
            .map_err(|e| AppError::InternalError(format!("Failed to parse reverse action: {e}")))?;

        Ok(MlUndoReceipt {
            receipt_id: row.receipt_id,
            vault_id: row.vault_id,
            file_path: row.file_path,
            description: row.description,
            reverse_action,
            applied_at: parse_rfc3339_utc(&row.applied_at),
        })
    }

    // ── Bookmarks ─────────────────────────────────────────────────────────────

    pub async fn list_bookmarks(
        &self,
        vault_id: &str,
    ) -> AppResult<Vec<crate::models::bookmarks::Bookmark>> {
        let rows = sqlx::query_as::<_, crate::models::bookmarks::Bookmark>(
            "SELECT id, vault_id, path, title, created_at FROM bookmarks WHERE vault_id = ? ORDER BY created_at DESC"
        )
        .bind(vault_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows)
    }

    pub async fn create_bookmark(&self, bm: &crate::models::bookmarks::Bookmark) -> AppResult<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO bookmarks (id, vault_id, path, title, created_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&bm.id)
        .bind(&bm.vault_id)
        .bind(&bm.path)
        .bind(&bm.title)
        .bind(&bm.created_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    pub async fn delete_bookmark(&self, vault_id: &str, bookmark_id: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM bookmarks WHERE vault_id = ? AND id = ?")
            .bind(vault_id)
            .bind(bookmark_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    pub async fn create_user(&self, username: &str, password_hash: &str) -> AppResult<()> {
        self.create_user_with_options(username, password_hash, false, false)
            .await
            .map(|_| ())
    }

    pub async fn create_user_with_options(
        &self,
        username: &str,
        password_hash: &str,
        is_admin: bool,
        must_change_password: bool,
    ) -> AppResult<(String, String, bool, bool)> {
        if username.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "Username cannot be empty".to_string(),
            ));
        }
        if password_hash.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "Password hash cannot be empty".to_string(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO users (id, username, password_hash, is_admin, must_change_password, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(username.trim())
        .bind(password_hash)
        .bind(if is_admin { 1_i64 } else { 0_i64 })
        .bind(if must_change_password { 1_i64 } else { 0_i64 })
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.message().contains("UNIQUE") => {
                AppError::Conflict("User with this username already exists".to_string())
            }
            _ => AppError::from(e),
        })?;

        Ok((
            id,
            username.trim().to_string(),
            is_admin,
            must_change_password,
        ))
    }

    pub async fn get_user_auth_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<(String, String, String)>> {
        let row: Option<(String, String, String)> =
            sqlx::query_as("SELECT id, username, password_hash FROM users WHERE username = ?")
                .bind(username)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::from)?;

        Ok(row)
    }

    pub async fn get_user_auth_by_id(
        &self,
        user_id: &str,
    ) -> AppResult<Option<(String, String, String)>> {
        let row: Option<(String, String, String)> =
            sqlx::query_as("SELECT id, username, password_hash FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::from)?;

        Ok(row)
    }

    pub async fn is_user_admin(&self, user_id: &str) -> AppResult<bool> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT is_admin FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(row.map(|(value,)| value != 0).unwrap_or(false))
    }

    pub async fn user_must_change_password(&self, user_id: &str) -> AppResult<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT must_change_password FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::from)?;
        Ok(row.map(|(value,)| value != 0).unwrap_or(false))
    }

    pub async fn list_users(&self) -> AppResult<Vec<AdminUser>> {
        let rows = sqlx::query_as::<_, (String, String, i64, i64, i64, String)>(
            "SELECT id, username, is_admin, must_change_password, is_active, created_at FROM users ORDER BY username ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(
                |(id, username, is_admin, must_change_password, is_active, created_at)| AdminUser {
                    id,
                    username,
                    is_admin: is_admin != 0,
                    must_change_password: must_change_password != 0,
                    is_active: is_active != 0,
                    created_at: parse_rfc3339_utc(&created_at),
                },
            )
            .collect())
    }

    pub async fn set_user_password(
        &self,
        user_id: &str,
        password_hash: &str,
        must_change_password: bool,
    ) -> AppResult<()> {
        let result = sqlx::query(
            "UPDATE users SET password_hash = ?, must_change_password = ? WHERE id = ?",
        )
        .bind(password_hash)
        .bind(if must_change_password { 1_i64 } else { 0_i64 })
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("User {} not found", user_id)));
        }

        Ok(())
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> AppResult<Option<(String, String)>> {
        let row =
            sqlx::query_as::<_, (String, String)>("SELECT id, username FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::from)?;

        Ok(row)
    }

    pub async fn get_user_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<(String, String)>> {
        let row = sqlx::query_as::<_, (String, String)>(
            "SELECT id, username FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row)
    }

    pub async fn create_group(&self, name: &str, created_by_user_id: &str) -> AppResult<GroupInfo> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::InvalidInput(
                "Group name cannot be empty".to_string(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO groups (id, name, created_at, created_by_user_id)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(trimmed)
        .bind(&now)
        .bind(created_by_user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.message().contains("UNIQUE") => {
                AppError::Conflict("Group with this name already exists".to_string())
            }
            _ => AppError::from(e),
        })?;

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO group_members (group_id, user_id, added_at)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(created_by_user_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(GroupInfo {
            id,
            name: trimmed.to_string(),
            created_at: parse_rfc3339_utc(&now),
        })
    }

    pub async fn list_groups_for_user(&self, user_id: &str) -> AppResult<Vec<GroupInfo>> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT DISTINCT g.id, g.name, g.created_at
            FROM groups g
            LEFT JOIN group_members gm ON gm.group_id = g.id
            WHERE gm.user_id = ? OR g.created_by_user_id = ?
            ORDER BY g.name ASC
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(|(id, name, created_at)| GroupInfo {
                id,
                name,
                created_at: parse_rfc3339_utc(&created_at),
            })
            .collect())
    }

    pub async fn is_group_manager(&self, group_id: &str, user_id: &str) -> AppResult<bool> {
        let row = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM groups WHERE id = ? AND created_by_user_id = ?",
        )
        .bind(group_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0 > 0)
    }

    pub async fn list_group_members(&self, group_id: &str) -> AppResult<Vec<GroupMember>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT u.id, u.username
            FROM group_members gm
            INNER JOIN users u ON u.id = gm.user_id
            WHERE gm.group_id = ?
            ORDER BY u.username ASC
            "#,
        )
        .bind(group_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(|(user_id, username)| GroupMember { user_id, username })
            .collect())
    }

    pub async fn add_user_to_group(&self, group_id: &str, user_id: &str) -> AppResult<()> {
        let group_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM groups WHERE id = ?")
            .bind(group_id)
            .fetch_one(&self.pool)
            .await?;
        if group_exists.0 == 0 {
            return Err(AppError::NotFound(format!("Group {} not found", group_id)));
        }

        let user_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;
        if user_exists.0 == 0 {
            return Err(AppError::NotFound(format!("User {} not found", user_id)));
        }

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO group_members (group_id, user_id, added_at)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(group_id)
        .bind(user_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_user_from_group(&self, group_id: &str, user_id: &str) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM group_members WHERE group_id = ? AND user_id = ?")
            .bind(group_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Group member not found".to_string()));
        }

        Ok(())
    }

    pub async fn bootstrap_admin_if_empty(
        &self,
        username: Option<&str>,
        password: Option<&str>,
    ) -> AppResult<bool> {
        if self.user_count().await? > 0 {
            return Ok(false);
        }

        let username = username
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                AppError::InvalidInput(
                    "No users exist. Set auth.bootstrap_admin_username in config.toml".to_string(),
                )
            })?;

        let password = password
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                AppError::InvalidInput(
                    "No users exist. Set auth.bootstrap_admin_password in config.toml".to_string(),
                )
            })?;

        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| {
                AppError::InternalError(format!("Failed to hash bootstrap password: {e}"))
            })?
            .to_string();

        self.create_user_with_options(username, &password_hash, true, false)
            .await?;
        Ok(true)
    }

    // Vault operations
    pub async fn create_vault(&self, name: String, path: String) -> AppResult<Vault> {
        self.create_vault_for_owner(name, path, None).await
    }

    pub async fn create_vault_for_owner(
        &self,
        name: String,
        path: String,
        owner_user_id: Option<&str>,
    ) -> AppResult<Vault> {
        if name.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "Vault name cannot be empty".to_string(),
            ));
        }
        if path.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "Vault path cannot be empty".to_string(),
            ));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let row = sqlx::query_as::<_, VaultRow>(
            r#"
            INSERT INTO vaults (id, name, path, created_at, updated_at, owner_user_id)
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING id, name, path, created_at, updated_at
            "#,
        )
        .bind(&id)
        .bind(&name)
        .bind(&path)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(owner_user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.message().contains("UNIQUE") => {
                AppError::Conflict("Vault with this path already exists".to_string())
            }
            _ => AppError::from(e),
        })?;

        Ok(row.into())
    }

    pub async fn get_vault(&self, id: &str) -> AppResult<Vault> {
        let row = sqlx::query_as::<_, VaultRow>("SELECT * FROM vaults WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => AppError::NotFound(format!("Vault {} not found", id)),
                _ => AppError::from(e),
            })?;

        Ok(row.into())
    }

    pub async fn list_vaults(&self) -> AppResult<Vec<Vault>> {
        let rows = sqlx::query_as::<_, VaultRow>("SELECT * FROM vaults ORDER BY name ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn list_vaults_for_user(&self, user_id: &str) -> AppResult<Vec<Vault>> {
        let rows = sqlx::query_as::<_, VaultRow>(
            r#"
            SELECT DISTINCT v.id, v.name, v.path, v.created_at, v.updated_at
            FROM vaults v
            LEFT JOIN vault_user_shares vus ON vus.vault_id = v.id AND vus.user_id = ?
            LEFT JOIN group_members gm ON gm.user_id = ?
            LEFT JOIN vault_group_shares vgs ON vgs.vault_id = v.id AND vgs.group_id = gm.group_id
            WHERE v.owner_user_id = ?
               OR vus.user_id IS NOT NULL
               OR vgs.group_id IS NOT NULL
               OR (
                    v.owner_user_id IS NULL
                    AND NOT EXISTS (SELECT 1 FROM vault_user_shares vus2 WHERE vus2.vault_id = v.id)
                    AND NOT EXISTS (SELECT 1 FROM vault_group_shares vgs2 WHERE vgs2.vault_id = v.id)
               )
            ORDER BY v.name ASC
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn get_vault_role_for_user(
        &self,
        vault_id: &str,
        user_id: &str,
    ) -> AppResult<Option<VaultRole>> {
        let owner_row =
            sqlx::query_as::<_, (Option<String>,)>("SELECT owner_user_id FROM vaults WHERE id = ?")
                .bind(vault_id)
                .fetch_optional(&self.pool)
                .await?;

        let Some((owner_user_id,)) = owner_row else {
            return Ok(None);
        };

        if owner_user_id.as_deref() == Some(user_id) {
            return Ok(Some(VaultRole::Owner));
        }

        let direct_roles = sqlx::query_as::<_, (String,)>(
            "SELECT role FROM vault_user_shares WHERE vault_id = ? AND user_id = ?",
        )
        .bind(vault_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        if direct_roles
            .iter()
            .any(|(role,)| role == "editor" || role == "owner")
        {
            return Ok(Some(VaultRole::Editor));
        }

        let group_roles = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT vgs.role
            FROM vault_group_shares vgs
            INNER JOIN group_members gm ON gm.group_id = vgs.group_id
            WHERE vgs.vault_id = ? AND gm.user_id = ?
            "#,
        )
        .bind(vault_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        if group_roles
            .iter()
            .any(|(role,)| role == "editor" || role == "owner")
        {
            return Ok(Some(VaultRole::Editor));
        }

        if direct_roles.iter().any(|(role,)| role == "viewer")
            || group_roles.iter().any(|(role,)| role == "viewer")
        {
            return Ok(Some(VaultRole::Viewer));
        }

        let share_count: (i64,) = sqlx::query_as(
            r#"
            SELECT (
                (SELECT COUNT(*) FROM vault_user_shares WHERE vault_id = ?)
                +
                (SELECT COUNT(*) FROM vault_group_shares WHERE vault_id = ?)
            )
            "#,
        )
        .bind(vault_id)
        .bind(vault_id)
        .fetch_one(&self.pool)
        .await?;

        if owner_user_id.is_none() && share_count.0 == 0 {
            return Ok(Some(VaultRole::Editor));
        }

        Ok(None)
    }

    pub async fn share_vault_with_user(
        &self,
        vault_id: &str,
        user_id: &str,
        role: &VaultRole,
    ) -> AppResult<()> {
        if matches!(role, VaultRole::Owner) {
            return Err(AppError::InvalidInput(
                "Vault shares may only grant editor or viewer access".to_string(),
            ));
        }

        let user_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;
        if user_exists.0 == 0 {
            return Err(AppError::NotFound(format!("User {} not found", user_id)));
        }

        sqlx::query(
            r#"
            INSERT INTO vault_user_shares (vault_id, user_id, role, created_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(vault_id, user_id) DO UPDATE SET role = excluded.role
            "#,
        )
        .bind(vault_id)
        .bind(user_id)
        .bind(format_vault_role(role))
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn share_vault_with_group(
        &self,
        vault_id: &str,
        group_id: &str,
        role: &VaultRole,
    ) -> AppResult<()> {
        if matches!(role, VaultRole::Owner) {
            return Err(AppError::InvalidInput(
                "Vault shares may only grant editor or viewer access".to_string(),
            ));
        }

        let group_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM groups WHERE id = ?")
            .bind(group_id)
            .fetch_one(&self.pool)
            .await?;
        if group_exists.0 == 0 {
            return Err(AppError::NotFound(format!("Group {} not found", group_id)));
        }

        sqlx::query(
            r#"
            INSERT INTO vault_group_shares (vault_id, group_id, role, created_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(vault_id, group_id) DO UPDATE SET role = excluded.role
            "#,
        )
        .bind(vault_id)
        .bind(group_id)
        .bind(format_vault_role(role))
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn revoke_vault_user_share(&self, vault_id: &str, user_id: &str) -> AppResult<()> {
        let result =
            sqlx::query("DELETE FROM vault_user_shares WHERE vault_id = ? AND user_id = ?")
                .bind(vault_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "User share not found for vault {} and user {}",
                vault_id, user_id
            )));
        }

        Ok(())
    }

    pub async fn revoke_vault_group_share(&self, vault_id: &str, group_id: &str) -> AppResult<()> {
        let result =
            sqlx::query("DELETE FROM vault_group_shares WHERE vault_id = ? AND group_id = ?")
                .bind(vault_id)
                .bind(group_id)
                .execute(&self.pool)
                .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Group share not found for vault {} and group {}",
                vault_id, group_id
            )));
        }

        Ok(())
    }

    pub async fn list_vault_shares(&self, vault_id: &str) -> AppResult<VaultShareList> {
        let owner =
            sqlx::query_as::<_, (Option<String>,)>("SELECT owner_user_id FROM vaults WHERE id = ?")
                .bind(vault_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Vault {} not found", vault_id)))?;

        let user_rows = sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT u.id, u.username, vus.role
            FROM vault_user_shares vus
            INNER JOIN users u ON u.id = vus.user_id
            WHERE vus.vault_id = ?
            ORDER BY u.username ASC
            "#,
        )
        .bind(vault_id)
        .fetch_all(&self.pool)
        .await?;

        let group_rows = sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT g.id, g.name, vgs.role
            FROM vault_group_shares vgs
            INNER JOIN groups g ON g.id = vgs.group_id
            WHERE vgs.vault_id = ?
            ORDER BY g.name ASC
            "#,
        )
        .bind(vault_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(VaultShareList {
            owner_user_id: owner.0,
            user_shares: user_rows
                .into_iter()
                .filter_map(|(principal_id, principal_name, role)| {
                    parse_vault_role(&role).map(|role| VaultShareEntry {
                        principal_type: "user".to_string(),
                        principal_id,
                        principal_name,
                        role,
                    })
                })
                .collect(),
            group_shares: group_rows
                .into_iter()
                .filter_map(|(principal_id, principal_name, role)| {
                    parse_vault_role(&role).map(|role| VaultShareEntry {
                        principal_type: "group".to_string(),
                        principal_id,
                        principal_name,
                        role,
                    })
                })
                .collect(),
        })
    }

    pub async fn delete_vault(&self, id: &str) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM vaults WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Vault {} not found", id)));
        }

        Ok(())
    }

    pub async fn update_vault_timestamp(&self, id: &str) -> AppResult<()> {
        sqlx::query("UPDATE vaults SET updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // Preferences operations
    pub async fn get_preferences(&self) -> AppResult<UserPreferences> {
        self.get_preferences_for_user(None).await
    }

    pub async fn get_preferences_for_user(
        &self,
        user_id: Option<&str>,
    ) -> AppResult<UserPreferences> {
        let row: Option<(String, String, i64, Option<String>, Option<String>)> = if let Some(
            user_id,
        ) = user_id
        {
            sqlx::query_as(
                "SELECT theme, editor_mode, font_size, window_layout, icon_map FROM user_preferences WHERE user_id = ?",
            )
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?
        } else {
            sqlx::query_as(
                "SELECT theme, editor_mode, font_size, window_layout, icon_map FROM preferences WHERE id = 1",
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?
        };

        if let Some((theme, editor_mode, font_size, window_layout, icon_map_raw)) = row {
            let icon_map = icon_map_raw
                .as_deref()
                .and_then(|raw| serde_json::from_str(raw).ok());
            return Ok(UserPreferences {
                theme,
                editor_mode: editor_mode_from_str(&editor_mode),
                font_size: font_size as u16,
                window_layout,
                icon_map,
            });
        }

        if user_id.is_none() {
            let default = UserPreferences::default();
            self.update_preferences_for_user(None, &default).await?;
            return Ok(default);
        }

        let fallback_row: Option<(String, String, i64, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT theme, editor_mode, font_size, window_layout, icon_map FROM preferences WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        let fallback = if let Some((theme, editor_mode, font_size, window_layout, icon_map_raw)) =
            fallback_row
        {
            let icon_map = icon_map_raw
                .as_deref()
                .and_then(|raw| serde_json::from_str(raw).ok());
            UserPreferences {
                theme,
                editor_mode: editor_mode_from_str(&editor_mode),
                font_size: font_size as u16,
                window_layout,
                icon_map,
            }
        } else {
            UserPreferences::default()
        };

        self.update_preferences_for_user(user_id, &fallback).await?;
        Ok(fallback)
    }

    pub async fn update_preferences(&self, prefs: &UserPreferences) -> AppResult<()> {
        self.update_preferences_for_user(None, prefs).await
    }

    pub async fn update_preferences_for_user(
        &self,
        user_id: Option<&str>,
        prefs: &UserPreferences,
    ) -> AppResult<()> {
        let mode_str = editor_mode_to_str(&prefs.editor_mode);
        let now = Utc::now().to_rfc3339();
        let icon_map_json = prefs
            .icon_map
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok());

        if let Some(user_id) = user_id {
            sqlx::query(
                r#"
                INSERT INTO user_preferences (user_id, theme, editor_mode, font_size, window_layout, icon_map, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(user_id) DO UPDATE SET
                    theme = excluded.theme,
                    editor_mode = excluded.editor_mode,
                    font_size = excluded.font_size,
                    window_layout = excluded.window_layout,
                    icon_map = excluded.icon_map,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(user_id)
            .bind(&prefs.theme)
            .bind(mode_str)
            .bind(prefs.font_size as i64)
            .bind(&prefs.window_layout)
            .bind(&icon_map_json)
            .bind(&now)
            .execute(&self.pool)
            .await?;
            return Ok(());
        }

        sqlx::query(
            r#"
            UPDATE preferences
            SET theme = ?, editor_mode = ?, font_size = ?, window_layout = ?, icon_map = ?, updated_at = ?
            WHERE id = 1
            "#,
        )
        .bind(&prefs.theme)
        .bind(mode_str)
        .bind(prefs.font_size as i64)
        .bind(&prefs.window_layout)
        .bind(&icon_map_json)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_preference(&self, key: &str, value: &str) -> AppResult<()> {
        self.update_preference_for_user(None, key, value).await
    }

    pub async fn update_preference_for_user(
        &self,
        user_id: Option<&str>,
        key: &str,
        value: &str,
    ) -> AppResult<()> {
        let mut prefs = self.get_preferences_for_user(user_id).await?;

        match key {
            "theme" => prefs.theme = value.to_string(),
            "editor_mode" => {
                prefs.editor_mode = match value {
                    "raw" => EditorMode::Raw,
                    "side_by_side" => EditorMode::SideBySide,
                    "formatted_raw" => EditorMode::FormattedRaw,
                    "wysiwyg" | "fully_rendered" => EditorMode::FullyRendered,
                    _ => {
                        return Err(AppError::InvalidInput(format!(
                            "Invalid editor mode: {}",
                            value
                        )))
                    }
                }
            }
            "font_size" => {
                let size = value
                    .parse::<u16>()
                    .map_err(|_| AppError::InvalidInput("Invalid font size".to_string()))?;
                prefs.font_size = size;
            }
            "window_layout" => prefs.window_layout = Some(value.to_string()),
            _ => {
                return Err(AppError::InvalidInput(format!(
                    "Invalid preference key: {}",
                    key
                )))
            }
        }

        self.update_preferences_for_user(user_id, &prefs).await
    }

    pub async fn reset_preferences(&self) -> AppResult<()> {
        self.reset_preferences_for_user(None).await
    }

    pub async fn reset_preferences_for_user(&self, user_id: Option<&str>) -> AppResult<()> {
        let default = UserPreferences::default();
        self.update_preferences_for_user(user_id, &default).await
    }

    // Recent files operations
    pub async fn record_recent_file(&self, vault_id: &str, path: &str) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO recent_files (vault_id, path, last_accessed)
            VALUES (?, ?, ?)
            ON CONFLICT(vault_id, path) DO UPDATE SET last_accessed = excluded.last_accessed
            "#,
        )
        .bind(vault_id)
        .bind(path)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        // Enforce limit of 20
        sqlx::query(
            r#"
            DELETE FROM recent_files 
            WHERE vault_id = ? AND path NOT IN (
                SELECT path FROM recent_files 
                WHERE vault_id = ? 
                ORDER BY last_accessed DESC 
                LIMIT 20
            )
            "#,
        )
        .bind(vault_id)
        .bind(vault_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_recent_files(&self, vault_id: &str, limit: i32) -> AppResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT path FROM recent_files 
            WHERE vault_id = ? 
            ORDER BY last_accessed DESC 
            LIMIT ?
            "#,
        )
        .bind(vault_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn log_file_change(
        &self,
        vault_id: &str,
        path: &str,
        event_type: &str,
        etag: Option<&str>,
        old_path: Option<&str>,
        retention_days: u64,
    ) -> AppResult<()> {
        let now_ms = Utc::now().timestamp_millis();

        sqlx::query(
            r#"
            INSERT INTO file_change_log (vault_id, path, event_type, etag, old_path, timestamp)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(vault_id)
        .bind(path)
        .bind(event_type)
        .bind(etag)
        .bind(old_path)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;

        if retention_days > 0 {
            let retention_ms = (retention_days as i64).saturating_mul(24 * 60 * 60 * 1000);
            let cutoff = now_ms.saturating_sub(retention_ms);

            sqlx::query("DELETE FROM file_change_log WHERE timestamp < ?")
                .bind(cutoff)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    pub async fn get_file_changes_since(
        &self,
        vault_id: &str,
        since_unix_ms: i64,
    ) -> AppResult<Vec<crate::models::FileChangeEvent>> {
        let rows = sqlx::query_as::<_, FileChangeLogRow>(
            r#"
            SELECT path, event_type, old_path, timestamp
            FROM file_change_log
            WHERE vault_id = ? AND timestamp > ?
            ORDER BY timestamp ASC
            "#,
        )
        .bind(vault_id)
        .bind(since_unix_ms)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_type = match row.event_type.as_str() {
                "created" => crate::models::FileChangeType::Created,
                "modified" => crate::models::FileChangeType::Modified,
                "deleted" => crate::models::FileChangeType::Deleted,
                "renamed" => crate::models::FileChangeType::Renamed {
                    from: row.old_path.unwrap_or_default(),
                    to: row.path.clone(),
                },
                other => {
                    return Err(AppError::InternalError(format!(
                        "Unknown file change event type in DB: {other}"
                    )));
                }
            };

            let timestamp = chrono::TimeZone::timestamp_millis_opt(&Utc, row.timestamp)
                .single()
                .unwrap_or_else(Utc::now);

            events.push(crate::models::FileChangeEvent {
                vault_id: vault_id.to_string(),
                path: row.path,
                event_type,
                timestamp,
            });
        }

        Ok(events)
    }

    // ── User deactivation / deletion ────────────────────────────────────

    /// Soft-deactivate a user (keeps data, prevents login).
    pub async fn deactivate_user(&self, user_id: &str) -> AppResult<()> {
        let result = sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("User {user_id} not found")));
        }
        Ok(())
    }

    /// Reactivate a previously deactivated user.
    pub async fn reactivate_user(&self, user_id: &str) -> AppResult<()> {
        let result = sqlx::query("UPDATE users SET is_active = 1 WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("User {user_id} not found")));
        }
        Ok(())
    }

    /// Check whether a user account is active.
    pub async fn is_user_active(&self, user_id: &str) -> AppResult<bool> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT is_active FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(row.map(|(v,)| v != 0).unwrap_or(false))
    }

    /// Hard-delete a user and cascade-remove group memberships and vault shares.
    pub async fn delete_user(&self, user_id: &str) -> AppResult<()> {
        // Remove vault shares
        sqlx::query("DELETE FROM vault_user_shares WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        // Remove group memberships
        sqlx::query("DELETE FROM group_members WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        // Remove user preferences
        sqlx::query("DELETE FROM user_preferences WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        // Unset vault ownership (vaults become unowned, not deleted)
        sqlx::query("UPDATE vaults SET owner_user_id = NULL WHERE owner_user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        // Delete the user row
        let result = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("User {user_id} not found")));
        }
        Ok(())
    }

    // ── Failed login tracking ───────────────────────────────────────────

    /// Record a failed login attempt and return the new attempt count.
    pub async fn record_failed_login(&self, user_id: &str) -> AppResult<i64> {
        sqlx::query(
            "UPDATE users SET failed_login_attempts = failed_login_attempts + 1 WHERE id = ?",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        let row: (i64,) =
            sqlx::query_as("SELECT failed_login_attempts FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await
                .map_err(AppError::from)?;
        Ok(row.0)
    }

    /// Lock a user account until a specified time.
    pub async fn lock_user_until(
        &self,
        user_id: &str,
        until: DateTime<Utc>,
    ) -> AppResult<()> {
        sqlx::query("UPDATE users SET locked_until = ? WHERE id = ?")
            .bind(until.to_rfc3339())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Clear failed login attempts and unlock the account after successful login.
    pub async fn clear_failed_logins(&self, user_id: &str) -> AppResult<()> {
        sqlx::query(
            "UPDATE users SET failed_login_attempts = 0, locked_until = NULL WHERE id = ?",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Check if a user is currently locked out. Returns `Some(locked_until)` if
    /// locked, `None` if not.
    pub async fn get_lockout_status(
        &self,
        user_id: &str,
    ) -> AppResult<Option<DateTime<Utc>>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT locked_until FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::from)?;

        if let Some((Some(locked_until_str),)) = row {
            let locked_until = parse_rfc3339_utc(&locked_until_str);
            if locked_until > Utc::now() {
                return Ok(Some(locked_until));
            }
        }
        Ok(None)
    }

    // ── Audit logging ───────────────────────────────────────────────────

    /// Record an audit log entry for a security-relevant event.
    pub async fn write_audit_log(
        &self,
        user_id: Option<&str>,
        username: Option<&str>,
        event_type: &str,
        detail: Option<&str>,
        ip_address: Option<&str>,
        success: bool,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO audit_log (timestamp, user_id, username, event_type, detail, ip_address, success)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&now)
        .bind(user_id)
        .bind(username)
        .bind(event_type)
        .bind(detail)
        .bind(ip_address)
        .bind(if success { 1_i64 } else { 0_i64 })
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get recent audit log entries (newest first), with an optional limit.
    pub async fn get_audit_log(
        &self,
        limit: Option<i64>,
    ) -> AppResult<Vec<AuditLogEntry>> {
        let limit = limit.unwrap_or(100).min(1000);
        let rows = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, String, Option<String>, Option<String>, i64)>(
            "SELECT id, timestamp, user_id, username, event_type, detail, ip_address, success FROM audit_log ORDER BY id DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(
                |(id, timestamp, user_id, username, event_type, detail, ip_address, success)| {
                    AuditLogEntry {
                        id,
                        timestamp: parse_rfc3339_utc(&timestamp),
                        user_id,
                        username,
                        event_type,
                        detail,
                        ip_address,
                        success: success != 0,
                    }
                },
            )
            .collect())
    }

    // ── Session management ──────────────────────────────────────────────

    /// Record a new token session for revocation tracking.
    pub async fn create_session(
        &self,
        token_id: &str,
        user_id: &str,
        expires_at: DateTime<Utc>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO sessions (token_id, user_id, created_at, expires_at) VALUES (?, ?, ?, ?)",
        )
        .bind(token_id)
        .bind(user_id)
        .bind(&now)
        .bind(expires_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Check whether a session has been revoked.
    pub async fn is_session_revoked(&self, token_id: &str) -> AppResult<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT revoked FROM sessions WHERE token_id = ?")
                .bind(token_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::from)?;
        // If the session isn't tracked at all, treat it as valid (backwards compat).
        Ok(row.map(|(v,)| v != 0).unwrap_or(false))
    }

    /// Revoke a specific session.
    pub async fn revoke_session(&self, token_id: &str) -> AppResult<()> {
        sqlx::query("UPDATE sessions SET revoked = 1 WHERE token_id = ?")
            .bind(token_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Revoke all active sessions for a user (log out everywhere).
    pub async fn revoke_all_sessions(&self, user_id: &str) -> AppResult<u64> {
        let result = sqlx::query(
            "UPDATE sessions SET revoked = 1 WHERE user_id = ? AND revoked = 0",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// List active (non-revoked, non-expired) sessions for a user.
    pub async fn list_active_sessions(
        &self,
        user_id: &str,
    ) -> AppResult<Vec<SessionInfo>> {
        let now = Utc::now().to_rfc3339();
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT token_id, created_at, expires_at FROM sessions WHERE user_id = ? AND revoked = 0 AND expires_at > ? ORDER BY created_at DESC",
        )
        .bind(user_id)
        .bind(&now)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(|(token_id, created_at, expires_at)| SessionInfo {
                token_id,
                created_at: parse_rfc3339_utc(&created_at),
                expires_at: parse_rfc3339_utc(&expires_at),
            })
            .collect())
    }

    /// Clean up expired sessions (housekeeping).
    pub async fn purge_expired_sessions(&self) -> AppResult<u64> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
            .bind(&now)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    // ── API keys ────────────────────────────────────────────────────────

    /// Store a new API key record (the hash, not the raw key).
    pub async fn create_api_key(
        &self,
        id: &str,
        name: &str,
        prefix: &str,
        key_hash: &str,
        user_id: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO api_keys (id, name, prefix, key_hash, user_id, created_at, expires_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(prefix)
        .bind(key_hash)
        .bind(user_id)
        .bind(&now)
        .bind(expires_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Look up an API key by its prefix, returning (id, key_hash, user_id, expires_at, revoked).
    pub async fn get_api_key_by_prefix(
        &self,
        prefix: &str,
    ) -> AppResult<Option<(String, String, String, Option<String>, bool)>> {
        let row: Option<(String, String, String, Option<String>, i64)> = sqlx::query_as(
            "SELECT id, key_hash, user_id, expires_at, revoked FROM api_keys WHERE prefix = ?",
        )
        .bind(prefix)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(|(id, hash, uid, exp, rev)| (id, hash, uid, exp, rev != 0)))
    }

    /// List all API keys for a user (without hashes).
    pub async fn list_api_keys(&self, user_id: &str) -> AppResult<Vec<ApiKeyInfo>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64)>(
            "SELECT id, name, prefix, user_id, created_at, expires_at, revoked FROM api_keys WHERE user_id = ? ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(|(id, name, prefix, user_id, created_at, expires_at, revoked)| ApiKeyInfo {
                id,
                name,
                prefix,
                user_id,
                created_at: parse_rfc3339_utc(&created_at),
                expires_at: expires_at.map(|s| parse_rfc3339_utc(&s)),
                revoked: revoked != 0,
            })
            .collect())
    }

    /// Revoke an API key.
    pub async fn revoke_api_key(&self, key_id: &str, user_id: &str) -> AppResult<()> {
        let result = sqlx::query(
            "UPDATE api_keys SET revoked = 1 WHERE id = ? AND user_id = ?",
        )
        .bind(key_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("API key not found".to_string()));
        }
        Ok(())
    }

    // ── Vault visibility / ownership transfer ───────────────────────────

    /// Set vault visibility to 'public' or 'private'.
    pub async fn set_vault_visibility(
        &self,
        vault_id: &str,
        visibility: &str,
    ) -> AppResult<()> {
        let result = sqlx::query("UPDATE vaults SET visibility = ? WHERE id = ?")
            .bind(visibility)
            .bind(vault_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Vault not found".to_string()));
        }
        Ok(())
    }

    /// Get vault visibility ('public' or 'private').
    pub async fn get_vault_visibility(&self, vault_id: &str) -> AppResult<String> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT visibility FROM vaults WHERE id = ?")
                .bind(vault_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::from)?;
        Ok(row.map(|(v,)| v).unwrap_or_else(|| "private".to_string()))
    }

    /// Transfer vault ownership to a new user.
    pub async fn transfer_vault_ownership(
        &self,
        vault_id: &str,
        new_owner_id: &str,
    ) -> AppResult<()> {
        // Set the new owner.
        sqlx::query("UPDATE vaults SET owner_user_id = ? WHERE id = ?")
            .bind(new_owner_id)
            .bind(vault_id)
            .execute(&self.pool)
            .await?;

        // Ensure the new owner has an 'owner' share entry.
        sqlx::query(
            r#"
            INSERT INTO vault_user_shares (vault_id, user_id, role, created_at)
            VALUES (?, ?, 'owner', ?)
            ON CONFLICT (vault_id, user_id) DO UPDATE SET role = 'owner'
            "#,
        )
        .bind(vault_id)
        .bind(new_owner_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ── TOTP 2FA ────────────────────────────────────────────────────────

    /// Store a TOTP secret for a user (enrollment step).
    pub async fn set_totp_secret(
        &self,
        user_id: &str,
        secret: &str,
        backup_codes: &[String],
    ) -> AppResult<()> {
        let codes_json = serde_json::to_string(backup_codes)
            .map_err(|e| AppError::InternalError(format!("Failed to serialize backup codes: {e}")))?;
        sqlx::query(
            "UPDATE users SET totp_secret = ?, totp_backup_codes = ? WHERE id = ?",
        )
        .bind(secret)
        .bind(&codes_json)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Enable TOTP after successful verification.
    pub async fn enable_totp(&self, user_id: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET totp_enabled = 1 WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Disable TOTP and clear the secret.
    pub async fn disable_totp(&self, user_id: &str) -> AppResult<()> {
        sqlx::query(
            "UPDATE users SET totp_enabled = 0, totp_secret = NULL, totp_backup_codes = NULL WHERE id = ?",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get TOTP state for a user: (totp_enabled, totp_secret, backup_codes_json).
    pub async fn get_totp_state(
        &self,
        user_id: &str,
    ) -> AppResult<(bool, Option<String>, Option<String>)> {
        let row: Option<(i64, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT totp_enabled, totp_secret, totp_backup_codes FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        match row {
            Some((enabled, secret, codes)) => Ok((enabled != 0, secret, codes)),
            None => Err(AppError::NotFound("User not found".to_string())),
        }
    }

    /// Consume a backup code (marks it as used by removing from the list).
    pub async fn consume_backup_code(
        &self,
        user_id: &str,
        code: &str,
    ) -> AppResult<bool> {
        let (_, _, codes_json) = self.get_totp_state(user_id).await?;
        let Some(codes_json) = codes_json else {
            return Ok(false);
        };
        let mut codes: Vec<String> = serde_json::from_str(&codes_json).unwrap_or_default();
        if let Some(pos) = codes.iter().position(|c| c == code) {
            codes.remove(pos);
            let updated = serde_json::to_string(&codes)
                .map_err(|e| AppError::InternalError(e.to_string()))?;
            sqlx::query("UPDATE users SET totp_backup_codes = ? WHERE id = ?")
                .bind(&updated)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // ── Invitations ─────────────────────────────────────────────────────

    /// Create an invitation.
    pub async fn create_invitation(
        &self,
        id: &str,
        token: &str,
        role: &str,
        vault_id: Option<&str>,
        created_by: &str,
        expires_at: DateTime<Utc>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO invitations (id, token, role, vault_id, created_by_user_id, created_at, expires_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id)
        .bind(token)
        .bind(role)
        .bind(vault_id)
        .bind(created_by)
        .bind(&now)
        .bind(expires_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Look up an invitation by token.
    pub async fn get_invitation_by_token(
        &self,
        token: &str,
    ) -> AppResult<Option<(String, String, Option<String>, String, String, String, bool, Option<String>)>> {
        let row = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, Option<String>)>(
            "SELECT id, role, vault_id, created_by_user_id, created_at, expires_at, accepted, accepted_by_user_id FROM invitations WHERE token = ?",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row.map(|(id, role, vid, cby, cat, eat, acc, aby)| {
            (id, role, vid, cby, cat, eat, acc != 0, aby)
        }))
    }

    /// Mark an invitation as accepted.
    pub async fn accept_invitation(
        &self,
        invite_id: &str,
        accepted_by_user_id: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE invitations SET accepted = 1, accepted_by_user_id = ? WHERE id = ?",
        )
        .bind(accepted_by_user_id)
        .bind(invite_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List all invitations created by a user.
    pub async fn list_invitations_by_creator(
        &self,
        user_id: &str,
    ) -> AppResult<Vec<crate::models::InviteInfo>> {
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String, String, String, i64, Option<String>)>(
            "SELECT id, token, role, vault_id, created_by_user_id, created_at, expires_at, accepted, accepted_by_user_id FROM invitations WHERE created_by_user_id = ? ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(|(id, token, role, vault_id, created_by, created_at, expires_at, accepted, accepted_by)| {
                crate::models::InviteInfo {
                    id,
                    token,
                    role,
                    vault_id,
                    created_by,
                    created_at: parse_rfc3339_utc(&created_at),
                    expires_at: parse_rfc3339_utc(&expires_at),
                    accepted: accepted != 0,
                    accepted_by,
                }
            })
            .collect())
    }
}

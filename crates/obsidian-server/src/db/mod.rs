use crate::error::{AppError, AppResult};
use crate::models::{
    AdminUser, EditorMode, GroupInfo, GroupMember, UserPreferences, Vault, VaultRole, VaultRow,
    VaultShareEntry, VaultShareList,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::Utc;
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
                updated_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

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
        let rows = sqlx::query_as::<_, (String, String, i64, i64, String)>(
            "SELECT id, username, is_admin, must_change_password, created_at FROM users ORDER BY username ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(
                |(id, username, is_admin, must_change_password, created_at)| AdminUser {
                    id,
                    username,
                    is_admin: is_admin != 0,
                    must_change_password: must_change_password != 0,
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
        let row: Option<(String, String, i64, Option<String>)> = if let Some(user_id) = user_id {
            sqlx::query_as(
                "SELECT theme, editor_mode, font_size, window_layout FROM user_preferences WHERE user_id = ?",
            )
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?
        } else {
            sqlx::query_as(
                "SELECT theme, editor_mode, font_size, window_layout FROM preferences WHERE id = 1",
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?
        };

        if let Some((theme, editor_mode, font_size, window_layout)) = row {
            return Ok(UserPreferences {
                theme,
                editor_mode: editor_mode_from_str(&editor_mode),
                font_size: font_size as u16,
                window_layout,
            });
        }

        if user_id.is_none() {
            let default = UserPreferences::default();
            self.update_preferences_for_user(None, &default).await?;
            return Ok(default);
        }

        let fallback_row: Option<(String, String, i64, Option<String>)> = sqlx::query_as(
            "SELECT theme, editor_mode, font_size, window_layout FROM preferences WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        let fallback = if let Some((theme, editor_mode, font_size, window_layout)) = fallback_row {
            UserPreferences {
                theme,
                editor_mode: editor_mode_from_str(&editor_mode),
                font_size: font_size as u16,
                window_layout,
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

        if let Some(user_id) = user_id {
            sqlx::query(
                r#"
                INSERT INTO user_preferences (user_id, theme, editor_mode, font_size, window_layout, updated_at)
                VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT(user_id) DO UPDATE SET
                    theme = excluded.theme,
                    editor_mode = excluded.editor_mode,
                    font_size = excluded.font_size,
                    window_layout = excluded.window_layout,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(user_id)
            .bind(&prefs.theme)
            .bind(mode_str)
            .bind(prefs.font_size as i64)
            .bind(&prefs.window_layout)
            .bind(&now)
            .execute(&self.pool)
            .await?;
            return Ok(());
        }

        sqlx::query(
            r#"
            UPDATE preferences
            SET theme = ?, editor_mode = ?, font_size = ?, window_layout = ?, updated_at = ?
            WHERE id = 1
            "#,
        )
        .bind(&prefs.theme)
        .bind(mode_str)
        .bind(prefs.font_size as i64)
        .bind(&prefs.window_layout)
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
}

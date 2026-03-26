# Deployment Guide

## Quick Start (Docker)

```bash
# 1. Clone and build
git clone <repo-url> obsidian-host
cd obsidian-host

# 2. Generate a JWT secret
export JWT_SECRET=$(openssl rand -hex 32)

# 3. Start with Docker Compose
docker compose up -d

# 4. Log in at http://localhost:8080
#    Default admin: admin / changeme-on-first-login
#    You will be prompted to change the password on first login.
```

### Customizing Docker

Edit `docker-compose.yml` environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `OBSIDIAN__AUTH__ENABLED` | `true` | Enable/disable authentication |
| `OBSIDIAN__AUTH__JWT_SECRET` | (auto) | Stable JWT signing secret |
| `OBSIDIAN__AUTH__BOOTSTRAP_ADMIN_USERNAME` | `admin` | Initial admin username |
| `OBSIDIAN__AUTH__BOOTSTRAP_ADMIN_PASSWORD` | — | Initial admin password |
| `RATE_LIMIT_REQUESTS` | `120` | Max requests per 60s per IP |
| `RUST_LOG` | `info` | Log level |

## Quick Start (Binary)

```bash
# 1. Build
npm --prefix frontend ci && npm --prefix frontend run build
cargo build --release

# 2. Configure
cp config.toml config.local.toml
# Edit config.local.toml:
#   - Set auth.enabled = true
#   - Set auth.jwt_secret = "<random-hex>"
#   - Set auth.bootstrap_admin_username = "admin"
#   - Set auth.bootstrap_admin_password = "<strong-password>"

# 3. Run
./target/release/obsidian-host
# Server starts at http://127.0.0.1:8080
```

## Desktop App

```bash
# Build
cargo build --release -p obsidian-desktop

# Run
./target/release/obsidian-desktop
```

The desktop app connects to a running server. Enter the server URL, username, and password to log in. If you previously logged in, the app will attempt auto-login using a saved refresh token.

### Desktop Features
- **Cloud mode**: Connect to a remote server
- **Standalone mode**: Server runs locally
- **Hybrid mode**: Remote server + local mirror for faster reads

## Production Recommendations

### Reverse Proxy (TLS)

The server is HTTP-only. Use nginx or Caddy in front for HTTPS:

```nginx
server {
    listen 443 ssl;
    server_name notes.example.com;

    ssl_certificate /etc/ssl/cert.pem;
    ssl_certificate_key /etc/ssl/key.pem;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

### CORS

Update `config.toml` `[cors].allowed_origins` to include your domain:

```toml
[cors]
allowed_origins = ["https://notes.example.com"]
```

### Authentication Providers

**Password** (default):
```toml
[auth]
provider = "password"
```

**LDAP/Active Directory**:
```toml
[auth]
provider = "ldap"
ldap_url = "ldap://ldap.example.com:389"
ldap_base_dn = "ou=people,dc=example,dc=com"
ldap_bind_dn = "cn=admin,dc=example,dc=com"
ldap_bind_password = "secret"
```

**OAuth2/OIDC** (Google, GitHub, etc.):
```toml
[auth]
provider = "oidc"
oidc_issuer_url = "https://accounts.google.com"
oidc_client_id = "your-client-id"
oidc_client_secret = "your-client-secret"
oidc_redirect_uri = "https://notes.example.com/api/auth/oidc/callback"
```

### API Keys

Users can generate API keys for programmatic access:

```bash
# Create a key
curl -X POST http://localhost:8080/api/auth/api-keys \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"name": "my-script", "expires_in_days": 90}'

# Use the key
curl http://localhost:8080/api/vaults \
  -H "X-API-Key: obh_<key>"
```

### Health Check

```bash
curl http://localhost:8080/api/health
# {"status":"healthy","database":"connected"}
```

### Backup

The server uses a single SQLite file. To back up:

```bash
sqlite3 /data/obsidian-host.db ".backup /backups/obsidian-host-$(date +%F).db"
```

Vault files are regular filesystem files in the configured `vault.base_dir`.

## API Endpoints Summary

| Category | Endpoint | Auth |
|----------|----------|------|
| Health | `GET /api/health` | No |
| Login | `POST /api/auth/login` | No |
| OIDC | `GET /api/auth/oidc/authorize` | No |
| OIDC | `GET /api/auth/oidc/callback` | No |
| Invites | `POST /api/invitations/accept` | No |
| Token refresh | `POST /api/auth/refresh` | No |
| Profile | `GET /api/auth/me` | Yes |
| Change password | `POST /api/auth/change-password` | Yes |
| Sessions | `GET /api/auth/sessions` | Yes |
| Revoke all | `POST /api/auth/revoke-all` | Yes |
| TOTP enroll | `POST /api/auth/totp/enroll` | Yes |
| TOTP verify | `POST /api/auth/totp/verify` | Yes |
| TOTP disable | `POST /api/auth/totp/disable` | Yes |
| API keys | `POST/GET/DELETE /api/auth/api-keys` | Yes |
| Vaults | `POST/GET/DELETE /api/vaults` | Yes |
| Files | `GET/PUT/POST/DELETE /api/vaults/{id}/files` | Yes |
| Search | `GET /api/vaults/{id}/search` | Yes |
| WebSocket | `GET /api/ws` | Yes |
| Admin users | `GET/POST /api/admin/users` | Admin |
| Audit log | `GET /api/admin/audit-log` | Admin |
| Bulk import | `POST /api/admin/users/bulk-import` | Admin |

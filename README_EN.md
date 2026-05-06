# BrookFile

English | [中文](./README.md)

A cloud storage system designed for personal/home users, an experimental project fully developed by AI.

## Features

### Core Features

- **Cloud Storage Basics** - File upload, download, browsing, rename, move, copy, search, and compressed download
- **Multi-Tenancy** - Supports multiple independent users with admin account management and complete data isolation
- **File Sharing** - Share files or folders via links with optional password protection and expiration
- **Recycle Bin** - Deleted files go to the recycle bin with support for restoration or permanent deletion

### Extended Features

- **Encrypted Cloud Backup** - Secure file encryption and cloud backup
- **Cloud Notes** - Convenient cloud-based note management with end-to-end encryption
- **Password Manager** - End-to-end encrypted password storage and management
- **WebDAV Support** - Standard WebDAV protocol support, compatible with various clients

## Roadmap

- **E-book Reader** - Online reading support for multiple e-book formats
- **Music Library** - Online music management and streaming playback
- **Video Transcoding** - Online video transcoding and playback
- **Photo Browser** - Photo browsing by time/content with fast search

## Deployment

1. Download the appropriate version from [Release](../../releases) (currently only Windows/Linux x86_64 pre-built binaries are available)
2. Extract and run the executable
3. Open `http://<IP>:3000` in your browser and follow the prompts to initialize the system

> **Other Platforms**: macOS, ARM Linux, ARM Windows, etc. should theoretically work, but the author doesn't have the corresponding devices to provide pre-built binaries. If needed, it's recommended to use AI assistance to compile on the target platform.

### HTTPS Deployment (Recommended)

For production use, HTTPS deployment via Nginx reverse proxy is recommended. End-to-end encryption features such as notebooks and password vault depend on the WebCrypto API, which is only available in HTTPS environments. Even without considering secure transmission requirements, WebCrypto performance under HTTPS is significantly better than the JavaScript fallback under HTTP.

Nginx reverse proxy reference configuration:

```nginx
server {
    listen 443 ssl;
    server_name your-domain.com;

    ssl_certificate     /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    client_max_body_size 0;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Configuration

`config.json` only contains deployment-time settings (restart required after changes):

```json
{
    "port": 3000,
    "argon2": {
        "m_cost": 19456,
        "t_cost": 2,
        "p_cost": 1
    }
}
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `port` | integer | `3000` | Server listening port |
| `argon2.m_cost` | integer | `19456` | Argon2 memory cost (KB), affects memory usage for password hashing |
| `argon2.t_cost` | integer | `2` | Argon2 iterations, affects password hashing computation time |
| `argon2.p_cost` | integer | `1` | Argon2 parallelism, affects thread count for password hashing |

> **Note**: The `argon2` parameters affect the hash strength of user login passwords and backup encryption passwords. After modifying these parameters, existing passwords will no longer validate — you'll need to delete `database.db` and reinitialize the system. On low-performance devices (e.g., routers), you can reduce `m_cost` (e.g., `4096`) and `t_cost` (e.g., `1`) to decrease memory usage and login time, but this will reduce resistance to brute-force attacks.

### System Settings

After logging in as admin, the "Settings" menu allows online configuration of:

| Setting | Takes Effect | Description |
|---------|-------------|-------------|
| System Name | Immediately | Displayed on login page, browser title, etc. |
| System Logo | Immediately | Custom brand icon shown in sidebar and login page |
| Session Timeout | After restart | How long before idle users are logged out (seconds) |
| Notebook Fulltext Search | After restart | Disabling saves runtime memory; note search only matches titles |
| Rebuild Notebook Index | Immediately | Manually trigger fulltext index rebuild for all non-encrypted notebooks |

## Development

This project is fully developed by AI (GLM-5/5.1).

### Tech Stack

#### Backend
- **Rust** - High-performance systems programming language
- **Actix Web** - Powerful asynchronous web framework
- **SQLite (rusqlite)** - Lightweight embedded database
- **r2d2** - Database connection pool
- **Serde** - Serialization/deserialization framework
- **HMAC/SHA256** - Secure cryptographic algorithms

#### Frontend
- **Vue 3** - Progressive JavaScript framework
- **TypeScript** - Type-safe JavaScript superset
- **Vite** - Next-generation frontend build tool
- **Element Plus** - Vue 3 UI component library
- **Pinia** - Vue 3 state management
- **Vue Router** - Official router
- **Vue I18n** - Internationalization support
- **Tailwind CSS** - Utility-first CSS framework
- **Axios** - HTTP client

#### Testing
- **Python** - Automated API testing

### Project Structure

```
BrookFile/
├── backend/           # Rust backend service
│   ├── src/
│   │   ├── handlers/  # Request handlers
│   │   ├── middleware/ # Middleware
│   │   ├── models/    # Data models
│   │   ├── storage/   # Storage module
│   │   ├── backup/    # Backup module
│   │   ├── restore/   # Restore module
│   │   ├── search/    # Search module
│   │   └── ...
│   └── Cargo.toml
├── frontend/          # Vue frontend application
│   ├── src/
│   │   ├── api/       # API interfaces
│   │   ├── components/# Components
│   │   ├── views/     # Page views
│   │   ├── stores/    # State management
│   │   ├── i18n/      # Internationalization
│   │   ├── router/    # Router configuration
│   │   └── ...
│   └── package.json
├── tests/             # Python automated tests
│   ├── backend/       # Backend API tests
│   ├── frontend/      # Frontend page tests
│   ├── common.py      # Common test utilities
│   └── run_all.py     # Run all tests
├── api/               # API documentation
├── database/          # Database documentation
├── build.py           # Build script
```

### Quick Start

#### Prerequisites
- Rust 1.70+
- Node.js 18+
- Python 3.8+ (for testing)

#### Start Backend

```bash
cd backend
cargo run
```

#### Start Frontend

```bash
cd frontend
npm install
npm run dev
```

#### Run Tests

```bash
cd tests
pip install -r requirements.txt
python run_all.py
```

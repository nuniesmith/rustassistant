# Deployment Guide

This directory contains deployment configurations for running Rustassistant services in production.

## GitHub Background Sync Daemon

The GitHub sync daemon continuously syncs your GitHub repositories to the local database, enabling fast local queries and reducing LLM API costs.

### Quick Start

1. **Build the daemon:**
   ```bash
   cargo build --release --bin github-sync-daemon
   ```

2. **Set up environment variables:**
   ```bash
   # Copy .env.example to .env and configure:
   GITHUB_TOKEN=ghp_your_token_here
   DATABASE_URL=sqlite:/opt/rustassistant/data/rustassistant.db
   
   # Optional: Configure sync intervals (in seconds)
   GITHUB_FULL_SYNC_INTERVAL=86400        # 24 hours (default)
   GITHUB_INCREMENTAL_SYNC_INTERVAL=3600  # 1 hour (default)
   GITHUB_MAX_ITEMS_PER_REPO=100          # Limit items per repo (optional)
   GITHUB_SYNC_ON_STARTUP=true            # Sync immediately on start (default)
   
   # Optional: Configure logging
   RUST_LOG=info,rustassistant=debug
   ```

3. **Run the daemon:**
   ```bash
   ./target/release/github-sync-daemon
   ```

### Running as a Systemd Service

For production deployments on Linux:

1. **Install the service:**
   ```bash
   # Copy the service file
   sudo cp deployment/github-sync-daemon.service /etc/systemd/system/
   
   # Create a dedicated user (recommended)
   sudo useradd -r -s /bin/false rustassistant
   
   # Set up the application directory
   sudo mkdir -p /opt/rustassistant/data
   sudo cp target/release/github-sync-daemon /opt/rustassistant/
   sudo cp .env /opt/rustassistant/
   sudo chown -R rustassistant:rustassistant /opt/rustassistant
   ```

2. **Edit the service file if needed:**
   ```bash
   sudo nano /etc/systemd/system/github-sync-daemon.service
   ```
   
   Adjust paths, user, and resource limits as needed.

3. **Enable and start the service:**
   ```bash
   # Reload systemd to pick up the new service
   sudo systemctl daemon-reload
   
   # Enable auto-start on boot
   sudo systemctl enable github-sync-daemon
   
   # Start the service
   sudo systemctl start github-sync-daemon
   
   # Check status
   sudo systemctl status github-sync-daemon
   ```

4. **View logs:**
   ```bash
   # Follow logs in real-time
   sudo journalctl -u github-sync-daemon -f
   
   # View recent logs
   sudo journalctl -u github-sync-daemon -n 100
   
   # View logs since last boot
   sudo journalctl -u github-sync-daemon -b
   ```

5. **Manage the service:**
   ```bash
   # Stop the service
   sudo systemctl stop github-sync-daemon
   
   # Restart the service
   sudo systemctl restart github-sync-daemon
   
   # Disable auto-start
   sudo systemctl disable github-sync-daemon
   ```

### Configuration Options

#### Sync Intervals

- **GITHUB_FULL_SYNC_INTERVAL**: How often to perform a complete sync of all repositories
  - Default: `86400` (24 hours)
  - Recommended: 12-24 hours
  - Full sync fetches all data fresh from GitHub

- **GITHUB_INCREMENTAL_SYNC_INTERVAL**: How often to check for updates
  - Default: `3600` (1 hour)
  - Recommended: 30 minutes - 2 hours
  - Incremental sync only fetches new/changed data

#### Resource Limits

- **GITHUB_MAX_ITEMS_PER_REPO**: Limit how many items to sync per repository
  - Default: `None` (unlimited)
  - Set to `100` or `500` for large repos to limit API usage

#### Startup Behavior

- **GITHUB_SYNC_ON_STARTUP**: Whether to sync immediately when daemon starts
  - Default: `true`
  - Set to `false` if you want to wait for the first interval

### Monitoring

The daemon logs important events:

- **INFO**: Normal operation (sync start/complete, intervals)
- **DEBUG**: Detailed sync progress (repositories, issues, commits)
- **WARN**: Non-fatal issues (API rate limits, skipped items)
- **ERROR**: Fatal errors (database issues, auth failures)

Example log output:
```
INFO rustassistant::github::sync: Starting GitHub sync with options: ...
INFO rustassistant::github::sync: Syncing repositories from GitHub
INFO rustassistant::github::sync: Found 1 repositories to sync
DEBUG rustassistant::github::client: GET /repos/owner/repo/commits (page 1)
INFO rustassistant::github::sync: Sync complete: 1 repos, 0 issues, 0 PRs, 601 commits
```

### Troubleshooting

#### Service won't start
```bash
# Check for errors
sudo systemctl status github-sync-daemon
sudo journalctl -u github-sync-daemon -n 50

# Common issues:
# - GITHUB_TOKEN not set in .env
# - Database file not accessible
# - Port conflicts (if using webhooks)
```

#### High memory/CPU usage
```bash
# Reduce sync frequency
GITHUB_INCREMENTAL_SYNC_INTERVAL=7200  # 2 hours instead of 1

# Limit items per repo
GITHUB_MAX_ITEMS_PER_REPO=100

# Adjust systemd limits in the service file
MemoryLimit=256M
CPUQuota=25%
```

#### Rate limit issues
```bash
# Check your rate limit status
cargo run --bin rustassistant -- github rate-limit

# Increase sync intervals
GITHUB_FULL_SYNC_INTERVAL=172800       # 48 hours
GITHUB_INCREMENTAL_SYNC_INTERVAL=7200  # 2 hours
```

### Security Best Practices

1. **Protect your GitHub token:**
   ```bash
   # Ensure .env is not world-readable
   chmod 600 /opt/rustassistant/.env
   ```

2. **Use a dedicated user:**
   - Never run as root
   - Use the systemd service with a dedicated user

3. **Limit permissions:**
   - Database directory should be writable only by the service user
   - Binary should be owned by root, executable by the service user

4. **Token scopes:**
   - Use fine-grained tokens with minimal scopes
   - Read-only access is sufficient for syncing
   - Recommended scopes: `repo`, `read:org` (if syncing org repos)

### Performance Tuning

For large repositories (1000+ commits, 100+ issues):

1. **Adjust database settings:**
   ```sql
   PRAGMA journal_mode=WAL;
   PRAGMA synchronous=NORMAL;
   PRAGMA cache_size=10000;
   ```

2. **Limit sync scope:**
   ```bash
   # Only sync specific repos
   cargo run --bin rustassistant -- github sync --repo owner/repo
   
   # Or configure in database:
   sqlite3 data/rustassistant.db "UPDATE github_repositories SET sync_enabled=0 WHERE full_name NOT IN ('owner/repo1', 'owner/repo2')"
   ```

3. **Use incremental sync:**
   - Full syncs are expensive
   - Rely on incremental syncs for regular updates
   - Only run full syncs when needed (weekly/monthly)

### Docker Deployment (Coming Soon)

A Docker image and docker-compose configuration will be provided for easier deployment.

### Health Checks

To monitor daemon health:

```bash
# Check last sync time
cargo run --bin rustassistant -- github stats

# Query database directly
sqlite3 data/rustassistant.db "SELECT datetime(MAX(last_synced_at), 'unixepoch') FROM github_repositories"

# Set up monitoring alerts if last sync is too old
# Example: Alert if no sync in last 2 hours
```

### Backup

The SQLite database contains all synced data:

```bash
# Back up database
sqlite3 data/rustassistant.db ".backup backup_$(date +%Y%m%d).db"

# Or use filesystem backup
cp data/rustassistant.db backups/rustassistant_$(date +%Y%m%d_%H%M%S).db
```

### Upgrading

When upgrading Rustassistant:

1. Stop the service:
   ```bash
   sudo systemctl stop github-sync-daemon
   ```

2. Build new version:
   ```bash
   git pull
   cargo build --release --bin github-sync-daemon
   ```

3. Run migrations if needed:
   ```bash
   cargo run --example github_migration
   ```

4. Replace binary:
   ```bash
   sudo cp target/release/github-sync-daemon /opt/rustassistant/
   ```

5. Restart service:
   ```bash
   sudo systemctl start github-sync-daemon
   ```

---

For more information, see the main [GitHub Integration documentation](../GITHUB_INTEGRATION.md).
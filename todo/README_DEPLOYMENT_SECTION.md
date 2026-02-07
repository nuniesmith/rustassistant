# Deployment Guide

RustAssistant uses Docker for containerized deployment with zero host filesystem dependencies.

## Quick Start

### Prerequisites

- Docker and Docker Compose (v1.27+)
- GitHub token (optional, for private repositories)
- 1GB+ disk space

### 1. Clone the Repository

```bash
git clone https://github.com/yourusername/rustassistant.git
cd rustassistant
```

### 2. Configure Environment

Create a `.env` file:

```bash
# Required
DATABASE_URL=sqlite:/app/data/rustassistant.db
REPOS_DIR=/app/repos
XAI_API_KEY=your_xai_api_key_here

# Optional - for private GitHub repositories
GITHUB_TOKEN=ghp_your_github_token_here

# Optional - customize behavior
AUTO_SCAN_ENABLED=true
AUTO_SCAN_INTERVAL=60
AUTO_SCAN_MAX_CONCURRENT=2
RUST_LOG=info,rustassistant=debug
```

### 3. Build and Deploy

```bash
# Build the Docker image
docker compose build

# Start services
docker compose up -d

# Verify deployment
docker compose logs -f rustassistant
```

### 4. Access the Web UI

Open your browser to `http://localhost:3001`

Default ports:
- Web UI: `3001`
- Redis: `6379` (internal)

---

## Architecture

### Container Structure

```
rustassistant/
├── Docker Containers
│   ├── rustassistant (main application)
│   └── redis (LLM response cache)
│
├── Named Volumes (Docker-managed)
│   ├── rustassistant_data (SQLite database)
│   ├── rustassistant_repos_data (cloned repositories)
│   └── rustassistant_redis_data (Redis persistence)
│
└── Application Files (baked into image)
    ├── /app/static (web assets)
    ├── /app/config (configuration)
    └── /app/bin (compiled binary)
```

### Data Persistence

All data is stored in **Docker named volumes** (not bind mounts):

- **rustassistant_data** - SQLite database, scan results, notes
- **rustassistant_repos_data** - Git repositories (cloned at runtime)
- **rustassistant_redis_data** - LLM response cache

This architecture ensures:
- ✅ Portable deployment (works on any Docker host)
- ✅ No host filesystem dependencies
- ✅ Automatic repository syncing from git
- ✅ Easy backup/restore procedures

---

## Repository Management

### How It Works

RustAssistant automatically clones and updates repositories from their git URLs:

1. **Add Repository** via Web UI (`/repos/add`)
   - Provide git URL: `https://github.com/username/repo.git`
   - Repository is cloned on first scan
   
2. **Automatic Updates**
   - Before each scan, `git pull` fetches latest changes
   - Shallow clones (`--depth=1`) save disk space
   
3. **Authentication**
   - Public repos: No token needed
   - Private repos: Set `GITHUB_TOKEN` environment variable

### Adding a Repository

**Via Web UI:**
1. Navigate to http://localhost:3001/repos
2. Click "Add Repository"
3. Enter git URL: `https://github.com/username/repo.git`
4. Enter repo name: `repo-name`
5. Click "Add"

**Via CLI:**
```bash
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
  "INSERT INTO repositories (id, path, name, git_url, auto_scan_enabled, scan_interval_minutes, created_at, updated_at) \
   VALUES ('$(uuidgen)', '/app/repos/myrepo', 'myrepo', 'https://github.com/username/repo.git', 1, 60, strftime('%s', 'now'), strftime('%s', 'now'));"
```

---

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | `sqlite:/app/data/rustassistant.db` | SQLite database path |
| `REPOS_DIR` | Yes | `/app/repos` | Repository clone directory |
| `XAI_API_KEY` | Yes | - | X.AI/Grok API key |
| `GITHUB_TOKEN` | No | - | GitHub token for private repos |
| `AUTO_SCAN_ENABLED` | No | `true` | Enable automatic scanning |
| `AUTO_SCAN_INTERVAL` | No | `60` | Default scan interval (minutes) |
| `AUTO_SCAN_MAX_CONCURRENT` | No | `2` | Max concurrent scans |
| `RUST_LOG` | No | `info,rustassistant=debug` | Logging level |
| `PORT` | No | `3001` | Web UI port |
| `HOST` | No | `0.0.0.0` | Web UI bind address |
| `REDIS_URL` | No | `redis://redis:6379` | Redis connection URL |

### Obtaining API Keys

**X.AI/Grok API Key:**
1. Visit https://console.x.ai
2. Create an account or sign in
3. Navigate to API Keys
4. Generate a new key
5. Add to `.env`: `XAI_API_KEY=xai-...`

**GitHub Token (for private repos):**
1. Visit https://github.com/settings/tokens
2. Click "Generate new token" (classic)
3. Select scopes: `repo` (full control of private repositories)
4. Generate token
5. Add to `.env`: `GITHUB_TOKEN=ghp_...`

---

## Backup and Restore

### Backup Volumes

```bash
# Create backup directory
mkdir -p backups

# Backup database
docker run --rm \
  -v rustassistant_data:/data \
  -v $(pwd)/backups:/backup \
  alpine tar czf /backup/database-$(date +%Y%m%d).tar.gz -C /data .

# Backup repositories (optional)
docker run --rm \
  -v rustassistant_repos_data:/data \
  -v $(pwd)/backups:/backup \
  alpine tar czf /backup/repos-$(date +%Y%m%d).tar.gz -C /data .
```

### Restore Volumes

```bash
# Stop containers
docker compose down

# Restore database
docker run --rm \
  -v rustassistant_data:/data \
  -v $(pwd)/backups:/backup \
  alpine sh -c "rm -rf /data/* && tar xzf /backup/database-YYYYMMDD.tar.gz -C /data"

# Restart
docker compose up -d
```

### Automated Backups

Add to crontab:

```bash
# Daily backup at 2 AM
0 2 * * * cd /path/to/rustassistant && docker run --rm -v rustassistant_data:/data -v $(pwd)/backups:/backup alpine tar czf /backup/database-$(date +\%Y\%m\%d).tar.gz -C /data .
```

---

## Monitoring

### View Logs

```bash
# All logs
docker compose logs -f

# Rustassistant only
docker compose logs -f rustassistant

# Last 100 lines
docker compose logs --tail=100 rustassistant

# Filter for errors
docker compose logs rustassistant | grep -i error
```

### Health Check

```bash
# Check service health
curl http://localhost:3001/health

# Expected response:
# {"status":"healthy","version":"0.1.0"}

# Check container status
docker compose ps
```

### Database Queries

```bash
# Connect to database
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db

# Repository status
SELECT name, auto_scan_enabled, scan_interval_minutes, datetime(last_scan_check, 'unixepoch') 
FROM repositories;

# Scan activity
SELECT * FROM scan_events ORDER BY created_at DESC LIMIT 20;

# Repository health
SELECT * FROM repository_health;

# Exit
.quit
```

---

## Updating

### Update Application

```bash
# Pull latest code
git pull

# Rebuild and restart
docker compose down
docker compose build
docker compose up -d

# Verify
docker compose logs -f rustassistant
```

### Update Repositories

Repositories are automatically updated before each scan. To force an update:

```bash
# Via database
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
  "UPDATE repositories SET last_scan_check = NULL;"

# Wait for next scan cycle (1 minute)
docker compose logs -f rustassistant | grep -i "Cloning\|Updating"
```

---

## Troubleshooting

### Container Won't Start

```bash
# Check logs
docker compose logs rustassistant

# Common issues:
# - Missing environment variables → Check .env file
# - Port already in use → Change PORT in .env
# - Volume permissions → Run: docker volume rm rustassistant_data && docker compose up -d
```

### Database Errors

```bash
# Check database exists
docker run --rm -v rustassistant_data:/data alpine ls -lh /data/

# Re-initialize if corrupted
docker compose down
docker volume rm rustassistant_data
docker compose up -d
```

### Repository Clone Failures

```bash
# Check git URL is valid
docker compose exec rustassistant git ls-remote https://github.com/username/repo.git

# Check GitHub token (for private repos)
echo $GITHUB_TOKEN  # Should start with ghp_

# Manual clone test
docker compose exec rustassistant git clone --depth=1 https://github.com/username/repo.git /tmp/test
```

### Auto-Scanner Not Running

```bash
# Check environment variable
docker compose exec rustassistant env | grep AUTO_SCAN

# Should show:
# AUTO_SCAN_ENABLED=true

# Check logs for scanner activity
docker compose logs rustassistant | grep -i "auto-scanner\|scanning"
```

---

## Production Deployment

### Raspberry Pi

```bash
# On your Pi
cd ~/rustassistant
git clone https://github.com/yourusername/rustassistant.git
cd rustassistant

# Create .env file
nano .env
# (add your configuration)

# Deploy
docker compose up -d

# Access from network
http://raspberry-pi-ip:3001
```

### Cloud Deployment (AWS/GCP/Azure)

```bash
# Same steps as local deployment
# Ensure security group/firewall allows port 3001
# Consider using Docker secrets for sensitive values

# Docker secrets example:
echo "your_api_key" | docker secret create xai_api_key -
echo "your_github_token" | docker secret create github_token -

# Update docker-compose.yml to use secrets
```

### Reverse Proxy (Nginx)

```nginx
server {
    listen 80;
    server_name rustassistant.yourdomain.com;

    location / {
        proxy_pass http://localhost:3001;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### SSL with Let's Encrypt

```bash
# Install certbot
sudo apt-get install certbot python3-certbot-nginx

# Obtain certificate
sudo certbot --nginx -d rustassistant.yourdomain.com

# Auto-renewal is set up automatically
```

---

## Performance Tuning

### Resource Limits

Edit `docker-compose.yml`:

```yaml
rustassistant:
  deploy:
    resources:
      limits:
        memory: 2G      # Increase for large codebases
        cpus: "4.0"     # Increase for parallel scans
      reservations:
        memory: 1G
        cpus: "2.0"
```

### Redis Cache

```yaml
redis:
  command: >
    redis-server
    --maxmemory 512mb    # Increase for more caching
    --maxmemory-policy allkeys-lru
```

### Concurrent Scans

```bash
# In .env
AUTO_SCAN_MAX_CONCURRENT=4  # Increase for faster scanning
```

---

## Uninstall

```bash
# Stop containers
docker compose down

# Remove volumes (WARNING: Deletes all data!)
docker volume rm rustassistant_data
docker volume rm rustassistant_repos_data
docker volume rm rustassistant_redis_data

# Remove images
docker rmi rustassistant:latest
docker rmi redis:7-alpine

# Remove project directory
cd ..
rm -rf rustassistant
```

---

## Support

- **Documentation:** See `todo/` directory for detailed guides
- **Issues:** https://github.com/yourusername/rustassistant/issues
- **Logs:** `docker compose logs rustassistant`

---

## Migration from Bind Mounts

If upgrading from an older version that used bind mounts, see:
- `todo/DOCKER_MIGRATION_GUIDE.md` - Complete migration procedure
- Includes backup, migration, and rollback steps
- Estimated time: 15-30 minutes
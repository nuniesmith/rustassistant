# 🚀 RustAssistant Quick Start Guide

**Welcome!** This guide will get you up and running with RustAssistant in minutes.

---

## 🎯 What You Just Deployed

RustAssistant is now running with these powerful features:

- ✅ **Automated Repository Scanning** - Track TODOs, issues, and code health
- ✅ **Smart Notes System** - Capture ideas with hashtag organization
- ✅ **Real-time Progress Tracking** - Watch scans happen with live progress bars
- ✅ **Git-Based Workflow** - Auto-clones and updates repos from GitHub

---

## 🌐 Access Your Instance

**Web Interface:** http://localhost:3000

**Quick Links:**
- Dashboard: http://localhost:3000/
- Repositories: http://localhost:3000/repos
- Notes: http://localhost:3000/notes
- Queue: http://localhost:3000/queue

---

## 📦 Add Your First Repository

### Option 1: Via Web UI (Recommended)

1. Open http://localhost:3000/repos
2. Click **"Add Repository"**
3. Enter:
   - **Git URL:** `https://github.com/username/repo.git`
   - **Name:** `My Project`
4. Click **"Add Repository"**

The repo will be automatically cloned to `/app/repos` inside the container!

### Option 2: Via CLI

```bash
# Add a public repository
curl -X POST http://localhost:3000/repos/add \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "git_url=https://github.com/rust-lang/rust.git&name=Rust"
```

### 🔐 For Private Repositories

Set your GitHub token in `.env`:

```bash
GITHUB_TOKEN=ghp_your_token_here
```

Then restart:

```bash
docker compose down && docker compose up -d
```

---

## ⚙️ Configure Auto-Scanning

Once you've added a repository:

1. Go to http://localhost:3000/repos
2. Find your repository card
3. **Toggle "Auto-scan enabled"** ✓
4. **Set scan interval** (e.g., `30` minutes)
5. Click **"Save Settings"**

You'll see a green success notification!

### Scan Intervals

- **Minimum:** 5 minutes
- **Maximum:** 1440 minutes (24 hours)
- **Recommended:** 30-60 minutes for active projects

---

## 📝 Capture Quick Notes

### Via Web UI

1. Go to http://localhost:3000/notes
2. Click **"Quick Note"** or find the capture input
3. Type your note with hashtags:
   ```
   Implement user authentication #backend #security #priority
   ```
4. Hit Enter or click **"Save"**

Tags are automatically extracted from `#hashtags`!

### Via API

```bash
curl -X POST http://localhost:3000/api/notes \
  -H "Content-Type: application/json" \
  -d '{
    "content": "Add dark mode support #frontend #ui #enhancement",
    "repo_id": null
  }'
```

### Filter Notes by Tag

Click any tag on the notes page to filter notes with that tag.

---

## 🔍 Watch Scan Progress

When a scan is running, you'll see:

- **Live progress bar** showing completion percentage
- **Current file** being analyzed
- **Files processed** vs total files
- **Real-time updates** every 3 seconds (via HTMX polling)

Example:
```
┌────────────────────────────────┐
│ Scanning: src/main.rs          │
│ [████████░░] 42 / 87 (48%)     │
└────────────────────────────────┘
```

---

## 🗄️ Database Access

### View Database Directly

```bash
# Enter SQLite shell
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db

# List all repositories
sqlite> SELECT name, auto_scan_enabled, scan_interval_minutes FROM repositories;

# List recent notes
sqlite> SELECT content, tags FROM notes ORDER BY created_at DESC LIMIT 10;

# View scan events
sqlite> SELECT * FROM scan_events ORDER BY created_at DESC LIMIT 20;

# Exit
sqlite> .quit
```

### Useful Queries

```sql
-- Repository health overview
SELECT * FROM repository_health;

-- Active scans
SELECT * FROM active_scans;

-- Recent scan activity
SELECT * FROM recent_scan_activity;

-- Tag statistics
SELECT name, usage_count FROM tags ORDER BY usage_count DESC;

-- Repository sync status
SELECT * FROM repository_sync_status;
```

---

## 🛠️ Common Tasks

### Force a Manual Scan

```bash
# Via web UI: Go to /repos and click "Scan Now" (if available)

# Via database (triggers next scan immediately)
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
  "UPDATE repositories SET last_scan_check = NULL WHERE id = 'repo-id';"
```

### Check Logs

```bash
# Follow logs in real-time
docker compose logs -f rustassistant

# Last 100 lines
docker compose logs rustassistant --tail=100

# Search for errors
docker compose logs rustassistant | grep -i error

# Search for scan activity
docker compose logs rustassistant | grep -i scan
```

### Restart Services

```bash
# Restart everything
docker compose restart

# Restart just the app (keeps Redis running)
docker compose restart rustassistant

# Full rebuild (after code changes)
docker compose down
docker compose build
docker compose up -d
```

### Backup Database

```bash
# Create timestamped backup
cp data/rustassistant.db "data/rustassistant.db.backup-$(date +%Y%m%d-%H%M%S)"

# Or use the backup directory
mkdir -p data/backups
cp data/rustassistant.db "data/backups/backup-$(date +%Y%m%d).db"
```

---

## 🎨 Web UI Tips

### Navigation

- **Dashboard** - Overview stats and quick actions
- **Repositories** - Manage repos and scan settings
- **Notes** - Capture and organize ideas
- **Queue** - View processing queue
- **Scanner** - Monitor auto-scanner status

### Keyboard Shortcuts (Coming Soon)

- `Ctrl+K` - Quick note capture
- `/` - Search
- `?` - Help

### Toast Notifications

- **Green** = Success (settings saved, repo added, etc.)
- **Red** = Error (validation failed, operation failed)
- **Yellow** = Warning (informational messages)

---

## 🐛 Troubleshooting

### Container Won't Start

```bash
# Check container status
docker compose ps

# Check logs for errors
docker compose logs rustassistant

# Verify database integrity
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
  "PRAGMA integrity_check;"
```

### Repository Won't Clone

**Check logs:**
```bash
docker compose logs rustassistant | grep -i clone
```

**Common issues:**
- Invalid git URL (must be HTTPS)
- Private repo without `GITHUB_TOKEN`
- Network connectivity issues

**Fix:**
```bash
# Set GitHub token if needed
echo "GITHUB_TOKEN=ghp_your_token" >> .env
docker compose restart
```

### Scan Progress Not Updating

**Verify HTMX is working:**
1. Open browser DevTools (F12)
2. Go to Network tab
3. Watch for requests to `/repos/:id/progress` every 3 seconds

**If not working:**
- Check for JavaScript errors in Console tab
- Verify repo has `auto_scan_enabled = 1`
- Check if scan is actually running

### Notes Not Saving

**Check API endpoint:**
```bash
curl -X POST http://localhost:3000/api/notes \
  -H "Content-Type: application/json" \
  -d '{"content":"Test note #test","repo_id":null}'
```

**Expected response:**
```json
{"id":"uuid-here","content":"Test note #test",...}
```

---

## 📊 Monitoring

### Health Check

```bash
curl http://localhost:3000/health
```

**Expected:**
```json
{"service":"rustassistant","status":"ok","version":"0.1.0"}
```

### Container Stats

```bash
# Real-time resource usage
docker stats rustassistant rustassistant-redis

# Disk usage
docker system df

# Volume inspection
docker volume ls | grep rustassistant
```

### Database Size

```bash
ls -lh data/rustassistant.db
```

---

## 🔒 Security Notes

1. **GitHub Token** - Store in `.env`, never commit to git
2. **Database** - Located at `data/rustassistant.db`, backup regularly
3. **Ports** - Default 3000, change in `docker-compose.yml` if needed
4. **Redis** - Exposed on 6379, restrict in production

---

## 🚀 What's Next?

### Immediate Actions

1. ✅ Add your first repository
2. ✅ Enable auto-scanning
3. ✅ Create a few test notes
4. ✅ Watch a scan complete with progress tracking

### Advanced Features

- **Priority 5: RAG Integration** - Coming soon!
  - Document embeddings
  - Semantic search
  - Intelligent context retrieval for LLMs

- **Enhanced UI**
  - Tag management (colors, merge, delete)
  - Advanced filtering
  - Bulk operations
  - Activity feed with real-time updates

### Customization

Edit these files to customize:

- `docker-compose.yml` - Container configuration
- `.env` - Environment variables
- `config/` - Application configuration (if exists)

---

## 📚 Documentation

- **Full Progress Report:** `todo/OVERALL_PROGRESS.md`
- **Deployment Status:** `DEPLOYMENT_COMPLETE.md`
- **Testing Guide:** `todo/TESTING_GUIDE.md`
- **Migration Guide:** `todo/DOCKER_MIGRATION_GUIDE.md`

---

## 💬 Support

**Check Logs:**
```bash
docker compose logs -f rustassistant
```

**Database Access:**
```bash
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db
```

**Container Shell:**
```bash
docker compose exec rustassistant sh
```

---

## ✨ Quick Command Reference

```bash
# Start
docker compose up -d

# Stop
docker compose down

# Restart
docker compose restart

# Rebuild
docker compose build && docker compose up -d

# Logs
docker compose logs -f rustassistant

# Database
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db

# Backup
cp data/rustassistant.db "data/backup-$(date +%Y%m%d).db"

# Health check
curl http://localhost:3000/health
```

---

**🎉 You're all set! Start by adding a repository and creating your first note.**

**Need help?** Check the logs and documentation above.

**Happy coding!** 🦀
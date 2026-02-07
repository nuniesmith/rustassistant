# Docker Volume Migration Guide - From Bind Mounts to Named Volumes

**Priority 2 Implementation**  
**Status:** Ready for Testing  
**Risk Level:** Medium (data migration required)

---

## Overview

This guide explains how to migrate from the old bind-mount based deployment to the new named-volume based deployment that eliminates host filesystem dependencies.

### What Changed

**Before (Bind Mounts):**
```yaml
volumes:
  - ./data:/app/data
  - ./config:/app/config:ro
  - /home/jordan/github/fks:/repos:ro
```

**After (Named Volumes):**
```yaml
volumes:
  - rustassistant_data:/app/data
  - repos_data:/app/repos
  # config is baked into Docker image
```

### Benefits

✅ **Portable Deployment** - No host paths required  
✅ **Git Clone at Runtime** - Repos cloned automatically from git URLs  
✅ **Easier Backups** - Docker manages volume lifecycle  
✅ **CI/CD Ready** - Works on any Docker host  
✅ **No Permission Issues** - Docker handles ownership  

---

## Architecture Changes

### 1. Configuration Files

**Old:** Bind-mounted from `./config`  
**New:** Copied into Docker image at build time

```dockerfile
COPY config ./config
```

Configuration is now part of the application image. Override via environment variables if needed.

### 2. Data (SQLite Database)

**Old:** Bind-mounted from `./data`  
**New:** Named volume `rustassistant_data`

```yaml
volumes:
  rustassistant_data:
    driver: local
    name: rustassistant_data
```

### 3. Repositories

**Old:** Bind-mounted from host directory  
**New:** Cloned at runtime into `repos_data` volume

The new `RepoManager` handles:
- Clone on first scan
- Pull updates before each scan
- HTTPS authentication with GitHub token
- Shallow clones (--depth=1) to save space

---

## Migration Steps

### Pre-Migration Checklist

- [ ] All repositories have `git_url` configured
- [ ] `GITHUB_TOKEN` environment variable set (for private repos)
- [ ] Database backup created
- [ ] Current deployment is working
- [ ] Docker Compose version ≥ 1.27

### Step 1: Backup Current Data

```bash
cd rustassistant

# Backup database
cp data/rustassistant.db data/rustassistant.db.backup-$(date +%Y%m%d-%H%M%S)

# List current repositories
sqlite3 data/rustassistant.db "SELECT id, name, path, git_url FROM repositories;"

# Note which repos have NULL git_url (need manual update)
sqlite3 data/rustassistant.db "SELECT id, name FROM repositories WHERE git_url IS NULL;"
```

### Step 2: Update Repository git_url (if needed)

For any repositories without a git_url, update them:

```sql
-- Connect to database
sqlite3 data/rustassistant.db

-- Update git_url for each repo
UPDATE repositories 
SET git_url = 'https://github.com/username/repo.git' 
WHERE id = 'repo-id-here';

-- Verify all repos have git URLs
SELECT id, name, git_url FROM repositories WHERE git_url IS NULL OR git_url = '';
-- Should return no rows

.quit
```

### Step 3: Apply Migrations

```bash
# Apply scan progress migration (if not already done)
sqlite3 data/rustassistant.db < migrations/003_scan_progress.sql

# Apply git_url requirement migration
sqlite3 data/rustassistant.db < migrations/004_require_git_url.sql

# Verify migrations applied
sqlite3 data/rustassistant.db "SELECT name FROM sqlite_master WHERE type='table' AND name='scan_events';"
# Should show: scan_events
```

### Step 4: Export Data from Bind Mount

```bash
# Create migration directory
mkdir -p migration-temp

# Copy database to temp location
cp data/rustassistant.db migration-temp/

# Note the database file size
ls -lh migration-temp/rustassistant.db
```

### Step 5: Update docker-compose.yml

Your `docker-compose.yml` should already be updated with the new volume configuration. Verify:

```yaml
rustassistant:
  volumes:
    - rustassistant_data:/app/data  # Named volume
    - repos_data:/app/repos          # Named volume
    # No config mount - baked into image
```

### Step 6: Stop Current Deployment

```bash
# Stop containers
docker compose down

# Verify stopped
docker compose ps
# Should show no running containers
```

### Step 7: Create Named Volumes and Import Data

```bash
# Create named volumes
docker volume create rustassistant_data
docker volume create rustassistant_repos_data

# Import database into named volume
# Use a temporary container to copy data
docker run --rm \
  -v rustassistant_data:/data \
  -v $(pwd)/migration-temp:/source \
  alpine sh -c "cp /source/rustassistant.db /data/"

# Verify data copied
docker run --rm \
  -v rustassistant_data:/data \
  alpine ls -lh /data/
# Should show rustassistant.db
```

### Step 8: Rebuild and Deploy

```bash
# Rebuild with new configuration
docker compose build

# Start services
docker compose up -d

# Watch logs
docker compose logs -f rustassistant
```

### Step 9: Verify Migration

```bash
# Check health
curl http://localhost:3001/health

# Verify database is accessible
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db "SELECT COUNT(*) FROM repositories;"

# Check repository sync status
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db "SELECT * FROM repository_sync_status;"

# Navigate to web UI
open http://localhost:3001/repos
```

### Step 10: Test Repository Cloning

```bash
# Trigger a scan (this will clone repos)
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
  "UPDATE repositories SET last_scan_check = NULL;"

# Watch logs for cloning activity
docker compose logs -f rustassistant | grep -i clone

# Verify repos were cloned
docker compose exec rustassistant ls -la /app/repos/
```

---

## Verification Checklist

After migration, verify:

- [ ] Server starts without errors
- [ ] Dashboard loads (`http://localhost:3001/`)
- [ ] Repositories page shows all repos
- [ ] Database queries work
- [ ] Auto-scanner is running
- [ ] Repositories are cloned on first scan
- [ ] Settings can be changed via web UI
- [ ] No bind mount warnings in logs

---

## Troubleshooting

### Issue: "Failed to create RepoManager"

**Cause:** Repos directory doesn't exist or isn't writable

**Solution:**
```bash
docker compose exec rustassistant mkdir -p /app/repos
docker compose exec rustassistant chown -R rustassistant:rustassistant /app/repos
```

### Issue: "Git clone failed: Authentication failed"

**Cause:** Missing or invalid GITHUB_TOKEN

**Solution:**
```bash
# Stop containers
docker compose down

# Add/update token in .env file
echo "GITHUB_TOKEN=ghp_your_token_here" >> .env

# Restart
docker compose up -d
```

### Issue: Database not found

**Cause:** Named volume not properly initialized

**Solution:**
```bash
# Check volume exists
docker volume ls | grep rustassistant_data

# Check volume contents
docker run --rm -v rustassistant_data:/data alpine ls -la /data/

# If empty, re-import
docker run --rm \
  -v rustassistant_data:/data \
  -v $(pwd)/migration-temp:/source \
  alpine cp /source/rustassistant.db /data/
```

### Issue: Repositories not cloning

**Cause:** git_url is NULL or invalid

**Solution:**
```bash
# Check git URLs
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
  "SELECT id, name, git_url FROM repositories;"

# Update invalid URLs
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
  "UPDATE repositories SET git_url = 'https://github.com/user/repo.git' WHERE id = 'repo-id';"
```

### Issue: Permission denied on /app/data

**Cause:** Volume ownership mismatch

**Solution:**
```bash
docker run --rm \
  -v rustassistant_data:/data \
  alpine chown -R 1000:1000 /data
```

---

## Rollback Procedure

If migration fails, rollback to bind mounts:

### 1. Stop New Deployment

```bash
docker compose down
```

### 2. Restore Old docker-compose.yml

```yaml
# Restore bind mounts
rustassistant:
  volumes:
    - ./data:/app/data
    - ./config:/app/config:ro
```

### 3. Restore Database Backup

```bash
# Find your backup
ls -lh data/rustassistant.db.backup-*

# Restore latest backup
cp data/rustassistant.db.backup-YYYYMMDD-HHMMSS data/rustassistant.db
```

### 4. Restart Old Configuration

```bash
docker compose build
docker compose up -d
```

---

## Post-Migration Cleanup

After successful migration and verification (wait 24-48 hours):

### Clean Up Old Bind Mount Data

```bash
# Backup directory (can be deleted after verification)
rm -rf migration-temp/

# Old bind mount data is still in ./data
# ONLY delete after confirming named volume works
# cp -r data data.old-bind-mount
# rm -rf data/*
```

### Remove Unused Volumes

```bash
# List all volumes
docker volume ls

# Remove old volumes if they exist
docker volume rm old-volume-name
```

---

## Volume Management

### Backup Named Volumes

```bash
# Backup database volume
docker run --rm \
  -v rustassistant_data:/data \
  -v $(pwd)/backups:/backup \
  alpine tar czf /backup/rustassistant-data-$(date +%Y%m%d).tar.gz -C /data .

# Backup repos volume (if needed)
docker run --rm \
  -v rustassistant_repos_data:/data \
  -v $(pwd)/backups:/backup \
  alpine tar czf /backup/rustassistant-repos-$(date +%Y%m%d).tar.gz -C /data .
```

### Restore Named Volumes

```bash
# Stop containers
docker compose down

# Restore database volume
docker run --rm \
  -v rustassistant_data:/data \
  -v $(pwd)/backups:/backup \
  alpine sh -c "rm -rf /data/* && tar xzf /backup/rustassistant-data-YYYYMMDD.tar.gz -C /data"

# Restart
docker compose up -d
```

### Inspect Volume Contents

```bash
# List files in data volume
docker run --rm -v rustassistant_data:/data alpine ls -la /data/

# List files in repos volume
docker run --rm -v rustassistant_repos_data:/data alpine ls -la /data/

# Check database file size
docker run --rm -v rustassistant_data:/data alpine du -h /data/rustassistant.db
```

### Remove All Volumes (DANGER!)

```bash
# Stop containers
docker compose down

# Remove all project volumes
docker volume rm rustassistant_data
docker volume rm rustassistant_repos_data
docker volume rm rustassistant_redis_data

# This will DELETE ALL DATA!
# Only do this if you have backups!
```

---

## Performance Considerations

### Named Volumes vs Bind Mounts

| Aspect | Bind Mounts | Named Volumes |
|--------|-------------|---------------|
| Performance | Good on Linux, slow on Mac/Windows | Optimized by Docker |
| Portability | Host-dependent | Portable |
| Backup | Manual file copy | Docker managed |
| Permissions | Host UID/GID | Container UID/GID |
| Space | Host filesystem | Docker storage |

### Shallow Clones

Repositories are cloned with `--depth=1` to save space:

```bash
# Check repo sizes
docker compose exec rustassistant du -sh /app/repos/*

# Full clone would be much larger
# Shallow clone typically 80-90% smaller
```

### Volume Drivers

For production with multiple hosts, consider:
- **Local driver** (default) - Single host
- **NFS driver** - Network storage
- **Cloud drivers** - AWS EBS, GCE Persistent Disk

---

## Environment Variables

Required for new deployment:

```bash
# .env file
DATABASE_URL=sqlite:/app/data/rustassistant.db
REPOS_DIR=/app/repos
GITHUB_TOKEN=ghp_your_token_here  # For private repos
XAI_API_KEY=your_xai_key_here
```

---

## Success Criteria

✅ Migration is complete when:
- All repositories have valid git URLs
- Database is in named volume
- Repos clone automatically on first scan
- Auto-scanner updates repos before scanning
- No bind mount warnings in logs
- Backups work correctly
- Rollback procedure tested

---

## Next Steps

After successful migration:

1. **Monitor for 24 hours**
   - Check logs for errors
   - Verify auto-scanner works
   - Confirm repos update properly

2. **Update documentation**
   - README.md deployment section
   - CI/CD pipeline (if applicable)

3. **Begin Priority 3**
   - Scan progress indicators
   - See `implementation-plan.md`

---

## FAQ

**Q: Can I still use bind mounts?**  
A: Yes, but it's not recommended. The new system is more portable.

**Q: What happens to my existing repos on disk?**  
A: They're cloned fresh from git URLs. Your original files aren't touched.

**Q: Do I need GitHub token for public repos?**  
A: No, only for private repositories.

**Q: How much disk space do shallow clones use?**  
A: Typically 10-20% of a full clone, depending on repo history.

**Q: Can I clone from GitLab/Bitbucket?**  
A: Yes, any HTTPS git URL works. Token format may differ.

**Q: What if a repo has uncommitted changes?**  
A: The scanner detects this and warns. Auto-update skipped.

**Q: How do I change clone depth?**  
A: Set `clone_depth` in database:
```sql
UPDATE repositories SET clone_depth = 10 WHERE id = 'repo-id';
```

---

## Support

If you encounter issues:
1. Check logs: `docker compose logs rustassistant`
2. Verify volumes: `docker volume ls`
3. Test manually: `docker compose exec rustassistant bash`
4. Review this guide's Troubleshooting section
5. Check `todo/TESTING_GUIDE.md`

---

**Migration Time Estimate:** 15-30 minutes  
**Rollback Time:** 5 minutes  
**Risk:** Medium (backup required)  
**Impact:** High (architecture change)

Good luck! 🚀
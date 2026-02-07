# Deployment Checklist - Priorities 1 & 2

**Project:** RustAssistant TODO Implementation  
**Version:** 1.0  
**Date:** 2024-01-15

---

## Pre-Deployment Checklist

### Environment Verification
- [ ] Docker and Docker Compose installed (v1.27+)
- [ ] Git installed and configured
- [ ] Required ports available (3001, 6379)
- [ ] Minimum 1GB disk space available
- [ ] API keys obtained (XAI_API_KEY)
- [ ] GitHub token generated (GITHUB_TOKEN) - if using private repos

### Code Verification
- [ ] Latest code pulled from repository
- [ ] All files present in `src/` directory
- [ ] Migrations present in `migrations/` directory
- [ ] `docker-compose.yml` updated
- [ ] `.env` file created with required variables

### Backup Preparation
- [ ] Current database backed up
- [ ] Backup location documented
- [ ] Backup restoration tested (optional but recommended)
- [ ] Rollback procedure reviewed

---

## Priority 1 Deployment - Scan Interval Editing

**Risk Level:** Low  
**Estimated Time:** 10 minutes  
**Rollback Time:** 2 minutes

### Step 1: Pre-Deployment
- [ ] Review `todo/TESTING_GUIDE.md`
- [ ] Backup database:
  ```bash
  cp data/rustassistant.db data/rustassistant.db.backup-$(date +%Y%m%d-%H%M%S)
  ```
- [ ] Verify database backup size matches original

### Step 2: Apply Migration
- [ ] Apply migration 003:
  ```bash
  sqlite3 data/rustassistant.db < migrations/003_scan_progress.sql
  ```
- [ ] Verify migration applied:
  ```bash
  sqlite3 data/rustassistant.db "PRAGMA table_info(repositories);" | grep scan_status
  ```
- [ ] Check for scan_events table:
  ```bash
  sqlite3 data/rustassistant.db "SELECT name FROM sqlite_master WHERE type='table' AND name='scan_events';"
  ```

### Step 3: Build and Deploy
- [ ] Build Docker image:
  ```bash
  docker compose build
  ```
- [ ] Stop current containers:
  ```bash
  docker compose down
  ```
- [ ] Start new containers:
  ```bash
  docker compose up -d
  ```
- [ ] Verify containers running:
  ```bash
  docker compose ps
  ```

### Step 4: Verify Deployment
- [ ] Check logs for errors:
  ```bash
  docker compose logs rustassistant | grep -i error
  ```
- [ ] Health check passes:
  ```bash
  curl http://localhost:3001/health
  ```
- [ ] Navigate to repos page:
  ```bash
  open http://localhost:3001/repos
  ```
- [ ] Settings form visible in repo cards
- [ ] Auto-scan checkbox works
- [ ] Scan interval input accepts values
- [ ] Save button present

### Step 5: Functional Testing
- [ ] Toggle auto-scan checkbox
  - [ ] See green success toast
  - [ ] Setting persists after refresh
- [ ] Change scan interval to 30 minutes
  - [ ] Click Save
  - [ ] See green success toast
  - [ ] Value persists after refresh
- [ ] Test validation - try interval of 3
  - [ ] See red error toast
  - [ ] Error message clear
- [ ] Test validation - try interval of 2000
  - [ ] See red error toast
  - [ ] Error message clear
- [ ] Verify in database:
  ```bash
  docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
    "SELECT name, auto_scan_enabled, scan_interval_minutes FROM repositories;"
  ```

### Step 6: Monitor
- [ ] Watch logs for 5 minutes:
  ```bash
  docker compose logs -f rustassistant
  ```
- [ ] No errors appear
- [ ] Auto-scanner respects new intervals
- [ ] No JavaScript errors in browser console (F12)

### Priority 1 Rollback (if needed)
```bash
# Stop containers
docker compose down

# Restore database backup
cp data/rustassistant.db.backup-YYYYMMDD-HHMMSS data/rustassistant.db

# Revert code (if changes committed)
git revert HEAD

# Rebuild and restart
docker compose build
docker compose up -d
```

---

## Priority 2 Deployment - Docker Volume Migration

**Risk Level:** Medium  
**Estimated Time:** 15-30 minutes  
**Rollback Time:** 5 minutes

### Step 1: Pre-Migration Verification
- [ ] All repositories have `git_url` configured:
  ```bash
  sqlite3 data/rustassistant.db "SELECT id, name, git_url FROM repositories WHERE git_url IS NULL OR git_url = '';"
  # Should return no rows
  ```
- [ ] Update missing git URLs:
  ```bash
  sqlite3 data/rustassistant.db "UPDATE repositories SET git_url = 'https://github.com/user/repo.git' WHERE id = 'repo-id';"
  ```
- [ ] GITHUB_TOKEN set in .env (for private repos)
- [ ] Read `todo/DOCKER_MIGRATION_GUIDE.md` thoroughly

### Step 2: Backup Everything
- [ ] Backup database:
  ```bash
  cp data/rustassistant.db data/rustassistant.db.backup-$(date +%Y%m%d-%H%M%S)
  ```
- [ ] Create migration temp directory:
  ```bash
  mkdir -p migration-temp
  cp data/rustassistant.db migration-temp/
  ```
- [ ] Note database size:
  ```bash
  ls -lh migration-temp/rustassistant.db
  ```

### Step 3: Apply Migration
- [ ] Apply migration 004:
  ```bash
  sqlite3 data/rustassistant.db < migrations/004_require_git_url.sql
  ```
- [ ] Verify migration applied:
  ```bash
  sqlite3 data/rustassistant.db "SELECT * FROM repository_sync_status LIMIT 1;"
  ```
- [ ] Check new columns exist:
  ```bash
  sqlite3 data/rustassistant.db "PRAGMA table_info(repositories);" | grep -E "source_type|clone_depth|last_sync_at"
  ```

### Step 4: Stop Current Deployment
- [ ] Stop containers:
  ```bash
  docker compose down
  ```
- [ ] Verify stopped:
  ```bash
  docker compose ps
  # Should show no running containers
  ```

### Step 5: Create and Populate Named Volumes
- [ ] Create named volumes:
  ```bash
  docker volume create rustassistant_data
  docker volume create rustassistant_repos_data
  ```
- [ ] Verify volumes created:
  ```bash
  docker volume ls | grep rustassistant
  ```
- [ ] Import database into named volume:
  ```bash
  docker run --rm \
    -v rustassistant_data:/data \
    -v $(pwd)/migration-temp:/source \
    alpine cp /source/rustassistant.db /data/
  ```
- [ ] Verify data copied:
  ```bash
  docker run --rm \
    -v rustassistant_data:/data \
    alpine ls -lh /data/
  # Should show rustassistant.db
  ```

### Step 6: Verify docker-compose.yml
- [ ] Config bind mount removed
- [ ] Data bind mount removed
- [ ] rustassistant_data volume configured
- [ ] repos_data volume configured
- [ ] Volume definitions in volumes section

Expected configuration:
```yaml
rustassistant:
  volumes:
    - rustassistant_data:/app/data
    - repos_data:/app/repos
```

### Step 7: Rebuild and Deploy
- [ ] Rebuild with new configuration:
  ```bash
  docker compose build
  ```
- [ ] Start services:
  ```bash
  docker compose up -d
  ```
- [ ] Watch logs for startup:
  ```bash
  docker compose logs -f rustassistant
  ```
- [ ] Look for RepoManager initialization
- [ ] No bind mount errors

### Step 8: Verify Deployment
- [ ] Health check passes:
  ```bash
  curl http://localhost:3001/health
  ```
- [ ] Database accessible:
  ```bash
  docker compose exec rustassistant sqlite3 /app/data/rustassistant.db "SELECT COUNT(*) FROM repositories;"
  ```
- [ ] Web UI loads:
  ```bash
  open http://localhost:3001/repos
  ```
- [ ] Check repository sync status:
  ```bash
  docker compose exec rustassistant sqlite3 /app/data/rustassistant.db "SELECT * FROM repository_sync_status;"
  ```

### Step 9: Test Repository Cloning
- [ ] Force repos to re-clone:
  ```bash
  docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
    "UPDATE repositories SET last_scan_check = NULL;"
  ```
- [ ] Watch for cloning in logs:
  ```bash
  docker compose logs -f rustassistant | grep -i "cloning\|updating\|clone"
  ```
- [ ] Verify repos cloned:
  ```bash
  docker compose exec rustassistant ls -la /app/repos/
  ```
- [ ] Check for .git directories:
  ```bash
  docker compose exec rustassistant find /app/repos -name .git -type d
  ```

### Step 10: Integration Testing
- [ ] Verify auto-scanner runs
- [ ] Repos update before scan (check logs)
- [ ] Settings still editable via UI
- [ ] No errors in logs
- [ ] Performance acceptable

### Priority 2 Rollback (if needed)
```bash
# Stop new deployment
docker compose down

# Restore old docker-compose.yml (from git)
git checkout HEAD -- docker-compose.yml

# Restore database backup
cp data/rustassistant.db.backup-YYYYMMDD-HHMMSS data/rustassistant.db

# Restart old configuration
docker compose build
docker compose up -d

# Verify
curl http://localhost:3001/health
```

---

## Post-Deployment Monitoring

### First Hour
- [ ] Monitor logs continuously:
  ```bash
  docker compose logs -f rustassistant
  ```
- [ ] Watch for errors or warnings
- [ ] Verify auto-scanner runs
- [ ] Check repository updates

### First Day
- [ ] Check logs every 2-3 hours
- [ ] Verify scans complete successfully
- [ ] Monitor disk usage:
  ```bash
  docker system df
  ```
- [ ] Check cache hit rates
- [ ] User acceptance testing

### First Week
- [ ] Daily log review
- [ ] Performance monitoring
- [ ] Verify all features working
- [ ] Collect user feedback
- [ ] Document any issues

---

## Verification Queries

### Database Health
```sql
-- Connect to database
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db

-- Repository status
SELECT name, auto_scan_enabled, scan_interval_minutes, 
       datetime(last_scan_check, 'unixepoch') as last_scan
FROM repositories;

-- Scan events (recent activity)
SELECT event_type, message, datetime(created_at, 'unixepoch') as event_time
FROM scan_events
ORDER BY created_at DESC
LIMIT 20;

-- Repository health
SELECT * FROM repository_health;

-- Sync status
SELECT * FROM repository_sync_status;

.quit
```

### Container Health
```bash
# Container status
docker compose ps

# Resource usage
docker stats --no-stream

# Volume usage
docker system df -v | grep rustassistant

# Network connectivity
docker compose exec rustassistant ping -c 3 redis
```

---

## Success Criteria

### Priority 1 Success ✅
- [ ] Web UI accessible at http://localhost:3001
- [ ] Settings form visible in repo cards
- [ ] Auto-scan toggle works with toast notification
- [ ] Scan interval saves with validation
- [ ] Invalid values show error toast
- [ ] Settings persist after page refresh
- [ ] Database contains new columns
- [ ] No JavaScript errors in browser console
- [ ] Auto-scanner respects new intervals
- [ ] No errors in server logs

### Priority 2 Success ✅
- [ ] No bind mounts in docker-compose.yml
- [ ] Database accessible via named volume
- [ ] Repositories clone automatically from git URLs
- [ ] Auto-scanner updates repos before scanning
- [ ] GitHub token authentication works (if applicable)
- [ ] Shallow clones confirmed (check repo sizes)
- [ ] No "path not found" errors
- [ ] All features from Priority 1 still work
- [ ] Performance is acceptable
- [ ] Backup/restore procedures work

---

## Troubleshooting

### Issue: Migration fails
**Solution:** Check SQL syntax, verify database not corrupted
```bash
sqlite3 data/rustassistant.db "PRAGMA integrity_check;"
```

### Issue: Containers won't start
**Solution:** Check logs, verify environment variables
```bash
docker compose logs rustassistant
docker compose exec rustassistant env
```

### Issue: Database not accessible
**Solution:** Verify volume permissions
```bash
docker run --rm -v rustassistant_data:/data alpine ls -la /data/
docker run --rm -v rustassistant_data:/data alpine chown -R 1000:1000 /data
```

### Issue: Repositories not cloning
**Solution:** Verify git URLs and token
```bash
# Test git URL
docker compose exec rustassistant git ls-remote https://github.com/user/repo.git

# Check token
docker compose exec rustassistant env | grep GITHUB_TOKEN
```

### Issue: Settings not saving
**Solution:** Check database write permissions, verify API endpoint
```bash
docker compose logs rustassistant | grep -i settings
curl -X POST http://localhost:3001/repos/REPO-ID/settings -d "scan_interval_minutes=30"
```

---

## Documentation References

- **Implementation Plan:** `todo/implementation-plan.md`
- **Testing Guide:** `todo/TESTING_GUIDE.md`
- **Docker Migration:** `todo/DOCKER_MIGRATION_GUIDE.md`
- **Priority 1 Summary:** `todo/COMPLETION_SUMMARY.md`
- **Priority 2 Summary:** `todo/PRIORITY2_SUMMARY.md`
- **Quick Reference:** `todo/QUICK_REFERENCE.md`
- **Overall Progress:** `todo/OVERALL_PROGRESS.md`

---

## Sign-Off

### Priority 1 Deployment
- [ ] Pre-deployment checklist complete
- [ ] Migration applied successfully
- [ ] Functional testing passed
- [ ] Monitoring started
- [ ] Documentation updated
- [ ] Deployed by: ________________
- [ ] Date: ________________
- [ ] Time: ________________

### Priority 2 Deployment
- [ ] Pre-migration verification complete
- [ ] Backup created and verified
- [ ] Migration applied successfully
- [ ] Named volumes populated
- [ ] Repository cloning verified
- [ ] Integration testing passed
- [ ] Monitoring started
- [ ] Documentation updated
- [ ] Deployed by: ________________
- [ ] Date: ________________
- [ ] Time: ________________

---

**Deployment Status:** Ready for Production  
**Risk Assessment:** Priority 1 (Low) | Priority 2 (Medium with rollback)  
**Estimated Total Time:** 25-40 minutes  
**Rollback Time:** 5-7 minutes

Good luck! 🚀
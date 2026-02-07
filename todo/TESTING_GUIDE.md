# Testing Guide - Priority 1: Scan Interval Editing

This guide walks through testing the newly implemented scan interval editing feature in the web UI.

---

## What Was Implemented

### Backend Changes
1. **New API Endpoint**: `POST /repos/{id}/settings`
   - Accepts form data: `scan_interval_minutes` (optional) and `auto_scan_enabled` (optional)
   - Validates interval is between 5 and 1440 minutes
   - Returns HTMX-compatible responses with toast triggers

2. **Database Function**: `update_repo_settings()`
   - Dynamically builds UPDATE query based on provided fields
   - Updates `updated_at` timestamp
   - Handles both interval and auto-scan toggle

3. **Repository Struct Updates** (`src/db/core.rs`)
   - Added scan progress tracking fields (for future Priority 3)
   - New helper methods: `scan_status_display()`, `progress_percentage()`, `is_auto_scan_enabled()`

### Frontend Changes
1. **Interactive Settings Form** in `repos.html`
   - Inline edit form in each repository card
   - Auto-scan toggle checkbox (submits on change)
   - Scan interval input with "Save" button
   - HTMX integration for seamless updates

2. **Toast Notifications**
   - Success/error messages for settings updates
   - Animated slide-in/slide-out effects
   - Auto-dismiss after 3 seconds

3. **Migration 003**
   - Adds scan progress columns to `repositories` table
   - Creates `scan_events` table for activity logging
   - Adds helpful views: `active_scans`, `recent_scan_activity`, `repository_health`
   - Includes indexes for performance

---

## Pre-Deployment Steps

### 1. Run the Migration

```bash
cd rustassistant

# Apply the migration manually
sqlite3 data/rustassistant.db < migrations/003_scan_progress.sql

# Verify the schema was updated
sqlite3 data/rustassistant.db "PRAGMA table_info(repositories);"
```

Expected output should include new columns:
- `scan_status`
- `scan_progress`
- `scan_current_file`
- `scan_files_total`
- `scan_files_processed`
- `last_scan_duration_ms`
- `last_scan_files_found`
- `last_scan_issues_found`
- `last_error`

### 2. Verify scan_events Table

```bash
sqlite3 data/rustassistant.db ".schema scan_events"
```

Should show the new event tracking table.

### 3. Check Views

```bash
sqlite3 data/rustassistant.db ".tables"
```

Should include: `active_scans`, `recent_scan_activity`, `repository_health`

---

## Building and Deploying

### Option 1: Local Development Build

```bash
cd rustassistant

# Build the server
cargo build --release --bin rustassistant-server

# Run locally (without Docker)
export DATABASE_URL=sqlite:data/rustassistant.db
export REPOS_DIR=./repos
export PORT=3001
./target/release/rustassistant-server
```

### Option 2: Docker Build

```bash
cd rustassistant

# Build the Docker image
docker compose build

# Start the services
docker compose up -d

# Check logs
docker compose logs -f rustassistant
```

### Option 3: Production Deployment to Pi

```bash
# On your development machine
cd rustassistant
git add -A
git commit -m "feat: add scan interval editing in web UI (Priority 1)"
git push

# SSH to your Pi
ssh pi@your-pi-address

# Pull and deploy
cd ~/github/rustassistant
git pull
docker compose down
docker compose build
docker compose up -d

# Watch logs
docker compose logs -f rustassistant
```

---

## Testing Checklist

### Basic Functionality Tests

- [ ] **View Repos Page**
  - Navigate to `/repos`
  - Verify all existing repos display
  - Each repo card should show a "⚙️ Scan Settings" section

- [ ] **Auto-Scan Toggle**
  - Click the checkbox to toggle auto-scan on/off
  - Should see a toast notification: "Settings updated successfully"
  - Page should refresh automatically (HX-Refresh header)
  - Verify the checkbox state persists after refresh

- [ ] **Scan Interval Change**
  - Change the interval value (try: 15, 30, 60, 120)
  - Click "Save" button
  - Should see success toast
  - Verify new value persists after page refresh

- [ ] **Validation Tests**
  - Try setting interval to 4 (below minimum)
  - Should see error toast: "Scan interval must be between 5 and 1440 minutes"
  - Try setting interval to 1500 (above maximum)
  - Should see same error toast
  - Try valid values (5, 1440) - should work

- [ ] **UI/UX Tests**
  - Toast should slide in from right
  - Toast should auto-dismiss after ~3 seconds
  - Success toasts should be green
  - Error toasts should be red
  - HTMX should work without page reload (except for HX-Refresh)

### Database Verification

After changing settings, verify in the database:

```bash
sqlite3 data/rustassistant.db

SELECT 
    name, 
    auto_scan_enabled, 
    scan_interval_minutes, 
    datetime(updated_at, 'unixepoch') as updated 
FROM repositories;
```

Verify:
- `auto_scan_enabled` is 0 or 1 (matches checkbox)
- `scan_interval_minutes` matches what you set
- `updated_at` timestamp is recent

### Auto-Scanner Integration Tests

- [ ] **Scan Interval Respected**
  - Set interval to 5 minutes
  - Enable auto-scan
  - Wait 5+ minutes
  - Check logs: `docker compose logs rustassistant | grep -i scan`
  - Should see scan starting after ~5 minutes

- [ ] **Auto-Scan Disable**
  - Disable auto-scan for a repo
  - Wait past the interval period
  - Repo should NOT be scanned automatically

- [ ] **Interval Change Takes Effect**
  - Set interval to 10 minutes
  - Wait for next scan cycle (auto-scanner checks every 1 minute)
  - Next scan should respect new interval

### Edge Cases

- [ ] **Multiple Repos**
  - Each repo should have independent settings
  - Changing one shouldn't affect others

- [ ] **Concurrent Updates**
  - Open two browser tabs
  - Update settings in both
  - Last update should win (optimistic locking not implemented)

- [ ] **Invalid Repo ID**
  - Try: `curl -X POST http://localhost:3001/repos/nonexistent/settings`
  - Should handle gracefully (no crash)

---

## Common Issues & Solutions

### Issue: HTMX not working (page does full reload)

**Solution**: Verify HTMX is loaded
```html
<!-- Should be in repos.html -->
<script src="https://unpkg.com/htmx.org@1.9.10"></script>
```

### Issue: Toast not showing

**Solution**: Check browser console for JS errors. Verify event listener:
```javascript
document.body.addEventListener("showToast", function(evt) { ... });
```

### Issue: "column does not exist" error

**Solution**: Migration not applied. Run:
```bash
sqlite3 data/rustassistant.db < migrations/003_scan_progress.sql
```

### Issue: Settings don't persist

**Solution**: Check database write permissions:
```bash
ls -la data/rustassistant.db
# Should be writable by user running server
```

### Issue: Validation not working

**Solution**: Check browser developer tools > Network tab. Verify:
- Request is being sent
- Response status (400 for validation errors)
- HX-Trigger header is present

---

## Verification Queries

### Check All Settings

```sql
SELECT 
    id,
    name,
    auto_scan_enabled,
    scan_interval_minutes,
    datetime(last_scan_check, 'unixepoch') as last_scan,
    datetime(updated_at, 'unixepoch') as updated
FROM repositories
ORDER BY name;
```

### Check Recent Updates

```sql
SELECT 
    name,
    datetime(updated_at, 'unixepoch') as updated
FROM repositories
WHERE updated_at > strftime('%s', 'now', '-1 hour')
ORDER BY updated_at DESC;
```

### Check Repository Health

```sql
SELECT * FROM repository_health;
```

This view shows overall health status of each repo.

---

## Performance Considerations

The current implementation is efficient for typical usage:
- Single UPDATE query per settings change
- No complex joins
- Indexes on key columns (from migration 003)

For future optimization:
- Consider debouncing rapid changes (client-side)
- Batch updates if managing 100+ repos
- Add caching if settings are read frequently

---

## Next Steps After Testing

Once Priority 1 is verified:

1. **Monitor in Production**
   - Watch for auto-scanner respecting new intervals
   - Check for any performance issues
   - Gather user feedback

2. **Begin Priority 2**
   - Docker volume mount elimination
   - Git clone at runtime
   - See `implementation-plan.md` for details

3. **Consider Enhancements** (optional)
   - Bulk settings update (select multiple repos)
   - Preset interval buttons (5min, 15min, 1hr, 6hr, 1day)
   - Visual feedback when settings form is saving
   - Keyboard shortcuts (Enter to save, Esc to cancel)

---

## Success Criteria

✅ Priority 1 is complete when:
- Users can change scan interval via web UI
- Changes persist to database
- Auto-scanner respects new intervals within 1 minute
- Toast notifications work reliably
- No errors in server logs
- No JavaScript errors in browser console

---

## Rollback Plan

If issues arise:

```bash
# Revert code changes
git revert HEAD

# Rollback migration (if needed)
sqlite3 data/rustassistant.db

-- Remove new columns
ALTER TABLE repositories DROP COLUMN scan_status;
-- (Note: SQLite doesn't support DROP COLUMN easily, may need table recreation)

-- Or restore from backup
.restore /path/to/backup.db

# Rebuild and redeploy
docker compose down
docker compose build
docker compose up -d
```

**Recommendation**: Take a database backup before deploying:
```bash
cp data/rustassistant.db data/rustassistant.db.backup-$(date +%Y%m%d-%H%M%S)
```

---

## Questions or Issues?

If you encounter problems:

1. Check server logs: `docker compose logs rustassistant`
2. Check browser console: F12 > Console
3. Verify migration applied: `sqlite3 data/rustassistant.db "PRAGMA table_info(repositories);"`
4. Review this guide's "Common Issues" section
5. Check the implementation in `src/web_ui.rs` and `src/templates/pages/repos.html`

---

Happy testing! 🚀
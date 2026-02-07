# Quick Reference Card - RustAssistant TODO Implementation

**Last Updated:** 2024-01-15  
**Current Status:** Priority 1 Complete ✅

---

## 🚀 Quick Commands

### Deploy & Test
```bash
# Apply migration
sqlite3 data/rustassistant.db < migrations/003_scan_progress.sql

# Build and run
cargo build --release --bin rustassistant-server
./target/release/rustassistant-server

# OR with Docker
docker compose build && docker compose up -d
docker compose logs -f rustassistant
```

### Check Status
```bash
# View repository settings
sqlite3 data/rustassistant.db "SELECT name, auto_scan_enabled, scan_interval_minutes FROM repositories;"

# Check recent updates
sqlite3 data/rustassistant.db "SELECT name, datetime(updated_at, 'unixepoch') FROM repositories ORDER BY updated_at DESC LIMIT 5;"

# View repository health
sqlite3 data/rustassistant.db "SELECT * FROM repository_health;"
```

### Rollback
```bash
# Backup first
cp data/rustassistant.db data/rustassistant.db.backup-$(date +%Y%m%d-%H%M%S)

# Revert code
git revert HEAD

# Rebuild
docker compose down && docker compose build && docker compose up -d
```

---

## 📋 Priority Status

| Priority | Feature | Status | Effort | Files |
|----------|---------|--------|--------|-------|
| 1 | Scan Interval UI | ✅ DONE | 2-3h | 3 modified |
| 2 | Docker Volumes | ⏭️ NEXT | 6-8h | 5 files |
| 3 | Progress Indicators | 📝 TODO | 8-10h | 6 files |
| 4 | Notes/Ideas | 📝 TODO | 10-12h | 8 files |
| 5 | RAG Integration | 📝 TODO | 15-20h | 10 files |

**Total Remaining:** ~41 hours over 5-6 weeks

---

## 🎯 Priority 1 Deliverables

### What Was Built
- ✅ API endpoint: `POST /repos/{id}/settings`
- ✅ Settings form in web UI (inline edit)
- ✅ Toast notifications (success/error)
- ✅ Input validation (5-1440 minutes)
- ✅ HTMX integration
- ✅ Database migration 003
- ✅ Repository struct updates

### Files Modified
1. `src/db/core.rs` - Added scan tracking fields
2. `src/web_ui.rs` - Added settings endpoint
3. `src/templates/pages/repos.html` - Added settings form

### Files Created
1. `migrations/003_scan_progress.sql`
2. `todo/implementation-plan.md`
3. `todo/TESTING_GUIDE.md`
4. `todo/COMPLETION_SUMMARY.md`
5. `todo/QUICK_REFERENCE.md` (this file)

---

## 🧪 Quick Test

```bash
# 1. Navigate to web UI
open http://localhost:3001/repos

# 2. Test auto-scan toggle
# - Click checkbox → should see green toast

# 3. Test interval change
# - Change to 30 minutes → click Save → green toast
# - Change to 3 minutes → click Save → red error toast

# 4. Verify persistence
# - Refresh page → settings should persist
```

---

## 🔍 Key Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/repos` | List repositories |
| POST | `/repos/{id}/settings` | Update scan settings |
| GET | `/repos/{id}/toggle-scan` | Toggle auto-scan |
| GET | `/scanner/{id}/force` | Force immediate scan |

---

## 📊 Database Schema (New Fields)

### repositories table
```sql
scan_status TEXT DEFAULT 'idle'  -- idle/scanning/error
scan_progress TEXT               -- "Processing file 12/47"
scan_current_file TEXT           -- Current file path
scan_files_total INTEGER         -- Total files to process
scan_files_processed INTEGER     -- Files completed
last_scan_duration_ms INTEGER    -- Scan duration
last_scan_files_found INTEGER    -- Files discovered
last_scan_issues_found INTEGER   -- Issues detected
last_error TEXT                  -- Last error message
```

### scan_events table
```sql
id INTEGER PRIMARY KEY
repo_id TEXT
event_type TEXT  -- scan_started/scan_completed/scan_error/todo_found/issue_found/git_update
message TEXT
metadata TEXT    -- JSON blob
created_at INTEGER
```

---

## 🎨 UI Components

### Settings Form
```html
<form hx-post="/repos/{id}/settings" hx-swap="none">
  <input type="checkbox" name="auto_scan_enabled" value="true" />
  <input type="number" name="scan_interval_minutes" min="5" max="1440" />
  <button type="submit">Save</button>
</form>
```

### Toast Trigger
```javascript
// Server response header:
HX-Trigger: {"showToast": {"message": "Settings updated", "type": "success"}}

// Client event listener:
document.body.addEventListener("showToast", function(evt) { ... });
```

---

## 🐛 Troubleshooting

| Issue | Solution |
|-------|----------|
| HTMX not working | Add `<script src="https://unpkg.com/htmx.org@1.9.10"></script>` |
| Toast not showing | Check browser console for JS errors |
| "Column does not exist" | Run migration: `sqlite3 data/rustassistant.db < migrations/003_scan_progress.sql` |
| Settings don't persist | Check DB file permissions: `ls -la data/rustassistant.db` |
| Validation not working | Check Network tab in DevTools, verify 400 status |

---

## 📚 Documentation Links

- **Implementation Plan**: `todo/implementation-plan.md`
- **Testing Guide**: `todo/TESTING_GUIDE.md`
- **Completion Summary**: `todo/COMPLETION_SUMMARY.md`
- **Main README**: `README.md`

---

## 🎯 Next Steps

1. **Test Priority 1**
   - Follow TESTING_GUIDE.md
   - Verify all functionality
   - Monitor for issues

2. **Priority 2 Tasks**
   - Remove bind mounts
   - Create `src/repo_manager.rs`
   - Implement git clone at runtime
   - Update docker-compose.yml

3. **Priority 3 Prep**
   - Migration already done ✅
   - Schema ready ✅
   - Need scanner updates
   - Need progress UI components

---

## 💡 Pro Tips

1. **Always backup before migration**
   ```bash
   cp data/rustassistant.db data/rustassistant.db.backup-$(date +%Y%m%d-%H%M%S)
   ```

2. **Check logs for errors**
   ```bash
   docker compose logs rustassistant | grep -i error
   ```

3. **Verify HTMX responses**
   - Open DevTools → Network tab
   - Look for HX-Trigger headers
   - Check response status codes

4. **Use database views for debugging**
   ```sql
   SELECT * FROM repository_health;
   SELECT * FROM active_scans;
   SELECT * FROM recent_scan_activity;
   ```

---

## 🔐 Security Notes

- ✅ Input validation: 5-1440 range enforced
- ✅ SQL injection: Parameterized queries used
- ✅ XSS: Askama templates auto-escape
- ⚠️ CSRF: Not implemented (single-user system)
- ⚠️ Auth: None (Pi deployment assumed secure)

---

## 📞 Support

If you encounter issues:
1. Check `todo/TESTING_GUIDE.md` → Common Issues section
2. Review server logs: `docker compose logs rustassistant`
3. Check browser console (F12)
4. Verify migration applied: `sqlite3 data/rustassistant.db "PRAGMA table_info(repositories);"`

---

## ✨ Feature Highlights

**Before Priority 1:**
- Edit scan settings via SQL only
- No visual feedback
- Manual timestamp updates

**After Priority 1:**
- Click and edit in web UI
- Toast notifications
- Auto-refresh on save
- Validation built-in
- No CLI needed

**Time Saved:** ~2 minutes per configuration change  
**User Friction:** Eliminated

---

**Ready to deploy!** 🚀
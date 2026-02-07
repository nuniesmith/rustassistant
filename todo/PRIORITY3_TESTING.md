# Priority 3: Scan Progress Testing Checklist

**Feature:** Real-time Scan Progress Indicators  
**Date:** 2024-01-15  
**Tester:** _____________  

---

## Pre-Testing Setup

### 1. Apply Migration (if not already done)

```bash
# Backup database first
cp data/rustassistant.db data/rustassistant.db.backup-$(date +%Y%m%d-%H%M%S)

# Apply migration 003
sqlite3 data/rustassistant.db < migrations/003_scan_progress.sql

# Verify migration
sqlite3 data/rustassistant.db "SELECT name FROM sqlite_master WHERE type='table' AND name='scan_events';"
# Should return: scan_events
```

**Status:** ☐ Migration applied ☐ Database backed up

### 2. Rebuild and Start Server

```bash
# If using Docker
docker compose build
docker compose up -d
docker compose logs -f rustassistant

# If running locally
cargo build
cargo run
```

**Status:** ☐ Server running ☐ No startup errors

### 3. Verify Server Health

```bash
curl http://localhost:3001/health
```

**Status:** ☐ Health endpoint responds

---

## Test 1: Progress Bar Renders on Page Load

### Steps
1. Navigate to http://localhost:3001/repos
2. Observe the repository cards

### Expected Results
- ☐ Each repo card has a progress section below scan settings
- ☐ If no scan data: Shows "No scan data available"
- ☐ If previous scan: Shows "Last scan: X files, Y issues in Zms"
- ☐ No JavaScript errors in browser console

### Actual Results
```
_______________________________________________________
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 2: Progress Updates During Scan

### Prerequisites
- Add a repository with 20+ analyzable files (.rs, .js, .py, etc.)

### Steps
1. Add repository via UI
2. Enable auto-scan
3. Set scan interval to 5 minutes
4. Force scan manually or wait for auto-scan
5. Watch the progress indicator

### Expected Results
- ☐ Progress bar appears with "🔄 Scanning..."
- ☐ Shows file count (e.g., "23/50")
- ☐ Shows percentage (e.g., "46%")
- ☐ Progress bar fills from left to right
- ☐ Progress bar has gradient color (blue to purple)
- ☐ Current file name appears below bar
- ☐ Updates every ~3 seconds
- ☐ No page refresh required

### Actual Results
```
_______________________________________________________
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 3: Scan Completion Metrics

### Steps
1. Wait for scan to complete (or use a small repo)
2. Observe the progress indicator after completion

### Expected Results
- ☐ Progress bar disappears
- ☐ Shows "✅ Last scan: X files, Y issues in Zms"
- ☐ Duration is reasonable (> 0ms)
- ☐ File count matches repository size
- ☐ Issue count is >= 0

### Actual Results
```
Files: ____
Issues: ____
Duration: ____ ms
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 4: Error Handling

### Steps
1. Add a repository with an invalid path or git URL
2. Trigger a scan
3. Observe the error display

### Expected Results
- ☐ Shows "❌ Scan failed"
- ☐ Error message appears in red box
- ☐ Error message is descriptive
- ☐ No progress bar shown
- ☐ Scan status = 'error' in database

### Actual Results
```
Error message: _______________________________________
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 5: HTMX Polling Behavior

### Steps
1. Start a scan on a large repository
2. Open browser DevTools → Network tab
3. Filter for XHR/Fetch requests
4. Observe polling behavior

### Expected Results
- ☐ Requests to `/repos/{id}/progress` every 3 seconds
- ☐ Requests return 200 OK
- ☐ Response is HTML fragment
- ☐ Polling stops when scan completes
- ☐ No duplicate requests

### Actual Results
```
Polling interval: ____ seconds
Request count during scan: ____
Response time: ____ ms
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 6: Database Event Logging

### Steps
1. Trigger a scan
2. Query the scan_events table

```bash
sqlite3 data/rustassistant.db "SELECT * FROM scan_events ORDER BY created_at DESC LIMIT 10;"
```

### Expected Results
- ☐ `scan_started` event logged at start
- ☐ `scan_completed` event logged on success
- ☐ Events have correct repo_id
- ☐ Timestamps are recent
- ☐ Messages are descriptive
- ☐ Metadata JSON is valid (if present)

### Actual Results
```
Event count: ____
Event types found: _________________________________
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 7: Progress Percentage Calculation

### Steps
1. Start scanning a repo with exactly 10 files
2. Watch progress updates

### Expected Results
- ☐ At 5/10 files: shows 50%
- ☐ At 10/10 files: shows 100%
- ☐ Percentage never exceeds 100%
- ☐ Calculation is accurate

### Actual Results
```
Files processed | Expected % | Actual %
5/10           | 50%        | ____%
10/10          | 100%       | ____%
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 8: Multiple Concurrent Scans

### Steps
1. Add 3+ repositories
2. Enable auto-scan on all
3. Force scan on all simultaneously
4. Navigate to repos page

### Expected Results
- ☐ Each repo shows independent progress
- ☐ No progress data mixed between repos
- ☐ All progress bars update independently
- ☐ Server remains responsive
- ☐ No database locks or errors

### Actual Results
```
Repos scanning simultaneously: ____
Server response time: ____ ms
Errors: ___________________________________________
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 9: Database Views

### Steps
Query the new database views:

```bash
# Active scans
sqlite3 data/rustassistant.db "SELECT * FROM active_scans;"

# Recent activity
sqlite3 data/rustassistant.db "SELECT * FROM recent_scan_activity LIMIT 5;"

# Repository health
sqlite3 data/rustassistant.db "SELECT * FROM repository_health;"
```

### Expected Results
- ☐ `active_scans` shows currently scanning repos
- ☐ `recent_scan_activity` shows last 5 events
- ☐ `repository_health` shows all repos with health status
- ☐ All views return valid data
- ☐ No SQL errors

### Actual Results
```
Active scans count: ____
Recent events count: ____
Health statuses: ___________________________________
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 10: Performance Impact

### Steps
1. Add a large repository (500+ files)
2. Monitor server logs during scan
3. Check database size before/after

### Expected Results
- ☐ Progress updates don't slow scan significantly
- ☐ Memory usage remains stable
- ☐ CPU usage is reasonable
- ☐ Database size increase < 1MB for events
- ☐ No timeouts or crashes

### Actual Results
```
Scan duration: ____ ms
Memory usage: ____ MB
CPU usage: ____%
DB size increase: ____ KB
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 11: Browser Compatibility

### Browsers to Test
- ☐ Chrome/Chromium
- ☐ Firefox
- ☐ Safari
- ☐ Edge

### Expected Results (All Browsers)
- ☐ Progress bar renders correctly
- ☐ HTMX polling works
- ☐ CSS animations smooth
- ☐ No JavaScript errors
- ☐ Mobile responsive (bonus)

### Actual Results
```
Chrome: ___________________________________________
Firefox: __________________________________________
Safari: ___________________________________________
Edge: _____________________________________________
```

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Test 12: Regression Testing

### Verify Previous Features Still Work

- ☐ Scan interval editing still works
- ☐ Auto-scan toggle still works
- ☐ Settings save successfully
- ☐ Toast notifications appear
- ☐ Repository add/delete works
- ☐ No broken functionality

**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Edge Cases

### Test 13: Zero Files Repository

**Steps:** Add empty repository, trigger scan  
**Expected:** Shows "0 files" or handles gracefully  
**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

### Test 14: Very Large Repository (1000+ files)

**Steps:** Scan large repo  
**Expected:** Progress updates work, no timeouts  
**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

### Test 15: Network Interruption

**Steps:** Start scan, disconnect network briefly  
**Expected:** HTMX retries or fails gracefully  
**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

### Test 16: Server Restart During Scan

**Steps:** Start scan, restart server  
**Expected:** Status resets to 'idle', no orphaned data  
**Status:** ☐ PASS ☐ FAIL ☐ BLOCKED

---

## Final Checks

### Code Quality
- ☐ No compiler warnings (except unused variables)
- ☐ No clippy warnings
- ☐ Code is well-documented
- ☐ No TODO comments in critical paths

### Documentation
- ☐ PRIORITY3_IMPLEMENTATION.md is accurate
- ☐ PRIORITY3_SUMMARY.md is complete
- ☐ Code comments are clear
- ☐ API endpoints documented

### Deployment Readiness
- ☐ Migration script tested
- ☐ Rollback procedure documented
- ☐ No breaking changes
- ☐ Backward compatible

---

## Test Summary

**Total Tests:** 16  
**Passed:** ____  
**Failed:** ____  
**Blocked:** ____  

**Pass Rate:** ____%

### Critical Issues Found
```
1. _________________________________________________
2. _________________________________________________
3. _________________________________________________
```

### Minor Issues Found
```
1. _________________________________________________
2. _________________________________________________
3. _________________________________________________
```

### Recommendations
```
_____________________________________________________
_____________________________________________________
_____________________________________________________
```

---

## Sign-Off

**Tester:** ________________  
**Date:** __________________  
**Approval:** ☐ APPROVED ☐ NEEDS WORK  

**Notes:**
```
_____________________________________________________
_____________________________________________________
_____________________________________________________
```

---

## Quick Debug Commands

If issues are found:

```bash
# Check scan status in DB
sqlite3 data/rustassistant.db "SELECT id, name, scan_status, scan_progress FROM repositories;"

# View recent events
sqlite3 data/rustassistant.db "SELECT * FROM scan_events ORDER BY created_at DESC LIMIT 20;"

# Check for stuck scans
sqlite3 data/rustassistant.db "SELECT * FROM active_scans;"

# Reset stuck scan
sqlite3 data/rustassistant.db "UPDATE repositories SET scan_status='idle' WHERE id='<repo-id>';"

# View server logs
docker compose logs -f rustassistant | grep -i "scan"

# Check for errors
docker compose logs rustassistant | grep -i "error"
```

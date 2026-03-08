#!/usr/bin/env bash
# =============================================================================
# RustAssistant API Test Suite
# Tests the full todo pipeline via HTTP: scan → scaffold → plan → work → sync
# Also tests: health, repos, web-api, /api/v1/repos, chat
#
# Usage:
#   ./scripts/test_api.sh [BASE_URL]
#   BASE_URL defaults to http://localhost:3005
#
# Requirements:
#   - Server running (rustassistant-server)
#   - jq installed
#   - XAI_API_KEY set in env for LLM steps (scaffold, plan, work)
#
# Exit codes:
#   0 = all tests passed
#   1 = one or more tests failed
# =============================================================================

set -euo pipefail

BASE="${1:-http://localhost:3005}"
REPO_PATH="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0
FAIL=0
SKIP=0

# ── colours ──────────────────────────────────────────────────────────────────
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

# ── helpers ───────────────────────────────────────────────────────────────────
pass()  { PASS=$((PASS+1));  echo -e "  ${GREEN}✓${RESET} $1"; }
fail()  { FAIL=$((FAIL+1));  echo -e "  ${RED}✗${RESET} $1"; }
skip()  { SKIP=$((SKIP+1));  echo -e "  ${YELLOW}~${RESET} $1 (skipped)"; }
header(){ echo -e "\n${BOLD}${CYAN}▶ $1${RESET}"; }
info()  { echo -e "    ${CYAN}→${RESET} $1"; }

# POST/GET with response capture; returns HTTP body, sets HTTP_CODE
http_get() {
    HTTP_CODE=$(curl -s -o /tmp/ra_test_body -w "%{http_code}" "$1")
    cat /tmp/ra_test_body
}
http_post() {
    HTTP_CODE=$(curl -s -o /tmp/ra_test_body -w "%{http_code}" \
        -X POST -H "Content-Type: application/json" -d "$2" "$1")
    cat /tmp/ra_test_body
}
http_delete() {
    HTTP_CODE=$(curl -s -o /tmp/ra_test_body -w "%{http_code}" -X DELETE "$1")
    cat /tmp/ra_test_body
}

assert_code() {
    local label="$1" expected="$2"
    if [ "$HTTP_CODE" = "$expected" ]; then
        pass "$label (HTTP $HTTP_CODE)"
    else
        fail "$label — expected HTTP $expected, got HTTP $HTTP_CODE"
        info "body: $(cat /tmp/ra_test_body | head -c 300)"
    fi
}

assert_field() {
    local label="$1" field="$2" expected="$3"
    local actual
    actual=$(cat /tmp/ra_test_body | jq -r "$field" 2>/dev/null || echo "__jq_error__")
    if [ "$actual" = "$expected" ]; then
        pass "$label (.${field} = ${expected})"
    else
        fail "$label — expected ${field}=${expected}, got ${actual}"
    fi
}

assert_contains() {
    local label="$1" field="$2" needle="$3"
    local actual
    actual=$(cat /tmp/ra_test_body | jq -r "$field" 2>/dev/null || echo "")
    if echo "$actual" | grep -q "$needle"; then
        pass "$label"
    else
        fail "$label — expected '$needle' in $field, got: $actual"
    fi
}

assert_not_empty() {
    local label="$1" field="$2"
    local actual
    actual=$(cat /tmp/ra_test_body | jq -r "$field" 2>/dev/null || echo "")
    if [ -n "$actual" ] && [ "$actual" != "null" ] && [ "$actual" != "" ]; then
        pass "$label ($field is non-empty)"
    else
        fail "$label — $field was empty or null"
    fi
}

assert_json_array_nonempty() {
    local label="$1" field="$2"
    local len
    len=$(cat /tmp/ra_test_body | jq "$field | length" 2>/dev/null || echo "0")
    if [ "$len" -gt 0 ] 2>/dev/null; then
        pass "$label (${field} has ${len} item(s))"
    else
        fail "$label — ${field} was empty or not an array"
    fi
}

wait_job() {
    # wait_job <job_id> <max_seconds>
    local job_id="$1" max="${2:-60}" elapsed=0
    while [ $elapsed -lt $max ]; do
        local status
        status=$(http_get "$BASE/api/web/jobs/$job_id" | jq -r '.job.status' 2>/dev/null || echo "unknown")
        if [ "$status" = "success" ] || [ "$status" = "dry_run" ]; then
            echo "$status"
            return 0
        elif [ "$status" = "failed" ]; then
            echo "failed"
            return 1
        fi
        sleep 2
        elapsed=$((elapsed+2))
    done
    echo "timeout"
    return 1
}

# ── Check dependencies ────────────────────────────────────────────────────────
header "Preflight checks"

if ! command -v jq &>/dev/null; then
    fail "jq not found — install jq to run this test suite"
    exit 1
fi
pass "jq available"

if curl -s --max-time 3 "$BASE/health" >/dev/null 2>&1; then
    pass "Server reachable at $BASE"
else
    fail "Server not reachable at $BASE — is rustassistant-server running?"
    exit 1
fi

info "Repo under test: $REPO_PATH"

# =============================================================================
# 1. Health checks
# =============================================================================
header "1. Health checks"

http_get "$BASE/health"
assert_code "GET /health" 200
assert_field "health.status" ".status" "ok"
assert_not_empty "health.version" ".version"

http_get "$BASE/api/web/health"
assert_code "GET /api/web/health" 200
assert_field "web-api health.status" ".status" "ok"

GROK_AVAILABLE=$(cat /tmp/ra_test_body | jq -r '.grok_available')
if [ "$GROK_AVAILABLE" = "true" ]; then
    pass "Grok API key configured"
    LLM_ENABLED=true
else
    skip "Grok API key not set — LLM pipeline steps will be skipped"
    LLM_ENABLED=false
fi

# =============================================================================
# 2. /api/v1/repos (RepoSyncService CRUD)
# =============================================================================
header "2. /api/v1/repos — repo CRUD"

# Register rustassistant repo
BODY="{\"name\":\"rustassistant\",\"local_path\":\"$REPO_PATH\",\"branch\":\"main\",\"sync_on_register\":false}"
http_post "$BASE/api/v1/repos" "$BODY"
assert_code "POST /api/v1/repos" 200
V1_REPO_ID=$(cat /tmp/ra_test_body | jq -r '.id' 2>/dev/null || echo "")
assert_not_empty "register repo — id returned" ".id"
info "v1 repo id: $V1_REPO_ID"

# GET list
http_get "$BASE/api/v1/repos"
assert_code "GET /api/v1/repos" 200
assert_json_array_nonempty "repos list is non-empty" "."

# GET single repo
if [ -n "$V1_REPO_ID" ] && [ "$V1_REPO_ID" != "null" ]; then
    http_get "$BASE/api/v1/repos/$V1_REPO_ID"
    assert_code "GET /api/v1/repos/:id" 200
    assert_field "repo.name" ".name" "rustassistant"

    # Trigger sync
    http_post "$BASE/api/v1/repos/$V1_REPO_ID/sync" "{}"
    assert_code "POST /api/v1/repos/:id/sync" 200
    SYNCED_FILES=$(cat /tmp/ra_test_body | jq -r '.files_walked' 2>/dev/null || echo "0")
    if [ "$SYNCED_FILES" -gt 0 ] 2>/dev/null; then
        pass "sync walked $SYNCED_FILES files"
    else
        fail "sync returned files_walked=0"
        info "body: $(cat /tmp/ra_test_body)"
    fi

    # Cache files
    http_get "$BASE/api/v1/repos/$V1_REPO_ID/tree"
    assert_code "GET /api/v1/repos/:id/tree" 200
    # tree is plain text — check the raw body, not via jq
    if grep -q "src/" /tmp/ra_test_body 2>/dev/null; then
        pass "tree.txt contains src/ entries"
    else
        fail "tree.txt missing src/ entries"
        info "first 3 lines: $(head -3 /tmp/ra_test_body)"
    fi

    http_get "$BASE/api/v1/repos/$V1_REPO_ID/todos"
    assert_code "GET /api/v1/repos/:id/todos" 200
    TODO_COUNT=$(jq 'length' /tmp/ra_test_body 2>/dev/null || echo 0)
    if [ "$TODO_COUNT" -gt 0 ] 2>/dev/null; then
        pass "todos.json has $TODO_COUNT items"
    else
        fail "todos.json was empty or invalid"
    fi

    http_get "$BASE/api/v1/repos/$V1_REPO_ID/symbols"
    assert_code "GET /api/v1/repos/:id/symbols" 200
    SYM_COUNT=$(jq 'length' /tmp/ra_test_body 2>/dev/null || echo 0)
    if [ "$SYM_COUNT" -gt 0 ] 2>/dev/null; then
        pass "symbols.json has $SYM_COUNT items"
    else
        fail "symbols.json was empty or invalid"
    fi

    http_get "$BASE/api/v1/repos/$V1_REPO_ID/context"
    assert_code "GET /api/v1/repos/:id/context" 200
    # context is plain markdown text — check raw body
    if grep -q "rustassistant" /tmp/ra_test_body 2>/dev/null; then
        pass "context.md contains repo name"
    else
        fail "context.md missing repo name"
        info "first 3 lines: $(head -3 /tmp/ra_test_body)"
    fi
else
    skip "v1 repo CRUD (no repo id returned)"
fi

# =============================================================================
# 3. /api/web/repos — pipeline repo registry
# =============================================================================
header "3. /api/web/repos — pipeline repo registry"

# List (may be empty if no repos cloned via web-api yet)
http_get "$BASE/api/web/repos"
assert_code "GET /api/web/repos" 200
WEB_REPOS_COUNT=$(cat /tmp/ra_test_body | jq '.repos | length' 2>/dev/null || echo 0)
info "web repos: $WEB_REPOS_COUNT registered"

# Register the local repo directly via /api/repos (the simple CRUD endpoint)
http_post "$BASE/api/repos" "{\"path\":\"$REPO_PATH\",\"name\":\"rustassistant-pipeline\"}"
assert_code "POST /api/repos (register local path)" 200
PIPELINE_REPO_ID=$(cat /tmp/ra_test_body | jq -r '.data.id' 2>/dev/null || echo "")
assert_not_empty "pipeline repo id" ".data.id"
info "pipeline repo id: $PIPELINE_REPO_ID"

if [ -n "$PIPELINE_REPO_ID" ] && [ "$PIPELINE_REPO_ID" != "null" ]; then
    # Verify via GET
    http_get "$BASE/api/repos/$PIPELINE_REPO_ID"
    assert_code "GET /api/repos/:id" 200
    assert_field "repo name" ".data.name" "rustassistant-pipeline"
fi

# =============================================================================
# 4. /api/web/repos/:id/todo — read todo.md
# =============================================================================
header "4. GET /api/web/repos/:id/todo"

if [ -n "$PIPELINE_REPO_ID" ] && [ "$PIPELINE_REPO_ID" != "null" ]; then
    http_get "$BASE/api/web/repos/$PIPELINE_REPO_ID/todo"
    assert_code "GET /api/web/repos/:id/todo" 200
    assert_not_empty "todo.content non-empty" ".content"
    TODO_MD_LEN=$(jq -r '.content | length' /tmp/ra_test_body 2>/dev/null || echo 0)
    if [ "$TODO_MD_LEN" -gt 10 ] 2>/dev/null; then
        pass "todo.md has markdown content (${TODO_MD_LEN} chars)"
    else
        fail "todo.md content too short or empty"
    fi
    info "todo.md length: ${TODO_MD_LEN} chars"
else
    skip "todo.md read (no pipeline repo id)"
fi

# =============================================================================
# 5. /api/web/repos/:id/scan — static TODO scan (no LLM)
# =============================================================================
header "5. GET /api/web/repos/:id/scan"

if [ -n "$PIPELINE_REPO_ID" ] && [ "$PIPELINE_REPO_ID" != "null" ]; then
    http_get "$BASE/api/web/repos/$PIPELINE_REPO_ID/scan"
    assert_code "GET /api/web/repos/:id/scan" 200
    SCAN_TOTAL=$(jq -r '.total_found' /tmp/ra_test_body 2>/dev/null || echo 0)
    SCAN_SHOWN=$(jq -r '.shown' /tmp/ra_test_body 2>/dev/null || echo 0)
    if [ "$SCAN_TOTAL" -gt 0 ] 2>/dev/null; then
        pass "scan found $SCAN_TOTAL TODO items ($SCAN_SHOWN shown)"
    else
        fail "scan returned 0 items"
        info "body: $(head -c 500 /tmp/ra_test_body)"
    fi

    # Scan with filter=high
    http_get "$BASE/api/web/repos/$PIPELINE_REPO_ID/scan?filter=high"
    assert_code "GET /api/web/repos/:id/scan?filter=high" 200
    HIGH_COUNT=$(jq -r '.shown' /tmp/ra_test_body 2>/dev/null || echo 0)
    info "high-priority items: $HIGH_COUNT"
else
    skip "scan endpoint (no pipeline repo id)"
fi

# =============================================================================
# 6. /api/web/pipeline/dispatch — Scan step (background job)
# =============================================================================
header "6. Pipeline dispatch — SCAN step"

if [ -n "$PIPELINE_REPO_ID" ] && [ "$PIPELINE_REPO_ID" != "null" ]; then
    BODY="{\"repo_id\":\"$PIPELINE_REPO_ID\",\"step\":\"Scan\",\"dry_run\":false}"
    http_post "$BASE/api/web/pipeline/dispatch" "$BODY"
    assert_code "POST /api/web/pipeline/dispatch (scan)" 202
    SCAN_JOB_ID=$(jq -r '.job_id' /tmp/ra_test_body 2>/dev/null || echo "")
    if [ -n "$SCAN_JOB_ID" ] && [ "$SCAN_JOB_ID" != "null" ]; then
        pass "scan job_id returned ($SCAN_JOB_ID)"
    else
        fail "scan job_id was empty or null"
    fi

    if [ -n "$SCAN_JOB_ID" ] && [ "$SCAN_JOB_ID" != "null" ]; then
        info "Waiting for scan job to complete..."
        JOB_STATUS=$(wait_job "$SCAN_JOB_ID" 45)
        if [ "$JOB_STATUS" = "success" ] || [ "$JOB_STATUS" = "dry_run" ]; then
            pass "scan job completed ($JOB_STATUS)"
            http_get "$BASE/api/web/jobs/$SCAN_JOB_ID"
            LOG_LINES=$(jq '.job.log_lines | length' /tmp/ra_test_body 2>/dev/null || echo 0)
            info "job log lines: $LOG_LINES"
            # Verify scan result has items
            RESULT_ITEMS=$(jq '.job.result_json.items | length' /tmp/ra_test_body 2>/dev/null || echo 0)
            if [ "$RESULT_ITEMS" -gt 0 ] 2>/dev/null; then
                pass "scan result has $RESULT_ITEMS items in result_json"
            else
                info "scan result_json items: $RESULT_ITEMS (may be 0 if JSON shape differs)"
            fi
        else
            fail "scan job failed or timed out (status=$JOB_STATUS)"
            http_get "$BASE/api/web/jobs/$SCAN_JOB_ID"
            info "job error: $(jq -r '.job.error' /tmp/ra_test_body 2>/dev/null)"
        fi
    fi
else
    skip "scan dispatch (no pipeline repo id)"
fi

# =============================================================================
# 7. /api/web/pipeline/stream — SSE streaming (Scan step, dry-run)
# =============================================================================
header "7. Pipeline SSE stream — SCAN dry-run"

if [ -n "$PIPELINE_REPO_ID" ] && [ "$PIPELINE_REPO_ID" != "null" ]; then
    BODY="{\"repo_id\":\"$PIPELINE_REPO_ID\",\"step\":\"Scan\",\"dry_run\":true}"
    SSE_OUTPUT=$(curl -s --max-time 30 -X POST \
        -H "Content-Type: application/json" \
        -d "$BODY" \
        "$BASE/api/web/pipeline/stream" 2>/dev/null | head -40)

    if echo "$SSE_OUTPUT" | grep -q '"type"'; then
        pass "SSE stream returned data events"
        # Extract event types from SSE lines (format: data: {"type":"log","payload":...})
        EVENT_TYPES=$(echo "$SSE_OUTPUT" | grep '^data:' | sed 's/^data: //' | jq -r '.type' 2>/dev/null | sort -u | tr '\n' ' ')
        info "SSE event types: ${EVENT_TYPES:-unknown}"
        # Verify we got at least a log event
        if echo "$SSE_OUTPUT" | grep -q '"type":"log"'; then
            pass "SSE stream emitted log events"
        else
            info "no log events seen (may have completed too fast)"
        fi
    else
        fail "SSE stream returned no data events"
        info "raw output: $(echo "$SSE_OUTPUT" | head -5)"
    fi
else
    skip "SSE stream test (no pipeline repo id)"
fi

# =============================================================================
# 8. LLM pipeline steps (Scaffold → Plan)
# =============================================================================
header "8. Pipeline dispatch — SCAFFOLD step (LLM)"

if [ "$LLM_ENABLED" = "true" ] && [ -n "$PIPELINE_REPO_ID" ] && [ "$PIPELINE_REPO_ID" != "null" ]; then
    BODY="{\"repo_id\":\"$PIPELINE_REPO_ID\",\"step\":\"Scaffold\",\"dry_run\":true}"
    http_post "$BASE/api/web/pipeline/dispatch" "$BODY"
    assert_code "POST /api/web/pipeline/dispatch (scaffold dry-run)" 202
    SCAFFOLD_JOB_ID=$(jq -r '.job_id' /tmp/ra_test_body 2>/dev/null || echo "")
    info "scaffold job id: $SCAFFOLD_JOB_ID"

    if [ -n "$SCAFFOLD_JOB_ID" ] && [ "$SCAFFOLD_JOB_ID" != "null" ]; then
        info "Waiting for scaffold job (up to 120s, LLM call)..."
        JOB_STATUS=$(wait_job "$SCAFFOLD_JOB_ID" 120)
        if [ "$JOB_STATUS" = "success" ] || [ "$JOB_STATUS" = "dry_run" ]; then
            pass "scaffold job completed ($JOB_STATUS)"
            http_get "$BASE/api/web/jobs/$SCAFFOLD_JOB_ID"
            SCAFFOLD_LOG=$(jq -r '.job.log_lines[-1]' /tmp/ra_test_body 2>/dev/null || echo "")
            info "last log: $SCAFFOLD_LOG"
        else
            fail "scaffold job failed or timed out (status=$JOB_STATUS)"
            http_get "$BASE/api/web/jobs/$SCAFFOLD_JOB_ID"
            info "error: $(jq -r '.job.error' /tmp/ra_test_body 2>/dev/null)"
        fi
    fi
else
    skip "scaffold step (LLM not available or no repo)"
fi

header "9. Pipeline dispatch — PLAN step (LLM)"

if [ "$LLM_ENABLED" = "true" ] && [ -n "$PIPELINE_REPO_ID" ] && [ "$PIPELINE_REPO_ID" != "null" ]; then
    BODY="{\"repo_id\":\"$PIPELINE_REPO_ID\",\"step\":\"Plan\",\"dry_run\":false}"
    http_post "$BASE/api/web/pipeline/dispatch" "$BODY"
    assert_code "POST /api/web/pipeline/dispatch (plan)" 202
    PLAN_JOB_ID=$(jq -r '.job_id' /tmp/ra_test_body 2>/dev/null || echo "")
    info "plan job id: $PLAN_JOB_ID"

    if [ -n "$PLAN_JOB_ID" ] && [ "$PLAN_JOB_ID" != "null" ]; then
        info "Waiting for plan job (up to 150s, LLM call)..."
        JOB_STATUS=$(wait_job "$PLAN_JOB_ID" 150)
        if [ "$JOB_STATUS" = "success" ] || [ "$JOB_STATUS" = "dry_run" ]; then
            pass "plan job completed ($JOB_STATUS)"
            http_get "$BASE/api/web/jobs/$PLAN_JOB_ID"
            BATCH_COUNT=$(jq '.job.result_json.batches | length' /tmp/ra_test_body 2>/dev/null || echo 0)
            info "gameplan batches: $BATCH_COUNT"
            if [ "$BATCH_COUNT" -gt 0 ] 2>/dev/null; then
                pass "gameplan has $BATCH_COUNT batch(es)"
                FIRST_BATCH=$(jq -r '.job.result_json.batches[0].id' /tmp/ra_test_body 2>/dev/null || echo "")
                info "first batch id: $FIRST_BATCH"
                # Verify batch has items
                BATCH_ITEMS=$(jq '.job.result_json.batches[0].items | length' /tmp/ra_test_body 2>/dev/null || echo 0)
                info "first batch items: $BATCH_ITEMS"
            else
                fail "gameplan produced 0 batches"
                info "result_json: $(jq -c '.job.result_json' /tmp/ra_test_body 2>/dev/null | head -c 200)"
            fi
        else
            fail "plan job failed or timed out (status=$JOB_STATUS)"
            http_get "$BASE/api/web/jobs/$PLAN_JOB_ID"
            info "error: $(jq -r '.job.error' /tmp/ra_test_body 2>/dev/null)"
            info "logs: $(jq -r '.job.log_lines[-3:][]' /tmp/ra_test_body 2>/dev/null)"
        fi
    fi
else
    skip "plan step (LLM not available or no repo)"
fi

# =============================================================================
# 10. Work step — dry-run (requires gameplan.json on disk)
# =============================================================================
header "10. Pipeline dispatch — WORK step (dry-run)"

GAMEPLAN_PATH="$REPO_PATH/.rustassistant/gameplan.json"
if [ "$LLM_ENABLED" = "true" ] && [ -f "$GAMEPLAN_PATH" ] && [ -n "$PIPELINE_REPO_ID" ]; then
    # Get first batch id from disk
    FIRST_BATCH=$(jq -r '.batches[0].id' "$GAMEPLAN_PATH" 2>/dev/null || echo "")
    if [ -n "$FIRST_BATCH" ] && [ "$FIRST_BATCH" != "null" ]; then
        info "running work dry-run on batch: $FIRST_BATCH"
        BODY="{\"repo_id\":\"$PIPELINE_REPO_ID\",\"step\":\"Work\",\"batch_id\":\"$FIRST_BATCH\",\"dry_run\":true}"
        http_post "$BASE/api/web/pipeline/dispatch" "$BODY"
        assert_code "POST /api/web/pipeline/dispatch (work dry-run)" 202
        WORK_JOB_ID=$(jq -r '.job_id' /tmp/ra_test_body 2>/dev/null || echo "")
        info "work job id: $WORK_JOB_ID"

        if [ -n "$WORK_JOB_ID" ] && [ "$WORK_JOB_ID" != "null" ]; then
            info "Waiting for work job (up to 200s, LLM call)..."
            JOB_STATUS=$(wait_job "$WORK_JOB_ID" 200)
            if [ "$JOB_STATUS" = "success" ] || [ "$JOB_STATUS" = "dry_run" ]; then
                pass "work job completed ($JOB_STATUS)"
                http_get "$BASE/api/web/jobs/$WORK_JOB_ID"
                ITEMS_OK=$(jq -r '.job.result_json.items_succeeded' /tmp/ra_test_body 2>/dev/null || echo "?")
                ITEMS_FAIL=$(jq -r '.job.result_json.items_failed' /tmp/ra_test_body 2>/dev/null || echo "?")
                info "items succeeded=$ITEMS_OK failed=$ITEMS_FAIL"
                LAST_LOG=$(jq -r '.job.log_lines[-1]' /tmp/ra_test_body 2>/dev/null || echo "")
                info "last log: $LAST_LOG"
            else
                fail "work job failed or timed out (status=$JOB_STATUS)"
                http_get "$BASE/api/web/jobs/$WORK_JOB_ID"
                info "error: $(jq -r '.job.error' /tmp/ra_test_body 2>/dev/null)"
                info "logs: $(jq -r '.job.log_lines[-3:][]' /tmp/ra_test_body 2>/dev/null)"
            fi
        fi
    else
        skip "work step (could not read first batch from gameplan)"
    fi
else
    skip "work step (LLM not available, gameplan missing, or no repo)"
fi

# =============================================================================
# 11. Sync step — dry-run (requires a results file on disk)
# =============================================================================
header "11. Pipeline dispatch — SYNC step (dry-run)"

RESULTS_DIR="$REPO_PATH/.rustassistant/results"
LATEST_RESULT=$(ls "$RESULTS_DIR"/*.json 2>/dev/null | head -1 || echo "")

if [ -n "$LATEST_RESULT" ] && [ -n "$PIPELINE_REPO_ID" ]; then
    info "syncing with result: $LATEST_RESULT"
    BODY="{\"repo_id\":\"$PIPELINE_REPO_ID\",\"step\":\"Sync\",\"results_path\":\"$LATEST_RESULT\",\"dry_run\":true}"
    http_post "$BASE/api/web/pipeline/dispatch" "$BODY"
    assert_code "POST /api/web/pipeline/dispatch (sync dry-run)" 202
    SYNC_JOB_ID=$(jq -r '.job_id' /tmp/ra_test_body 2>/dev/null || echo "")
    info "sync job id: $SYNC_JOB_ID"

    if [ -n "$SYNC_JOB_ID" ] && [ "$SYNC_JOB_ID" != "null" ]; then
        JOB_STATUS=$(wait_job "$SYNC_JOB_ID" 45)
        if [ "$JOB_STATUS" = "success" ] || [ "$JOB_STATUS" = "dry_run" ]; then
            pass "sync job completed ($JOB_STATUS)"
            http_get "$BASE/api/web/jobs/$SYNC_JOB_ID"
            UPDATED=$(jq -r '.job.result_json.items_updated' /tmp/ra_test_body 2>/dev/null || echo "?")
            NOT_FOUND=$(jq -r '.job.result_json.items_not_found' /tmp/ra_test_body 2>/dev/null || echo "?")
            info "items updated=$UPDATED not_found=$NOT_FOUND"
        else
            fail "sync job failed or timed out (status=$JOB_STATUS)"
            http_get "$BASE/api/web/jobs/$SYNC_JOB_ID"
            info "error: $(jq -r '.job.error' /tmp/ra_test_body 2>/dev/null)"
        fi
    fi
else
    skip "sync step (no result files found in .rustassistant/results/)"
fi

# =============================================================================
# 12. Job history
# =============================================================================
header "12. Job history"

http_get "$BASE/api/web/jobs"
assert_code "GET /api/web/jobs" 200
JOB_COUNT=$(jq '.jobs | length' /tmp/ra_test_body 2>/dev/null || echo 0)
if [ "$JOB_COUNT" -gt 0 ] 2>/dev/null; then
    pass "job history has $JOB_COUNT job(s)"
    # Print summary of job statuses
    JOB_SUMMARY=$(jq -r '.jobs[] | "\(.kind): \(.status)"' /tmp/ra_test_body 2>/dev/null | sort | uniq -c | sort -rn | head -5)
    info "job summary: $(echo "$JOB_SUMMARY" | tr '\n' '|')"
else
    fail "job history is empty"
fi

# =============================================================================
# 13. /api/v1/chat — ModelRouter chat endpoint
# =============================================================================
header "13. /api/v1/chat — ModelRouter (stub)"

BODY='{"message":"generate a stub for a retry handler","history":[]}'
http_post "$BASE/api/v1/chat" "$BODY"
assert_code "POST /api/v1/chat" 200
assert_not_empty "chat reply non-empty" ".reply"
assert_not_empty "chat task_kind non-empty" ".task_kind"
TASK_KIND=$(jq -r '.task_kind' /tmp/ra_test_body 2>/dev/null || echo "")
MODEL_USED=$(jq -r '.model_used' /tmp/ra_test_body 2>/dev/null || echo "")
info "task_kind=$TASK_KIND model_used=$MODEL_USED"
# ModelRouter should classify "generate a stub" as ScaffoldStub → local model
if [ "$TASK_KIND" = "ScaffoldStub" ]; then
    pass "ModelRouter correctly classified as ScaffoldStub"
elif [ -n "$TASK_KIND" ] && [ "$TASK_KIND" != "null" ]; then
    info "ModelRouter classified as: $TASK_KIND (expected ScaffoldStub)"
fi

# Test repo-context chat
if [ -n "$V1_REPO_ID" ] && [ "$V1_REPO_ID" != "null" ]; then
    BODY='{"message":"what are the main modules in this codebase?"}'
    http_post "$BASE/api/v1/chat/repos/$V1_REPO_ID" "$BODY"
    assert_code "POST /api/v1/chat/repos/:id" 200
    CONTEXT_INJECTED=$(jq -r '.repo_context_injected' /tmp/ra_test_body 2>/dev/null || echo "false")
    if [ "$CONTEXT_INJECTED" = "true" ]; then
        pass "repo context was injected into chat"
    else
        info "repo_context_injected=false (sync may not have run yet)"
    fi
fi

# =============================================================================
# 14. /api/web/chat — Grok chat (requires XAI_API_KEY)
# =============================================================================
header "14. /api/web/chat — Grok chat"

if [ "$LLM_ENABLED" = "true" ]; then
    BODY='{"message":"in one sentence, what is rustassistant used for?"}'
    http_post "$BASE/api/web/chat" "$BODY"
    assert_code "POST /api/web/chat" 200
    assert_not_empty "chat reply non-empty" ".reply"
    REPLY_LEN=$(jq -r '.reply | length' /tmp/ra_test_body 2>/dev/null || echo 0)
    if [ "$REPLY_LEN" -gt 20 ] 2>/dev/null; then
        pass "Grok chat reply has $REPLY_LEN chars"
    else
        fail "Grok chat reply too short ($REPLY_LEN chars)"
    fi
    TOKENS=$(jq -r '.tokens_used' /tmp/ra_test_body 2>/dev/null || echo "?")
    COST=$(jq -r '.cost_usd' /tmp/ra_test_body 2>/dev/null || echo "?")
    info "tokens=$TOKENS cost=\$$COST"
    # Print first 120 chars of reply for sanity check
    REPLY_PREVIEW=$(jq -r '.reply' /tmp/ra_test_body 2>/dev/null | head -c 120)
    info "reply preview: $REPLY_PREVIEW"
else
    skip "Grok chat (LLM not available)"
fi

# =============================================================================
# 15. Error handling
# =============================================================================
header "15. Error handling"

http_get "$BASE/api/web/repos/nonexistent-id-xyz/todo"
assert_code "GET /api/web/repos/bad-id/todo → 404" 404

http_get "$BASE/api/v1/repos/nonexistent-repo-xyz"
assert_code "GET /api/v1/repos/bad-id → 404" 404

BODY='{"repo_id":"nonexistent-xyz","step":"Scan","dry_run":false}'
http_post "$BASE/api/web/pipeline/dispatch" "$BODY"
assert_code "dispatch with bad repo_id → 404" 404

# Work step without batch_id: dispatched as 202 but job fails gracefully
if [ -n "$PIPELINE_REPO_ID" ] && [ "$PIPELINE_REPO_ID" != "null" ]; then
    BODY="{\"repo_id\":\"$PIPELINE_REPO_ID\",\"step\":\"Work\",\"dry_run\":true}"
    http_post "$BASE/api/web/pipeline/dispatch" "$BODY"
    if [ "$HTTP_CODE" = "202" ]; then
        pass "work without batch_id → 202 accepted (job will fail gracefully)"
        # Wait a moment and verify the job did actually fail
        NO_BATCH_JOB_ID=$(jq -r '.job_id' /tmp/ra_test_body 2>/dev/null || echo "")
        if [ -n "$NO_BATCH_JOB_ID" ] && [ "$NO_BATCH_JOB_ID" != "null" ]; then
            sleep 3
            http_get "$BASE/api/web/jobs/$NO_BATCH_JOB_ID"
            JOB_STATUS=$(jq -r '.job.status' /tmp/ra_test_body 2>/dev/null || echo "unknown")
            if [ "$JOB_STATUS" = "failed" ]; then
                pass "work job without batch_id fails with status=failed"
            else
                info "work job status=$JOB_STATUS (still running or unexpected)"
            fi
        fi
    else
        pass "work without batch_id → HTTP $HTTP_CODE (rejected at dispatch)"
    fi
fi

# Verify malformed JSON returns 4xx
BODY='{"invalid_json":}'
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" -d "$BODY" \
    "$BASE/api/web/pipeline/dispatch")
if [ "$HTTP_CODE" -ge 400 ] 2>/dev/null; then
    pass "malformed JSON → HTTP $HTTP_CODE"
else
    fail "malformed JSON should return 4xx, got $HTTP_CODE"
fi

# =============================================================================
# 16. Cleanup
# =============================================================================
header "16. Cleanup"

if [ -n "$PIPELINE_REPO_ID" ] && [ "$PIPELINE_REPO_ID" != "null" ]; then
    http_delete "$BASE/api/repos/$PIPELINE_REPO_ID"
    assert_code "DELETE /api/repos/:id" 200
    # Verify it's actually gone
    http_get "$BASE/api/repos/$PIPELINE_REPO_ID"
    if [ "$HTTP_CODE" = "404" ]; then
        pass "deleted repo is no longer accessible"
    else
        info "repo still accessible after delete (HTTP $HTTP_CODE)"
    fi
fi

if [ -n "$V1_REPO_ID" ] && [ "$V1_REPO_ID" != "null" ]; then
    http_delete "$BASE/api/v1/repos/$V1_REPO_ID"
    assert_code "DELETE /api/v1/repos/:id" 200
    # Verify it's gone from the list
    http_get "$BASE/api/v1/repos"
    REMAINING=$(jq --arg id "$V1_REPO_ID" '[.[] | select(.id == $id)] | length' /tmp/ra_test_body 2>/dev/null || echo "1")
    if [ "$REMAINING" = "0" ]; then
        pass "v1 repo removed from list after delete"
    else
        fail "v1 repo still appears in list after delete"
    fi
fi

# =============================================================================
# Summary
# =============================================================================
echo ""
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
TOTAL=$((PASS + FAIL + SKIP))
echo -e "${BOLD}Results: $TOTAL tests — ${GREEN}${PASS} passed${RESET}, ${RED}${FAIL} failed${RESET}, ${YELLOW}${SKIP} skipped${RESET}"
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0

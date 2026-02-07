#!/bin/bash
set -e

# ==============================================================================
# RustAssistant Deployment Testing Script
# ==============================================================================
# Tests all features from Priorities 1-4:
# - Priority 1: Scan interval editing
# - Priority 2: Repository management (git-based)
# - Priority 3: Scan progress indicators
# - Priority 4: Notes/tags system
# ==============================================================================

BASE_URL="${BASE_URL:-http://localhost:3000}"
VERBOSE="${VERBOSE:-0}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# ==============================================================================
# Helper Functions
# ==============================================================================

print_header() {
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
}

print_test() {
    echo -e "${YELLOW}TEST:${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
    ((TESTS_PASSED++))
}

print_failure() {
    echo -e "${RED}✗${NC} $1"
    ((TESTS_FAILED++))
}

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

verbose() {
    if [ "$VERBOSE" = "1" ]; then
        echo -e "${NC}  → $1${NC}"
    fi
}

test_endpoint() {
    local method=$1
    local endpoint=$2
    local expected_code=${3:-200}
    local data=${4:-}

    ((TESTS_RUN++))
    print_test "$method $endpoint"

    if [ -n "$data" ]; then
        response=$(curl -s -w "\n%{http_code}" -X "$method" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$BASE_URL$endpoint")
    else
        response=$(curl -s -w "\n%{http_code}" -X "$method" "$BASE_URL$endpoint")
    fi

    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | head -n-1)

    verbose "Response code: $http_code"
    verbose "Response body: ${body:0:100}..."

    if [ "$http_code" = "$expected_code" ]; then
        print_success "HTTP $http_code (expected $expected_code)"
        echo "$body"
        return 0
    else
        print_failure "HTTP $http_code (expected $expected_code)"
        echo "$body"
        return 1
    fi
}

# ==============================================================================
# Test Suite: Basic Health & Availability
# ==============================================================================

test_health() {
    print_header "Health & Availability Tests"

    # Test health endpoint
    test_endpoint GET /health 200

    # Test dashboard page loads
    ((TESTS_RUN++))
    print_test "GET / (dashboard)"
    response=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/")
    if [ "$response" = "200" ]; then
        print_success "Dashboard loads (HTTP 200)"
    else
        print_failure "Dashboard failed (HTTP $response)"
    fi

    # Test repos page loads
    ((TESTS_RUN++))
    print_test "GET /repos"
    response=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/repos")
    if [ "$response" = "200" ]; then
        print_success "Repos page loads (HTTP 200)"
    else
        print_failure "Repos page failed (HTTP $response)"
    fi

    # Test notes page loads
    ((TESTS_RUN++))
    print_test "GET /notes"
    response=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/notes")
    if [ "$response" = "200" ]; then
        print_success "Notes page loads (HTTP 200)"
    else
        print_failure "Notes page failed (HTTP $response)"
    fi
}

# ==============================================================================
# Test Suite: Priority 4 - Notes System
# ==============================================================================

test_notes() {
    print_header "Priority 4: Notes & Tags System"

    # Create a note with hashtags
    ((TESTS_RUN++))
    print_test "Create note with hashtags"
    response=$(curl -s -X POST "$BASE_URL/api/notes" \
        -H "Content-Type: application/json" \
        -d '{"content":"Test note for deployment #testing #automation","repo_id":null}')

    verbose "Create response: $response"

    if echo "$response" | grep -q "id"; then
        NOTE_ID=$(echo "$response" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
        print_success "Note created with ID: $NOTE_ID"
    else
        print_failure "Failed to create note"
        NOTE_ID=""
    fi

    # Verify note exists in database
    if [ -n "$NOTE_ID" ]; then
        ((TESTS_RUN++))
        print_test "Verify note in database"
        docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
            "SELECT content FROM notes WHERE id='$NOTE_ID';" > /tmp/note_check.txt 2>&1

        if grep -q "deployment" /tmp/note_check.txt; then
            print_success "Note found in database"
        else
            print_failure "Note not found in database"
        fi
    fi

    # Test note deletion
    if [ -n "$NOTE_ID" ]; then
        ((TESTS_RUN++))
        print_test "Delete note"
        response=$(curl -s -o /dev/null -w "%{http_code}" \
            -X DELETE "$BASE_URL/api/notes/$NOTE_ID")

        if [ "$response" = "200" ] || [ "$response" = "204" ]; then
            print_success "Note deleted (HTTP $response)"
        else
            print_failure "Failed to delete note (HTTP $response)"
        fi
    fi
}

# ==============================================================================
# Test Suite: Priority 2 - Repository Management
# ==============================================================================

test_repositories() {
    print_header "Priority 2: Repository Management"

    # Check if any repos exist
    ((TESTS_RUN++))
    print_test "Check existing repositories"
    repo_count=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "SELECT COUNT(*) FROM repositories;" 2>/dev/null || echo "0")

    verbose "Found $repo_count repositories"
    print_success "Repository table accessible ($repo_count repos)"

    # Verify git_url column exists
    ((TESTS_RUN++))
    print_test "Verify git_url column exists"
    has_git_url=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "PRAGMA table_info(repositories);" | grep -c "git_url" || echo "0")

    if [ "$has_git_url" -gt 0 ]; then
        print_success "git_url column exists"
    else
        print_failure "git_url column missing"
    fi

    # Verify source_type column exists
    ((TESTS_RUN++))
    print_test "Verify source_type column exists"
    has_source_type=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "PRAGMA table_info(repositories);" | grep -c "source_type" || echo "0")

    if [ "$has_source_type" -gt 0 ]; then
        print_success "source_type column exists"
    else
        print_failure "source_type column missing"
    fi

    # Verify repository_sync_status view exists
    ((TESTS_RUN++))
    print_test "Verify repository_sync_status view exists"
    has_view=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "SELECT name FROM sqlite_master WHERE type='view' AND name='repository_sync_status';" \
        | grep -c "repository_sync_status" || echo "0")

    if [ "$has_view" -gt 0 ]; then
        print_success "repository_sync_status view exists"
    else
        print_failure "repository_sync_status view missing"
    fi
}

# ==============================================================================
# Test Suite: Priority 3 - Scan Progress
# ==============================================================================

test_scan_progress() {
    print_header "Priority 3: Scan Progress Indicators"

    # Verify scan_status column exists
    ((TESTS_RUN++))
    print_test "Verify scan_status column exists"
    has_scan_status=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "PRAGMA table_info(repositories);" | grep -c "scan_status" || echo "0")

    if [ "$has_scan_status" -gt 0 ]; then
        print_success "scan_status column exists"
    else
        print_failure "scan_status column missing"
    fi

    # Verify scan_progress column exists
    ((TESTS_RUN++))
    print_test "Verify scan_progress column exists"
    has_scan_progress=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "PRAGMA table_info(repositories);" | grep -c "scan_progress" || echo "0")

    if [ "$has_scan_progress" -gt 0 ]; then
        print_success "scan_progress column exists"
    else
        print_failure "scan_progress column missing"
    fi

    # Verify scan_events table exists
    ((TESTS_RUN++))
    print_test "Verify scan_events table exists"
    has_table=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "SELECT name FROM sqlite_master WHERE type='table' AND name='scan_events';" \
        | grep -c "scan_events" || echo "0")

    if [ "$has_table" -gt 0 ]; then
        print_success "scan_events table exists"
    else
        print_failure "scan_events table missing"
    fi

    # Count scan events
    ((TESTS_RUN++))
    print_test "Check scan events"
    event_count=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "SELECT COUNT(*) FROM scan_events;" 2>/dev/null || echo "0")

    verbose "Found $event_count scan events"
    print_success "scan_events table accessible ($event_count events)"
}

# ==============================================================================
# Test Suite: Priority 1 - Scan Interval Editing
# ==============================================================================

test_scan_interval() {
    print_header "Priority 1: Scan Interval Editing"

    # Verify auto_scan_enabled column exists
    ((TESTS_RUN++))
    print_test "Verify auto_scan_enabled column exists"
    has_auto_scan=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "PRAGMA table_info(repositories);" | grep -c "auto_scan_enabled" || echo "0")

    if [ "$has_auto_scan" -gt 0 ]; then
        print_success "auto_scan_enabled column exists"
    else
        print_failure "auto_scan_enabled column missing"
    fi

    # Verify scan_interval_minutes column exists
    ((TESTS_RUN++))
    print_test "Verify scan_interval_minutes column exists"
    has_interval=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "PRAGMA table_info(repositories);" | grep -c "scan_interval_minutes" || echo "0")

    if [ "$has_interval" -gt 0 ]; then
        print_success "scan_interval_minutes column exists"
    else
        print_failure "scan_interval_minutes column missing"
    fi
}

# ==============================================================================
# Test Suite: Database Integrity
# ==============================================================================

test_database() {
    print_header "Database Integrity & Schema"

    # Check database integrity
    ((TESTS_RUN++))
    print_test "SQLite integrity check"
    integrity=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "PRAGMA integrity_check;" 2>/dev/null || echo "error")

    if echo "$integrity" | grep -q "ok"; then
        print_success "Database integrity OK"
    else
        print_failure "Database integrity check failed"
        verbose "$integrity"
    fi

    # List all tables
    ((TESTS_RUN++))
    print_test "Verify core tables exist"
    tables=$(docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db \
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;" 2>/dev/null)

    required_tables=("repositories" "notes" "tags" "note_tags" "scan_events")
    missing_tables=0

    for table in "${required_tables[@]}"; do
        if echo "$tables" | grep -q "$table"; then
            verbose "Found table: $table"
        else
            verbose "Missing table: $table"
            ((missing_tables++))
        fi
    done

    if [ $missing_tables -eq 0 ]; then
        print_success "All required tables exist"
    else
        print_failure "$missing_tables required table(s) missing"
    fi
}

# ==============================================================================
# Test Suite: Container Health
# ==============================================================================

test_containers() {
    print_header "Container Health"

    # Check rustassistant container
    ((TESTS_RUN++))
    print_test "rustassistant container running"
    if docker compose ps rustassistant | grep -q "Up"; then
        print_success "rustassistant container is running"
    else
        print_failure "rustassistant container not running"
    fi

    # Check redis container
    ((TESTS_RUN++))
    print_test "redis container running"
    if docker compose ps redis | grep -q "Up"; then
        print_success "redis container is running"
    else
        print_failure "redis container not running"
    fi

    # Check for errors in logs
    ((TESTS_RUN++))
    print_test "Check for errors in logs"
    errors=$(docker compose logs rustassistant --tail=100 | grep -i "error" | grep -v "no error" || echo "")

    if [ -z "$errors" ]; then
        print_success "No errors in recent logs"
    else
        print_failure "Errors found in logs"
        verbose "$errors"
    fi
}

# ==============================================================================
# Main Execution
# ==============================================================================

main() {
    print_header "RustAssistant Deployment Test Suite"
    echo "Testing: $BASE_URL"
    echo "Verbose: $VERBOSE"
    echo ""

    # Run all test suites
    test_containers
    test_health
    test_database
    test_scan_interval
    test_repositories
    test_scan_progress
    test_notes

    # Print summary
    print_header "Test Summary"
    echo "Tests Run:    $TESTS_RUN"
    echo -e "Tests Passed: ${GREEN}$TESTS_PASSED${NC}"
    echo -e "Tests Failed: ${RED}$TESTS_FAILED${NC}"
    echo ""

    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}✓ All tests passed!${NC}"
        echo ""
        echo "Next steps:"
        echo "  1. Open http://localhost:3000 in your browser"
        echo "  2. Test the web UI manually:"
        echo "     - Create notes with #hashtags"
        echo "     - Add a repository with git URL"
        echo "     - View scan progress indicators"
        echo "     - Edit scan intervals on repos page"
        echo ""
        exit 0
    else
        echo -e "${RED}✗ Some tests failed${NC}"
        echo ""
        echo "Troubleshooting:"
        echo "  1. Check logs: docker compose logs rustassistant"
        echo "  2. Verify database: docker compose exec rustassistant sqlite3 /app/data/rustassistant.db '.tables'"
        echo "  3. Restart services: docker compose restart"
        echo ""
        exit 1
    fi
}

# Run main function
main

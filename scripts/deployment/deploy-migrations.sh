#!/bin/bash
set -e

# ==============================================================================
# RustAssistant Migration Deployment Script
# ==============================================================================
# This script safely applies migrations 003, 004, and 005
# Includes automatic backup and rollback capability
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DB_PATH="$SCRIPT_DIR/data/rustassistant.db"
MIGRATIONS_DIR="$SCRIPT_DIR/migrations"
BACKUP_DIR="$SCRIPT_DIR/data/backups"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_PATH="$BACKUP_DIR/rustassistant.db.backup-$TIMESTAMP"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

check_prerequisites() {
    print_header "Checking Prerequisites"

    # Check if database exists
    if [ ! -f "$DB_PATH" ]; then
        print_error "Database not found at: $DB_PATH"
        exit 1
    fi
    print_success "Database found"

    # Check if sqlite3 is installed
    if ! command -v sqlite3 &> /dev/null; then
        print_error "sqlite3 command not found. Please install sqlite3."
        exit 1
    fi
    print_success "sqlite3 installed"

    # Check if migrations exist
    local missing=0
    for migration in 003_scan_progress.sql 004_require_git_url.sql 005_notes_enhancements.sql; do
        if [ ! -f "$MIGRATIONS_DIR/$migration" ]; then
            print_error "Migration not found: $migration"
            missing=1
        fi
    done

    if [ $missing -eq 1 ]; then
        exit 1
    fi
    print_success "All migration files found"

    # Check database integrity
    print_info "Checking database integrity..."
    if ! sqlite3 "$DB_PATH" "PRAGMA integrity_check;" | grep -q "ok"; then
        print_error "Database integrity check failed!"
        exit 1
    fi
    print_success "Database integrity OK"
}

create_backup() {
    print_header "Creating Backup"

    # Create backup directory if it doesn't exist
    mkdir -p "$BACKUP_DIR"

    # Copy database
    print_info "Backing up database to: $BACKUP_PATH"
    cp "$DB_PATH" "$BACKUP_PATH"

    # Verify backup
    if [ ! -f "$BACKUP_PATH" ]; then
        print_error "Backup failed!"
        exit 1
    fi

    # Check backup size
    ORIGINAL_SIZE=$(stat -f%z "$DB_PATH" 2>/dev/null || stat -c%s "$DB_PATH" 2>/dev/null)
    BACKUP_SIZE=$(stat -f%z "$BACKUP_PATH" 2>/dev/null || stat -c%s "$BACKUP_PATH" 2>/dev/null)

    if [ "$ORIGINAL_SIZE" != "$BACKUP_SIZE" ]; then
        print_error "Backup size mismatch! Original: $ORIGINAL_SIZE, Backup: $BACKUP_SIZE"
        exit 1
    fi

    print_success "Backup created successfully"
    print_info "Backup location: $BACKUP_PATH"
    print_info "Backup size: $(numfmt --to=iec $BACKUP_SIZE 2>/dev/null || echo "$BACKUP_SIZE bytes")"
}

check_migration_needed() {
    local migration_name=$1
    print_info "Checking if migration needed: $migration_name"

    case $migration_name in
        "003_scan_progress")
            # Check if scan_events table exists
            if sqlite3 "$DB_PATH" "SELECT name FROM sqlite_master WHERE type='table' AND name='scan_events';" | grep -q "scan_events"; then
                return 1  # Already applied
            fi
            return 0  # Needs to be applied
            ;;
        "004_require_git_url")
            # Check if repository_sync_status table exists
            if sqlite3 "$DB_PATH" "SELECT name FROM sqlite_master WHERE type='table' AND name='repository_sync_status';" | grep -q "repository_sync_status"; then
                return 1
            fi
            return 0
            ;;
        "005_notes_enhancements")
            # Check if notes table has repo_id column
            if sqlite3 "$DB_PATH" "PRAGMA table_info(notes);" | grep -q "repo_id"; then
                return 1
            fi
            return 0
            ;;
    esac
    return 0
}

apply_migration() {
    local migration_file=$1
    local migration_name=$(basename "$migration_file" .sql)

    print_info "Applying migration: $migration_name"

    # Check if migration is needed
    if ! check_migration_needed "$migration_name"; then
        print_warning "Migration $migration_name already applied, skipping"
        return 0
    fi

    # Apply migration
    if sqlite3 "$DB_PATH" < "$migration_file"; then
        print_success "Migration $migration_name applied successfully"
        return 0
    else
        print_error "Migration $migration_name failed!"
        return 1
    fi
}

verify_migrations() {
    print_header "Verifying Migrations"

    # Check Priority 3: Scan progress
    print_info "Verifying Priority 3 (Scan Progress)..."

    if ! sqlite3 "$DB_PATH" "SELECT name FROM sqlite_master WHERE type='table' AND name='scan_events';" | grep -q "scan_events"; then
        print_error "scan_events table not found"
        return 1
    fi
    print_success "scan_events table exists"

    # Check for scan_status column
    if ! sqlite3 "$DB_PATH" "PRAGMA table_info(repositories);" | grep -q "scan_status"; then
        print_error "scan_status column not found in repositories table"
        return 1
    fi
    print_success "scan_status column exists"

    # Check Priority 4: Repository sync
    print_info "Verifying Priority 4 (Repository Sync)..."

    if ! sqlite3 "$DB_PATH" "SELECT name FROM sqlite_master WHERE type='table' AND name='repository_sync_status';" | grep -q "repository_sync_status"; then
        print_error "repository_sync_status table not found"
        return 1
    fi
    print_success "repository_sync_status table exists"

    # Check Priority 5: Notes enhancements
    print_info "Verifying Priority 5 (Notes Enhancements)..."

    if ! sqlite3 "$DB_PATH" "PRAGMA table_info(notes);" | grep -q "repo_id"; then
        print_error "repo_id column not found in notes table"
        return 1
    fi
    print_success "repo_id column exists in notes table"

    if ! sqlite3 "$DB_PATH" "SELECT name FROM sqlite_master WHERE type='table' AND name='tags';" | grep -q "tags"; then
        print_error "tags table not found"
        return 1
    fi
    print_success "tags table exists"

    if ! sqlite3 "$DB_PATH" "SELECT name FROM sqlite_master WHERE type='table' AND name='note_tags';" | grep -q "note_tags"; then
        print_error "note_tags table not found"
        return 1
    fi
    print_success "note_tags table exists"

    print_success "All migrations verified successfully!"
    return 0
}

show_summary() {
    print_header "Migration Summary"

    echo "Database: $DB_PATH"
    echo "Backup: $BACKUP_PATH"
    echo ""

    # Count repositories
    local repo_count=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM repositories;")
    print_info "Repositories: $repo_count"

    # Count notes
    local notes_count=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM notes;")
    print_info "Notes: $notes_count"

    # Count tags
    local tags_count=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM tags;" 2>/dev/null || echo "0")
    print_info "Tags: $tags_count"

    # Count scan events
    local events_count=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM scan_events;" 2>/dev/null || echo "0")
    print_info "Scan events: $events_count"

    echo ""
    print_success "All migrations completed successfully!"
    echo ""
    print_info "Next steps:"
    echo "  1. Rebuild and restart the application:"
    echo "     docker compose build && docker compose up -d"
    echo ""
    echo "  2. Monitor logs for startup:"
    echo "     docker compose logs -f rustassistant"
    echo ""
    echo "  3. Test the web UI:"
    echo "     Open http://localhost:3001"
    echo ""
    echo "  4. To rollback if needed:"
    echo "     cp $BACKUP_PATH $DB_PATH"
    echo "     docker compose restart"
}

rollback_migration() {
    print_header "Rolling Back Migration"

    print_warning "Restoring backup from: $BACKUP_PATH"

    if [ ! -f "$BACKUP_PATH" ]; then
        print_error "Backup file not found!"
        exit 1
    fi

    cp "$BACKUP_PATH" "$DB_PATH"
    print_success "Database restored from backup"
}

# ==============================================================================
# Main Execution
# ==============================================================================

main() {
    print_header "RustAssistant Migration Deployment"
    echo "Timestamp: $TIMESTAMP"
    echo "Database: $DB_PATH"
    echo ""

    # Step 1: Check prerequisites
    check_prerequisites

    # Step 2: Create backup
    create_backup

    # Step 3: Apply migrations
    print_header "Applying Migrations"

    local failed=0

    # Migration 003: Scan progress
    if ! apply_migration "$MIGRATIONS_DIR/003_scan_progress.sql"; then
        failed=1
    fi

    # Migration 004: Repository sync
    if [ $failed -eq 0 ]; then
        if ! apply_migration "$MIGRATIONS_DIR/004_require_git_url.sql"; then
            failed=1
        fi
    fi

    # Migration 005: Notes enhancements
    if [ $failed -eq 0 ]; then
        if ! apply_migration "$MIGRATIONS_DIR/005_notes_enhancements.sql"; then
            failed=1
        fi
    fi

    # Step 4: Handle failure
    if [ $failed -eq 1 ]; then
        print_error "Migration failed!"
        read -p "Do you want to rollback? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            rollback_migration
        fi
        exit 1
    fi

    # Step 5: Verify migrations
    if ! verify_migrations; then
        print_error "Migration verification failed!"
        read -p "Do you want to rollback? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            rollback_migration
        fi
        exit 1
    fi

    # Step 6: Show summary
    show_summary
}

# Run main function
main

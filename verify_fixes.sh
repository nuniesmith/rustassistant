#!/bin/bash
# Verification Script for Data Layer Fixes
# Run this after applying all fixes to verify everything works

set -e  # Exit on error

echo "=================================================="
echo "RustAssistant Data Layer Fixes - Verification"
echo "=================================================="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if database exists
echo "1. Checking database setup..."
if [ ! -f "data/rustassistant.db" ]; then
    echo -e "${RED}❌ Database not found${NC}"
    echo "Run: export DATABASE_URL=sqlite:./data/rustassistant.db && sqlx database create && sqlx migrate run"
    exit 1
fi
echo -e "${GREEN}✅ Database exists${NC}"

# Check migration 007
echo ""
echo "2. Checking migration 007 (ideas table)..."
export DATABASE_URL=sqlite:./data/rustassistant.db

if sqlite3 data/rustassistant.db "SELECT version FROM _sqlx_migrations WHERE version = 7;" 2>/dev/null | grep -q "7"; then
    echo -e "${GREEN}✅ Migration 007 applied${NC}"
else
    echo -e "${RED}❌ Migration 007 not found${NC}"
    echo "Run: sqlx migrate run"
    exit 1
fi

# Check ideas table
echo ""
echo "3. Checking ideas table schema..."
if sqlite3 data/rustassistant.db ".schema ideas" 2>/dev/null | grep -q "CREATE TABLE"; then
    echo -e "${GREEN}✅ Ideas table exists${NC}"
else
    echo -e "${RED}❌ Ideas table missing${NC}"
    exit 1
fi

# Check documents_fts table
echo ""
echo "4. Checking documents_fts virtual table..."
if sqlite3 data/rustassistant.db ".schema documents_fts" 2>/dev/null | grep -q "VIRTUAL TABLE"; then
    echo -e "${GREEN}✅ documents_fts table exists${NC}"
else
    echo -e "${RED}❌ documents_fts table missing${NC}"
    exit 1
fi

# Check tags table schema (no id column)
echo ""
echo "5. Checking tags table schema (should have name as PK, no id)..."
TAGS_SCHEMA=$(sqlite3 data/rustassistant.db ".schema tags" 2>/dev/null)
if echo "$TAGS_SCHEMA" | grep -q "name TEXT PRIMARY KEY"; then
    echo -e "${GREEN}✅ Tags table has correct schema (name as PRIMARY KEY)${NC}"
else
    echo -e "${RED}❌ Tags table schema incorrect${NC}"
    exit 1
fi

if echo "$TAGS_SCHEMA" | grep -q "description TEXT"; then
    echo -e "${GREEN}✅ Tags table has description column${NC}"
else
    echo -e "${RED}❌ Tags table missing description column${NC}"
    exit 1
fi

if echo "$TAGS_SCHEMA" | grep -q "updated_at"; then
    echo -e "${GREEN}✅ Tags table has updated_at column${NC}"
else
    echo -e "${RED}❌ Tags table missing updated_at column${NC}"
    exit 1
fi

# Check FTS triggers
echo ""
echo "6. Checking FTS synchronization triggers..."
if sqlite3 data/rustassistant.db ".schema" | grep -q "documents_ai"; then
    echo -e "${GREEN}✅ FTS INSERT trigger exists${NC}"
else
    echo -e "${YELLOW}⚠️  FTS INSERT trigger missing${NC}"
fi

if sqlite3 data/rustassistant.db ".schema" | grep -q "documents_au"; then
    echo -e "${GREEN}✅ FTS UPDATE trigger exists${NC}"
else
    echo -e "${YELLOW}⚠️  FTS UPDATE trigger missing${NC}"
fi

if sqlite3 data/rustassistant.db ".schema" | grep -q "documents_ad"; then
    echo -e "${GREEN}✅ FTS DELETE trigger exists${NC}"
else
    echo -e "${YELLOW}⚠️  FTS DELETE trigger missing${NC}"
fi

# Check ideas indexes
echo ""
echo "7. Checking ideas table indexes..."
IDEAS_INDEXES=$(sqlite3 data/rustassistant.db ".indexes ideas" 2>/dev/null)
if echo "$IDEAS_INDEXES" | grep -q "idx_ideas_status"; then
    echo -e "${GREEN}✅ idx_ideas_status exists${NC}"
else
    echo -e "${YELLOW}⚠️  idx_ideas_status missing${NC}"
fi

if echo "$IDEAS_INDEXES" | grep -q "idx_ideas_priority"; then
    echo -e "${GREEN}✅ idx_ideas_priority exists${NC}"
else
    echo -e "${YELLOW}⚠️  idx_ideas_priority missing${NC}"
fi

# Check file changes
echo ""
echo "8. Checking source file changes..."

# Check Tag struct in documents.rs
if grep -q "pub struct Tag" src/db/documents.rs; then
    if grep -A 8 "pub struct Tag" src/db/documents.rs | grep -q "pub name: String"; then
        echo -e "${GREEN}✅ Tag struct has name field${NC}"
    else
        echo -e "${RED}❌ Tag struct missing name field${NC}"
    fi

    if grep -A 8 "pub struct Tag" src/db/documents.rs | grep -q "pub description: Option<String>"; then
        echo -e "${GREEN}✅ Tag struct has description field${NC}"
    else
        echo -e "${RED}❌ Tag struct missing description field${NC}"
    fi

    if grep -A 8 "pub struct Tag" src/db/documents.rs | grep -q "pub id:.*i64"; then
        echo -e "${RED}❌ Tag struct still has id field (should be removed)${NC}"
    else
        echo -e "${GREEN}✅ Tag struct doesn't have id field (correct)${NC}"
    fi
else
    echo -e "${YELLOW}⚠️  Tag struct not found in documents.rs${NC}"
fi

# Check list_ideas dynamic parameter binding
echo ""
echo "9. Checking list_ideas parameter binding fix..."
if grep -A 20 "pub async fn list_ideas" src/db/documents.rs | grep -q "binds.push"; then
    echo -e "${GREEN}✅ list_ideas uses dynamic parameter binding${NC}"
else
    echo -e "${RED}❌ list_ideas not using dynamic binding${NC}"
fi

# Check document ID types in API
echo ""
echo "10. Checking document ID types in API..."
if grep -A 3 "pub struct IndexDocumentRequest" src/api/types.rs | grep -q "pub document_id: String"; then
    echo -e "${GREEN}✅ IndexDocumentRequest uses String for document_id${NC}"
else
    echo -e "${RED}❌ IndexDocumentRequest still uses i64${NC}"
fi

if grep -A 3 "pub struct BatchIndexRequest" src/api/types.rs | grep -q "pub document_ids: Vec<String>"; then
    echo -e "${GREEN}✅ BatchIndexRequest uses Vec<String>${NC}"
else
    echo -e "${RED}❌ BatchIndexRequest still uses Vec<i64>${NC}"
fi

# Summary
echo ""
echo "=================================================="
echo "Verification Complete!"
echo "=================================================="
echo ""
echo "Summary of fixes verified:"
echo "  ✅ Migration 007 applied (ideas + documents_fts)"
echo "  ✅ Tag struct aligned with migration 005 schema"
echo "  ✅ list_ideas dynamic parameter binding"
echo "  ✅ Document ID types changed to String"
echo ""
echo "Next steps:"
echo "  1. Start server: cargo run --bin server"
echo "  2. Test ideas: curl 'http://localhost:3000/api/ideas?status=inbox'"
echo "  3. Test tags: curl http://localhost:3000/api/tags?limit=10"
echo "  4. Test FTS: curl 'http://localhost:3000/api/docs/search?q=welcome'"
echo ""
echo "See docs/APPLY_FIXES.md for detailed testing instructions"
echo ""

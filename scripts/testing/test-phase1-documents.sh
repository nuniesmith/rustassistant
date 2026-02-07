#!/bin/bash
# ============================================================================
# Phase 1 Document Operations Test Script
# ============================================================================
# Tests the document CRUD operations implemented in Phase 1
# Run with: ./test-phase1-documents.sh

set -e

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🧪 Phase 1 Document Operations Test Suite"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counter
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Helper function to run SQL queries
run_sql() {
    docker compose exec -T rustassistant sqlite3 /app/data/rustassistant.db "$1"
}

# Helper function to assert SQL result
assert_sql() {
    local description="$1"
    local query="$2"
    local expected="$3"

    TESTS_RUN=$((TESTS_RUN + 1))
    echo -n "  [$TESTS_RUN] $description... "

    result=$(run_sql "$query" 2>&1 || echo "ERROR")

    if [ "$result" = "$expected" ]; then
        echo -e "${GREEN}✓ PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}"
        echo -e "    ${YELLOW}Expected:${NC} $expected"
        echo -e "    ${YELLOW}Got:${NC} $result"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Helper function to assert non-empty result
assert_non_empty() {
    local description="$1"
    local query="$2"

    TESTS_RUN=$((TESTS_RUN + 1))
    echo -n "  [$TESTS_RUN] $description... "

    result=$(run_sql "$query" 2>&1)

    if [ -n "$result" ] && [ "$result" != "ERROR" ]; then
        echo -e "${GREEN}✓ PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}"
        echo -e "    ${YELLOW}Expected non-empty result${NC}"
        echo -e "    ${YELLOW}Got:${NC} $result"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Helper function to count rows
count_rows() {
    local table="$1"
    run_sql "SELECT COUNT(*) FROM $table;"
}

echo -e "${BLUE}📋 Test 1: Database Schema Verification${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

assert_sql "documents table exists" \
    "SELECT name FROM sqlite_master WHERE type='table' AND name='documents';" \
    "documents"

assert_sql "document_chunks table exists" \
    "SELECT name FROM sqlite_master WHERE type='table' AND name='document_chunks';" \
    "document_chunks"

assert_sql "document_embeddings table exists" \
    "SELECT name FROM sqlite_master WHERE type='table' AND name='document_embeddings';" \
    "document_embeddings"

assert_sql "document_tags table exists" \
    "SELECT name FROM sqlite_master WHERE type='table' AND name='document_tags';" \
    "document_tags"

assert_sql "documents_with_tags view exists" \
    "SELECT name FROM sqlite_master WHERE type='view' AND name='documents_with_tags';" \
    "documents_with_tags"

assert_sql "indexed_documents view exists" \
    "SELECT name FROM sqlite_master WHERE type='view' AND name='indexed_documents';" \
    "indexed_documents"

assert_sql "unindexed_documents view exists" \
    "SELECT name FROM sqlite_master WHERE type='view' AND name='unindexed_documents';" \
    "unindexed_documents"

echo ""
echo -e "${BLUE}📋 Test 2: Document CRUD Operations${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Clean up any existing test data first
run_sql "DELETE FROM document_embeddings WHERE chunk_id IN (SELECT id FROM document_chunks WHERE document_id='test-doc-1');" 2>/dev/null || true
run_sql "DELETE FROM document_chunks WHERE document_id='test-doc-1';" 2>/dev/null || true
run_sql "DELETE FROM document_tags WHERE document_id='test-doc-1';" 2>/dev/null || true
run_sql "DELETE FROM documents WHERE id='test-doc-1';" 2>/dev/null || true

# Insert a test document
run_sql "INSERT INTO documents (id, title, content, content_type, source_type, doc_type, word_count, char_count, created_at, updated_at)
VALUES ('test-doc-1', 'Test Document', 'This is a test document content.', 'text', 'manual', 'note', 6, 33, strftime('%s', 'now'), strftime('%s', 'now'));" 2>/dev/null || true

assert_sql "Can insert document" \
    "SELECT title FROM documents WHERE id='test-doc-1';" \
    "Test Document"

assert_sql "Document has correct content_type" \
    "SELECT content_type FROM documents WHERE id='test-doc-1';" \
    "text"

assert_sql "Document has correct word_count" \
    "SELECT word_count FROM documents WHERE id='test-doc-1';" \
    "6"

# Update document
run_sql "UPDATE documents SET title='Updated Test Document' WHERE id='test-doc-1';" 2>/dev/null || true

assert_sql "Can update document" \
    "SELECT title FROM documents WHERE id='test-doc-1';" \
    "Updated Test Document"

echo ""
echo -e "${BLUE}📋 Test 3: Document Chunks${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Insert test chunks
run_sql "INSERT INTO document_chunks (id, document_id, chunk_index, content, char_start, char_end, word_count, created_at)
VALUES ('chunk-1', 'test-doc-1', 0, 'First chunk content', 0, 19, 3, strftime('%s', 'now'));" 2>/dev/null || true

run_sql "INSERT INTO document_chunks (id, document_id, chunk_index, content, char_start, char_end, word_count, created_at)
VALUES ('chunk-2', 'test-doc-1', 1, 'Second chunk content', 20, 40, 3, strftime('%s', 'now'));" 2>/dev/null || true

assert_sql "Can insert chunk" \
    "SELECT content FROM document_chunks WHERE id='chunk-1';" \
    "First chunk content"

assert_sql "Chunk has correct index" \
    "SELECT chunk_index FROM document_chunks WHERE id='chunk-1';" \
    "0"

assert_sql "Can retrieve chunks for document" \
    "SELECT COUNT(*) FROM document_chunks WHERE document_id='test-doc-1';" \
    "2"

assert_sql "Chunks are ordered by index" \
    "SELECT content FROM document_chunks WHERE document_id='test-doc-1' ORDER BY chunk_index LIMIT 1;" \
    "First chunk content"

echo ""
echo -e "${BLUE}📋 Test 4: Document Tags${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Insert test tags
run_sql "INSERT INTO document_tags (document_id, tag, created_at)
VALUES ('test-doc-1', 'testing', strftime('%s', 'now'));" 2>/dev/null || true

run_sql "INSERT INTO document_tags (document_id, tag, created_at)
VALUES ('test-doc-1', 'phase1', strftime('%s', 'now'));" 2>/dev/null || true

assert_sql "Can insert tag" \
    "SELECT tag FROM document_tags WHERE document_id='test-doc-1' AND tag='testing';" \
    "testing"

assert_sql "Can retrieve multiple tags" \
    "SELECT COUNT(*) FROM document_tags WHERE document_id='test-doc-1';" \
    "2"

assert_non_empty "documents_with_tags view works" \
    "SELECT tags FROM documents_with_tags WHERE id='test-doc-1';"

echo ""
echo -e "${BLUE}📋 Test 5: Document Embeddings${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Insert test embedding
run_sql "INSERT INTO document_embeddings (id, chunk_id, embedding, model, dimension, created_at)
VALUES ('embed-1', 'chunk-1', x'000000000000f03f', 'test-model', 384, strftime('%s', 'now'));" 2>/dev/null || true

assert_sql "Can insert embedding" \
    "SELECT model FROM document_embeddings WHERE id='embed-1';" \
    "test-model"

assert_sql "Embedding has correct dimension" \
    "SELECT dimension FROM document_embeddings WHERE id='embed-1';" \
    "384"

assert_sql "Can retrieve embedding by chunk" \
    "SELECT COUNT(*) FROM document_embeddings WHERE chunk_id='chunk-1';" \
    "1"

echo ""
echo -e "${BLUE}📋 Test 6: Views and Aggregations${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Mark document as indexed
run_sql "UPDATE documents SET indexed_at=strftime('%s', 'now') WHERE id='test-doc-1';" 2>/dev/null || true

assert_non_empty "indexed_documents view shows indexed doc" \
    "SELECT title FROM indexed_documents WHERE id='test-doc-1';"

assert_sql "unindexed_documents view excludes indexed doc" \
    "SELECT COUNT(*) FROM unindexed_documents WHERE id='test-doc-1';" \
    "0"

# Insert an unindexed document
run_sql "INSERT INTO documents (id, title, content, content_type, source_type, doc_type, word_count, char_count, created_at, updated_at)
VALUES ('test-doc-2', 'Unindexed Doc', 'Not indexed yet', 'text', 'manual', 'note', 3, 15, strftime('%s', 'now'), strftime('%s', 'now'));" 2>/dev/null || true

assert_sql "unindexed_documents view shows unindexed doc" \
    "SELECT title FROM unindexed_documents WHERE id='test-doc-2';" \
    "Unindexed Doc"

echo ""
echo -e "${BLUE}📋 Test 7: Indexes and Constraints${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

assert_non_empty "idx_documents_doc_type exists" \
    "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_documents_doc_type';"

assert_non_empty "idx_documents_repo_id exists" \
    "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_documents_repo_id';"

assert_non_empty "idx_document_chunks_document_id exists" \
    "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_document_chunks_document_id';"

assert_non_empty "idx_document_embeddings_chunk_id exists" \
    "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_document_embeddings_chunk_id';"

assert_non_empty "idx_document_tags_document_id exists" \
    "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_document_tags_document_id';"

echo ""
echo -e "${BLUE}📋 Test 8: Cleanup${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Delete test data
run_sql "DELETE FROM document_embeddings WHERE chunk_id IN (SELECT id FROM document_chunks WHERE document_id LIKE 'test-doc-%');" 2>/dev/null || true
run_sql "DELETE FROM document_chunks WHERE document_id LIKE 'test-doc-%';" 2>/dev/null || true
run_sql "DELETE FROM document_tags WHERE document_id LIKE 'test-doc-%';" 2>/dev/null || true
run_sql "DELETE FROM documents WHERE id LIKE 'test-doc-%';" 2>/dev/null || true

assert_sql "Test documents cleaned up" \
    "SELECT COUNT(*) FROM documents WHERE id LIKE 'test-doc-%';" \
    "0"

assert_sql "Test chunks cleaned up" \
    "SELECT COUNT(*) FROM document_chunks WHERE document_id LIKE 'test-doc-%';" \
    "0"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${BLUE}📊 Test Summary${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  Total tests run:    $TESTS_RUN"
echo -e "  ${GREEN}Tests passed:      $TESTS_PASSED${NC}"
if [ $TESTS_FAILED -gt 0 ]; then
    echo -e "  ${RED}Tests failed:      $TESTS_FAILED${NC}"
else
    echo -e "  ${GREEN}Tests failed:      $TESTS_FAILED${NC}"
fi
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}✅ All tests passed! Phase 1 implementation is working correctly.${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Proceed to Phase 2 - Chunking implementation"
    echo "  2. Test document chunking logic"
    echo "  3. Implement Phase 3 - Embedding generation"
    echo ""
    exit 0
else
    echo -e "${RED}❌ Some tests failed. Please review the implementation.${NC}"
    echo ""
    exit 1
fi

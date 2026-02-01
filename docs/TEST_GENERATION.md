# Test Generation - RustAssistant

**AI-powered test generation for comprehensive code coverage**

---

## 🎯 Overview

The Test Generation feature provides automated, AI-powered test creation for:
- Unit tests for functions and modules
- Test coverage gap analysis
- Test fixture generation
- Property-based test suggestions
- Integration test recommendations

---

## ⚡ Quick Start

### Generate Tests for a File

```bash
# Generate tests for entire file
rustassistant test generate src/utils.rs

# Save to test file
rustassistant test generate src/utils.rs --output tests/utils_test.rs
```

### Generate Tests for Specific Function

```bash
# Target specific function
rustassistant test generate src/api.rs --function calculate_score

# Save as markdown documentation
rustassistant test generate src/api.rs --function calculate_score --markdown --output test-plan.md
```

### Analyze Test Gaps

```bash
# Find untested functions
rustassistant test gaps src/

# Analyze specific file
rustassistant test gaps src/auth.rs --output coverage-gaps.md
```

### Generate Test Fixtures

```bash
# Generate fixtures for data structures
rustassistant test fixtures src/models.rs --output fixtures.rs
```

---

## 📋 Commands Reference

### `test generate`

Generate comprehensive tests for code.

```bash
rustassistant test generate <FILE> [OPTIONS]
```

**Options:**
- `-f, --function <NAME>` - Target specific function
- `-o, --output <FILE>` - Save tests to file
- `--markdown` - Format as markdown documentation

**Examples:**

```bash
# Generate tests for entire file
rustassistant test generate src/calculator.rs

# Specific function
rustassistant test generate src/auth.rs --function validate_token

# Save to test file
rustassistant test generate src/utils.rs --output tests/utils_test.rs

# Documentation format
rustassistant test generate src/api.rs --markdown --output test-docs.md
```

---

### `test gaps`

Analyze test coverage gaps.

```bash
rustassistant test gaps <PATH> [OPTIONS]
```

**Options:**
- `-o, --output <FILE>` - Save gap analysis to file

**Examples:**

```bash
# Analyze directory
rustassistant test gaps src/

# Single file
rustassistant test gaps src/database.rs --output db-gaps.md

# Entire project
rustassistant test gaps . --output coverage-report.md
```

---

### `test fixtures`

Generate test fixtures and mock data.

```bash
rustassistant test fixtures <FILE> [OPTIONS]
```

**Options:**
- `-o, --output <FILE>` - Save fixtures to file

**Examples:**

```bash
# Generate fixtures
rustassistant test fixtures src/models.rs

# Save to fixtures file
rustassistant test fixtures src/types.rs --output tests/fixtures.rs
```

---

## 📊 Output Formats

### Generated Test Code

Default format - compilable Rust code:

```rust
// Tests generated for: src/utils.rs
// Framework: Rust Test
// Estimated coverage improvement: 45.0%

#[cfg(test)]
mod tests {
    use super::*;

    // Validates addition of positive numbers
    // Type: Unit
    #[test]
    fn test_add_positive_numbers() {
        let result = add(2, 3);
        assert_eq!(result, 5);
    }

    // Tests addition with zero
    // Type: EdgeCase
    #[test]
    fn test_add_with_zero() {
        assert_eq!(add(0, 5), 5);
        assert_eq!(add(5, 0), 5);
        assert_eq!(add(0, 0), 0);
    }

    // Tests addition overflow behavior
    // Type: EdgeCase
    #[test]
    fn test_add_overflow() {
        let result = add(i32::MAX, 1);
        // Should handle overflow appropriately
        assert!(result.is_err() || result == i32::MIN);
    }

    // Tests error handling for invalid inputs
    // Type: ErrorHandling
    #[test]
    fn test_add_invalid_inputs() {
        // Test with invalid combinations
        let result = add(i32::MIN, -1);
        assert!(result.is_err());
    }
}
```

### Markdown Documentation

With `--markdown` flag:

```markdown
# Generated Tests for src/utils.rs

**Framework:** Rust Test
**Coverage Improvement:** 45.0%

---

## Test Cases (4)

### 1. test_add_positive_numbers

**Description:** Validates addition of positive numbers

**Type:** Unit

**Assertions:**
- Result equals expected sum

\```rust
#[test]
fn test_add_positive_numbers() {
    let result = add(2, 3);
    assert_eq!(result, 5);
}
\```

### 2. test_add_with_zero

**Description:** Tests addition with zero

**Type:** EdgeCase

\```rust
#[test]
fn test_add_with_zero() {
    assert_eq!(add(0, 5), 5);
    assert_eq!(add(5, 0), 5);
}
\```
```

### Gap Analysis Report

```markdown
# Test Gap Analysis: src/auth.rs

## Coverage Summary

- **Total Functions:** 12
- **Tested Functions:** 7
- **Untested Functions:** 5
- **Estimated Coverage:** 58.3%

## Untested Functions

### pub validate_token()

**Signature:** `fn validate_token(token: &str) -> Result<Claims, AuthError>`
**Complexity:** 7/10

**Recommended Tests:**
- Test with valid token
- Test with expired token
- Test with malformed token
- Test with missing signature
- Test with invalid algorithm

### pub hash_password()

**Signature:** `fn hash_password(password: &str) -> Result<String, HashError>`
**Complexity:** 5/10

**Recommended Tests:**
- Test with valid password
- Test with empty password
- Test with very long password
- Test deterministic hashing

## Missing Test Types

- Integration
- Property
- Performance

## Recommendations

1. Add integration tests for authentication flow
2. Add property-based tests for password hashing
3. Add edge case tests for token validation
4. Consider adding performance tests for hash operations
```

---

## 🔄 Workflows

### Test-Driven Development (TDD)

```bash
# 1. Write your function signature
# src/calculator.rs
pub fn multiply(a: i32, b: i32) -> i32 {
    todo!()
}

# 2. Generate tests
rustassistant test generate src/calculator.rs --function multiply --output tests/calculator_test.rs

# 3. Run tests (they fail)
cargo test

# 4. Implement function
# ... write code ...

# 5. Tests pass!
cargo test
```

### Coverage Improvement Workflow

```bash
# 1. Analyze current coverage gaps
rustassistant test gaps src/ --output gaps-before.md

# 2. Review untested functions
cat gaps-before.md

# 3. Generate tests for critical functions
rustassistant test generate src/auth.rs --output tests/auth_test.rs
rustassistant test generate src/database.rs --output tests/database_test.rs

# 4. Add generated tests to project
# ... copy tests ...

# 5. Re-analyze gaps
rustassistant test gaps src/ --output gaps-after.md

# 6. Compare improvement
diff gaps-before.md gaps-after.md
```

### CI/CD Integration

```yaml
# .github/workflows/test-coverage.yml
name: Test Coverage

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install RustAssistant
        run: cargo install --git https://github.com/jordanistan/rustassistant
      
      - name: Analyze Test Gaps
        env:
          XAI_API_KEY: ${{ secrets.XAI_API_KEY }}
        run: |
          rustassistant test gaps src/ --output coverage-gaps.md
      
      - name: Check Coverage Threshold
        run: |
          COVERAGE=$(grep "Estimated Coverage:" coverage-gaps.md | awk '{print $3}' | cut -d'%' -f1)
          if (( $(echo "$COVERAGE < 70" | bc -l) )); then
            echo "Coverage $COVERAGE% is below 70% threshold"
            exit 1
          fi
      
      - name: Upload Gap Report
        uses: actions/upload-artifact@v3
        with:
          name: coverage-gaps
          path: coverage-gaps.md
```

### Pre-Commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit

# Get staged Rust files
STAGED_RS_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep "\.rs$")

if [ -z "$STAGED_RS_FILES" ]; then
  exit 0
fi

echo "🧪 Analyzing test coverage for staged files..."

for file in $STAGED_RS_FILES; do
  # Skip test files themselves
  if [[ $file == tests/* ]]; then
    continue
  fi
  
  # Analyze gaps
  rustassistant test gaps "$file" > /tmp/gaps-$$.txt
  
  # Check for untested functions
  UNTESTED=$(grep "Untested Functions:" /tmp/gaps-$$.txt | awk '{print $3}')
  
  if [ "$UNTESTED" -gt 0 ]; then
    echo "⚠️  $file has $UNTESTED untested functions"
    echo "   Run: rustassistant test generate $file"
  fi
done

echo "✅ Test coverage check complete"
```

---

## 🎓 Understanding Test Types

### Unit Tests
**Purpose:** Test individual functions in isolation

**Generated for:**
- Pure functions
- Simple logic
- Calculations
- Transformations

**Example:**
```rust
#[test]
fn test_calculate_total() {
    let items = vec![10, 20, 30];
    assert_eq!(calculate_total(&items), 60);
}
```

### Integration Tests
**Purpose:** Test components working together

**Generated for:**
- API endpoints
- Database operations
- External service calls
- Multi-step workflows

**Example:**
```rust
#[tokio::test]
async fn test_create_user_flow() {
    let db = setup_test_db().await;
    let user = create_user(&db, "test@example.com").await.unwrap();
    assert_eq!(user.email, "test@example.com");
}
```

### Edge Case Tests
**Purpose:** Test boundary conditions

**Generated for:**
- Empty inputs
- Null/None values
- Maximum/minimum values
- Overflow scenarios

**Example:**
```rust
#[test]
fn test_divide_edge_cases() {
    assert_eq!(divide(0, 5), 0);
    assert!(divide(5, 0).is_err()); // Division by zero
    assert_eq!(divide(i32::MAX, 1), i32::MAX);
}
```

### Error Handling Tests
**Purpose:** Test error paths

**Generated for:**
- Invalid inputs
- Network failures
- Permission errors
- Validation failures

**Example:**
```rust
#[test]
fn test_parse_invalid_json() {
    let result = parse_json("invalid");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::ParseError);
}
```

### Property-Based Tests
**Purpose:** Test invariants with random inputs

**Generated for:**
- Serialization/deserialization
- Reversible operations
- Idempotent functions
- Commutative operations

**Example:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_encode_decode_roundtrip(s: String) {
        let encoded = encode(&s);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(s, decoded);
    }
}
```

### Performance Tests
**Purpose:** Benchmark performance

**Generated for:**
- Critical path functions
- Algorithm implementations
- Data structure operations
- Resource-intensive code

**Example:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_sort(c: &mut Criterion) {
    c.bench_function("sort 1000", |b| {
        b.iter(|| {
            let mut vec = generate_test_data(1000);
            sort_algorithm(black_box(&mut vec))
        });
    });
}
```

---

## 💡 Best Practices

### 1. Review Generated Tests

Always review and customize generated tests:

```bash
# Generate tests
rustassistant test generate src/utils.rs --output tests/utils_test.rs

# Review the output
cat tests/utils_test.rs

# Customize as needed
# - Adjust assertions
# - Add more edge cases
# - Fix any AI mistakes
```

### 2. Use as Starting Point

Treat generated tests as a foundation:

```rust
// Generated test (starting point)
#[test]
fn test_user_creation() {
    let user = User::new("Alice");
    assert_eq!(user.name, "Alice");
}

// Enhanced version (after review)
#[test]
fn test_user_creation() {
    let user = User::new("Alice");
    assert_eq!(user.name, "Alice");
    assert!(user.id > 0); // Added
    assert_eq!(user.created_at.date(), Utc::today()); // Added
    assert_eq!(user.status, UserStatus::Active); // Added
}
```

### 3. Combine with Manual Testing

```bash
# Generate baseline tests
rustassistant test generate src/api.rs --output tests/api_generated.rs

# Write custom integration tests manually
# tests/api_integration.rs

# Use both together for comprehensive coverage
```

### 4. Iterate on Coverage

```bash
# First pass
rustassistant test gaps src/

# Generate tests for critical functions
rustassistant test generate src/critical.rs

# Second pass - see improvement
rustassistant test gaps src/

# Repeat until satisfied
```

### 5. Focus on Critical Paths

```bash
# Analyze complexity
rustassistant test gaps src/ --output gaps.md

# Priority: Functions with complexity > 7
grep "Complexity: [7-9]/10" gaps.md

# Generate tests for high-complexity functions first
```

---

## 💰 Cost Optimization

### Caching Benefits

Test generation uses response caching:

```bash
# First generation (API call)
rustassistant test generate src/utils.rs
# Cost: ~$0.02

# Re-generate after small changes (cached)
rustassistant test generate src/utils.rs
# Cost: $0.00 (instant!)

# Generate for similar file (partial cache)
rustassistant test generate src/helpers.rs
# Cost: ~$0.01 (some patterns cached)
```

### Cost Estimates

| Operation | First Run | Cached | Typical |
|-----------|-----------|--------|---------|
| Generate for small file (<200 lines) | $0.015 | $0.00 | $0.003 |
| Generate for medium file (200-500 lines) | $0.030 | $0.00 | $0.006 |
| Generate for large file (500+ lines) | $0.050 | $0.00 | $0.010 |
| Gap analysis (10 files) | $0.080 | $0.00 | $0.016 |
| Fixtures generation | $0.010 | $0.00 | $0.002 |

**Monthly estimate (typical usage):**
- 20 test generations × $0.003 = $0.06
- 5 gap analyses × $0.016 = $0.08
- Total: **~$0.14/month**

### Cost-Saving Tips

1. **Batch operations:**
```bash
# Generate all at once
for file in src/*.rs; do
  rustassistant test generate "$file" --output "tests/$(basename $file)"
done
```

2. **Use markdown for planning:**
```bash
# Free to review and plan
rustassistant test generate src/api.rs --markdown --output plan.md
# Review plan.md, decide what to actually implement
```

3. **Focus on gaps:**
```bash
# Only generate for untested functions
rustassistant test gaps src/ | grep "Complexity: [7-9]" | \
  xargs -I {} rustassistant test generate {}
```

---

## 🐛 Troubleshooting

### "No test cases generated"

**Cause:** File too simple or already well-tested

**Fix:**
```bash
# Check file complexity
wc -l src/file.rs

# Try specific function
rustassistant test generate src/file.rs --function complex_function

# Or generate fixtures instead
rustassistant test fixtures src/file.rs
```

### "JSON parse error"

**Cause:** AI response wasn't valid JSON

**Fix:**
- Try again (AI responses vary)
- Use markdown format instead
- Simplify the file (split into smaller files)

### "Coverage estimate is 0%"

**Cause:** File already has comprehensive tests

**Fix:**
```bash
# This is good! Run gap analysis to confirm
rustassistant test gaps src/file.rs
```

### Generated tests don't compile

**Cause:** AI made assumptions about types/traits

**Fix:**
1. Review generated code
2. Adjust imports and types
3. Fix any AI mistakes
4. Consider this a starting point, not final code

---

## 📈 Metrics & Analytics

### Track Coverage Improvement

```bash
# Create tracking script
cat > scripts/track-coverage.sh << 'EOF'
#!/bin/bash
DATE=$(date +%Y-%m-%d)
rustassistant test gaps src/ --output /tmp/gaps-$$.txt

COVERAGE=$(grep "Estimated Coverage:" /tmp/gaps-$$.txt | head -1 | awk '{print $3}' | cut -d'%' -f1)
UNTESTED=$(grep "Untested Functions:" /tmp/gaps-$$.txt | head -1 | awk '{print $3}')

echo "$DATE,$COVERAGE,$UNTESTED" >> metrics/coverage-history.csv
echo "Coverage: $COVERAGE% | Untested: $UNTESTED"
EOF

chmod +x scripts/track-coverage.sh
```

### Generate Coverage Report

```bash
# Weekly coverage report
cat metrics/coverage-history.csv | tail -7 | \
  awk -F',' '{sum+=$2; count++} END {print "Avg Coverage:", sum/count"%"}'
```

---

## 🎯 Examples

### Example 1: Basic Function Testing

```bash
# Source code
cat src/math.rs
# pub fn factorial(n: u32) -> u32 { ... }

# Generate tests
rustassistant test generate src/math.rs --function factorial

# Output:
#[test]
fn test_factorial_base_cases() {
    assert_eq!(factorial(0), 1);
    assert_eq!(factorial(1), 1);
}

#[test]
fn test_factorial_small_numbers() {
    assert_eq!(factorial(5), 120);
    assert_eq!(factorial(6), 720);
}

#[test]
fn test_factorial_edge_cases() {
    // Test maximum safe value
    assert_eq!(factorial(12), 479001600);
}
```

### Example 2: API Handler Testing

```bash
# Generate tests for API handler
rustassistant test generate src/api/users.rs --output tests/api_users_test.rs

# Includes:
# - Success case tests
# - Error handling tests
# - Validation tests
# - Integration test suggestions
```

### Example 3: Full Module Coverage

```bash
# 1. Check current coverage
rustassistant test gaps src/database/ --output db-gaps.md

# Output shows 40% coverage, 15 untested functions

# 2. Generate tests for each untested function
cat db-gaps.md | grep "### " | while read line; do
  FUNC=$(echo $line | cut -d' ' -f2)
  rustassistant test generate src/database/mod.rs --function $FUNC >> tests/db_test.rs
done

# 3. Verify improvement
rustassistant test gaps src/database/ --output db-gaps-after.md

# Coverage improved to 85%!
```

---

## 🚀 Advanced Usage

### Custom Test Strategies

```bash
# Generate property-based tests
rustassistant test generate src/parser.rs --function parse --markdown | \
  grep -A 20 "Property"

# Generate benchmarks
rustassistant test generate src/algorithm.rs --function sort --markdown | \
  grep -A 20 "Performance"
```

### Integration with Coverage Tools

```bash
# Generate tests
rustassistant test generate src/lib.rs --output tests/generated.rs

# Run with coverage
cargo tarpaulin --out Html

# Review coverage report
open tarpaulin-report.html
```

### Automated Test Maintenance

```bash
#!/bin/bash
# scripts/maintain-tests.sh

# Find files with low coverage
rustassistant test gaps src/ | grep "Coverage: [0-6]" > low-coverage.txt

# Generate tests for low-coverage files
while read line; do
  FILE=$(echo $line | awk '{print $1}')
  rustassistant test generate "$FILE" --output "tests/$(basename $FILE)"
done < low-coverage.txt

# Commit generated tests
git add tests/
git commit -m "chore: add generated tests for low-coverage files"
```

---

## 📚 Related Features

- **[Code Review](CODE_REVIEW.md)** - Review tests along with code
- **[Batch Analysis](BATCH_OPERATIONS.md)** - Analyze multiple test files
- **[CLI Reference](CLI_CHEATSHEET.md)** - All available commands

---

## 🎉 Summary

Test Generation helps you:
- ✅ Quickly bootstrap test suites
- ✅ Identify coverage gaps
- ✅ Generate edge case tests
- ✅ Create test fixtures
- ✅ Improve code quality
- ✅ Save time on boilerplate

**Start using it today:**

```bash
# Find what needs testing
rustassistant test gaps src/

# Generate tests
rustassistant test generate src/your-file.rs --output tests/your-file_test.rs

# Review and customize
cat tests/your-file_test.rs
```

---

**Last Updated:** February 1, 2026  
**Status:** Production Ready  
**Cost:** ~$0.003 per file (cached runs free)  
**Coverage Improvement:** Typically 30-50% per iteration
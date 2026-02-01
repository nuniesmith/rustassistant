# Advanced Features Guide

**Status:** ✅ ALL 4 FEATURES COMPLETE AND READY TO USE  
**Date:** 2024  
**Version:** 1.0

---

## 🎉 Overview

Rustassistant now includes **four production-ready advanced features** to supercharge your development workflow:

1. **✅ Code Review Automation** - Automated PR reviews with actionable feedback
2. **✅ Test Generation** - Generate unit tests and identify coverage gaps
3. **✅ Refactoring Assistant** - Detect code smells and suggest improvements
4. **✅ Query Templates** - Pre-built query patterns for common tasks

All features leverage Grok AI with intelligent caching for cost efficiency.

---

## 📋 Quick Reference

```bash
# Code Review
rustassistant review diff                  # Review git changes
rustassistant review files src/*.rs        # Review specific files
rustassistant review pr --base main        # Generate PR description

# Test Generation
rustassistant test generate src/main.rs    # Generate tests for file
rustassistant test gaps src/               # Find coverage gaps
rustassistant test fixtures src/models.rs  # Create test fixtures

# Refactoring
rustassistant refactor analyze src/        # Analyze for code smells
rustassistant refactor suggest src/lib.rs  # Get refactoring suggestions
rustassistant refactor plan src/ --goal "improve maintainability"

# Templates
rustassistant template list                # Show available templates
rustassistant template use architecture    # Use architecture template
```

---

## 🔍 Feature 1: Code Review Automation

### What It Does

Automatically reviews code changes and provides:
- Quality and security scores (0-100)
- Issue detection (critical, high, medium, low)
- Actionable improvement suggestions
- PR-ready descriptions and summaries

### Commands

#### Review Git Diff

Review uncommitted changes or changes from a branch:

```bash
# Review current changes
rustassistant review diff

# Compare against specific branch
rustassistant review diff --base main

# Save to file
rustassistant review diff --output review-report.md

# Format for GitHub PR
rustassistant review diff --github --output pr-review.md
```

#### Review Specific Files

Review selected files without git:

```bash
# Review single file
rustassistant review files src/main.rs

# Review multiple files
rustassistant review files src/*.rs

# Review with GitHub formatting
rustassistant review files src/api/*.rs --github --output api-review.md
```

#### Generate PR Description

Create comprehensive PR descriptions automatically:

```bash
# Generate PR description from changes
rustassistant review pr --base main

# Save to file
rustassistant review pr --base main --output PR.md

# Specify repository path
rustassistant review pr --path /path/to/repo --base develop
```

### Output Example

```markdown
# Code Review Report

**Generated:** 2024-01-15 10:30:00

## Summary Statistics

- **Files Reviewed:** 5
- **Total Issues:** 12
- **Average Quality Score:** 87.3/100
- **Average Security Score:** 92.1/100
- **Lines Changed:** 342

## Issue Breakdown

- 🔴 Critical: 1
- 🟠 High: 3
- 🟡 Medium: 5
- 🟢 Low: 3

## Files Analyzed

### src/api/auth.rs

**Quality:** 82/100 | **Security:** 88/100

**Issues:**
- **High:** Missing error handling for database operations (line 45)
- **Medium:** Consider using constant for timeout value (line 67)
- **Low:** Function could be split for better readability (line 89-145)

**Suggestions:**
1. Add proper error handling with custom error types
2. Extract database query logic to separate function
3. Add input validation for user credentials
```

### Use Cases

**Daily Development:**
```bash
# Before committing
rustassistant review diff --output review.md
# Review feedback, make fixes
git add .
git commit -m "feat: add authentication"
```

**Pull Request Creation:**
```bash
# Generate PR description
rustassistant review pr --base main --output PR_DESCRIPTION.md
# Copy content to GitHub PR
```

**Code Quality Checks:**
```bash
# Review specific module
rustassistant review files src/api/*.rs --output api-quality-report.md
```

### Configuration

Review settings use standard Grok API configuration:
- Automatically caches results (saves cost on re-reviews)
- Tracks costs in database
- Uses retry logic for reliability

---

## 🧪 Feature 2: Test Generation

### What It Does

AI-powered test generation that:
- Creates unit tests from existing code
- Identifies test coverage gaps
- Generates test fixtures and sample data
- Supports multiple test types (unit, integration, edge cases)

### Commands

#### Generate Tests for File

Generate comprehensive tests for a file or function:

```bash
# Generate tests for entire file
rustassistant test generate src/utils.rs

# Generate tests for specific function
rustassistant test generate src/api.rs --function validate_user

# Save to test file
rustassistant test generate src/models.rs --output tests/models_test.rs

# Output as markdown documentation
rustassistant test generate src/lib.rs --markdown --output test-plan.md
```

#### Analyze Test Gaps

Find untested code and coverage gaps:

```bash
# Analyze single file
rustassistant test gaps src/main.rs

# Analyze entire directory
rustassistant test gaps src/

# Save gap analysis
rustassistant test gaps src/ --output coverage-gaps.md
```

#### Generate Test Fixtures

Create test data and fixtures:

```bash
# Generate fixtures from models
rustassistant test fixtures src/models.rs

# Save to file
rustassistant test fixtures src/types.rs --output tests/fixtures.rs
```

### Output Example

#### Generated Tests

```rust
// Generated tests for: src/utils.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_score_valid_input() {
        // Test with valid positive numbers
        let result = calculate_score(100, 80);
        assert_eq!(result, 180);
    }

    #[test]
    fn test_calculate_score_zero_values() {
        // Edge case: zero values
        let result = calculate_score(0, 0);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_calculate_score_negative_values() {
        // Edge case: negative values
        let result = calculate_score(-50, 30);
        assert_eq!(result, -20);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn test_calculate_score_overflow() {
        // Error handling: integer overflow
        calculate_score(i32::MAX, 1);
    }
}
```

#### Gap Analysis Report

```markdown
# Test Coverage Gap Analysis

**Files Analyzed:** 8
**Average Coverage:** 67.5%
**Untested Functions:** 23

---

## src/api/users.rs

**Coverage:** 45.2%
**Total Functions:** 12
**Tested Functions:** 5
**Untested Functions:** 7

### Untested Functions

1. **`create_user`** (public, medium complexity)
   - Recommended: Unit test, error handling test
   - Priority: High

2. **`update_user_email`** (public, low complexity)
   - Recommended: Unit test, validation test
   - Priority: Medium

3. **`delete_user_cascade`** (public, high complexity)
   - Recommended: Integration test, edge case test
   - Priority: Critical

### Missing Test Types
- ❌ Integration tests
- ❌ Edge case tests
- ⚠️  Limited error handling tests
```

### Use Cases

**Test-Driven Development:**
```bash
# Write function
# Generate initial test cases
rustassistant test generate src/new_feature.rs --output tests/new_feature_test.rs
# Refine and expand tests manually
```

**Improve Coverage:**
```bash
# Find gaps
rustassistant test gaps src/ --output gaps.md
# Review gaps
# Generate tests for critical untested code
rustassistant test generate src/critical.rs --output tests/critical_test.rs
```

**Legacy Code:**
```bash
# Analyze legacy module
rustassistant test gaps src/legacy/
# Generate tests for risky code
rustassistant test generate src/legacy/core.rs
```

---

## 🔧 Feature 3: Refactoring Assistant

### What It Does

Detects code quality issues and suggests improvements:
- Identifies code smells (long functions, duplicated code, etc.)
- Scores complexity and maintainability
- Provides step-by-step refactoring instructions
- Generates comprehensive refactoring plans

### Commands

#### Analyze for Refactoring

Analyze code for improvement opportunities:

```bash
# Analyze single file
rustassistant refactor analyze src/main.rs

# Analyze entire directory
rustassistant refactor analyze src/

# Save analysis report
rustassistant refactor analyze src/ --output refactor-analysis.md
```

#### Get Refactoring Suggestions

Get specific, actionable refactoring advice:

```bash
# Suggest improvements for file
rustassistant refactor suggest src/legacy.rs

# Analyze specific code section
rustassistant refactor suggest src/api.rs --start-line 42 --end-line 150

# Save suggestions
rustassistant refactor suggest src/utils.rs --output suggestions.md
```

#### Generate Refactoring Plan

Create comprehensive, step-by-step refactoring plans:

```bash
# Generate plan with goal
rustassistant refactor plan src/ --goal "improve testability"

# Specific improvement goal
rustassistant refactor plan src/legacy/ --goal "reduce coupling between modules"

# Save plan
rustassistant refactor plan src/ --goal "modernize error handling" --output refactor-plan.md
```

### Output Example

#### Analysis Report

```markdown
# Refactoring Analysis: src/api/handlers.rs

**Complexity Score:** 78.5/100 (high)
**Maintainability Score:** 62.3/100 (needs improvement)
**Estimated Effort:** Medium
**Code Smells:** 8

---

## Code Smells Detected

### 🔴 Critical: Long Function

**Location:** `handle_user_request` (lines 45-180)
**Impact:** Difficult to test, understand, and maintain

**Description:**
Function is 135 lines long, exceeds recommended 50 line limit. Contains multiple responsibilities including validation, database operations, and response formatting.

### 🟠 High: Deep Nesting

**Location:** `process_payment` (lines 220-285)
**Impact:** Reduced readability, increased cyclomatic complexity

**Description:**
Nesting depth reaches 6 levels. Consider early returns or extracting nested logic to separate functions.

### 🟡 Medium: Magic Numbers

**Location:** Multiple locations
**Impact:** Unclear intent, difficult to maintain

**Description:**
Found hardcoded values: 3600 (line 34), 100 (line 67), 5 (line 89). Replace with named constants.

---

## Refactoring Suggestions

### 1. Extract Function (Priority: High)

**Title:** Split `handle_user_request` into smaller functions

**Benefits:**
- Improved testability
- Better separation of concerns
- Enhanced readability

**Steps:**
1. Extract validation logic to `validate_user_input()`
2. Extract database operations to `save_user_data()`
3. Extract response formatting to `format_user_response()`
4. Keep main function as coordinator

**Before:**
```rust
fn handle_user_request(req: Request) -> Result<Response> {
    // 135 lines of mixed concerns
}
```

**After:**
```rust
fn handle_user_request(req: Request) -> Result<Response> {
    let validated = validate_user_input(&req)?;
    let user = save_user_data(validated)?;
    format_user_response(user)
}

fn validate_user_input(req: &Request) -> Result<ValidatedInput> { ... }
fn save_user_data(input: ValidatedInput) -> Result<User> { ... }
fn format_user_response(user: User) -> Response { ... }
```

**Effort:** Small (2-3 hours)

### 2. Replace Magic Numbers (Priority: Medium)

**Title:** Define constants for timeout and limit values

**Before:**
```rust
let timeout = 3600;
if count > 100 { ... }
```

**After:**
```rust
const SESSION_TIMEOUT_SECONDS: u64 = 3600;
const MAX_ITEMS_PER_PAGE: usize = 100;

let timeout = SESSION_TIMEOUT_SECONDS;
if count > MAX_ITEMS_PER_PAGE { ... }
```

**Effort:** Trivial (30 minutes)
```

#### Refactoring Plan

```markdown
# Refactoring Plan: Improve Testability

**Goal:** Make codebase more testable by reducing coupling and improving dependency injection

**Total Effort:** Large (2-3 weeks)
**Files Affected:** 12
**Total Steps:** 8

---

## Steps

### Step 1: Extract Database Interface

**Description:** Create database trait to enable mocking in tests

**Affected Files:**
- src/db.rs
- src/api/users.rs
- src/api/posts.rs

**Dependencies:** None

**Effort:** Medium (1-2 days)

**Validation:**
- [ ] All database operations use trait methods
- [ ] Mock implementation compiles
- [ ] Existing tests still pass

---

### Step 2: Implement Dependency Injection

**Description:** Pass database as parameter instead of global access

**Affected Files:**
- src/api/handlers.rs
- src/services/user_service.rs

**Dependencies:** [1]

**Effort:** Large (3-4 days)

**Validation:**
- [ ] No global database access
- [ ] All functions accept DB parameter
- [ ] Integration tests use real DB
- [ ] Unit tests use mock DB

---

## Benefits

1. **Testability:** Can mock all external dependencies
2. **Flexibility:** Easy to swap implementations
3. **Maintainability:** Clear dependency graph
4. **Quality:** Higher test coverage possible

## Risks

### Risk 1: Breaking Changes

**Description:** API changes require updates across codebase
**Severity:** High
**Mitigation:** Create feature branch, update incrementally, maintain comprehensive test suite

### Risk 2: Performance Impact

**Description:** Additional trait indirection might affect performance
**Severity:** Low
**Mitigation:** Benchmark critical paths before and after, use static dispatch where possible
```

### Code Smell Types Detected

The refactoring assistant detects 16+ code smell types:

- **Long Function** - Functions exceeding 50 lines
- **Long Parameter List** - Functions with >4 parameters
- **Duplicated Code** - Similar code blocks
- **Large Module** - Modules with too many items
- **Deep Nesting** - Nesting depth >4 levels
- **Complex Conditional** - Hard-to-read conditionals
- **Magic Numbers** - Hardcoded values
- **Dead Code** - Unused code
- **God Object** - Classes doing too much
- **Tight Coupling** - High interdependence
- **Missing Error Handling** - Unhandled errors
- **Unsafe Unwrapping** - Excessive `.unwrap()`
- And more...

### Use Cases

**Code Quality Audit:**
```bash
# Analyze entire codebase
rustassistant refactor analyze src/ --output quality-audit.md
# Review findings with team
# Prioritize improvements
```

**Pre-Refactor Planning:**
```bash
# Generate refactoring plan
rustassistant refactor plan src/legacy/ --goal "modernize and improve maintainability" --output plan.md
# Break down into sprints
# Execute incrementally
```

**Technical Debt Reduction:**
```bash
# Analyze high-complexity modules
rustassistant refactor analyze src/core/ --output core-analysis.md
# Generate targeted improvements
rustassistant refactor suggest src/core/complex_module.rs
```

---

## 📚 Feature 4: Query Templates

### What It Does

Pre-built, reusable query templates for common development tasks:
- Architecture analysis
- Security review
- Performance optimization
- Documentation generation
- And more...

### Commands

#### List Available Templates

```bash
# Show all templates
rustassistant template list

# Search templates
rustassistant template list --search security
```

#### View Template Details

```bash
# Show template details
rustassistant template show architecture
rustassistant template show security-review
```

#### Use Template

```bash
# Use template with variables
rustassistant template use architecture --var project=myapp --var language=rust

# Execute immediately
rustassistant template use code-quality --execute

# Combine with file context
rustassistant template use optimize --var file=src/slow.rs --execute
```

### Available Templates

Template categories include:

- **Architecture** - System design analysis
- **Security** - Vulnerability scanning
- **Performance** - Optimization suggestions
- **Documentation** - Auto-generate docs
- **Code Quality** - Quality assessment
- **Testing** - Test strategy
- **Refactoring** - Improvement plans
- **API Design** - API review

---

## 💰 Cost Management

All advanced features use the same cost-optimized infrastructure:

### Caching Strategy

- **70%+ cache hit rate** on repeated analyses
- **SHA-256 content hashing** ensures accuracy
- **TTL-based expiration** keeps cache fresh
- **Hot entry tracking** for frequently accessed items

### Cost Tracking

Every operation is tracked:

```bash
# View costs
rustassistant costs

# Output:
# 💰 LLM Cost Statistics
# 
# Total Costs:
#   All time:     $12.4567
#   Last 24h:     $0.8923
#   Last 7 days:  $4.5612
#   Last 30 days: $12.4567
```

### Cache Management

```bash
# View cache stats
rustassistant cache stats

# Clear expired entries
rustassistant cache prune

# View hot entries
rustassistant cache hot --limit 20

# Clear all cache (fresh start)
rustassistant cache clear
```

### Cost Optimization Tips

1. **Batch operations** - Review multiple files together
2. **Use cache** - Re-running same analysis is nearly free
3. **Incremental analysis** - Only analyze changed files
4. **Template reuse** - Templates maximize cache hits

**Expected Monthly Costs:**
- Light use (5-10 operations/day): **$5-10/month**
- Medium use (20-30 operations/day): **$15-25/month**
- Heavy use (50+ operations/day): **$30-50/month**

With 70%+ cache hit rate, costs stay minimal even with daily use.

---

## 🎯 Workflow Integration

### Daily Development Workflow

```bash
# Morning: Plan the day
rustassistant next

# During coding: Quick reviews
rustassistant review diff

# Before commit: Final check
rustassistant review diff --output review.md
git add .
git commit -m "feat: implement feature"

# Evening: Update notes
rustassistant note add "Completed user auth, need to add tests tomorrow" --tags work,todo
```

### PR Creation Workflow

```bash
# 1. Review changes
rustassistant review diff --base main --output review.md

# 2. Fix any issues found
# ... make fixes ...

# 3. Generate PR description
rustassistant review pr --base main --output PR.md

# 4. Create PR with generated description
gh pr create --body-file PR.md
```

### Refactoring Workflow

```bash
# 1. Analyze current state
rustassistant refactor analyze src/ --output baseline.md

# 2. Create refactoring plan
rustassistant refactor plan src/ --goal "improve testability" --output plan.md

# 3. Execute plan incrementally
# ... implement step 1 ...

# 4. Verify improvement
rustassistant refactor analyze src/ --output after-step1.md

# 5. Generate tests
rustassistant test generate src/refactored.rs --output tests/refactored_test.rs
```

### Test Coverage Improvement

```bash
# 1. Find gaps
rustassistant test gaps src/ --output gaps.md

# 2. Review and prioritize
# ... review gaps.md ...

# 3. Generate tests for critical code
rustassistant test generate src/critical.rs --output tests/critical_test.rs

# 4. Review and refine generated tests
# ... edit tests/critical_test.rs ...

# 5. Run tests
cargo test

# 6. Check coverage again
rustassistant test gaps src/ --output gaps-after.md
```

---

## 🔧 Advanced Usage

### Combine Features

```bash
# Review + Refactor + Test workflow
rustassistant review files src/api/*.rs --output api-review.md
rustassistant refactor analyze src/api/ --output api-refactor.md
rustassistant test gaps src/api/ --output api-coverage.md

# Compare all three reports for comprehensive insight
```

### Scripting & Automation

```bash
#!/bin/bash
# pre-commit-quality-check.sh

echo "🔍 Running code quality checks..."

# Review changes
rustassistant review diff --output /tmp/review.md

# Check for critical issues
if grep -q "Critical:" /tmp/review.md; then
    echo "❌ Critical issues found!"
    cat /tmp/review.md
    exit 1
fi

echo "✅ Quality checks passed!"
```

### CI/CD Integration

```yaml
# .github/workflows/code-quality.yml
name: Code Quality

on: [pull_request]

jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Review PR
        run: |
          rustassistant review diff --base main --github --output review.md
          
      - name: Post Review
        uses: actions/github-script@v6
        with:
          script: |
            const fs = require('fs');
            const review = fs.readFileSync('review.md', 'utf8');
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: review
            });
```

---

## 📖 Best Practices

### Code Review

✅ **Do:**
- Review before committing
- Address critical and high-severity issues
- Use GitHub format for PR comments
- Keep review scope focused (single feature/bug)

❌ **Don't:**
- Ignore security warnings
- Review too many files at once (slower, more expensive)
- Skip re-review after fixes

### Test Generation

✅ **Do:**
- Review and refine generated tests
- Use as starting point, not final product
- Generate fixtures for complex data structures
- Focus on untested critical paths

❌ **Don't:**
- Trust generated tests blindly
- Commit without running tests
- Generate tests for trivial code
- Skip edge cases and error handling

### Refactoring

✅ **Do:**
- Start with analysis before changes
- Create plans for large refactors
- Refactor incrementally
- Test after each step
- Track complexity scores over time

❌ **Don't:**
- Refactor without tests
- Make all changes at once
- Ignore effort estimates
- Skip validation steps

---

## 🐛 Troubleshooting

### "No API key found"

```bash
# Set API key
export GROK_API_KEY="your-key-here"

# Or in .env file
echo "GROK_API_KEY=your-key" >> .env
```

### "Database not found"

```bash
# Create database directory
mkdir -p data

# Initialize database
rustassistant note add "test" --tags setup
```

### "Command failed with error"

```bash
# Enable verbose logging
rustassistant -v refactor analyze src/

# Check API connectivity
rustassistant costs
```

### High Costs

```bash
# Check cache hit rate
rustassistant cache stats

# If low hit rate:
# - Are you analyzing same files repeatedly?
# - Is content changing between runs?
# - Is cache database accessible?

# Prune old entries
rustassistant cache prune
```

### Slow Performance

```bash
# Use batch operations
rustassistant review files src/*.rs  # Good
# vs analyzing files one by one      # Slow

# Limit scope
rustassistant refactor analyze src/api/  # Focused
# vs entire codebase                     # Slow

# Use cache
# Second run of same analysis is ~100x faster
```

---

## 🎓 Learning Path

### Week 1: Basics
- Try each feature with `--help`
- Run review on small changes
- Generate tests for simple file
- Analyze single file for refactoring

### Week 2: Integration
- Use review before every commit
- Generate tests for new features
- Create refactoring plan for legacy code
- Explore query templates

### Week 3: Workflow
- Integrate into daily routine
- Set up pre-commit hooks
- Track cost trends
- Optimize cache usage

### Week 4: Advanced
- CI/CD integration
- Team adoption
- Custom workflows
- Batch operations

---

## 📊 Success Metrics

Track your progress:

```bash
# Weekly check-in
rustassistant stats
rustassistant costs
rustassistant cache stats

# What to track:
# - Issues prevented (from reviews)
# - Test coverage increase
# - Complexity score trends
# - Cost per week
# - Time saved vs manual review
```

**Target Metrics (after 1 month):**
- ✅ Review every PR before creation
- ✅ 80%+ test coverage on new code
- ✅ Complexity scores trending down
- ✅ Monthly costs <$30
- ✅ 30+ minutes saved per day

---

## 🚀 Next Steps

You now have **all 4 advanced features** ready to use!

**Start Today:**

```bash
# 1. Review your current work
rustassistant review diff

# 2. Find test gaps
rustassistant test gaps src/

# 3. Analyze code quality
rustassistant refactor analyze src/

# 4. Check costs
rustassistant costs
```

**This Week:**
- Integrate into daily workflow
- Review 5+ PRs
- Generate tests for new features
- Create refactoring plan for 1 module

**This Month:**
- Establish quality standards
- Track metrics
- Optimize costs
- Share with team (if applicable)

---

## 📚 Additional Resources

- **Main Documentation:** `README.md`
- **CLI Reference:** `docs/CLI_CHEATSHEET.md`
- **Cost Optimization:** `COST_OPTIMIZATION_RESULTS.md`
- **Batch Operations:** `docs/BATCH_OPERATIONS.md`
- **Work Plan:** `docs/devflow_work_plan.md`

---

**Ready to build better code?** 🎯

Run your first advanced feature command now:

```bash
rustassistant review diff --output first-review.md
```

*Last Updated: 2024*  
*All 4 Features: Production Ready ✅*
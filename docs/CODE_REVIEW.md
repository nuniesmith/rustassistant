# Code Review Automation - RustAssistant

**AI-powered code review for Pull Requests and changed files**

---

## 🎯 Overview

The Code Review feature provides automated, AI-powered code analysis for:
- Git diff changes (PRs, branches)
- Specific files
- Security and quality assessment
- Structured feedback
- GitHub/GitLab compatible output

---

## ⚡ Quick Start

### Review Your Changes

```bash
# Review uncommitted changes
rustassistant review diff

# Review changes against main branch
rustassistant review diff --base main

# Save report to file
rustassistant review diff --output review.md
```

### Review Specific Files

```bash
# Review one or more files
rustassistant review files src/api/auth.rs src/api/users.rs

# Save to file
rustassistant review files src/*.rs --output api-review.md
```

### Generate PR Description

```bash
# Generate PR description from changes
rustassistant review pr --base main --output pr-description.md
```

---

## 📋 Commands Reference

### `review diff`

Review git diff changes.

```bash
rustassistant review diff [OPTIONS]
```

**Options:**
- `-p, --path <PATH>` - Repository path (default: current directory)
- `-b, --base <BRANCH>` - Base branch to compare against
- `-o, --output <FILE>` - Save report to file
- `--github` - Format output for GitHub PR comment

**Examples:**

```bash
# Review uncommitted changes
rustassistant review diff

# Compare against main branch
rustassistant review diff --base main

# Review in specific repo
rustassistant review diff --path ~/projects/myapp --base develop

# Generate GitHub PR comment
rustassistant review diff --base main --github --output pr-comment.md
```

---

### `review files`

Review specific files.

```bash
rustassistant review files <FILES>... [OPTIONS]
```

**Options:**
- `-o, --output <FILE>` - Save report to file
- `--github` - Format output for GitHub PR comment

**Examples:**

```bash
# Review single file
rustassistant review files src/main.rs

# Review multiple files
rustassistant review files src/api/*.rs

# With output
rustassistant review files src/*.rs --output code-quality.md
```

---

### `review pr`

Generate PR description from changes.

```bash
rustassistant review pr [OPTIONS]
```

**Options:**
- `-p, --path <PATH>` - Repository path (default: current directory)
- `-b, --base <BRANCH>` - Base branch (default: main)
- `-o, --output <FILE>` - Save description to file

**Examples:**

```bash
# Generate PR description
rustassistant review pr --base main

# For feature branch
rustassistant review pr --base develop --output pr.md

# Different repo
rustassistant review pr --path ~/projects/api --base main
```

---

## 📊 Output Formats

### Standard Markdown Report

Default format with comprehensive details:

```markdown
# Code Review Report

**Generated:** 2026-02-01 12:34:56 UTC

---

## Summary

✅ **No critical issues** - Some improvements recommended.

**Quality Score:** 82.5/100 (Good)
**Security Score:** 88.3/100 (Good)

## Statistics

- **Files Reviewed:** 3
- **Files with Issues:** 2
- **Total Issues:** 5
- **Lines Changed:** 247

### Issues by Severity

- 🟠 **High:** 1
- 🟡 **Medium:** 3
- 🔵 **Low:** 1

## File Reviews

### src/api/auth.rs

- **Quality Score:** 78.2/100
- **Security Score:** 85.0/100
- **Lines Changed:** 134

**Issues Found:**

- 🟠 **High:** Consider adding rate limiting for login attempts
- 🟡 **Medium:** Error messages could expose implementation details

**Suggestions:**

- Add input validation for email format
- Consider using constant-time comparison for passwords
```

### GitHub PR Comment Format

Optimized for GitHub/GitLab:

```markdown
## ✅ Code Review - Looks Good

✅ **No critical issues** - Some improvements recommended.

**Quality Score:** 82.5/100 (Good)
**Security Score:** 88.3/100 (Good)

📊 **3 files** | 5 issues | 82.5% quality | 88.3% security

### 🔍 Files Requiring Attention

**src/api/auth.rs**
- High: Consider adding rate limiting for login attempts
- Medium: Error messages could expose implementation details
```

---

## 🔄 Workflows

### Pre-Commit Review

Review changes before committing:

```bash
#!/bin/bash
# .git/hooks/pre-commit

echo "🔍 Running code review..."
rustassistant review diff --output /tmp/review.txt

# Check if there are critical issues
if grep -q "CRITICAL" /tmp/review.txt; then
    echo "❌ Critical issues found! Please fix before committing."
    cat /tmp/review.txt
    exit 1
fi

echo "✅ Code review passed"
```

### PR Review Workflow

```bash
# 1. Create feature branch
git checkout -b feature/new-api

# 2. Make changes and commit
git add .
git commit -m "Add new API endpoint"

# 3. Review changes
rustassistant review diff --base main --output review.md

# 4. Fix any issues
# ... make improvements ...

# 5. Generate PR description
rustassistant review pr --base main --output pr-description.md

# 6. Create PR with generated description
gh pr create --body-file pr-description.md
```

### CI/CD Integration

**GitHub Actions:**

```yaml
name: Code Review

on:
  pull_request:
    branches: [main, develop]

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Install RustAssistant
        run: cargo install --git https://github.com/jordanistan/rustassistant
      
      - name: Run Code Review
        env:
          XAI_API_KEY: ${{ secrets.XAI_API_KEY }}
        run: |
          rustassistant review diff \
            --base ${{ github.base_ref }} \
            --github \
            --output review.md
      
      - name: Comment on PR
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

### Daily Code Quality Check

```bash
#!/bin/bash
# scripts/daily-review.sh

DATE=$(date +%Y-%m-%d)
REPORT_DIR="reports/quality"
mkdir -p "$REPORT_DIR"

# Review yesterday's changes
git log --since="1 day ago" --name-only --pretty=format: | \
  sort -u | \
  xargs rustassistant review files --output "$REPORT_DIR/$DATE.md"

echo "📊 Daily review saved to $REPORT_DIR/$DATE.md"

# Track quality over time
echo "$DATE,$(grep 'Quality Score:' $REPORT_DIR/$DATE.md | awk '{print $3}')" >> quality-trends.csv
```

---

## 🎓 Understanding Review Output

### Quality Scores

**Overall Score (0-100):**
- **90-100:** Excellent - Production ready
- **75-89:** Good - Minor improvements suggested
- **60-74:** Acceptable - Some issues to address
- **40-59:** Needs Improvement - Significant issues
- **0-39:** Poor - Major refactoring needed

**Security Score (0-100):**
- Similar scale focusing on security aspects
- Weighted toward critical security issues

### Issue Severity Levels

**🔴 Critical**
- Security vulnerabilities (SQL injection, XSS, etc.)
- Authentication/authorization bypasses
- Data exposure risks
- **Action:** Must fix before merge

**🟠 High**
- Security concerns (unsafe code, panics)
- Potential bugs (unwrap on Results)
- Performance issues
- **Action:** Should fix before merge

**🟡 Medium**
- Error handling improvements
- Code complexity
- Maintainability concerns
- **Action:** Consider addressing

**🔵 Low**
- Style issues
- Naming conventions
- Documentation gaps
- **Action:** Nice to have

**ℹ️ Info**
- Informational messages
- Suggestions for future improvements
- **Action:** Optional

---

## 💡 Best Practices

### 1. Review Early and Often

```bash
# Review after each significant change
rustassistant review diff

# Fix issues immediately while context is fresh
```

### 2. Use Consistent Baselines

```bash
# Always compare against the same branch
rustassistant review diff --base main

# Not against your local changes
```

### 3. Save Review History

```bash
# Track reviews over time
DATE=$(date +%Y%m%d)
rustassistant review diff --base main --output reviews/review-$DATE.md
```

### 4. Combine with Batch Analysis

```bash
# Full quality audit before PR
rustassistant analyze batch src/ --output quality.md
rustassistant review diff --base main --output review.md

# Compare both reports
```

### 5. Automate in CI/CD

- Run on every PR
- Post results as PR comments
- Block merge on critical issues
- Track quality trends over time

---

## 🔧 Configuration

### Environment Variables

```bash
# API key (required)
export XAI_API_KEY="your-api-key"

# Database path (optional)
export DATABASE_PATH="data/rustassistant.db"

# Cache path (optional)
export CACHE_DB_PATH="data/rustassistant_cache.db"
```

### Git Configuration

Ensure git is available and configured:

```bash
git --version  # Should work
git status     # Should show repo status
```

---

## 📊 Cost Optimization

### Caching Benefits

Code reviews automatically use response caching:

```bash
# First review (API call)
rustassistant review diff --base main
# Cost: ~$0.03 for 10 files

# Re-review after small changes (mostly cached)
rustassistant review diff --base main
# Cost: ~$0.005 (only changed files analyzed)

# Review same files again (fully cached)
rustassistant review diff --base main
# Cost: $0.00 (instant!)
```

### Cost Estimates

| Scenario | Files | First Run | Cached Run | Savings |
|----------|-------|-----------|------------|---------|
| Small PR | 5 | $0.015 | $0.00 | 100% |
| Medium PR | 15 | $0.045 | $0.005 | 89% |
| Large PR | 30 | $0.090 | $0.010 | 89% |

**Tips:**
- Review often to maximize cache hits
- Changed files trigger new analysis
- Unchanged files use cache

---

## 🐛 Troubleshooting

### "Git command failed"

**Cause:** Not in a git repository or git not installed

**Fix:**
```bash
# Check git
git --version

# Initialize repo if needed
git init
```

### "No changes detected"

**Cause:** No uncommitted changes or wrong base branch

**Fix:**
```bash
# Check git status
git status

# Try different base
rustassistant review diff --base develop

# Or review specific files
rustassistant review files src/*.rs
```

### "API key not set"

**Cause:** Missing XAI_API_KEY environment variable

**Fix:**
```bash
# Set API key
export XAI_API_KEY="your-key"

# Or add to .env file
echo "XAI_API_KEY=your-key" >> .env
```

### "File too large for analysis"

**Cause:** File exceeds 100KB limit

**Fix:**
- Review will skip large files with info message
- Consider refactoring large files into smaller modules
- Files >100KB are likely too complex anyway

---

## 📈 Metrics & Analytics

### Track Quality Over Time

```bash
# Create tracking script
cat > scripts/track-quality.sh << 'EOF'
#!/bin/bash
DATE=$(date +%Y-%m-%d)
rustassistant review diff --base main --output /tmp/review.txt

QUALITY=$(grep "Quality Score:" /tmp/review.txt | awk '{print $3}' | cut -d'/' -f1)
SECURITY=$(grep "Security Score:" /tmp/review.txt | awk '{print $3}' | cut -d'/' -f1)
ISSUES=$(grep "Total Issues:" /tmp/review.txt | awk '{print $3}')

echo "$DATE,$QUALITY,$SECURITY,$ISSUES" >> metrics/quality-history.csv
EOF

chmod +x scripts/track-quality.sh
```

### Generate Quality Report

```bash
# Weekly quality summary
cat metrics/quality-history.csv | \
  awk -F',' '{sum+=$2; count++} END {print "Avg Quality:", sum/count}'
```

---

## 🎯 Examples

### Example 1: Simple Feature Review

```bash
# Working on new feature
git checkout -b feature/user-profiles

# Make changes
# ... code ...

# Review before commit
rustassistant review diff

# Output:
# 🔍 Reviewing changes in .
# 
# 📊 Review Complete!
#    Files: 3
#    Issues: 4
#    Quality: 85.2/100
#    Security: 91.3/100
```

### Example 2: Security-Focused Review

```bash
# Review authentication changes
rustassistant review files src/auth/*.rs --output security-review.md

# Check for critical issues
grep "CRITICAL\|HIGH" security-review.md
```

### Example 3: PR Description Generation

```bash
# Generate comprehensive PR description
rustassistant review pr --base main --output pr.md

# Preview
cat pr.md

# Use with gh CLI
gh pr create --body-file pr.md --title "Add user profiles feature"
```

---

## 🚀 Advanced Usage

### Custom Review Scripts

```bash
#!/bin/bash
# scripts/smart-review.sh

# Get list of changed files
FILES=$(git diff --name-only main)

# Review only Rust files
RUST_FILES=$(echo "$FILES" | grep "\.rs$")

if [ -n "$RUST_FILES" ]; then
    echo "🦀 Reviewing Rust files..."
    echo "$RUST_FILES" | xargs rustassistant review files --output rust-review.md
fi

# Review only JS/TS files
JS_FILES=$(echo "$FILES" | grep -E "\.(js|ts)$")

if [ -n "$JS_FILES" ]; then
    echo "📜 Reviewing JavaScript files..."
    echo "$JS_FILES" | xargs rustassistant review files --output js-review.md
fi
```

### Integration with Code Formatters

```bash
# Format, then review
cargo fmt
rustassistant review diff --base main

# Should show fewer style issues
```

---

## 📚 Related Features

- **[Batch Analysis](BATCH_OPERATIONS.md)** - Analyze multiple files efficiently
- **[Cost Optimization](../COST_OPTIMIZATION_RESULTS.md)** - Maximize cache usage
- **[CLI Reference](CLI_CHEATSHEET.md)** - All available commands

---

## 🎉 Summary

Code Review Automation helps you:
- ✅ Catch issues before they reach production
- ✅ Maintain consistent code quality
- ✅ Save time on manual code reviews
- ✅ Learn from AI suggestions
- ✅ Improve security posture

**Start using it today:**

```bash
rustassistant review diff --base main --output review.md
```

---

**Last Updated:** February 1, 2026  
**Status:** Production Ready  
**Cost:** ~$0.003 per file (cached runs free)
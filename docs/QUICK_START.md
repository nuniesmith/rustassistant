# Rustassistant Quick Start Guide

**Get started in 5 minutes!** 🚀

---

## ✅ Prerequisites

You already have:
- ✅ Rust installed
- ✅ Binary built at `./target/release/rustassistant`
- ✅ All 4 advanced features ready

---

## 🎯 First Steps (5 minutes)

### 1. Set Up API Key (1 minute)

```bash
# Create .env file if you haven't already
echo "GROK_API_KEY=your-xai-api-key-here" > .env

# Or export directly
export GROK_API_KEY="your-xai-api-key-here"
```

Don't have a Grok API key? Get one at https://x.ai/api

### 2. Initialize Database (30 seconds)

```bash
# Create data directory
mkdir -p data

# Initialize with a test note
./target/release/rustassistant note add "Rustassistant setup complete!" --tags setup
```

### 3. Test All Features (3 minutes)

```bash
# Code Review - Review your current changes
./target/release/rustassistant review diff

# Refactoring - Analyze code quality
./target/release/rustassistant refactor analyze src/lib.rs

# Test Generation - Find coverage gaps
./target/release/rustassistant test gaps src/

# Check costs (should be minimal)
./target/release/rustassistant costs
```

---

## 🔥 Your First Quality Check

Run this complete quality pipeline on your code:

```bash
#!/bin/bash
# Save as: quality-check.sh

echo "🔍 Running Complete Quality Check..."
echo "=================================="
echo ""

# 1. Code Review
echo "📝 Code Review..."
./target/release/rustassistant review diff --output reports/review.md

# 2. Refactoring Analysis
echo "🔧 Refactoring Analysis..."
./target/release/rustassistant refactor analyze src/ --output reports/refactor.md

# 3. Test Coverage
echo "🧪 Test Coverage Analysis..."
./target/release/rustassistant test gaps src/ --output reports/coverage.md

# 4. Cost Summary
echo "💰 Cost Summary..."
./target/release/rustassistant costs

echo ""
echo "✅ Reports saved in reports/ directory"
echo "=================================="
```

Make it executable and run:

```bash
mkdir -p reports
chmod +x quality-check.sh
./quality-check.sh
```

---

## 📚 Essential Commands

### Code Review

```bash
# Review uncommitted changes
rustassistant review diff

# Review specific files
rustassistant review files src/api/*.rs

# Generate PR description
rustassistant review pr --base main --output PR.md
```

### Refactoring

```bash
# Analyze for code smells
rustassistant refactor analyze src/

# Get specific suggestions
rustassistant refactor suggest src/main.rs

# Create improvement plan
rustassistant refactor plan src/ --goal "improve maintainability"
```

### Test Generation

```bash
# Generate tests for file
rustassistant test generate src/utils.rs

# Find coverage gaps
rustassistant test gaps src/

# Create test fixtures
rustassistant test fixtures src/models.rs
```

### Cost Management

```bash
# View costs
rustassistant costs

# Check cache performance
rustassistant cache stats

# Clear cache if needed
rustassistant cache clear
```

---

## 🎓 Daily Workflow

### Morning Routine (2 minutes)

```bash
# See what to work on
rustassistant next

# Check yesterday's costs
rustassistant costs
```

### Before Each Commit (1 minute)

```bash
# Review your changes
rustassistant review diff

# Fix any critical issues
# ... make fixes ...

# Commit
git add .
git commit -m "your message"
```

### Weekly Quality Check (5 minutes)

```bash
# Run full analysis
rustassistant refactor analyze src/ --output weekly-report.md

# Check test coverage
rustassistant test gaps src/ --output coverage-report.md

# Review reports and plan improvements
```

---

## 💡 Pro Tips

### 1. Add Alias to Your Shell

```bash
# Add to ~/.bashrc or ~/.zshrc
alias dflow='./target/release/rustassistant'

# Now you can use:
dflow review diff
dflow costs
```

### 2. Create Pre-Commit Hook

```bash
# .git/hooks/pre-commit
#!/bin/bash

echo "Running code review..."
./target/release/rustassistant review diff --output /tmp/review.md

# Check for critical issues
if grep -q "Critical:" /tmp/review.md; then
    echo "❌ Critical issues found! Review /tmp/review.md"
    cat /tmp/review.md
    exit 1
fi

echo "✅ Quality check passed!"
```

Make it executable:
```bash
chmod +x .git/hooks/pre-commit
```

### 3. Track Costs Weekly

```bash
# Add to crontab or run manually
# crontab -e
# 0 9 * * MON /path/to/cost-report.sh

#!/bin/bash
# cost-report.sh
echo "Weekly Cost Report - $(date)" >> cost-log.txt
./target/release/rustassistant costs >> cost-log.txt
echo "---" >> cost-log.txt
```

---

## 🐛 Troubleshooting

### "Database not found"

```bash
mkdir -p data
./target/release/rustassistant note add "test" --tags setup
```

### "No API key"

```bash
export GROK_API_KEY="your-key-here"
# Or add to .env file
```

### "Command not found"

```bash
# Build the binary
cargo build --release

# Or use full path
./target/release/rustassistant --help
```

### High costs?

```bash
# Check cache hit rate
./target/release/rustassistant cache stats

# Should see 70%+ hit rate
# If not, you might be analyzing different content each time
```

---

## 📖 Next Steps

### Learn More

- **Complete Guide:** `docs/ADVANCED_FEATURES_GUIDE.md` (1,100+ lines)
- **All Commands:** `rustassistant --help`
- **Cost Optimization:** `COST_OPTIMIZATION_RESULTS.md`
- **Work Plan:** `docs/devflow_work_plan.md`

### Explore Features

```bash
# See all review options
rustassistant review --help

# See all refactor options
rustassistant refactor --help

# See all test options
rustassistant test --help
```

### Track Your Progress

After 1 week, measure:
- Issues caught by review
- Test coverage improvement
- Refactoring items completed
- Total costs
- Time saved

---

## 🎯 Your First Real Task

**Try this right now (2 minutes):**

```bash
# 1. Analyze your main source file
./target/release/rustassistant refactor analyze src/lib.rs --output my-first-analysis.md

# 2. Read the report
cat my-first-analysis.md

# 3. Check how much it cost
./target/release/rustassistant costs

# 4. Run it again (watch it use cache!)
./target/release/rustassistant refactor analyze src/lib.rs

# Notice: Second run is nearly instant and free!
```

---

## 🚀 You're Ready!

You now have **4 production-ready AI-powered development tools**:

1. ✅ **Code Review** - Catch issues before they ship
2. ✅ **Refactoring** - Improve code quality systematically  
3. ✅ **Test Generation** - Boost coverage quickly
4. ✅ **Query Templates** - Reuse common patterns

**Start with one command:**

```bash
./target/release/rustassistant review diff --output first-review.md
```

Then explore from there! 🎉

---

**Questions?** Check `docs/ADVANCED_FEATURES_GUIDE.md` for complete documentation.

**Happy coding!** 💻✨
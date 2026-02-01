# Rustassistant CLI Cheat Sheet

Quick reference for common commands. For detailed help: `devflow --help`

---

## 📝 Note Management

### Add Notes
```bash
devflow note add "Your note text"                    # Basic note
devflow note add "Note" --tags tag1,tag2            # With tags
devflow note add "Note" --tags urgent --status active  # Active note
```

### List & Filter
```bash
devflow note list                        # All notes
devflow note list --status inbox         # Only inbox
devflow note list --status active        # Only active
devflow note list --tag backend          # By tag
devflow note list --limit 5              # Top 5
```

### Search
```bash
devflow note search "keyword"            # Full-text search
devflow note search "API"                # Case-insensitive
```

### View & Update
```bash
devflow note show 1                      # View note #1
devflow note update 1 --status active    # Change status
devflow note update 1 --content "New"   # Change content
```

### Tags
```bash
devflow note tag 1 newtag               # Add tag
devflow note untag 1 oldtag             # Remove tag
```

### Delete
```bash
devflow note delete 1                    # Delete note #1
```

---

## 📂 Repository Management

### Track Repos
```bash
devflow repo add .                       # Track current dir
devflow repo add /path/to/repo          # Track specific path
devflow repo add . --name myproject     # With custom name
```

### Analyze Repos
```bash
devflow repo analyze projectname         # Analyze & cache tree
devflow repo analyze project --output tree.json  # Save to file
```

### View Structure
```bash
devflow repo tree projectname            # Show directory tree
devflow repo tree project --depth 3      # Limit depth
```

### List Files
```bash
devflow repo files projectname           # List all files
devflow repo files project --language Rust  # Filter by language
devflow repo files project --largest 10  # Top 10 largest files
devflow repo files project --recent 10   # Recently modified
```

### View Repos
```bash
devflow repo list                        # List all repos
devflow repo status projectname          # Detailed info
```

### Remove
```bash
devflow repo remove projectname          # Stop tracking
```

---

## 🤖 AI Analysis (Grok)

### Score Files
```bash
devflow analyze file src/main.rs          # Full file analysis
devflow analyze file lib.rs               # Get detailed scores
```

### Quick Analysis
```bash
devflow analyze quick "fn main() {}"      # Analyze code snippet
devflow analyze quick src/helper.rs       # Quick file check
```

### Ask Questions
```bash
devflow analyze ask "How to optimize this?"
devflow analyze ask "Is this secure?" --context src/auth.rs
devflow analyze ask "Best practices for error handling"
```

### Monitor Costs
```bash
devflow costs                             # Show all cost stats
```

---

## 🎯 Workflow Commands

### What's Next?
```bash
devflow next                             # Get recommendation
```

### Statistics
```bash
devflow stats                            # View dashboard
```

---

## 📊 Status Workflow

```
📥 inbox      → New/uncategorized
🔥 active     → Currently working on
✅ processed  → Completed/done
📦 archived   → Parked for later
```

### Status Changes
```bash
devflow note update ID --status inbox      # Back to inbox
devflow note update ID --status active     # Mark as active
devflow note update ID --status processed  # Mark done
devflow note update ID --status archived   # Archive
```

---

## 🏷️ Common Tag Patterns

### By Phase
```bash
--tags phase1,phase2,phase3
```

### By Type
```bash
--tags bug,feature,idea,research,decision
```

### By Tech
```bash
--tags rust,python,docker,kubernetes
```

### By Priority
```bash
--tags urgent,important,low
```

### By Domain
```bash
--tags backend,frontend,database,api,ui,devops
```

---

## ⚙️ Options

### Database Location
```bash
devflow --database /path/to/db.db note list    # Custom DB
export DEVFLOW_DB=~/.devflow/main.db           # Set default (add to .bashrc)
alias devflow='devflow --database ~/.devflow/main.db'  # Alias
```

### Verbose Logging
```bash
devflow --verbose note add "Debug mode"
devflow -v stats
```

---

## 🔥 Common Workflows

### Morning Routine
```bash
devflow next                             # See active work
devflow note list --status active        # Detail view
```

### Capture Ideas Throughout Day
```bash
devflow note add "idea" --tags idea
devflow note add "bug" --tags bug,urgent
devflow note add "research X vs Y" --tags research
```

### Analyze Repository
```bash
devflow repo analyze myproject           # Scan directory tree
devflow repo tree myproject --depth 2    # View structure
devflow repo files myproject --largest 10 # Find large files
```

### End of Day Review
```bash
devflow note list --status inbox         # Review new items
devflow note update 5 --status active    # Prioritize
devflow note update 9 --status archived  # Park unimportant
devflow stats                            # Check progress
```

### Weekly Review
```bash
devflow stats                            # Overview
devflow costs                            # Check AI spending
devflow note list --status processed     # What got done
devflow note list --status archived      # Revisit parked items
```

---

## 🎨 Output Examples

### Note List
```
📥 [inbox] Build web UI with HTMX (ID: 5)
   Tags: phase3, ui
   Created: 2026-02-01 10:30

🔥 [active] Complete note system (ID: 2)
   Tags: database, phase1
   Created: 2026-02-01 09:15
```

### Next Recommendations
```
📋 What should you work on next?

🔥 Active work (2 items):
  • Complete note system (ID: 2)
  • Add cost tracking (ID: 4)

📥 Inbox to process (3 items):
  • Build web UI (ID: 5)
  • Research LanceDB (ID: 3)

💡 Recommendation: Focus on active items first
```

### Statistics
```
📊 Rustassistant Statistics

Notes:
  Total: 15
  Inbox: 8

Tags: 12
  Top tags:
    backend (5)
    frontend (4)
    urgent (3)

Repositories: 2
```

### Cost Monitoring
```
💰 LLM Cost Statistics

Total Costs:
  All time:     $2.47
  Last 24h:     $0.45
  Last 7 days:  $1.23

By Model:
  grok-beta - $2.47 (156,000 tokens)

Recent Operations:
  2026-02-01 14:23 - file_scoring - $0.0425
```

---

## 🚀 Pro Tips

### Atomic Notes
```bash
# Good - one clear thought
devflow note add "Add pagination to API" --tags api,feature

# Bad - too broad
devflow note add "Improve API with pagination, sorting, caching" --tags api
```

### Use Consistent Tags
```bash
# Pick a convention and stick to it
--tags backend     # lowercase
--tags phase1      # numbers without spaces
--tags bug-fix     # kebab-case for multi-word
```

### Quick Capture
```bash
# Don't overthink - capture first, organize later
devflow note add "random idea about X"
# Review and tag during end-of-day routine
devflow note update ID --tags idea,backend
```

### Search Before Adding
```bash
# Check if you already captured this
devflow note search "authentication"
# Then decide if it's a duplicate or new angle
```

### Repository Analysis
```bash
# Analyze repos after making changes
devflow repo analyze webapp
# Find what needs attention
devflow repo files webapp --largest 5
devflow repo files webapp --recent 10
```

### AI-Powered Review
```bash
# Score critical files before committing
devflow analyze file src/security.rs
# Get improvement suggestions
devflow analyze ask "How to improve error handling?" --context src/lib.rs
# Monitor spending
devflow costs
```

---

## 🛠️ Troubleshooting

### Database Locked
```bash
# Close other devflow processes
pkill devflow
# Or use different database
devflow --database /tmp/test.db note list
```

### Permission Denied
```bash
# Check database file permissions
ls -la devflow.db
chmod 644 devflow.db
```

### Fresh Start
```bash
# Backup and recreate
mv devflow.db devflow.db.backup
devflow note add "Fresh start"
```

---

## 📖 Getting Help

```bash
devflow --help                # General help
devflow note --help           # Note commands
devflow analyze --help        # AI commands
devflow note add --help       # Specific command
```

**Documentation:**
- Full guide: `docs/QUICKSTART.md`
- Work plan: `docs/devflow_work_plan.md`
- Progress: `docs/PROGRESS_UPDATE.md`
- Session summaries: `SESSION*_SUMMARY.md`

**Setup AI Features:**
```bash
# Get API key from https://x.ai
export XAI_API_KEY='your-key-here'

# Test it
devflow analyze file README.md
devflow costs
```

---

**Quick Start:** `devflow note add "My first note" --tags getting-started`

# Rustassistant Web UI Guide

## Overview

The Rustassistant Web UI provides a modern, HTMX-powered dashboard for managing your AI-assisted development workflow. It offers a clean interface for managing notes, repositories, tracking LLM costs, and running code analyses.

## Features

### 🎯 Dashboard
- **Real-time Statistics**: View total notes, repositories, costs, and cache performance
- **Quick Actions**: Fast access to common tasks
- **Recent Notes**: See your latest notes at a glance
- **Activity Feed**: Track recent LLM operations and analyses
- **Smart Recommendations**: Get contextual next-step suggestions
- **Cost Insights**: Monitor daily, weekly, and monthly AI API spending

### 📝 Notes Management
- Create and organize development notes
- Tag-based filtering and organization
- Status tracking (Inbox, Active, Processed, Archived)
- Full-text search capabilities
- Quick edit and delete actions

### 📦 Repository Tracking
- Add and manage code repositories
- Track analysis history
- View repository metadata
- One-click analysis launching

### 💰 Cost Tracking
- Detailed LLM API cost monitoring
- Cost breakdowns by time period
- Cache hit rate visualization
- Savings calculations
- Budget alerts and recommendations
- Recent operation history

### 🔍 AI-Powered Analysis
- Select repositories for analysis
- Configure analysis parameters
- Real-time progress tracking
- View and export results

## Quick Start

### Starting the Web Server

```bash
# From the rustassistant directory
cargo build --release --bin webui-server
./target/release/webui-server
```

The server will start on `http://127.0.0.1:3001` by default.

### Configuration

Set environment variables to customize the server:

```bash
# Custom port (default: 3001)
export PORT=8080

# Custom database path (default: data/rustassistant.db)
export DATABASE_PATH=/path/to/your/database.db

# Start the server
./target/release/webui-server
```

### Accessing the Web UI

Once started, open your browser to:
- **Dashboard**: http://127.0.0.1:3001/
- **Notes**: http://127.0.0.1:3001/notes
- **Repositories**: http://127.0.0.1:3001/repos
- **Costs**: http://127.0.0.1:3001/costs
- **Analyze**: http://127.0.0.1:3001/analyze

## Architecture

### Technology Stack

- **Backend**: Axum (Rust async web framework)
- **Templates**: Askama (compile-time templates)
- **Frontend**: HTMX (dynamic HTML without heavy JavaScript)
- **Database**: SQLite (via existing Database layer)
- **Styling**: Custom CSS with utility classes

### Project Structure

```
rustassistant/
├── src/
│   ├── web_ui.rs              # Web handlers and route definitions
│   └── bin/
│       └── webui_server.rs    # Web server binary
├── templates/
│   ├── layouts/
│   │   └── base.html          # Base layout with navigation
│   └── pages/
│       ├── dashboard.html     # Dashboard page
│       ├── notes.html         # Notes list page
│       ├── repos.html         # Repositories page
│       ├── costs.html         # Cost tracking page
│       └── analyze.html       # Analysis page
└── static/                     # (future) Static assets
```

### Key Components

#### `WebAppState`
Shared application state containing the database connection:
```rust
pub struct WebAppState {
    pub db: Database,
}
```

#### Template Structs
Each page has a corresponding template struct:
- `DashboardTemplate` - Main dashboard
- `NotesTemplate` - Notes listing
- `ReposTemplate` - Repository management
- `CostsTemplate` - Cost tracking
- `AnalyzeTemplate` - Analysis interface

#### Handlers
Async route handlers that:
1. Query the database
2. Transform data for display
3. Render templates
4. Return HTML responses

## Development Guide

### Adding a New Page

1. **Create the template** in `templates/pages/`:
```html
{% extends "layouts/base.html" %}

{% block title %}My Page - Rustassistant{% endblock %}

{% block content %}
<div class="page-header mb-4">
    <h1>My Page</h1>
</div>
<!-- Your content here -->
{% endblock %}
```

2. **Define the template struct** in `src/web_ui.rs`:
```rust
#[derive(Template)]
#[template(path = "pages/mypage.html")]
pub struct MyPageTemplate {
    pub page: String,
    pub data: Vec<MyData>,
}
```

3. **Create the handler**:
```rust
pub async fn mypage_handler(
    State(state): State<Arc<WebAppState>>,
) -> Result<impl IntoResponse, AppError> {
    let data = state.db.get_my_data().await?;
    
    Ok(MyPageTemplate {
        page: "mypage".to_string(),
        data,
    })
}
```

4. **Add the route** in `create_router()`:
```rust
Router::new()
    .route("/mypage", get(mypage_handler))
    // ... other routes
```

### Adding HTMX Interactivity

HTMX is included in the base template. Add interactivity with attributes:

```html
<!-- Auto-refresh every 5 seconds -->
<div hx-get="/api/stats" hx-trigger="every 5s" hx-swap="innerHTML">
    Loading...
</div>

<!-- Button that updates a section -->
<button hx-post="/api/notes" 
        hx-target="#notes-list" 
        hx-swap="afterbegin">
    Add Note
</button>

<!-- Form with AJAX submission -->
<form hx-post="/api/repos" hx-swap="outerHTML">
    <input type="text" name="path" />
    <button type="submit">Add Repository</button>
</form>
```

### Database Helper Methods

The Web UI uses convenience methods added to the `Database` struct:

```rust
// Count methods
db.count_notes().await?;
db.count_repositories().await?;

// List with limit
db.list_notes(status, tag, Some(limit)).await?;

// Formatting helpers on models
note.status_str();           // "inbox", "active", etc.
note.tags_str();             // "tag1,tag2,tag3"
note.created_at_formatted(); // "2024-01-15 14:30"
```

## API Endpoints (Future)

The following API endpoints are planned for HTMX interactions:

### Notes API
- `POST /api/notes` - Create a new note
- `PUT /api/notes/:id` - Update note content/status
- `DELETE /api/notes/:id` - Delete a note
- `GET /api/notes?status=inbox&tag=bug` - Filter notes

### Repository API
- `POST /api/repos` - Add a repository
- `DELETE /api/repos/:id` - Remove a repository
- `POST /api/repos/:id/analyze` - Start analysis

### Analysis API
- `POST /api/analyze` - Run code analysis
- `GET /api/analyze/:id/status` - Check analysis progress
- `GET /api/analyze/:id/results` - Get analysis results

### Cache API
- `GET /api/cache/stats` - Get cache statistics
- `POST /api/cache/prune` - Prune old cache entries
- `POST /api/cache/clear` - Clear all cache

## Styling Guide

### CSS Variables

The UI uses CSS custom properties for theming:

```css
:root {
    --primary: #3b82f6;      /* Primary blue */
    --secondary: #64748b;    /* Secondary gray */
    --success: #10b981;      /* Green */
    --warning: #f59e0b;      /* Orange */
    --danger: #ef4444;       /* Red */
    --bg: #f8fafc;          /* Background */
    --surface: #ffffff;     /* Cards/surfaces */
    --text: #1e293b;        /* Primary text */
    --text-light: #64748b;  /* Secondary text */
    --border: #e2e8f0;      /* Borders */
}
```

### Utility Classes

```html
<!-- Layout -->
<div class="grid grid-2">...</div>      <!-- 2-column grid -->
<div class="grid grid-3">...</div>      <!-- 3-column grid -->
<div class="grid grid-4">...</div>      <!-- 4-column grid -->

<!-- Spacing -->
<div class="mb-4">...</div>             <!-- Margin bottom -->
<div class="mt-2">...</div>             <!-- Margin top -->

<!-- Components -->
<div class="card">...</div>             <!-- Card container -->
<div class="card-header">...</div>      <!-- Card header -->
<div class="stat-card">...</div>        <!-- Stat display -->

<!-- Buttons -->
<button class="btn btn-primary">...</button>
<button class="btn btn-secondary">...</button>
<button class="btn btn-danger">...</button>
<button class="btn btn-sm">...</button>  <!-- Small button -->

<!-- Badges -->
<span class="badge badge-primary">...</span>
<span class="badge badge-success">...</span>
<span class="badge badge-warning">...</span>
<span class="badge badge-danger">...</span>

<!-- Text -->
<p class="text-muted">...</p>           <!-- Muted text -->
<p class="text-center">...</p>          <!-- Centered text -->
<p class="text-right">...</p>           <!-- Right-aligned -->

<!-- Alerts -->
<div class="alert alert-info">...</div>
<div class="alert alert-success">...</div>
<div class="alert alert-warning">...</div>
<div class="alert alert-danger">...</div>
```

## Performance Optimization

### Database Query Optimization
- Use `count_notes()` instead of fetching all notes to count
- Use limit parameter to fetch only needed records
- Leverage database indexes for tag and status queries

### Template Caching
Askama templates are compiled at build time for zero-cost rendering.

### Future Optimizations
- [ ] Add compression middleware (gzip/brotli)
- [ ] Implement HTTP caching headers
- [ ] Add WebSocket support for real-time updates
- [ ] Lazy-load large lists with pagination
- [ ] Add service worker for offline support

## Troubleshooting

### Server won't start
```bash
# Check if port is in use
lsof -i :3001

# Try a different port
PORT=8080 ./target/release/webui-server
```

### Database connection errors
```bash
# Ensure database directory exists
mkdir -p data

# Check database path
DATABASE_PATH=data/rustassistant.db ./target/release/webui-server
```

### Template compilation errors
```bash
# Clean and rebuild
cargo clean
cargo build --bin webui-server

# Check template syntax
# Templates are in: rustassistant/templates/
```

### Pages not loading
- Check server logs for errors
- Verify database is initialized
- Check browser console for JavaScript errors
- Ensure templates exist in the templates directory

## Security Considerations

### Current Status (MVP)
The current MVP does not include authentication or authorization. It's designed for local development use.

### Future Security Enhancements
- [ ] Add user authentication (session-based or JWT)
- [ ] Implement CSRF protection
- [ ] Add rate limiting
- [ ] Implement input validation and sanitization
- [ ] Add CORS configuration for API endpoints
- [ ] Set security headers (CSP, X-Frame-Options, etc.)

## Deployment

### Local Development
```bash
cargo run --bin webui-server
```

### Production Build
```bash
cargo build --release --bin webui-server
./target/release/webui-server
```

### Running as a Service (Linux)

Create `/etc/systemd/system/devflow-webui.service`:
```ini
[Unit]
Description=Rustassistant Web UI
After=network.target

[Service]
Type=simple
User=youruser
WorkingDirectory=/path/to/rustassistant
Environment=DATABASE_PATH=/path/to/data/rustassistant.db
Environment=PORT=3001
ExecStart=/path/to/rustassistant/target/release/webui-server
Restart=always

[Install]
WantedBy=multi-user.target
```

Then:
```bash
sudo systemctl daemon-reload
sudo systemctl enable devflow-webui
sudo systemctl start devflow-webui
```

### Docker (Future)
```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin webui-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/webui-server /usr/local/bin/
COPY --from=builder /app/templates /app/templates
WORKDIR /app
EXPOSE 3001
CMD ["webui-server"]
```

## Next Steps

### Immediate TODOs
1. ✅ Database integration complete
2. ✅ All pages rendering
3. ✅ Basic styling complete
4. ⬜ Add API endpoints for CRUD operations
5. ⬜ Implement HTMX interactivity
6. ⬜ Add form validation
7. ⬜ Create note creation/edit forms
8. ⬜ Add repository add/remove forms

### Future Enhancements
- Real-time updates with Server-Sent Events (SSE)
- Charts and visualizations (Chart.js integration)
- Export functionality (JSON, CSV, Markdown)
- Keyboard shortcuts
- Dark mode toggle
- Search autocomplete
- Bulk operations
- Analysis progress indicators
- File upload for batch operations

## Contributing

When contributing to the Web UI:

1. Follow existing template structure
2. Use semantic HTML
3. Keep JavaScript minimal (prefer HTMX)
4. Add appropriate error handling
5. Update this documentation
6. Test all pages before submitting

## Resources

- [HTMX Documentation](https://htmx.org/docs/)
- [Askama Template Guide](https://djc.github.io/askama/)
- [Axum Web Framework](https://docs.rs/axum/)
- [SQLite Documentation](https://www.sqlite.org/docs.html)

## License

Same as the main project (see LICENSE file).
// ============================================================================
// Shared Navigation — Web UI Module
// ============================================================================
//
// Centralised navigation bar used by all RustAssistant web UI pages.
// This replaces the per-module `nav()` functions with a single source of truth.
//
// The nav is reorganised into logical groups:
//   • Core:    Dashboard, Repositories, Tasks, Activity
//   • Content: Ideas, Docs, Notes, Chat
//   • DevOps:  Auto-Scanner, Scan Progress, Cache, DB Explorer
//   • System:  Settings
//
// The nav is rendered full-width with grouped sections and dividers.

use crate::web_ui::timezone_selector_html;

/// A single navigation link.
struct NavItem {
    label: &'static str,
    href: &'static str,
    icon: &'static str,
}

/// A labelled group of navigation links.
struct NavGroup {
    label: &'static str,
    items: &'static [NavItem],
}

const NAV_GROUPS: &[NavGroup] = &[
    NavGroup {
        label: "Core",
        items: &[
            NavItem {
                label: "Dashboard",
                href: "/dashboard",
                icon: "📊",
            },
            NavItem {
                label: "Repositories",
                href: "/repos",
                icon: "📦",
            },
            NavItem {
                label: "Tasks",
                href: "/queue",
                icon: "✅",
            },
            NavItem {
                label: "Activity",
                href: "/activity",
                icon: "⚡",
            },
        ],
    },
    NavGroup {
        label: "Content",
        items: &[
            NavItem {
                label: "Ideas",
                href: "/ideas",
                icon: "💡",
            },
            NavItem {
                label: "Docs",
                href: "/docs",
                icon: "📄",
            },
            NavItem {
                label: "Notes",
                href: "/notes",
                icon: "📝",
            },
            NavItem {
                label: "Chat",
                href: "/chat",
                icon: "💬",
            },
        ],
    },
    NavGroup {
        label: "DevOps",
        items: &[
            NavItem {
                label: "Scanner",
                href: "/scanner",
                icon: "🔍",
            },
            NavItem {
                label: "Scan Progress",
                href: "/scan/dashboard",
                icon: "📈",
            },
            NavItem {
                label: "Audits",
                href: "/audit",
                icon: "🔬",
            },
            NavItem {
                label: "Cache",
                href: "/cache",
                icon: "🗄️",
            },
            NavItem {
                label: "DB Explorer",
                href: "/db",
                icon: "🛢️",
            },
        ],
    },
    NavGroup {
        label: "System",
        items: &[
            NavItem {
                label: "API Keys",
                href: "/api-keys",
                icon: "🔑",
            },
            NavItem {
                label: "Settings",
                href: "/settings",
                icon: "⚙️",
            },
        ],
    },
];

/// Render the full-width navigation bar HTML.
///
/// `active` should match the label of the currently active page (e.g. "Dashboard").
pub fn nav(active: &str) -> String {
    let mut links = String::new();

    for (gi, group) in NAV_GROUPS.iter().enumerate() {
        if gi > 0 {
            links.push_str(r#"<span class="nav-divider"></span>"#);
        }
        links.push_str(&format!(
            r#"<span class="nav-group-label">{}</span>"#,
            group.label
        ));
        for item in group.items {
            let class = if item.label == active {
                " class=\"active\""
            } else {
                ""
            };
            links.push_str(&format!(
                r#"<a href="{href}"{class}>{icon} {label}</a>"#,
                href = item.href,
                class = class,
                icon = item.icon,
                label = item.label,
            ));
        }
    }

    format!(
        r#"<nav class="main-nav">
        <a href="/dashboard" class="nav-brand">🦀 RustAssistant</a>
        <div class="nav-links">
            {links}
        </div>
        <div class="nav-right">
            {tz}
        </div>
    </nav>"#,
        links = links,
        tz = timezone_selector_html(),
    )
}

/// Render the `<nav>` element for pages rendered via the dashboard's inline format! style.
/// Returns just the inner nav content (used by `render_dashboard_page` etc.).
pub fn nav_for_dashboard(active: &str) -> String {
    nav(active)
}

/// Common CSS for the new navigation bar.
/// Include this inside a `<style>` block or concatenate with other styles.
pub fn nav_css() -> &'static str {
    r#"
    /* ── Navigation Bar ─────────────────────────────────────────────── */
    .main-nav {
        display: flex;
        align-items: center;
        gap: 0.75rem;
        padding: 0.75rem 1.5rem;
        background: #1e293b;
        border-bottom: 1px solid #334155;
        flex-wrap: wrap;
        width: 100%;
    }
    .nav-brand {
        font-weight: 700;
        font-size: 1.25rem;
        color: #38bdf8 !important;
        text-decoration: none;
        margin-right: 1rem;
        white-space: nowrap;
        flex-shrink: 0;
    }
    .nav-brand:hover {
        color: #7dd3fc !important;
    }
    .nav-links {
        display: flex;
        align-items: center;
        gap: 0.25rem;
        flex-wrap: wrap;
        flex: 1;
    }
    .nav-group-label {
        color: #475569;
        font-size: 0.65rem;
        font-weight: 700;
        text-transform: uppercase;
        letter-spacing: 0.1em;
        padding: 0.25rem 0.4rem;
        user-select: none;
    }
    .nav-divider {
        display: inline-block;
        width: 1px;
        height: 1.2rem;
        background: #334155;
        margin: 0 0.35rem;
        vertical-align: middle;
    }
    .nav-links a {
        color: #94a3b8;
        text-decoration: none;
        padding: 0.35rem 0.65rem;
        border-radius: 6px;
        font-size: 0.82rem;
        white-space: nowrap;
        transition: all 0.2s;
    }
    .nav-links a:hover {
        color: #e2e8f0;
        background: #334155;
    }
    .nav-links a.active {
        color: #0ea5e9;
        background: #0c2d4a;
        font-weight: 600;
    }
    .nav-right {
        margin-left: auto;
        flex-shrink: 0;
        display: flex;
        align-items: center;
    }

    /* Responsive: stack groups on narrow screens */
    @media (max-width: 1200px) {
        .nav-group-label {
            display: none;
        }
        .nav-divider {
            margin: 0 0.15rem;
        }
    }
    @media (max-width: 768px) {
        .main-nav {
            flex-direction: column;
            align-items: flex-start;
        }
        .nav-links {
            width: 100%;
        }
        .nav-right {
            width: 100%;
            margin-left: 0;
            margin-top: 0.5rem;
        }
    }
    "#
}

/// Returns the full `<style>` block containing nav CSS plus common page styles.
/// This is the recommended way to include navigation styles in page shells.
pub fn full_nav_style() -> String {
    format!(
        r#"<style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0f172a; color: #e2e8f0; line-height: 1.6; }}
        .container {{ max-width: 1400px; margin: 0 auto; padding: 1rem 2rem; }}
        h2 {{ font-size: 1.4rem; margin-bottom: 1rem; color: #f1f5f9; }}
        h3 {{ font-size: 1.1rem; margin-bottom: 0.75rem; color: #94a3b8; }}
        .card {{ background: #1e293b; border-radius: 8px; border: 1px solid #334155;
            padding: 1.25rem; margin-bottom: 1rem; }}
        .btn {{ padding: 0.6rem 1.2rem; border-radius: 6px; border: none; cursor: pointer;
            font-size: 0.9rem; font-weight: 500; text-decoration: none; display: inline-block;
            transition: all 0.2s; }}
        .btn-sm {{ padding: 0.3rem 0.6rem; font-size: 0.8rem; }}
        .btn-primary {{ background: #0ea5e9; color: white; }}
        .btn-primary:hover {{ background: #0284c7; }}
        .btn-success {{ background: #22c55e; color: white; }}
        .btn-success:hover {{ background: #16a34a; }}
        .btn-danger {{ background: #ef4444; color: white; }}
        .btn-danger:hover {{ background: #dc2626; }}
        .btn-muted {{ background: #475569; color: #cbd5e1; }}
        .btn-muted:hover {{ background: #64748b; }}
        .badge {{ padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 600; }}
        .badge-primary {{ background: #6366f1; color: white; }}
        .badge-success {{ background: #22c55e; color: white; }}
        .badge-warning {{ background: #f59e0b; color: white; }}
        .badge-danger {{ background: #ef4444; color: white; }}
        .badge-info {{ background: #0ea5e9; color: white; }}
        .badge-muted {{ background: #475569; color: #cbd5e1; }}
        .tag {{ background: #1e3a5f; color: #7dd3fc; padding: 2px 8px; border-radius: 4px;
            font-size: 0.75rem; text-decoration: none; }}
        .tag:hover {{ background: #0284c7; color: white; }}
        .stats-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1rem; margin-bottom: 1.5rem; }}
        .stat-card {{ background: #1e293b; padding: 1.5rem; border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.2); border-left: 4px solid #38bdf8; }}
        .stat-card h3 {{ color: #94a3b8; font-size: 0.85rem; font-weight: 500;
            margin-bottom: 0.5rem; text-transform: uppercase; }}
        .stat-card .value {{ color: #38bdf8; font-size: 2rem; font-weight: bold; }}
        .stat-card.success {{ border-left-color: #22c55e; }}
        .stat-card.success .value {{ color: #22c55e; }}
        .stat-card.warning {{ border-left-color: #f59e0b; }}
        .stat-card.warning .value {{ color: #f59e0b; }}
        .stat-card.danger {{ border-left-color: #ef4444; }}
        .stat-card.danger .value {{ color: #ef4444; }}
        .action-section {{ background: #1e293b; padding: 2rem; border-radius: 8px; margin-bottom: 2rem; }}
        .action-section h2 {{ color: #e2e8f0; margin-bottom: 1rem; }}
        .action-buttons {{ display: flex; gap: 1rem; flex-wrap: wrap; }}
        footer {{ margin-top: 3rem; text-align: center; color: #64748b; font-size: 0.9rem;
            padding: 2rem 0; border-top: 1px solid #1e293b; }}

        /* Tables */
        table {{ width: 100%; border-collapse: collapse; font-size: 0.85rem; }}
        th {{ text-align: left; padding: 0.6rem 0.75rem; background: #0f172a;
            border-bottom: 2px solid #334155; font-weight: 600; color: #94a3b8;
            font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.05em; }}
        td {{ padding: 0.5rem 0.75rem; border-bottom: 1px solid #1e293b; }}
        tr:hover {{ background: #1e293b; }}

        {nav_css}
    </style>"#,
        nav_css = nav_css()
    )
}

/// Render a complete page shell with navigation, content, and footer.
///
/// # Arguments
/// * `title` — Page title (appears in `<title>` tag)
/// * `nav_active` — Label of the active nav item (e.g. "Chat", "Settings")
/// * `extra_head` — Extra HTML to inject into `<head>` (styles, scripts, etc.)
/// * `content` — Main page content HTML
pub fn page_shell(title: &str, nav_active: &str, extra_head: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title} — RustAssistant</title>
<script src="https://unpkg.com/htmx.org@1.9.10"></script>
{style}
{extra_head}
</head>
<body>
    {nav}
    <div class="container">
        {content}
    </div>
    <footer>
        <p>RustAssistant v0.1.0 | Powered by Rust & Axum</p>
    </footer>
    {tz_js}
</body>
</html>"#,
        title = title,
        style = full_nav_style(),
        extra_head = extra_head,
        nav = nav(nav_active),
        content = content,
        tz_js = crate::web_ui::timezone_js(),
    )
}

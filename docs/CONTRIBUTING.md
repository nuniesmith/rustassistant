# Contributing to RustAssistant

Thank you for your interest in contributing to RustAssistant! This document provides guidelines and instructions for contributing to the project.

## 🎯 Ways to Contribute

- **Bug Reports** - Report issues you encounter
- **Feature Requests** - Suggest new features or improvements
- **Code Contributions** - Submit pull requests
- **Documentation** - Improve or add documentation
- **Testing** - Write tests or improve test coverage
- **Reviews** - Review pull requests from others

## 🚀 Getting Started

### Prerequisites

- **Rust 1.70+** - [Install Rust](https://rustup.rs/)
- **Docker & Docker Compose** - [Install Docker](https://docs.docker.com/get-docker/)
- **Git** - For version control
- **SQLite 3** or **PostgreSQL 15+** - Database

### Development Setup

1. **Fork the repository**
   ```bash
   # Click "Fork" on GitHub, then clone your fork
   git clone https://github.com/YOUR_USERNAME/rustassistant.git
   cd rustassistant
   ```

2. **Set up development environment**
   ```bash
   # Start services
   docker-compose up -d
   
   # Set environment variables
   export DATABASE_URL=postgresql://rustassistant:changeme123@localhost:5432/rustassistant
   export REDIS_URL=redis://:redis123@localhost:6379
   
   # Run migrations
   cargo sqlx migrate run
   ```

3. **Build and test**
   ```bash
   # Build
   cargo build
   
   # Run tests
   cargo test
   
   # Run server
   cargo run --bin rustassistant-server
   ```

4. **Create a branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

## 📝 Code Guidelines

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting: `cargo fmt --all`
- Use `clippy` for linting: `cargo clippy --all-targets --all-features`
- Write idiomatic Rust code

### Code Quality

- **Documentation**: Add rustdoc comments for public APIs
- **Error Handling**: Use `anyhow::Result` for application errors, `thiserror` for library errors
- **Testing**: Write unit tests for new functionality
- **Async**: Use `async/await` with Tokio runtime
- **Performance**: Consider performance implications, especially for hot paths

### Example

```rust
/// Calculate the similarity score between two document chunks.
///
/// # Arguments
///
/// * `chunk_a` - First chunk to compare
/// * `chunk_b` - Second chunk to compare
///
/// # Returns
///
/// A similarity score between 0.0 and 1.0, where 1.0 indicates identical chunks.
///
/// # Examples
///
/// ```
/// let score = calculate_similarity("hello world", "hello rust");
/// assert!(score > 0.0 && score < 1.0);
/// ```
pub fn calculate_similarity(chunk_a: &str, chunk_b: &str) -> Result<f64> {
    // Implementation
}
```

## 🧪 Testing

### Running Tests

```bash
# All tests
cargo test

# Specific module
cargo test cache_layer::tests

# Integration tests
cargo test --test api_integration_tests

# With output
cargo test -- --nocapture

# With logging
RUST_LOG=debug cargo test
```

### Writing Tests

- **Unit tests**: Place in the same file as the code, in a `tests` module
- **Integration tests**: Place in `tests/` directory
- **Test coverage**: Aim for >80% coverage on new code

Example:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_creation() {
        let content = "Test content";
        let chunk = Chunk::new(content);
        assert_eq!(chunk.content, content);
    }

    #[tokio::test]
    async fn test_async_operation() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

## 📋 Commit Guidelines

### Commit Message Format

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks
- `ci`: CI/CD changes

### Examples

```bash
feat(cache): add Redis distributed caching support

Implement Redis backend for cache layer with connection pooling,
automatic failover, and pattern-based invalidation.

Closes #123
```

```bash
fix(search): correct similarity scoring algorithm

The previous implementation didn't normalize scores correctly,
leading to inconsistent results.

Fixes #456
```

## 🔀 Pull Request Process

### Before Submitting

1. **Update documentation** - Add/update docs for new features
2. **Add tests** - Ensure new code is tested
3. **Run tests** - All tests must pass
4. **Format code** - Run `cargo fmt --all`
5. **Lint code** - Run `cargo clippy` and fix warnings
6. **Update CHANGELOG** - Add entry for your changes

### Submitting

1. **Push to your fork**
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Create Pull Request**
   - Go to the original repository on GitHub
   - Click "New Pull Request"
   - Select your fork and branch
   - Fill in the PR template

3. **PR Title Format**
   ```
   feat(module): brief description of changes
   ```

4. **PR Description Template**
   ```markdown
   ## Description
   Brief description of what this PR does.
   
   ## Type of Change
   - [ ] Bug fix
   - [ ] New feature
   - [ ] Breaking change
   - [ ] Documentation update
   
   ## Testing
   Describe how you tested your changes.
   
   ## Checklist
   - [ ] Tests pass locally
   - [ ] Code follows style guidelines
   - [ ] Documentation updated
   - [ ] CHANGELOG updated
   
   ## Related Issues
   Closes #123
   ```

### Review Process

- Maintainers will review your PR
- Address feedback and requested changes
- Once approved, a maintainer will merge your PR

## 🐛 Reporting Bugs

### Before Reporting

1. **Search existing issues** - Check if already reported
2. **Try latest version** - Bug might be fixed
3. **Minimal reproduction** - Create smallest example that reproduces issue

### Bug Report Template

```markdown
## Description
Clear description of the bug.

## Steps to Reproduce
1. Step one
2. Step two
3. ...

## Expected Behavior
What should happen.

## Actual Behavior
What actually happens.

## Environment
- OS: [e.g., Ubuntu 22.04]
- Rust version: [e.g., 1.75.0]
- RustAssistant version: [e.g., 0.1.0]

## Additional Context
Logs, screenshots, etc.
```

## 💡 Feature Requests

### Feature Request Template

```markdown
## Problem
Describe the problem this feature would solve.

## Proposed Solution
How should this feature work?

## Alternatives Considered
Other solutions you've thought about.

## Additional Context
Mockups, examples, etc.
```

## 📚 Documentation

### Types of Documentation

- **Code comments** - Explain complex logic
- **Rustdoc** - API documentation
- **Guides** - User-facing tutorials
- **README** - Project overview
- **CHANGELOG** - Version history

### Writing Documentation

- **Clear and concise** - Easy to understand
- **Examples** - Show, don't just tell
- **Up-to-date** - Keep in sync with code
- **Well-organized** - Logical structure

## 🏗️ Project Structure

```
rustassistant/
├── src/                    # Source code
│   ├── api/               # REST API endpoints
│   ├── bin/               # Binary executables
│   ├── templates/         # Web UI templates
│   └── *.rs               # Core modules
├── docs/                  # Documentation
│   ├── guides/            # User guides
│   ├── archive/           # Historical docs
│   └── *.md               # Reference docs
├── examples/              # Usage examples
├── migrations/            # Database migrations
├── scripts/               # Utility scripts
├── tests/                 # Integration tests
├── config/                # Configuration files
└── docker-compose*.yml    # Docker configurations
```

## 🔍 Code Review Checklist

When reviewing PRs, check for:

- [ ] Code follows Rust conventions
- [ ] Tests are included and pass
- [ ] Documentation is updated
- [ ] No unnecessary dependencies added
- [ ] Error handling is appropriate
- [ ] Performance considerations addressed
- [ ] Security implications considered
- [ ] Breaking changes documented

## 🎨 Coding Standards

### Naming Conventions

- **Modules**: `snake_case`
- **Types**: `PascalCase`
- **Functions**: `snake_case`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Lifetimes**: `'lowercase`

### Error Handling

```rust
// Use Result for recoverable errors
pub fn parse_document(content: &str) -> Result<Document> {
    // ...
}

// Use Option for optional values
pub fn find_chunk(id: &str) -> Option<Chunk> {
    // ...
}

// Use custom error types when appropriate
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChunkError {
    #[error("chunk too large: {0} bytes")]
    TooLarge(usize),
    #[error("invalid format")]
    InvalidFormat,
}
```

### Async Code

```rust
// Use async/await
pub async fn index_document(doc: &Document) -> Result<()> {
    let chunks = chunk_document(&doc.content).await?;
    let embeddings = generate_embeddings(&chunks).await?;
    store_embeddings(embeddings).await?;
    Ok(())
}

// Use tokio for async runtime
#[tokio::main]
async fn main() -> Result<()> {
    // ...
}
```

## 📊 Performance Guidelines

- **Avoid cloning** - Use references when possible
- **Batch operations** - Process items in batches
- **Use async for I/O** - Don't block threads
- **Profile before optimizing** - Measure first
- **Cache expensive operations** - Use Redis/memory cache

## 🔐 Security Guidelines

- **Never commit secrets** - Use environment variables
- **Validate input** - Sanitize user input
- **Use parameterized queries** - Prevent SQL injection
- **Hash passwords** - Never store plain text
- **Rate limiting** - Prevent abuse
- **HTTPS only** - In production

## 📞 Getting Help

- **Documentation**: Check [docs/](docs/)
- **Issues**: Search [GitHub Issues](https://github.com/yourusername/rustassistant/issues)
- **Discussions**: Ask in [GitHub Discussions](https://github.com/yourusername/rustassistant/discussions)
- **Discord**: Join our community server (link in README)

## 🙏 Thank You!

Your contributions make RustAssistant better for everyone. We appreciate your time and effort!

---

**Questions?** Feel free to ask in [Discussions](https://github.com/yourusername/rustassistant/discussions) or open an issue.
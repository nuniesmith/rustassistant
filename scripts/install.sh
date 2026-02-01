#!/bin/bash
# Rustassistant Installation Script
# Quick setup for Phase 1 MVP

set -e

echo "🚀 Rustassistant Installation Script"
echo "================================"
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Rust is not installed"
    echo ""
    echo "Install Rust from: https://rustup.rs/"
    echo "Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "✓ Rust found: $(rustc --version)"
echo ""

# Build the CLI
echo "📦 Building Rustassistant CLI..."
cargo build --release --bin devflow

if [ $? -eq 0 ]; then
    echo "✓ Build successful!"
else
    echo "❌ Build failed. Check the error messages above."
    exit 1
fi

echo ""

# Offer to install to PATH
echo "📍 Installation Options:"
echo ""
echo "1. Install to /usr/local/bin (requires sudo)"
echo "2. Add to current shell PATH only"
echo "3. Skip installation (use ./target/release/devflow)"
echo ""
read -p "Choose option [1-3]: " choice

case $choice in
    1)
        echo ""
        echo "Installing to /usr/local/bin..."
        sudo cp target/release/devflow /usr/local/bin/
        echo "✓ Installed to /usr/local/bin/devflow"
        INSTALLED=true
        ;;
    2)
        echo ""
        SHELL_RC=""
        if [ -n "$BASH_VERSION" ]; then
            SHELL_RC="$HOME/.bashrc"
        elif [ -n "$ZSH_VERSION" ]; then
            SHELL_RC="$HOME/.zshrc"
        else
            SHELL_RC="$HOME/.profile"
        fi

        INSTALL_PATH="$(pwd)/target/release"
        echo "export PATH=\"\$PATH:$INSTALL_PATH\"" >> "$SHELL_RC"
        echo "✓ Added to $SHELL_RC"
        echo ""
        echo "Run: source $SHELL_RC"
        echo "Or restart your terminal to use 'devflow' command"
        INSTALLED=false
        ;;
    3)
        echo ""
        echo "✓ Skipping installation"
        echo "Use: ./target/release/devflow"
        INSTALLED=false
        ;;
    *)
        echo "Invalid option. Skipping installation."
        INSTALLED=false
        ;;
esac

echo ""
echo "================================"
echo "✅ Rustassistant Installation Complete!"
echo "================================"
echo ""

# Test the installation
if [ "$INSTALLED" = true ] && command -v devflow &> /dev/null; then
    echo "🎉 Quick Start:"
    echo ""
    echo "  devflow note add \"My first note\" --tags getting-started"
    echo "  devflow next"
    echo "  devflow stats"
    echo ""
    echo "📚 Documentation:"
    echo "  docs/QUICKSTART.md      - Full tutorial"
    echo "  docs/CLI_CHEATSHEET.md  - Quick reference"
    echo ""
    echo "🚀 Try it now:"
    devflow --version
else
    echo "🎉 Quick Start:"
    echo ""
    echo "  ./target/release/devflow note add \"My first note\" --tags getting-started"
    echo "  ./target/release/devflow next"
    echo "  ./target/release/devflow stats"
    echo ""
    echo "📚 Documentation:"
    echo "  docs/QUICKSTART.md      - Full tutorial"
    echo "  docs/CLI_CHEATSHEET.md  - Quick reference"
fi

echo ""
echo "💡 Next Steps:"
echo "  1. Read docs/QUICKSTART.md for a comprehensive guide"
echo "  2. Start capturing notes with 'devflow note add'"
echo "  3. Track your repositories with 'devflow repo add'"
echo ""
echo "Happy building! 🎯"

#!/bin/bash

# Modbus Stream Build and Test Script

set -e

echo "🔧 Modbus Stream - Build and Test Script"
echo "========================================"

# Check Rust installation
echo "📋 Checking Rust installation..."
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found. Please install Rust from https://rustup.rs/"
    exit 1
fi

echo "✅ Rust version: $(rustc --version)"
echo "✅ Cargo version: $(cargo --version)"

# Check for required dependencies
echo ""
echo "📋 Checking system dependencies..."

# Check for serial port libraries
if command -v pkg-config &> /dev/null; then
    if pkg-config --exists libudev; then
        echo "✅ libudev found"
    else
        echo "⚠️  libudev not found. You may need to install libudev-dev or systemd-devel"
    fi
else
    echo "⚠️  pkg-config not found. Install it for better dependency checking"
fi

# Build the project
echo ""
echo "🔨 Building project..."
cargo check
echo "✅ Cargo check passed"

cargo build
echo "✅ Debug build completed"

cargo build --release
echo "✅ Release build completed"

# Run tests
echo ""
echo "🧪 Running tests..."
cargo test
echo "✅ All tests passed"

# Check for configuration file
echo ""
echo "📋 Checking configuration..."
if [ ! -f "config.toml" ]; then
    if [ -f "config.toml.example" ]; then
        cp config.toml.example config.toml
        echo "✅ Created config.toml from example"
    else
        echo "⚠️  No configuration file found. Create config.toml manually"
    fi
else
    echo "✅ Configuration file exists"
fi

# List available serial ports (Linux/macOS)
echo ""
echo "📋 Available serial ports:"
if [ "$(uname)" = "Linux" ]; then
    ls /dev/tty* 2>/dev/null | grep -E "(USB|ACM)" || echo "   No USB serial ports found"
elif [ "$(uname)" = "Darwin" ]; then
    ls /dev/cu.* 2>/dev/null || echo "   No serial ports found"
else
    echo "   Check available COM ports on Windows"
fi

# Check web interface
echo ""
echo "📋 Checking web interface..."
if [ -d "web" ] && [ -f "web/index.html" ] && [ -f "web/app.js" ]; then
    echo "✅ Web interface files found"
else
    echo "❌ Web interface files missing"
fi

echo ""
echo "🎉 Build and test completed successfully!"
echo ""
echo "📖 Next steps:"
echo "   1. Edit config.toml with your device settings"
echo "   2. Connect your Modbus accelerometer"
echo "   3. Run: ./target/release/modbus-stream"
echo "   4. Open http://localhost:3000/health to test API"
echo "   5. Open web/index.html to use the web interface"
echo ""
echo "📚 For more information, see README.md"

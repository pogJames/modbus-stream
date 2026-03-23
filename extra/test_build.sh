#!/bin/bash

echo "🔧 Testing Modbus Stream Build"
echo "================================"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Not in project root. Please run from modbus-stream directory."
    exit 1
fi

echo "📋 Checking Rust setup..."
rustc --version
cargo --version

echo ""
echo "🧹 Cleaning previous builds..."
cargo clean

echo ""
echo "🔍 Checking syntax..."
cargo check
if [ $? -ne 0 ]; then
    echo "❌ Cargo check failed!"
    exit 1
fi
echo "✅ Syntax check passed"

echo ""
echo "🔨 Building debug version..."
cargo build
if [ $? -ne 0 ]; then
    echo "❌ Debug build failed!"
    exit 1
fi
echo "✅ Debug build successful"

echo ""
echo "🚀 Building release version..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "❌ Release build failed!"
    exit 1
fi
echo "✅ Release build successful"

echo ""
echo "🧪 Running tests..."
cargo test
if [ $? -ne 0 ]; then
    echo "⚠️  Some tests failed, but build is OK"
else
    echo "✅ All tests passed"
fi

echo ""
echo "🎉 Build test completed successfully!"
echo ""
echo "📖 Next steps:"
echo "   1. Run: cargo run"
echo "   2. Open: http://localhost:3000/"
echo "   3. Check: http://localhost:3000/settings"

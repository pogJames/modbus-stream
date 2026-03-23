#!/bin/bash

echo "🔧 Testing Modbus Stream Compilation and Startup"
echo "================================================"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Not in modbus-stream directory"
    echo "Please run this script from the modbus-stream project root"
    exit 1
fi

echo "📋 Checking Rust environment..."
rustc --version
cargo --version

echo ""
echo "🔍 Running cargo check..."
cargo check

if [ $? -ne 0 ]; then
    echo "❌ Cargo check failed"
    exit 1
fi

echo ""
echo "🔨 Building project..."
cargo build

if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

echo ""
echo "✅ Build successful!"
echo ""
echo "🚀 Starting server (will run for 10 seconds for testing)..."
echo "📝 Check the logs for any template errors"
echo ""

# Start the server in background and capture its PID
timeout 10s cargo run &
SERVER_PID=$!

# Wait a moment for startup
sleep 2

echo "🌐 Testing endpoints..."

# Test health endpoint
echo "Testing /health..."
curl -s http://localhost:3000/health > /dev/null && echo "✅ Health endpoint working" || echo "❌ Health endpoint failed"

# Test settings endpoint (this will show template errors if any)
echo "Testing /settings..."
curl -s http://localhost:3000/settings > /dev/null && echo "✅ Settings page working" || echo "❌ Settings page failed"

# Test diagnostics
echo "Testing /diagnostics..."
curl -s http://localhost:3000/diagnostics > /dev/null && echo "✅ Diagnostics working" || echo "❌ Diagnostics failed"

# Wait for the timeout to finish
wait $SERVER_PID

echo ""
echo "🎉 Test completed!"
echo ""
echo "If you saw template errors above, check the console output."
echo "If all endpoints returned ✅, the server is working correctly!"

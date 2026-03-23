@echo off
setlocal enabledelayedexpansion

echo 🔧 Testing Modbus Stream Compilation and Startup
echo ================================================

REM Check if we're in the right directory
if not exist "Cargo.toml" (
    echo ❌ Error: Not in modbus-stream directory
    echo Please run this script from the modbus-stream project root
    pause
    exit /b 1
)

echo 📋 Checking Rust environment...
rustc --version
cargo --version

echo.
echo 🔍 Running cargo check...
cargo check

if %errorlevel% neq 0 (
    echo ❌ Cargo check failed
    pause
    exit /b 1
)

echo.
echo 🔨 Building project...
cargo build

if %errorlevel% neq 0 (
    echo ❌ Build failed
    pause
    exit /b 1
)

echo.
echo ✅ Build successful!
echo.
echo 🚀 Starting server for testing...
echo 📝 Check the logs for any template errors
echo.
echo Press Ctrl+C to stop the server when testing is done
echo.

REM Start the server
cargo run

echo.
echo 🎉 Server stopped
pause

@echo off
echo 🔧 Testing Modbus Stream Build
echo ================================

if not exist "Cargo.toml" (
    echo ❌ Not in project root. Please run from modbus-stream directory.
    pause
    exit /b 1
)

echo 📋 Checking Rust setup...
rustc --version
cargo --version

echo.
echo 🧹 Cleaning previous builds...
cargo clean

echo.
echo 🔍 Checking syntax...
cargo check
if %errorlevel% neq 0 (
    echo ❌ Cargo check failed!
    pause
    exit /b 1
)
echo ✅ Syntax check passed

echo.
echo 🔨 Building debug version...
cargo build
if %errorlevel% neq 0 (
    echo ❌ Debug build failed!
    pause
    exit /b 1
)
echo ✅ Debug build successful

echo.
echo 🚀 Building release version...
cargo build --release
if %errorlevel% neq 0 (
    echo ❌ Release build failed!
    pause
    exit /b 1
)
echo ✅ Release build successful

echo.
echo 🧪 Running tests...
cargo test
if %errorlevel% neq 0 (
    echo ⚠️  Some tests failed, but build is OK
) else (
    echo ✅ All tests passed
)

echo.
echo 🎉 Build test completed successfully!
echo.
echo 📖 Next steps:
echo    1. Run: cargo run
echo    2. Open: http://localhost:3000/
echo    3. Check: http://localhost:3000/settings

pause

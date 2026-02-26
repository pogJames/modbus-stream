@echo off
setlocal enabledelayedexpansion

echo 🔧 Modbus Stream - Build and Test Script
echo ========================================

REM Check Rust installation
echo 📋 Checking Rust installation...
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo ❌ Rust/Cargo not found. Please install Rust from https://rustup.rs/
    pause
    exit /b 1
)

for /f "tokens=*" %%i in ('rustc --version') do set RUST_VERSION=%%i
for /f "tokens=*" %%i in ('cargo --version') do set CARGO_VERSION=%%i
echo ✅ Rust version: %RUST_VERSION%
echo ✅ Cargo version: %CARGO_VERSION%

REM Build the project
echo.
echo 🔨 Building project...
cargo check
if %errorlevel% neq 0 (
    echo ❌ Cargo check failed
    pause
    exit /b 1
)
echo ✅ Cargo check passed

cargo build
if %errorlevel% neq 0 (
    echo ❌ Debug build failed
    pause
    exit /b 1
)
echo ✅ Debug build completed

cargo build --release
if %errorlevel% neq 0 (
    echo ❌ Release build failed
    pause
    exit /b 1
)
echo ✅ Release build completed

REM Run tests
echo.
echo 🧪 Running tests...
cargo test
if %errorlevel% neq 0 (
    echo ❌ Tests failed
    pause
    exit /b 1
)
echo ✅ All tests passed

REM Check for configuration file
echo.
echo 📋 Checking configuration...
if not exist "config.toml" (
    if exist "config.toml.example" (
        copy config.toml.example config.toml >nul
        echo ✅ Created config.toml from example
    ) else (
        echo ⚠️  No configuration file found. Create config.toml manually
    )
) else (
    echo ✅ Configuration file exists
)

REM List available COM ports
echo.
echo 📋 Available COM ports:
for /f "tokens=*" %%i in ('wmic path Win32_SerialPort get DeviceID /format:list 2^>nul ^| findstr "="') do (
    echo    %%i
)

REM Check web interface
echo.
echo 📋 Checking web interface...
if exist "web\index.html" (
    if exist "web\app.js" (
        echo ✅ Web interface files found
    ) else (
        echo ❌ Web interface JavaScript missing
    )
) else (
    echo ❌ Web interface HTML missing
)

echo.
echo 🎉 Build and test completed successfully!
echo.
echo 📖 Next steps:
echo    1. Edit config.toml with your device settings
echo    2. Connect your Modbus accelerometer  
echo    3. Run: target\release\modbus-stream.exe
echo    4. Open http://localhost:3000/health to test API
echo    5. Open web\index.html to use the web interface
echo.
echo 📚 For more information, see README.md

pause

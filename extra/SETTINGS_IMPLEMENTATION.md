# Settings Page Implementation

This implementation adds a complete `/settings` web interface for configuring the Modbus tri-axial accelerometer sensor.

## 🚀 Quick Start

1. **Build and run the server:**
   ```bash
   cd modbus-stream
   cargo run
   ```

2. **Open the web interface:**
   - Dashboard: http://localhost:3000/
   - Settings: http://localhost:3000/settings
   - Health: http://localhost:3000/health

## 📁 Files Added/Modified

### Templates
- `templates/base.html` - Base template with navigation and HTMX
- `templates/dashboard.html` - Main dashboard with links to settings
- `templates/settings.html` - Main settings page layout
- `templates/settings/form.html` - Configuration form with all sensor settings
- `templates/settings/status-header.html` - Connection status display
- `templates/settings/feedback.html` - Success/error feedback messages

### Styles
- `static/css/style.css` - Complete CSS styling for responsive UI

### Backend
- `src/routes/settings.rs` - Settings route handlers and validation
- `src/types.rs` - Added settings data structures
- `src/main.rs` - Updated with template support and settings routes
- `config.toml` - Example configuration file

## 🎯 Features Implemented

### ✅ Settings Configuration
- **Connection Settings**: Device path, baud rate, slave ID, timeout, retries
- **Sensor Settings**: Sample rate, stream size, high pass filter
- **Streaming Settings**: Max connections, buffer size, metrics rate, ping interval

### ✅ Validation
- **Client-side**: HTML5 form validation with helpful constraints
- **Server-side**: Comprehensive validation with business rules
- **Field-specific errors**: Clear feedback for each form field
- **Business rules**: E.g., high sample rates require 3 Mbps baud rate

### ✅ HTMX Integration
- **Form submission**: No page reload with real-time feedback
- **Connection testing**: Test settings before applying
- **Status polling**: Auto-refresh connection status every 5 seconds
- **Progressive enhancement**: Works without JavaScript as fallback

### ✅ User Experience
- **Responsive design**: Works on desktop and mobile
- **Loading indicators**: Visual feedback during operations
- **Error handling**: Clear error messages and recovery options
- **Success feedback**: Detailed confirmation of applied changes

## 🔧 Configuration Options

### Connection Settings
```toml
[modbus]
device = "/dev/ttyUSB0"     # Serial device path
baud_rate = 115200          # 115200 or 3000000
slave_id = 1                # Modbus slave ID (1-247)
timeout_ms = 5000           # Communication timeout
retry_attempts = 3          # Number of retries
```

### Sensor Settings
- **Sample Rate**: 1-10000 sps (common: 1600, 6400, 7812)
- **Stream Size**: 1-123 registers per read
- **High Pass Filter**: Enable/disable (3-2.5 kHz bandwidth)

### Streaming Settings
- **Max Connections**: 1-50 concurrent WebSocket clients
- **Buffer Size**: 256-8192 bytes
- **Metrics Rate**: 0.1-5.0 Hz (sensor limited to 5 Hz max)
- **WebSocket Ping**: 10-300 seconds

## 🌐 API Endpoints

### Settings Routes
- `GET /settings` - Main settings page
- `POST /settings/apply` - Apply configuration changes
- `POST /settings/test` - Test connection with current settings
- `GET /settings/status` - Get current status (HTMX polling)
- `POST /settings/reset` - Reset to default settings

### Other Routes
- `GET /` - Dashboard with navigation links
- `GET /health` - API health check
- `GET /diagnostics` - System diagnostics
- `POST /static/*` - Static file serving

## ⚠️ Important Notes

### Baud Rate Changes
- Changing baud rate requires **sensor power cycle**
- Connection will be temporarily lost during reconfiguration
- UI warns users about this requirement

### Sample Rate Limitations
- High sample rates (>1000 sps) require 3 Mbps baud rate
- Continuous streaming only available at 3 Mbps
- UI validates these constraints

### Error Recovery
- Form validation prevents invalid configurations
- Connection test allows verification before applying
- Rollback to previous settings on critical failures

## 🎨 UI Components

### Status Indicators
- **Green dot + "Connected"**: Sensor responding normally
- **Yellow dot + "Connecting"**: Attempting connection
- **Red dot + "Disconnected"**: No sensor connection
- **Red dot + "Error"**: Communication error

### Form Sections
1. **Connection Configuration**: Device path, baud rate, slave ID
2. **Sensor Configuration**: Sample rate, stream size, filters
3. **Streaming Configuration**: WebSocket and buffer settings

### Feedback Types
- **Success**: Green background, checkmark icon
- **Error**: Red background, X icon, field-specific errors
- **Warning**: Yellow background, warning icon
- **Info**: Blue background, info icon

## 🔄 HTMX Interactions

### Form Submission
```html
<form hx-post="/settings/apply" 
      hx-target="#feedback-area" 
      hx-swap="innerHTML">
```

### Connection Testing
```html
<button hx-post="/settings/test"
        hx-include="[name]"
        hx-target="#test-result">
```

### Status Polling
```html
<div hx-get="/settings/status" 
     hx-trigger="every 5s">
```

## 🚧 Development Notes

### Template Engine
- Uses `minijinja` (Jinja2-like templating for Rust)
- Hot reloading with `minijinja-autoreload` during development
- Template inheritance with `{% extends %}` and `{% block %}`

### Form Handling
- Parses HTML form data into structured `SettingsForm`
- Comprehensive validation with helpful error messages
- Supports both JSON and form-encoded submissions

### Future Enhancements
- [ ] Real-time configuration persistence to file
- [ ] Backup/restore configuration profiles
- [ ] Advanced sensor calibration settings
- [ ] Historical configuration change log
- [ ] Multi-sensor support

This implementation provides a production-ready settings interface that integrates seamlessly with the existing Modbus stream architecture while following modern web development best practices.

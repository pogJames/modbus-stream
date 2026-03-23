# Settings Page Testing Guide

## Quick Start

1. **Build and run the server:**
   ```bash
   cd modbus-stream
   cargo run
   ```

2. **Open the settings page:**
   ```
   http://localhost:3000/settings
   ```

## Features Implemented

### ✅ **Core Functionality**
- **Main settings page** with modern minimalistic design
- **HTMX-powered** dynamic interactions without page reloads
- **Real-time validation** with immediate feedback
- **Test connection** before applying changes
- **Auto-refreshing status** header (every 5 seconds)
- **Responsive design** that works on mobile and desktop

### ✅ **Settings Sections**
1. **Connection Configuration**
   - Device path selection with auto-detection
   - Baud rate selection (115200 vs 3 Mbps)
   - Slave ID, timeout, and retry settings

2. **Sensor Configuration**
   - Sample rate with preset buttons (800, 1600, 6400, 7812)
   - Stream size configuration
   - High pass filter toggle

3. **Streaming Configuration**
   - Max WebSocket connections
   - Buffer size settings
   - Metrics update rate
   - WebSocket ping interval

### ✅ **Dynamic Features**
- **Port auto-detection** (Windows COM ports, Linux /dev/tty*, macOS cu.*)
- **Live validation** warns about incompatible settings
- **Test connection** validates settings without applying
- **Preset buttons** for common sample rates
- **Custom device path** input with smooth transitions

## API Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| `GET` | `/settings` | Main settings page |
| `GET` | `/settings/status` | Connection status (auto-refresh) |
| `GET` | `/settings/ports` | Available serial ports |
| `POST` | `/settings/test` | Test connection |
| `POST` | `/settings/apply` | Apply settings |
| `POST` | `/settings/validate` | Live validation |
| `POST` | `/settings/reset` | Reset to defaults |

## Testing Scenarios

### **1. Normal Operation**
```bash
# Start server
cargo run

# Open browser
http://localhost:3000/settings

# Expected: Clean settings page loads with current configuration
```

### **2. Port Auto-Detection**
- Click on device path dropdown
- Should populate with available ports automatically
- Select "Enter manually..." to show custom input

### **3. Live Validation**
- Set sample rate to 8000
- Select 115200 bps baud rate
- Should show warning about incompatible settings

### **4. Test Connection**
- Configure valid settings
- Click "Test Connection"
- Should show connection result without applying changes

### **5. Apply Settings**
- Make configuration changes
- Click "Apply Settings"
- Should save configuration and restart connections

### **6. Reset to Defaults**
- Click "Reset to Defaults"
- Should restore original configuration

## UI Features

### **Modern Design Elements**
- Clean, minimalistic interface
- CSS variables for consistent theming
- Smooth transitions and hover effects
- Responsive grid layouts
- Professional color scheme

### **HTMX Interactions**
- **No JavaScript required** for core functionality
- **Partial page updates** for dynamic content
- **Loading indicators** during operations
- **Error handling** with contextual feedback

### **Accessibility**
- **Keyboard navigation** support
- **Focus indicators** for interactive elements
- **Screen reader friendly** semantic HTML
- **High contrast** color combinations

## Troubleshooting

### **Common Issues**

1. **Template not found errors**
   - Ensure `templates/` directory exists
   - Check template file paths in code

2. **CSS not loading**
   - Verify `static/css/style.css` exists
   - Check static file serving configuration

3. **Port detection not working**
   - Platform-specific implementation
   - May need actual hardware for full testing

4. **HTMX interactions not working**
   - Check network tab for API calls
   - Verify endpoint routing in main.rs

### **Development Tips**

1. **Hot reload templates:**
   ```bash
   RUST_LOG=debug cargo run
   # Templates reload automatically during development
   ```

2. **Check logs:**
   ```bash
   # Enable debug logging
   RUST_LOG=modbus_stream=debug cargo run
   ```

3. **Test without hardware:**
   - Settings page works without sensor connected
   - Connection tests will show "device not connected"

## Next Steps

### **Phase 1: Core Testing**
- [ ] Verify all routes work correctly
- [ ] Test form validation
- [ ] Confirm HTMX interactions
- [ ] Check responsive design

### **Phase 2: Hardware Integration**
- [ ] Test with actual sensor hardware
- [ ] Verify Modbus communication
- [ ] Test settings persistence
- [ ] Validate streaming functionality

### **Phase 3: Production Deployment**
- [ ] Performance optimization
- [ ] Error logging
- [ ] Security hardening
- [ ] Documentation completion

This implementation provides a solid foundation for the settings interface with modern web development practices and industrial-grade reliability.

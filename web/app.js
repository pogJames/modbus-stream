// Modbus Stream Web Interface JavaScript

class ModbusStreamClient {
    constructor() {
        this.apiUrl = 'http://localhost:3000';
        this.wsUrl = 'ws://localhost:3000';
        this.connected = false;
        this.streaming = false;
        this.rawSocket = null;
        this.metricsSocket = null;
        this.lastUpdate = 0;
        this.updateCount = 0;
        
        this.init();
    }

    init() {
        this.log('Web interface initialized', 'info');
        this.updateConnectionStatus(false);
        
        // Try to connect on load
        setTimeout(() => this.connectToSensor(), 1000);
        
        // Auto-refresh every 30 seconds
        setInterval(() => {
            if (this.connected && !this.streaming) {
                this.readLatestData();
            }
        }, 30000);
    }

    log(message, level = 'info') {
        const logContainer = document.getElementById('log-container');
        const timestamp = new Date().toLocaleTimeString();
        const logEntry = document.createElement('div');
        logEntry.className = 'log-entry';
        
        const levelClass = {
            'info': 'log-level-info',
            'error': 'log-level-error',
            'success': 'log-level-success'
        }[level] || 'log-level-info';
        
        logEntry.innerHTML = `
            <span class="log-timestamp">[${timestamp}]</span>
            <span class="${levelClass}">${level.toUpperCase()}</span>
            ${message}
        `;
        
        logContainer.insertBefore(logEntry, logContainer.firstChild);
        
        // Keep only last 50 entries
        while (logContainer.children.length > 50) {
            logContainer.removeChild(logContainer.lastChild);
        }
    }

    updateConnectionStatus(connected) {
        this.connected = connected;
        const statusDot = document.getElementById('status-dot');
        const statusText = document.getElementById('connection-status');
        
        if (connected) {
            statusDot.classList.add('connected');
            statusText.textContent = 'Connected';
        } else {
            statusDot.classList.remove('connected');
            statusText.textContent = 'Disconnected';
        }
    }

    async apiCall(endpoint, options = {}) {
        try {
            const response = await fetch(`${this.apiUrl}${endpoint}`, {
                ...options,
                headers: {
                    'Content-Type': 'application/json',
                    ...options.headers
                }
            });

            if (!response.ok) {
                const errorData = await response.json().catch(() => ({}));
                throw new Error(errorData.error || `HTTP ${response.status}: ${response.statusText}`);
            }

            return await response.json();
        } catch (error) {
            this.log(`API Error (${endpoint}): ${error.message}`, 'error');
            throw error;
        }
    }

    async connectToSensor() {
        try {
            this.log('Connecting to sensor...', 'info');
            const health = await this.apiCall('/health');
            
            if (health.status === 'healthy') {
                this.updateConnectionStatus(true);
                this.log('Connected to Modbus sensor successfully', 'success');
                
                // Load initial data
                await this.loadDeviceInfo();
                await this.readLatestData();
                
            } else {
                throw new Error('Service not healthy');
            }
        } catch (error) {
            this.updateConnectionStatus(false);
            this.log(`Connection failed: ${error.message}`, 'error');
        }
    }

    async loadDeviceInfo() {
        try {
            // Get UCID info
            const ucid = await this.apiCall('/read/ucid');
            document.getElementById('model').textContent = ucid.model;
            document.getElementById('gain').textContent = ucid.gain;
            document.getElementById('serial').textContent = ucid.serialNumber;

            // Get firmware version
            const firmware = await this.apiCall('/read/firmware-version');
            document.getElementById('firmware').textContent = firmware.firmwareVersion;

            // Get temperature
            const temp = await this.apiCall('/read/temperature');
            document.getElementById('temperature').textContent = `${temp.temperature.toFixed(1)}°C`;

            this.log('Device information loaded', 'success');
        } catch (error) {
            this.log(`Failed to load device info: ${error.message}`, 'error');
        }
    }

    async readLatestData() {
        try {
            // Read latest raw data
            const latestRaw = await this.apiCall('/read/latest-raw');
            document.getElementById('raw-x').textContent = latestRaw.x.toFixed(4);
            document.getElementById('raw-y').textContent = latestRaw.y.toFixed(4);
            document.getElementById('raw-z').textContent = latestRaw.z.toFixed(4);

            // Calculate update rate
            const now = Date.now();
            if (this.lastUpdate > 0) {
                const rate = 1000 / (now - this.lastUpdate);
                document.getElementById('raw-rate').textContent = `${rate.toFixed(1)} Hz`;
            }
            this.lastUpdate = now;

        } catch (error) {
            this.log(`Failed to read latest data: ${error.message}`, 'error');
        }
    }

    async readAllMetrics() {
        try {
            this.log('Reading all metrics...', 'info');
            const metrics = await this.apiCall('/read/all-metrics');

            // Update gravity metrics
            const gravity = metrics.gravity;
            document.getElementById('gravity-rms-x').textContent = gravity.rms.x.toFixed(3);
            document.getElementById('gravity-rms-y').textContent = gravity.rms.y.toFixed(3);
            document.getElementById('gravity-rms-z').textContent = gravity.rms.z.toFixed(3);
            
            document.getElementById('gravity-peak-x').textContent = gravity.peak.x.toFixed(3);
            document.getElementById('gravity-peak-y').textContent = gravity.peak.y.toFixed(3);
            document.getElementById('gravity-peak-z').textContent = gravity.peak.z.toFixed(3);
            
            document.getElementById('gravity-freq').textContent = `${gravity.primaryFrequency.toFixed(1)} Hz`;

            // Update velocity metrics
            const velocity = metrics.velocity;
            document.getElementById('velocity-rms-x').textContent = velocity.rms.x.toFixed(2);
            document.getElementById('velocity-rms-y').textContent = velocity.rms.y.toFixed(2);
            document.getElementById('velocity-rms-z').textContent = velocity.rms.z.toFixed(2);
            
            document.getElementById('velocity-peak-x').textContent = velocity.peak.x.toFixed(2);
            document.getElementById('velocity-peak-y').textContent = velocity.peak.y.toFixed(2);
            document.getElementById('velocity-peak-z').textContent = velocity.peak.z.toFixed(2);
            
            document.getElementById('velocity-freq').textContent = `${velocity.primaryFrequency.toFixed(1)} Hz`;

            this.log('All metrics updated successfully', 'success');
        } catch (error) {
            this.log(`Failed to read metrics: ${error.message}`, 'error');
        }
    }

    async getDiagnostics() {
        try {
            this.log('Getting system diagnostics...', 'info');
            const diagnostics = await this.apiCall('/diagnostics');
            
            // Update baud rate display
            document.getElementById('current-baud').textContent = `${diagnostics.config.baudRate} bps`;
            
            // Log diagnostics summary
            this.log(`System: ${diagnostics.system.os}/${diagnostics.system.arch}`, 'info');
            this.log(`Connection: ${diagnostics.connection.status}`, 
                diagnostics.connection.connected ? 'success' : 'error');
            this.log(`Streaming capability: ${diagnostics.streaming.capability}`, 'info');
            
            if (diagnostics.sensor) {
                this.log(`Sensor temperature: ${diagnostics.sensor.temperature.status}`, 
                    diagnostics.sensor.temperature.status === 'ok' ? 'success' : 'error');
            }
            
            this.log('Diagnostics completed', 'success');
        } catch (error) {
            this.log(`Diagnostics failed: ${error.message}`, 'error');
        }
    }

    async setSampleRate() {
        try {
            const sampleRate = parseInt(document.getElementById('sample-rate').value);
            if (isNaN(sampleRate) || sampleRate < 1 || sampleRate > 10000) {
                throw new Error('Invalid sample rate. Use 1-10000 sps.');
            }

            this.log(`Setting sample rate to ${sampleRate} sps...`, 'info');
            await this.apiCall('/config/sample-rate', {
                method: 'PUT',
                body: JSON.stringify({ sampleRate })
            });

            this.log(`Sample rate set to ${sampleRate} sps`, 'success');
        } catch (error) {
            this.log(`Failed to set sample rate: ${error.message}`, 'error');
        }
    }

    async toggleHighPassFilter() {
        try {
            // For demo purposes, just toggle enabled/disabled
            const enabled = Math.random() > 0.5; // Random for demo
            
            this.log(`${enabled ? 'Enabling' : 'Disabling'} high pass filter...`, 'info');
            await this.apiCall('/config/high-pass-filter', {
                method: 'PUT',
                body: JSON.stringify({ enabled })
            });

            this.log(`High pass filter ${enabled ? 'enabled' : 'disabled'}`, 'success');
        } catch (error) {
            this.log(`Failed to toggle high pass filter: ${error.message}`, 'error');
        }
    }

    async startStreaming() {
        if (this.streaming) {
            this.log('Streaming already active', 'info');
            return;
        }

        try {
            this.log('Starting streaming...', 'info');
            
            // Start metrics streaming
            this.metricsSocket = new WebSocket(`${this.wsUrl}/stream/metrics`);
            
            this.metricsSocket.onopen = () => {
                this.streaming = true;
                this.log('Metrics streaming started', 'success');
            };

            this.metricsSocket.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    this.handleStreamingData(data);
                } catch (error) {
                    this.log(`Failed to parse streaming data: ${error.message}`, 'error');
                }
            };

            this.metricsSocket.onerror = (error) => {
                this.log('WebSocket error: ' + error, 'error');
            };

            this.metricsSocket.onclose = () => {
                this.streaming = false;
                this.log('Metrics streaming stopped', 'info');
            };

            // Also try raw data streaming
            this.rawSocket = new WebSocket(`${this.wsUrl}/stream/raw`);
            
            this.rawSocket.onopen = () => {
                this.log('Raw data streaming started', 'success');
            };

            this.rawSocket.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    this.handleStreamingData(data);
                } catch (error) {
                    this.log(`Failed to parse raw data: ${error.message}`, 'error');
                }
            };

            this.rawSocket.onerror = (error) => {
                this.log('Raw data WebSocket error', 'error');
            };

        } catch (error) {
            this.log(`Failed to start streaming: ${error.message}`, 'error');
        }
    }

    handleStreamingData(data) {
        switch (data.type) {
            case 'metrics':
                this.updateMetricsFromStream(data);
                break;
            case 'raw':
                this.updateRawDataFromStream(data);
                break;
            case 'status':
                this.log(`Stream status: connected=${data.connected}, streaming=${data.streaming}`, 'info');
                break;
            case 'error':
                this.log(`Stream error: ${data.message}`, 'error');
                break;
            default:
                console.log('Unknown stream data type:', data.type);
        }
    }

    updateMetricsFromStream(data) {
        // Update gravity metrics
        const gravity = data.gravity;
        document.getElementById('gravity-rms-x').textContent = gravity.rms.x.toFixed(3);
        document.getElementById('gravity-rms-y').textContent = gravity.rms.y.toFixed(3);
        document.getElementById('gravity-rms-z').textContent = gravity.rms.z.toFixed(3);
        
        document.getElementById('gravity-peak-x').textContent = gravity.peak.x.toFixed(3);
        document.getElementById('gravity-peak-y').textContent = gravity.peak.y.toFixed(3);
        document.getElementById('gravity-peak-z').textContent = gravity.peak.z.toFixed(3);
        
        document.getElementById('gravity-freq').textContent = `${gravity.primaryFrequency.toFixed(1)} Hz`;

        // Update velocity metrics
        const velocity = data.velocity;
        document.getElementById('velocity-rms-x').textContent = velocity.rms.x.toFixed(2);
        document.getElementById('velocity-rms-y').textContent = velocity.rms.y.toFixed(2);
        document.getElementById('velocity-rms-z').textContent = velocity.rms.z.toFixed(2);
        
        document.getElementById('velocity-peak-x').textContent = velocity.peak.x.toFixed(2);
        document.getElementById('velocity-peak-y').textContent = velocity.peak.y.toFixed(2);
        document.getElementById('velocity-peak-z').textContent = velocity.peak.z.toFixed(2);
        
        document.getElementById('velocity-freq').textContent = `${velocity.primaryFrequency.toFixed(1)} Hz`;

        // Update temperature
        document.getElementById('temperature').textContent = `${data.temperature.toFixed(1)}°C`;
        
        this.updateCount++;
        if (this.updateCount % 10 === 0) {
            this.log(`Received ${this.updateCount} metric updates`, 'info');
        }
    }

    updateRawDataFromStream(data) {
        if (data.data && data.data.length > 0) {
            const latest = data.data[data.data.length - 1];
            document.getElementById('raw-x').textContent = latest.x.toFixed(4);
            document.getElementById('raw-y').textContent = latest.y.toFixed(4);
            document.getElementById('raw-z').textContent = latest.z.toFixed(4);

            // Calculate update rate
            const now = Date.now();
            if (this.lastUpdate > 0) {
                const rate = 1000 / (now - this.lastUpdate);
                document.getElementById('raw-rate').textContent = `${rate.toFixed(1)} Hz`;
            }
            this.lastUpdate = now;
        }
    }

    async stopStreaming() {
        if (!this.streaming) {
            this.log('No active streaming to stop', 'info');
            return;
        }

        this.log('Stopping streaming...', 'info');

        if (this.rawSocket) {
            this.rawSocket.close();
            this.rawSocket = null;
        }

        if (this.metricsSocket) {
            this.metricsSocket.close();
            this.metricsSocket = null;
        }

        this.streaming = false;
        this.log('Streaming stopped', 'success');
    }
}

// Global functions for HTML onclick handlers
let client;

function connectToSensor() {
    client.connectToSensor();
}

function startStreaming() {
    client.startStreaming();
}

function stopStreaming() {
    client.stopStreaming();
}

function readAllMetrics() {
    client.readAllMetrics();
}

function getDiagnostics() {
    client.getDiagnostics();
}

function setSampleRate() {
    client.setSampleRate();
}

function toggleHighPassFilter() {
    client.toggleHighPassFilter();
}

// Initialize when page loads
document.addEventListener('DOMContentLoaded', () => {
    client = new ModbusStreamClient();
});

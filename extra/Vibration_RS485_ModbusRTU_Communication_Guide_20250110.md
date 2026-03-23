# Vibration RS485 Modbus RTU Communication Guide

This document outlines the communication protocol for a tri-axial accelerometer with RS485 Modbus RTU interface, detailing settings, register map, and command formats.

## Communication Settings

The following table specifies the RS485 communication parameters for the vibration sensor:

| Property | Value |
|----------|-------|
| Baud Rate (bps) | 115200 or 3 Mbps (default) |
| Data Bits | 8 |
| Stop Bits | 1 |
| Parity | None |
| Supported Function Codes | Read Holding Registers (FC03)<br>Read Input Registers (FC04)<br>Write Single Register (FC06) |
| Slave ID | 0x0001 |

**Endian Type**: Big Endian

## Register Map and Commands

The following table lists the Modbus RTU commands and corresponding registers for interacting with the sensor. Notes indicate specific function codes and data handling requirements.

| Command | Register | Comment |
|---------|----------|---------|
| **Supported Baud Rate** | - | Default: 3 Mbps; Alternative: 115200 bps. Continuous raw data streaming supported only at 3 Mbps. |
| Raw Data FIFO Buffer Size | 0x0002 | Read with FC04. |
| Stream Size | 0x0015 | Configured with FC06 for bulk transfer mode. |
| Raw Data (XYZ) | 0x0003–0x007D | Read with FC04, up to 123 registers. |
| Raw Data (XYZ) Latest | 0x0083–0x0085 | Read with FC04, 3 registers for latest X, Y, Z values. |
| Sample Rate Change | 0x0001 | Write with FC06. Example: 0x0640 for 1600 sps. |
| Temperature | 0x0014 | Read with FC03. Value (°C) = Register value / 100. Max update rate: 5 Hz. |
| Baud Rate (High) | 0x0017 | Write with FC06. Set High and Low registers to change baud rate. |
| Baud Rate (Low) | 0x0018 | Write with FC06. Set after High register, then power cycle sensor. |
| High Pass Enable | 0x001C | Write with FC06. 1: Enable, 0: Disable. Bandwidth: 3–2.5 kHz @ 7812 sps (2 kHz for K-type). |
| UCID | 0x001B | Read with FC03. Contains model, gain, and serial number (see UCID format below). |
| Firmware Version | 0x001D | Read with FC03. |
| Chip ID | 0x0080 | Read with FC04, 3 registers. |
| RMS (Gravity) | 0x001E | Read with FC03, 3 registers. Value = Register value / 1000. Max update rate: 5 Hz. |
| Peak (Gravity) | 0x001F | Read with FC03, 3 registers. Value = Register value / 1000. Max update rate: 5 Hz. |
| Crest Factor (Gravity) | 0x0020 | Read with FC03, 3 registers. Value = Register value / 1000. Max update rate: 5 Hz. |
| Skewness (Gravity) | 0x0021 | Read with FC03, 3 registers. Value = Register value / 1000. Update period: 2–5 seconds. |
| Kurtosis (Gravity) | 0x0022 | Read with FC03, 3 registers. Value = Register value / 1000. Update period: 2–5 seconds. |
| Primary Frequency (Gravity) | 0x003D | Read with FC03. |
| RMS (Velocity) | 0x0032 | Read with FC03, 3 registers. Value = Register value / 100. Max update rate: 5 Hz. |
| Peak (Velocity) | 0x0033 | Read with FC03, 3 registers. Value = Register value / 100. Max update rate: 5 Hz. |
| Crest Factor (Velocity) | 0x0034 | Read with FC03, 3 registers. Value = Register value / 100. Max update rate: 5 Hz. |
| Primary Frequency (Velocity) | 0x003C | Read with FC03. |

**Notes**:
- Default sample rate: 7812 Hz (I-type) or 6400 Hz (K-type).
- Registers marked with *1 support only FC06 (Write Single Register).
- Registers marked with *2 support only FC04 (Read Input Registers).
- Registers marked with *3 support only FC03 (Read Holding Registers).
- For baud rate changes, set High (0x0017) and Low (0x0018) registers, then power cycle the sensor.

## UCID Format

The UCID (register 0x001B, read with FC03) is a 32-bit value containing model, gain, and serial number:

| Bits | Description |
|------|-------------|
| b31–b28 | Model: 0 (12B), 1 (15B), 2 (KAX301), 3 (KAX302), 4 (S6S), 5–15 (Reserved) |
| b27–b24 | Gain: 0 (4G), 1 (2G), 2 (8G), 3 (16G), 4 (32G), 5 (64G), 6–15 (Reserved) |
| b23–b0 | Serial Number: 00000–99999 |

## Statistical Formulas

The sensor calculates statistical metrics for vibration analysis. The following formulas are used (Page 10):

| Moment Number | Name | Measure of | Formula |
|---------------|------|------------|---------|
| 1 | Mean | Central Tendency | $\bar{X} = \frac{\sum_{i=1}^N X_i}{N}$ |
| 2 | Variance | Dispersion | $\sigma^2 = \frac{\sum_{i=1}^N (X_i - \bar{X})^2}{N}$ |
| 3 | Skewness | Symmetry (Positive or Negative) | $\text{Skew} = \frac{1}{N} \sum_{i=1}^N \left[ \frac{(X_i - \bar{X})}{\sigma} \right]^3$ |
| 4 | Kurtosis | Shape (Tall or Flat) | $\text{Kurt} = \frac{1}{N} \sum_{i=1}^N \left[ \frac{(X_i - \bar{X})}{\sigma} \right]^4$ |

## Modbus Command Examples

### 1. Sample Rate Change
Set the sample rate to 1600 sps (Page 4).

**Request**:
- Slave ID: 0x01
- Function Code: 0x06 (Write Single Register)
- Register Address: 0x0001
- Data: 0x0640 (1600 sps)
- CRC: 0x5ADA

**Hex Format**: `0x01 0x06 0x00 0x01 0x06 0x40 0xDA 0x5A`

**Byte Breakdown**:
| Byte | Description |
|------|-------------|
| 0x01 | Slave ID |
| 0x06 | Function Code 06 (Write Single Register) |
| 0x0001 | Register Address |
| 0x0640 | Data (1600 sps) |
| 0x5ADA | CRC |

**Note**: Send this command before reading data to initialize streaming.

### 2. Read Temperature
Read the temperature from register 0x0014 (Page 4).

**Request**:
- Slave ID: 0x01
- Function Code: 0x03 (Read Holding Registers)
- Register Address: 0x0014
- Data Length: 0x01
- CRC: 0x0EC4

**Hex Format**: `0x01 0x03 0x00 0x14 0x00 0x01 0x0E 0xC4`

**Byte Breakdown**:
| Byte | Description |
|------|-------------|
| 0x01 | Slave ID |
| 0x03 | Function Code 03 (Read Holding Registers) |
| 0x0014 | Register Address |
| 0x0001 | Data Length (1 register) |
| 0x0EC4 | CRC |

**Response**: Temperature (°C) = Register value / 100.

### 3. Read Raw Data FIFO Buffer Size
Read the FIFO buffer size from register 0x0002 (Page 5).

**Request**:
- Slave ID: 0x01
- Function Code: 0x04 (Read Input Registers)
- Register Address: 0x0002
- Data Length: 0x01
- CRC: 0x0A90

**Hex Format**: `0x01 0x04 0x00 0x02 0x00 0x01 0x0A 0x90`

**Byte Breakdown**:
| Byte | Description |
|------|-------------|
| 0x01 | Slave ID |
| 0x04 | Function Code 04 (Read Input Registers) |
| 0x0002 | Register Address |
| 0x0001 | Data Length (1 register) |
| 0x0A90 | CRC |

### 4. Read Raw Data (XYZ)
Read raw data from registers 0x0003–0x007D (Page 5).

**Request**:
- Slave ID: 0x01
- Function Code: 0x04 (Read Input Registers)
- Register Address: 0x0003
- Data Length: Up to 123 registers
- CRC: 0x?

**Hex Format**: `0x01 0x04 0x00 0x03 0x00 0x7B 0x?? 0x??`

**Note**: CRC depends on the number of registers requested. Continuous streaming requires 3 Mbps baud rate.

### 5. Read Latest Raw Data (XYZ)
Read the latest X, Y, Z acceleration values from registers 0x0083–0x0085 (Page 8).

**Request**:
- Slave ID: 0x01
- Function Code: 0x04 (Read Input Registers)
- Register Address: 0x0083
- Data Length: 0x03
- CRC: 0x?

**Hex Format**: `0x01 0x04 0x00 0x83 0x00 0x03 0x?? 0x??`

**Byte Breakdown**:
| Byte | Description |
|------|-------------|
| 0x01 | Slave ID |
| 0x04 | Function Code 04 (Read Input Registers) |
| 0x0083 | Register Address |
| 0x0003 | Data Length (3 registers) |
| 0x? | CRC |

**Response**: Convert values to gravity (g) using the sensor’s scaling factor (refer to datasheet).

### 6. Read Chip ID
Read the Chip ID from register 0x0080 (Page 8).

**Request**:
- Slave ID: 0x01
- Function Code: 0x04 (Read Input Registers)
- Register Address: 0x0080
- Data Length: 0x03
- CRC: 0xE3B1

**Hex Format**: `0x01 0x04 0x00 0x80 0x00 0x03 0xB1 0xE3`

**Byte Breakdown**:
| Byte | Description |
|------|-------------|
| 0x01 | Slave ID |
| 0x04 | Function Code 04 (Read Input Registers) |
| 0x0080 | Register Address |
| 0x0003 | Data Length (3 registers) |
| 0xE3B1 | CRC |

### 7. Read UCID
Read the UCID from register 0x001B (Page 8).

**Request**:
- Slave ID: 0x01
- Function Code: 0x03 (Read Holding Registers)
- Register Address: 0x001B
- Data Length: 0x02
- CRC: 0x?

**Hex Format**: `0x01 0x03 0x00 0x1B 0x00 0x02 0x?? 0x??`

**Byte Breakdown**:
| Byte | Description |
|------|-------------|
| 0x01 | Slave ID |
| 0x03 | Function Code 03 (Read Holding Registers) |
| 0x001B | Register Address |
| 0x0002 | Data Length (2 registers) |
| 0x? | CRC |

### 8. Enable Bulk Transfer Mode
Configure bulk transfer mode via register 0x0015 (Page 6).

**Request**:
- Slave ID: 0x01
- Function Code: 0x06 (Write Single Register)
- Register Address: 0x0015
- Data Length: 0x01
- CRC: 0x?

**Hex Format**: `0x01 0x06 0x00 0x15 0x00 0x01 0x?? 0x??`

### 9. Read Gravity RMS / Peak / Crest Factor / Skewness / Kurtosis
Read vibration metrics from registers 0x001E–0x0022 (Page 9).

**Request**:
- Slave ID: 0x01
- Function Code: 0x03 (Read Holding Registers)
- Register Address: 0x001E (RMS), 0x001F (Peak), 0x0020 (Crest Factor), 0x0021 (Skewness), 0x0022 (Kurtosis)
- Data Length: 0x03
- CRC: 0x?

**Hex Format** (example for RMS): `0x01 0x03 0x00 0x1E 0x00 0x03 0x?? 0x??`

**Byte Breakdown**:
| Byte | Description |
|------|-------------|
| 0x01 | Slave ID |
| 0x03 | Function Code 03 (Read Holding Registers) |
| 0x001E | Register Address (e.g., RMS) |
| 0x0003 | Data Length (3 registers) |
| 0x? | CRC |

**Response**: Value = Register value / 1000 (e.g., for gravity units).

### 10. Read Velocity RMS / Peak / Crest Factor
Read velocity metrics from registers 0x0032–0x0034 (Page 10).

**Request**:
- Slave ID: 0x01
- Function Code: 0x03 (Read Holding Registers)
- Register Address: 0x0032 (RMS), 0x0033 (Peak), 0x0034 (Crest Factor)
- Data Length: 0x03
- CRC: 0x?

**Hex Format** (example for RMS): `0x01 0x03 0x00 0x32 0x00 0x03 0x?? 0x??`

**Byte Breakdown**:
| Byte | Description |
|------|-------------|
| 0x01 | Slave ID |
| 0x03 | Function Code 03 (Read Holding Registers) |
| 0x0032 | Register Address (e.g., RMS) |
| 0x0003 | Data Length (3 registers) |
| 0x? | CRC |

**Response**: Value = Register value / 100 (e.g., for velocity units in mm/s).

**Note**: Recommended polling interval: 0.2 seconds (5 Hz).

## Additional Notes

- **FIFO Function**: The sensor supports a First-In, First-Out (FIFO) buffer for raw data, accessible via PC software with Modbus DLLs running in the background (Page 7). Enable via register 0x0002.
- **Data Conversion**:
  - Temperature: Divide by 100 for °C.
  - Gravity metrics (RMS, Peak, Crest Factor, Skewness, Kurtosis): Divide by 1000.
  - Velocity metrics (RMS, Peak, Crest Factor): Divide by 100.
- **High Pass Filter**: Configurable via register 0x001C. Bandwidth depends on sample rate (7812 sps for I-type, 6400 sps for K-type).
- **Baud Rate Limitation**: Continuous raw data streaming is supported only at 3 Mbps. At 115200 bps, only periodic data retrieval is possible.

## Example Code Snippet (C#)

The following C# snippet (from Page 11) demonstrates how to display velocity metrics:

```csharp
Console.Write("Velocity(mm/s): {0} / {1} / {2}",
    ((double)Vib1_dat[0] / 100).ToString("f2"), // RMS
    ((double)Vib1_dat[1] / 100).ToString("f2"), // Peak
    ((double)Vib1_dat[2] / 100).ToString("f2")); // Crest Factor
```

**Note**: Adjust array indices and scaling factors based on your application.
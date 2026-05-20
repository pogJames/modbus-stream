"""Disable high-pass filter on the sensor at /dev/ttyUSB0."""
from pymodbus.client import ModbusSerialClient

PORT = "/dev/ttyUSB0"
BAUD = 3000000
SLAVE_ID = 1
HIGH_PASS_ENABLE = 0x001C

client = ModbusSerialClient(port=PORT, baudrate=BAUD, bytesize=8, parity="N", stopbits=1, timeout=1)
if not client.connect():
    raise SystemExit(f"Failed to open {PORT}")

rsp = client.write_register(address=HIGH_PASS_ENABLE, value=0, device_id=SLAVE_ID)
if rsp.isError():
    raise SystemExit(f"Modbus error: {rsp}")

print("High-pass filter disabled.")
client.close()

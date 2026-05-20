"""Worker 1 — Modbus reader, one thread per sensor.

Streams raw XYZ samples from input register 0x02 (FC04) and emits
WINDOW_SIZE-sample sliding windows (HOP_SIZE hop) to the inference queue.
Each chunk is also fed to an optional RecordingManager for the /record page.
Read pattern mirrors DAQ_Modbus_MultiChs_v1.3.py; queue uses drop-oldest."""

import threading
import os, sys, glob, time, serial, subprocess
import numpy as np
from queue import Queue

# Up to 4 sensors. Add /dev/ttyUSB1..USB3 here as more come online.
ALLOWED_PORTS = ['/dev/ttyUSB0', 'COM3', 'COM4']

WINDOW_SIZE   = 2604     # 1/3 sec at 7812 Hz — must match the trained backbone
HOP_SIZE      = 1302     # 50 % overlap → ~6 emits/sec
MAX_PACKET    = 41 * 3   # Modbus packet upper bound: 41 XYZ triplets
TURN_GRAVITY  = 8192     # raw int16 / 8192 -> G value
SAMPLE_RATE   = 7812


class RawWindowReader(threading.Thread):
    def __init__(self, port, window_queue, latest_window_slot,
                 port_baud=3000000, bytesize=8, parity='N', stopbits=1,
                 timeout=3, sampleRate=SAMPLE_RATE,
                 window_size=WINDOW_SIZE, hop_size=HOP_SIZE, max_qsize=3,
                 recording_manager=None):
        super().__init__(daemon=True)
        self.port = port
        self.port_baud = port_baud
        self.bytesize = bytesize
        self.parity = parity
        self.stopbits = stopbits
        self.timeout = timeout
        self.sampleRate = sampleRate
        self.window_queue = window_queue
        self.latest_window_slot = latest_window_slot
        self.window_size = window_size
        self.hop_size = hop_size
        self.max_qsize = max_qsize
        self.recording_manager = recording_manager
        self._stopper = threading.Event()

    def stopIt(self):
        self._stopper.set()

    def stopped(self):
        return self._stopper.is_set()

    def _emit_window(self, window):
        # Drop-oldest on full so the inference worker always sees the freshest window
        while self.window_queue.qsize() >= self.max_qsize:
            print(f"[{self.port}] Warning! Queue Overwrite")
            try:
                self.window_queue.get_nowait()
            except Exception:
                break
        self.window_queue.put((self.port, window))
        self.latest_window_slot.set(window)

    def run(self):
        from pymodbus.client import ModbusSerialClient as ModbusClient

        client = ModbusClient(port=self.port,
                              baudrate=self.port_baud,
                              bytesize=self.bytesize,
                              parity=self.parity,
                              stopbits=self.stopbits,
                              timeout=self.timeout)
        client.connect()

        chip = client.read_input_registers(0x80, count=3, device_id=1).registers
        print(f"[{self.port}] ChipID: {hex(chip[0])}, {hex(chip[1])}, {hex(chip[2])}")
        print(f"[{self.port}] SampleRate: {self.sampleRate}")
        client.write_register(0x01, self.sampleRate, device_id=1)

        result = client.read_input_registers(0x02, count=1, device_id=1)
        data_len = result.registers[0]
        print(f"[{self.port}] Initial buffer length: {data_len}")

        # Drain the sensor FIFO once before emitting — otherwise pre-startup
        # backlog gets pumped through the model at 10× real-time as stale data.
        drain_total = 0
        drain_iters = 0
        while data_len > 6 and drain_iters < 200:
            count = min(data_len, MAX_PACKET)
            result = client.read_input_registers(0x02, count=1 + count, device_id=1)
            data_len = result.registers[0]
            drain_total += count
            drain_iters += 1
        print(f"[{self.port}] FIFO drain: discarded {drain_total} stale samples in {drain_iters} reads (data_len now {data_len})")

        buffer = np.empty((0, 3), dtype=np.float32)
        last_log_t = time.time()
        emits_since_log = 0

        while not self.stopped():
            # Three-branch read (mirror DAQ_Modbus_MultiChs_v1.3.py:109-117)
            if data_len >= MAX_PACKET:
                result = client.read_input_registers(0x02, count=1 + MAX_PACKET, device_id=1)
            elif data_len <= 6:                # < 2 complete XYZ triplets
                time.sleep(0.001)
                result = client.read_input_registers(0x02, count=1, device_id=1)
                data_len = result.registers[0]
                continue
            else:
                result = client.read_input_registers(0x02, count=1 + data_len, device_id=1)

            data_len = result.registers[0]     # updated remaining length for next iteration

            raw = np.array(result.registers[1:], dtype=np.uint16).astype(np.int16)
            samples = (raw / TURN_GRAVITY).reshape(-1, 3).astype(np.float32)

            # Recording tap: feed raw, non-overlapping samples to any active
            # recording session. No-op when idle (cheap port-match check).
            if self.recording_manager is not None:
                self.recording_manager.feed(self.port, samples)

            buffer = np.row_stack((buffer, samples))

            # Sliding-window emission: emit whenever we have >= WINDOW_SIZE
            # samples, then drop the oldest HOP_SIZE and wait for HOP_SIZE new.
            while buffer.shape[0] >= self.window_size:
                self._emit_window(buffer[:self.window_size].copy())
                buffer = buffer[self.hop_size:]
                emits_since_log += 1

            now = time.time()
            if now - last_log_t >= 1.0:
                print(f"[{self.port}] emit rate: {emits_since_log}/s · data_len={data_len} · buffer={buffer.shape[0]}")
                emits_since_log = 0
                last_log_t = now

        client.close()


def get_existed_serial_ports():
    if sys.platform.startswith('win'):
        candidates = ['COM%s' % (i + 1) for i in range(256)]
    elif sys.platform.startswith('linux') or sys.platform.startswith('cygwin'):
        candidates = glob.glob('/dev/ttyUSB*')
        # Drop USB-serial latency from 16ms to 1ms for sustained 7812 Hz throughput.
        for port in candidates:
            cmd = ('sudo bash -c "echo 1 > /sys/bus/usb-serial/devices/ttyUSB'
                   + port.split('USB')[-1] + '/latency_timer"')
            print(cmd)
            subprocess.run(cmd, shell=True, check=True, executable='/bin/bash')
    elif sys.platform.startswith('darwin'):
        candidates = glob.glob('/dev/tty.*')
    else:
        raise EnvironmentError('Unsupported platform')

    available = []
    for port in candidates:
        try:
            s = serial.Serial(port)
            s.close()
            available.append(port)
        except (OSError, serial.SerialException):
            pass
    return available

import threading
import os, sys, glob, serial, time, datetime, ctypes, subprocess
import numpy as np
from queue import Queue
import pandas as pd

REG_ACC_RMS  = 0x1E
REG_ACC_PEAK = 0x1F
REG_ACC_CF   = 0x20
REG_ACC_SKEW = 0x21
REG_ACC_KU   = 0x22
DATA_LEGNTH = 3
readqsize = 7812 # 滿多少sample才從buffer讀出來

# 子執行緒類別
class ServiceThread(threading.Thread):
    def __init__(self, num, port, queue, port_baud=3000000,
                 bytesize=8, parity='N', stopbits=1, timeout=3,
                 sampleRate=7812):
        threading.Thread.__init__(self)
        self.num = num
        self.port = port
        self.port_baud = port_baud
        self.bytesize = bytesize
        self.parity = parity
        self.stopbits = stopbits
        self.timeout = timeout
        self.sampleRate = sampleRate
        self._stopper = threading.Event()
        self.turn_gravity = 8192
        self.queue = queue
        self.maxqsize = 3
        
    def stopIt(self):
        self._stopper.set()
 
    def stopped(self):
        return self._stopper.isSet()
        
    def run(self):
        
        sampleRate = self.sampleRate
        queue = self.queue
        client = ModbusClient(port=self.port, 
                              baudrate=self.port_baud,
                              bytesize=self.bytesize,
                              parity=self.parity,
                              stopbits=self.stopbits,
                              timeout=self.timeout
                              )
        connection = client.connect()
        #time.sleep(2) # star up delay after the port is opened
        
        # Read start_address, count, device_id_id
        vib_dat = client.read_input_registers(0x80, count=3, device_id=1)  
        print(f"ChipID: {hex(vib_dat.registers[0])}, {hex(vib_dat.registers[1])}, {hex(vib_dat.registers[2])}")
        print(f"SampleRate: {sampleRate}")
        
        # Write SampleRate
        client.write_register(0x01, sampleRate, device_id=1)
        # Read 剩餘資料長度
        vib_dat = client.read_input_registers(0x02, count=1, device_id=1)
        prev_data_len = 0
        data_len = vib_dat.registers[0] # 更新剩餘資料長度
        counter = 0
        print(f"Data Length: {data_len}")
        maxSize = (41 * 3)  # Modbus封包最大長度 41個封包，每包含3個值分別為X,Y,Z
        turn_gravity = self.turn_gravity # 轉G值
        
        # buffer 設定
        #readqsize = 8192 # 滿多少sample才從buffer讀出來
        buffer = np.empty((0, 3), float)
        del_index = np.arange(0, readqsize, 1, int)
        
        while True:
            if self.stopped():
                return
                    
            # # Read Feature
            # RMS = np.array( client.read_holding_registers(REG_ACC_RMS, DATA_LEGNTH, device_id=1).registers ) 
            # RMS = RMS / 1000    # Gravity, 9.8 m/s^2
            # Peak = np.array( client.read_holding_registers(REG_ACC_PEAK, DATA_LEGNTH, device_id=1).registers )
            # Peak = Peak / 1000  # Gravity, 9.8 m/s^2
            # CrestFactor = np.array( client.read_holding_registers(REG_ACC_CF, DATA_LEGNTH, device_id=1).registers ) 
            # CrestFactor = CrestFactor / 1000
            # # SKEW & KU 約每3秒算1次 
            # Skewness = np.array( client.read_holding_registers(REG_ACC_SKEW, DATA_LEGNTH, device_id=1).registers ) 
            # Skewness = Skewness / 1000
            # Kurtosis = np.array( client.read_holding_registers(REG_ACC_KU, DATA_LEGNTH, device_id=1).registers ) 
            # Kurtosis = Kurtosis / 1000
            
            # RMS = RMS.astype(str)
            # Peak = Peak.astype(str)
            # CrestFactor = CrestFactor.astype(str)
            # Skewness = Skewness.astype(str)
            # Kurtosis = Kurtosis.astype(str)
            # #print(f"Gravity RMS: {RMS},{Peak},{CrestFactor},{Skewness},{Kurtosis}")
            # feature_jsonStr = {"id": self.port, 
                       # "RMS": {"x":RMS[0], "y":RMS[1], "z":RMS[2]}, 
                       # "Peak": {"x":Peak[0], "y":Peak[1], "z":Peak[2]},
                       # "CrestFactor": {"x":CrestFactor[0], "y":CrestFactor[1], "z":CrestFactor[2]},
                       # "Skewness": {"x":Skewness[0], "y":Skewness[1], "z":Skewness[2]},
                       # "Kurtosis": {"x":Kurtosis[0], "y":Kurtosis[1], "z":Kurtosis[2]}
                      # }
            # print(feature_jsonStr)
            
            start = time.perf_counter()
            
            if data_len >= maxSize:     # 最多一次可抓 maxSize個值
                vib_dat = client.read_input_registers(0x02, count=1+maxSize, device_id=1)
            elif data_len <= (2 * 3):   # 若感測器buffer不超過6個值，則只更新剩餘資料長度，不抓值
                time.sleep(0.001)
                vib_dat = client.read_input_registers(0x02, count=1, device_id=1)
                continue
            else:   
                vib_dat = client.read_input_registers(0x02, count=data_len + 1, device_id=1)
            
            end = time.perf_counter()            
            
            # Debug用，每10次迴圈Print 剩餘長度與一個封包
            # counter = counter + 1
            # if counter >= 10:
                
                # counter = 0
                # start1 = time.perf_counter()
                # print("{} ".format(self.port),
                      # "{}ms".format((end - start) * 1000), 
                      # "Data Length: 共:{} 增加:{} X:{} Y:{} Z:{}".format(
                                    # vib_dat.registers[0],
                                    # (vib_dat.registers[0] - prev_data_len),
                                    # ctypes.c_int16(vib_dat.registers[1]),
                                    # ctypes.c_int16(vib_dat.registers[2]),
                                    # ctypes.c_int16(vib_dat.registers[3])))
            
            
            data_len = vib_dat.registers[0] # 下一輪從感測器撈多少資料
            # print("剩餘長度:",data_len)
            # 資料轉為G值，並改以 筆數x三軸 的陣列
            # data = np.int16(vib_dat.registers[1:]) # Debug用 數值+9的Ramp模擬訊號
            data = np.array(vib_dat.registers[1:], dtype=np.uint16).astype(np.int16) / self.turn_gravity # 振動轉G值
            data = data.reshape((-1, 3))
            
            buffer = np.row_stack((buffer, data))
            
            # Buffer每滿readqsize資料，則打包放進Queue
            if buffer.shape[0] >= readqsize:
                # 超過max queue size則丟棄一個element，並發出警示訊息
                while queue.qsize() >= self.maxqsize:
                    print("Warning! Queue Overwrite")
                    queue.get() #丟一個舊的
                                 
                # print("buffer row",buffer.shape[0])
                queue.put(buffer[:readqsize]) # 放新資料進Queue
                buffer = np.delete(buffer, del_index, axis=0)
                # print("after buffer row",buffer.shape[0])
                
            
        time.sleep(1)
        

def get_existed_serial_ports():
    if sys.platform.startswith('win'):
        ports = ['COM%s' % (i + 1) for i in range(256)]
    elif sys.platform.startswith('linux') or sys.platform.startswith('cygwin'):
        # this excludes your current terminal '/dev/tty'
        # ports = glob.glob('/dev/serial*')
        ports = glob.glob('/dev/ttyUSB*')
        # Linux系統時自動下指令修改所有port的Latency timer從16ms降為1ms，加快收資料速度
        # 若有異常可將下面for loop註解掉
        for port in ports:
            bashCommand = 'sudo bash -c "echo 1 > /sys/bus/usb-serial/devices/ttyUSB' + port.split('USB')[-1] + '/latency_timer"'
            print(bashCommand)
            subprocess.run(bashCommand, shell=True, check=True, executable='/bin/bash')
    elif sys.platform.startswith('darwin'):
        ports = glob.glob('/dev/tty.*')
    else:
        raise EnvironmentError('Unsupported platform')

    # to check if serial exist
    result = []
    for port in ports:
        try:
            s = serial.Serial(port)
            s.close()
            result.append(port)
        except (OSError, serial.SerialException):
            pass
    return result

if __name__=='__main__':
    current_path = os.path.dirname(os.path.abspath(__file__))
    sys.path.insert(0, current_path + '/site-packages')
    from pymodbus.client import ModbusSerialClient as ModbusClient
    
    ports = get_existed_serial_ports()
    print("Existed Port:", ports)
    
    queues = ()
    q = (Queue(),) 
    qMap = {}

    # 建立多個子執行緒
    ServiceThreads = []
    for idx, port in enumerate(ports):
        queues = queues + q
        qMap[port] = idx            # 幫COM Port標索引數字 如map = {'COM 4': 0, 'COM 1': 1}
        ServiceThreads.append(ServiceThread(idx,port,queues[idx]))
        ServiceThreads[idx].start()
    
    # 主執行緒繼續執行自己的工作
    # ...
    # time.sleep(5) # Delay 5秒製造 Queue滿的warning，debug用
    start = time.time()
    # 主程式 Start ==================================================================
    try:
        while True:
            for idx, port in enumerate(ports):                         
                if queues[idx].qsize() >= 1:
                  # 取得新的資料
                  msg = queues[idx].get()
                  
                  # df = pd.DataFrame(msg)
                  # df.to_csv('file.csv', index=False)
                  # 處理資料                          
                  data_jsonStr = {"port": port, 
                       "vibrationData": msg
                       
                  }
                  end = time.time()   
                  print(f"Computation time = {1000*(end - start):.3f}ms")
                  start = time.time()
                  print(data_jsonStr)
            time.sleep(0.1)     # 每0.1s檢查queue的狀況
    except KeyboardInterrupt:
        print('interrupted!')
              
    # 主程式 end ====================================================================
    
    for idx, x in enumerate(ports):
      ServiceThreads[idx].stopIt()
    
    
    # 等待所有子執行緒結束
    for idx, x in enumerate(ports):
      ServiceThreads[idx].join()

    print("Done.")
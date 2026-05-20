// Lightweight client-side i18n. English is the source-of-truth in templates;
// this script swaps text nodes + selected attributes when zh-Hant is active.
//
// Usage:
//   - Static text in templates needs no markup — it's swapped if it matches
//     an entry in the EN→ZH dictionary below.
//   - JS code that sets text dynamically should call window.t('English string'),
//     optionally with placeholders: t('Sensor {n}', { n: 3 }).
//   - Pattern-based strings (e.g. "Sensor 1", "Sensor 2") use the PATTERNS list.

(function () {
  const ZH = {
    // ── Brand / footer ────────────────────────────────────────
    'Modbus Stream': 'Modbus 串流',
    'Modbus Stream v1.5.2 — Tri-axial Accelerometer Interface': 'Modbus 串流 v1.5.2 — 三軸加速度計介面',

    // ── Dashboard ─────────────────────────────────────────────
    'Real-time tri-axial accelerometer monitoring': '即時三軸加速度計監測',
    'Record Data': '錄製資料',
    'Capture 10 seconds of raw sensor data across all connected sensors.': '擷取所有已連線感測器 10 秒的原始資料。',
    'Start Record': '開始錄製',
    'Recording…': '錄製中…',
    'CSV Viewer': 'CSV 檢視',
    'Latest Data': '最新資料',
    'View the current sensor reading and computed metrics.': '檢視目前感測器讀數與計算指標。',
    'Raw Data': '原始資料',
    'All Metrics': '所有指標',
    'Live Streams': '即時串流',
    'Connect to real-time data streams with live-updating charts.': '連線至即時資料串流並即時更新圖表。',
    'Raw Stream': '原始串流',
    'Metrics Trend': '指標趨勢',
    'System Information': '系統資訊',
    'System health, diagnostics, and API status information.': '系統健康狀態、診斷與 API 狀態資訊。',
    'Diagnostics': '診斷',
    'Settings': '設定',
    'Connection:': '連線狀態:',
    'Failed to start': '啟動失敗',

    // ── Diagnostics page ──────────────────────────────────────
    'Model': '型號',
    'Gain': '增益',
    'Serial': '序號',
    'Firmware': '韌體',
    'Temperature': '溫度',
    'FIFO': 'FIFO',
    'Connection': '連線',
    'Device': '裝置',
    'Baud Rate': '鮑率',
    'Slave ID': '從機 ID',
    'Streaming': '串流',
    'Capability': '功能',
    'Max Connections': '最大連線數',
    'Buffer Size': '緩衝大小',
    'Metrics Rate': '指標頻率',
    'Raw Max Samples': '原始最大樣本數',
    'System': '系統',
    'Service': '服務',
    'Version': '版本',
    'OS': '作業系統',
    'Arch': '架構',
    'Rust': 'Rust',
    'Refreshes every 5 s': '每 5 秒更新',
    'Live': '即時',
    'Error': '錯誤',
    'Connected': '已連線',
    'Disconnected': '已中斷',
    'Connecting…': '連線中…',
    'No device': '無裝置',
    'Offline': '離線',
    'Full (raw + metrics)': '完整 (原始 + 指標)',
    'Metrics only': '僅指標',

    // ── Settings page ─────────────────────────────────────────
    '⚙️ Sensor Configuration': '⚙️ 感測器設定',
    'Device Path': '裝置路徑',
    'Serial port path for the Modbus device': 'Modbus 裝置的序列埠路徑',
    '115,200 bps': '115,200 bps',
    'Periodic monitoring only': '僅週期性監測',
    '3 Mbps': '3 Mbps',
    'Full streaming support': '支援完整串流',
    '⚠️ Changing baud rate requires sensor power cycle': '⚠️ 變更鮑率需重新啟動感測器',
    'Modbus slave ID (1–247)': 'Modbus 從機 ID (1–247)',
    'Timeout (ms)': '逾時 (毫秒)',
    'Communication timeout': '通訊逾時',
    'Retry Attempts': '重試次數',
    'Number of retry attempts': '重試次數',
    '⚡ Sensor Configuration': '⚡ 感測器設定',
    'Sample Rate (sps)': '取樣率 (sps)',
    'Common values: 1600 (balanced), 6400 (K-type), 7812 (I-type)': '常用值: 1600 (平衡), 6400 (K 型), 7812 (I 型)',
    'Stream Size (registers)': '串流大小 (暫存器)',
    'Bulk transfer size (1–123, preferably divisible by 3)': '批次傳輸大小 (1–123, 最好能被 3 整除)',
    'Enable High Pass Filter': '啟用高通濾波器',
    'Bandwidth: 3–2.5 kHz @ 7812 sps (2 kHz for K-type)': '頻寬: 3–2.5 kHz @ 7812 sps (K 型為 2 kHz)',
    '📊 Streaming Configuration': '📊 串流設定',
    'Max WebSocket Connections': '最大 WebSocket 連線數',
    'Concurrent WebSocket clients': '同時 WebSocket 用戶端',
    'Buffer Size (bytes)': '緩衝大小 (位元組)',
    'WebSocket message buffer': 'WebSocket 訊息緩衝',
    'Metrics Update Rate (Hz)': '指標更新頻率 (Hz)',
    'Max: 5 Hz (sensor limitation)': '最大: 5 Hz (感測器限制)',
    'WebSocket Ping Interval (sec)': 'WebSocket Ping 間隔 (秒)',
    'Keep-alive ping frequency': 'Keep-alive ping 頻率',
    '🔧 Test Connection (Sensor 1)': '🔧 測試連線 (感測器 1)',
    '✅ Apply Settings': '✅ 套用設定',
    '🔄 Reset to Defaults': '🔄 重置為預設',
    'Reset all settings to defaults? This will reload the configuration file.': '重置所有設定為預設值? 這將會重新載入設定檔。',
    '✏️ Enter manually...': '✏️ 手動輸入...',

    // ── View metrics ──────────────────────────────────────────
    'Metrics Stream': '指標串流',
    'Gravity RMS (g)': '重力 RMS (g)',
    'Gravity Peak (g)': '重力峰值 (g)',
    'Crest Factor': '波峰因子',
    'Skewness': '偏度',
    'Kurtosis': '峰度',
    'Primary Frequency (Hz)': '主頻率 (Hz)',
    'Gravity': '重力',
    'Velocity': '速度',
    'Velocity RMS (mm/s)': '速度 RMS (mm/s)',
    'Gravity RMS — live trend (last 120 readings)': '重力 RMS — 即時趨勢 (最近 120 筆)',

    // ── View all metrics ──────────────────────────────────────
    'Sensor Metrics': '感測器指標',
    'Gravity Analysis': '重力分析',
    'RMS (g)': 'RMS (g)',
    'Peak (g)': '峰值 (g)',
    'Primary Freq': '主頻率',
    'Velocity Analysis': '速度分析',
    'RMS (mm/s)': 'RMS (mm/s)',
    'Peak (mm/s)': '峰值 (mm/s)',

    // ── View raw ──────────────────────────────────────────────
    'Live Stream': '即時串流',
    'Time Domain': '時域',
    'Frequency': '頻率',
    'X Axis': 'X 軸',
    'Y Axis': 'Y 軸',
    'Z Axis': 'Z 軸',
    'X Peak': 'X 峰值',
    'Y Peak': 'Y 峰值',
    'Z Peak': 'Z 峰值',
    'min': '最小',
    'max': '最大',
    'Last 200 samples': '最近 200 個樣本',
    'Number of samples to display': '顯示樣本數',
    'Pause': '暫停',
    'Resume': '繼續',
    'Waiting for FFT frame…': '等待 FFT 訊框…',
    'Log scale': '對數刻度',
    'Freeze': '凍結',
    'Unfreeze': '解凍',
    'Frequency (Hz)': '頻率 (Hz)',
    'Amplitude (g pk)': '振幅 (g 峰值)',

    // ── View latest raw ───────────────────────────────────────
    'Latest Raw Reading': '最新原始讀數',
    'Refreshes every 500 ms': '每 500 毫秒更新',

    // ── View CSV ──────────────────────────────────────────────
    'CSV Files': 'CSV 檔案',
    'Total Samples': '樣本總數',
    'X Range': 'X 範圍',
    'Y Range': 'Y 範圍',
    'Z Range': 'Z 範圍',
    'Infer': '推論',
    'Err': '錯誤',
    'Window': '視窗',
    'Loading…': '載入中…',
    'No valid data found.': '找不到有效資料。',
    'Sample Index': '樣本索引',
    'Unknown error': '未知錯誤',
    'Select a file from the sidebar.': '從側邊欄選擇檔案。',
  };

  // Pattern-based translations (matches when exact lookup fails)
  // Each: { re: RegExp, zh: function(match) -> string }
  const PATTERNS = [
    { re: /^Sensor (\d+)$/,                            zh: m => `感測器 ${m[1]}` },
    { re: /^Modbus Config (\d+)$/,                     zh: m => `Modbus 設定 ${m[1]}` },
    { re: /^Last: (.+)$/,                              zh: m => `最後: ${m[1]}` },
    { re: /^Last (.+) samples$/,                       zh: m => `最近 ${m[1]} 個樣本` },
    { re: /^(\d[\d,]*) samples received$/,             zh: m => `已接收 ${m[1]} 個樣本` },
    { re: /^(\d[\d,]*) FFT frames received$/,          zh: m => `已接收 ${m[1]} 個 FFT 訊框` },
    { re: /^Frame (\d+) — window (\d+) samples @ (\d+) Hz$/, zh: m => `訊框 ${m[1]} — 視窗 ${m[2]} 個樣本 @ ${m[3]} Hz` },
    { re: /^Class (\d+)$/,                             zh: m => `類別 ${m[1]}` },
    { re: /^(\d[\d,]*) samples$/,                      zh: m => `${m[1]} 個樣本` },
    { re: /^Saved: (.+)$/,                             zh: m => `已儲存: ${m[1]}` },
    { re: /^Errors: (.+)$/,                            zh: m => `錯誤: ${m[1]}` },
    { re: /^Error loading file: (.+)$/,                zh: m => `載入檔案錯誤: ${m[1]}` },
    { re: /^Sensor (\d+): ([\d,]+) \/ ([\d,]+) samples$/, zh: m => `感測器 ${m[1]}: ${m[2]} / ${m[3]} 個樣本` },
    { re: /^Sensor (\d+): (.+)$/,                      zh: m => `感測器 ${m[1]}: ${m[2]}` },
    { re: /^Modbus Stream v([\d.]+) — Tri-axial Accelerometer Interface$/,
      zh: m => `Modbus 串流 v${m[1]} — 三軸加速度計介面` },
  ];

  // Attributes to translate
  const ATTRS = ['placeholder', 'title', 'aria-label'];

  // Cache the original English text per text node / element-attribute so we
  // can re-apply (in either direction) without losing the source string.
  const TEXT_CACHE = new WeakMap();   // textNode -> originalEn
  const ATTR_CACHE = new WeakMap();   // element  -> { attrName: originalEn }

  function translate(en) {
    if (!en) return en;
    const trimmed = en.trim();
    if (!trimmed) return en;
    const lead  = en.match(/^\s*/)[0];
    const trail = en.match(/\s*$/)[0];
    if (ZH[trimmed]) return lead + ZH[trimmed] + trail;
    for (const { re, zh } of PATTERNS) {
      const m = trimmed.match(re);
      if (m) return lead + zh(m) + trail;
    }
    return en;
  }

  function applyToTextNode(node, lang) {
    let original = TEXT_CACHE.get(node);
    if (original === undefined) {
      original = node.nodeValue;
      TEXT_CACHE.set(node, original);
    }
    const newVal = lang === 'zh-Hant' ? translate(original) : original;
    if (node.nodeValue !== newVal) node.nodeValue = newVal;
  }

  function applyToAttribute(el, attr, lang) {
    let cache = ATTR_CACHE.get(el);
    if (!cache) { cache = {}; ATTR_CACHE.set(el, cache); }
    if (cache[attr] === undefined) {
      cache[attr] = el.getAttribute(attr);
    }
    const original = cache[attr];
    if (original == null) return;
    const newVal = lang === 'zh-Hant' ? translate(original) : original;
    if (el.getAttribute(attr) !== newVal) el.setAttribute(attr, newVal);
  }

  // Special handling for <option> elements, button [value] and similar
  function applyToValue(el, lang) {
    let cache = ATTR_CACHE.get(el);
    if (!cache) { cache = {}; ATTR_CACHE.set(el, cache); }
    if (cache.value === undefined) cache.value = el.value;
    const original = cache.value;
    if (original == null) return;
    // Only translate non-numeric values (avoid breaking form data)
    if (/^[\d.\-+e ]+$/.test(original.trim())) return;
    el.value = lang === 'zh-Hant' ? translate(original) : original;
  }

  function walk(root, lang) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
    let node;
    while ((node = walker.nextNode())) {
      // Skip text inside <script> and <style>
      const parent = node.parentNode;
      if (!parent) continue;
      const tag = parent.nodeName;
      if (tag === 'SCRIPT' || tag === 'STYLE') continue;
      applyToTextNode(node, lang);
    }
    // Attributes
    for (const attr of ATTRS) {
      root.querySelectorAll('[' + attr + ']').forEach(el => applyToAttribute(el, attr, lang));
    }
    // <option> elements have meaningful text-via-value
    root.querySelectorAll('option').forEach(el => {
      // textContent is already handled by the TreeWalker; nothing else needed.
      // Skip value translation — option values are form data.
    });
  }

  function currentLang() {
    return localStorage.getItem('lang') === 'zh-Hant' ? 'zh-Hant' : 'en';
  }

  let observer = null;
  function withoutObserver(fn) {
    if (observer) observer.disconnect();
    try { fn(); } finally {
      if (observer) observer.observe(document.body, { childList: true, subtree: true, characterData: true });
    }
  }

  function applyAll() {
    withoutObserver(() => walk(document.body, currentLang()));
  }

  window.t = function (s, vars) {
    let out = currentLang() === 'zh-Hant' ? translate(s) : s;
    if (vars) {
      for (const k in vars) {
        out = out.replace(new RegExp('\\{' + k + '\\}', 'g'), vars[k]);
      }
    }
    return out;
  };

  window.setLang = function (lang) {
    lang = lang === 'zh-Hant' ? 'zh-Hant' : 'en';
    localStorage.setItem('lang', lang);
    document.documentElement.lang = lang;
    applyAll();
    const btn = document.getElementById('lang-toggle');
    if (btn) btn.textContent = lang === 'zh-Hant' ? '中文' : 'EN';
  };

  // Initial apply + wire up the toggle
  function init() {
    document.documentElement.lang = currentLang();
    applyAll();
    const btn = document.getElementById('lang-toggle');
    if (btn) {
      btn.textContent = currentLang() === 'zh-Hant' ? '中文' : 'EN';
      btn.addEventListener('click', function () {
        window.setLang(currentLang() === 'zh-Hant' ? 'en' : 'zh-Hant');
      });
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

  // Re-translate after htmx swaps in new content
  document.body.addEventListener('htmx:afterSettle', applyAll);

  // Catch JS-injected content (e.g. recording progress bars). Disconnect the
  // observer while we apply translations so our own writes don't loop back.
  observer = new MutationObserver(muts => {
    if (currentLang() !== 'zh-Hant') return;
    withoutObserver(() => {
      for (const m of muts) {
        for (const n of m.addedNodes) {
          if (n.nodeType === Node.ELEMENT_NODE) {
            walk(n, 'zh-Hant');
          } else if (n.nodeType === Node.TEXT_NODE) {
            applyToTextNode(n, 'zh-Hant');
          }
        }
        if (m.type === 'characterData' && m.target.nodeType === Node.TEXT_NODE) {
          // JS overwrote textContent — treat the new value as fresh English.
          TEXT_CACHE.delete(m.target);
          applyToTextNode(m.target, 'zh-Hant');
        }
      }
    });
  });
  observer.observe(document.body, { childList: true, subtree: true, characterData: true });
})();

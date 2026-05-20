---
name: add-web-language-toggle
description: Add a client-side language toggle to a server-rendered web app (Flask, FastAPI, Rails, Axum/Minijinja, etc.) where English is the source-of-truth in templates and a single JS file translates the DOM into another language on click. Use when the user asks to add i18n, a language switcher, or translate the UI without server-side template rewrites.
---

# Add a client-side language toggle

This is for server-rendered web apps where the templates contain English text and you want a **lightweight, no-rebuild toggle** to another language (e.g. Traditional Chinese, Spanish, etc.) without rewriting every template.

## When NOT to use this

- App is a SPA with a real i18n framework (react-i18next, vue-i18n) — use that instead.
- You need pluralization, gendered translations, RTL, or date/number localization beyond simple text swap — graduate to a real i18n library.
- The "language" needs to be set by URL (`/en/...` vs `/zh/...`) for SEO — that's server-side, this is client-side localStorage.

## Architecture

- English is the source-of-truth. Templates stay in English. Don't touch them.
- One JS file (`i18n.js`) holds the translation dictionary and a DOM walker.
- User preference is persisted in `localStorage` under `lang`.
- A toggle button in the layout/base template flips between languages.
- Three layers of translation hook into the page:
  1. **Static HTML**: A `TreeWalker` over `document.body` translates every text node and selected attributes (`placeholder`, `title`, `aria-label`) on page load.
  2. **HTMX / fetch swaps**: Listen to `htmx:afterSettle` (or the equivalent event) and re-walk the new content.
  3. **JS-injected text** (e.g. `el.textContent = '...'`): A `MutationObserver` catches `childList` + `characterData` changes and re-translates.

## Implementation steps

### 1. Add the toggle button to the layout template

```html
<!-- in base.html / layout.html — wherever the nav lives -->
<button type="button" id="lang-toggle" class="nav-link lang-toggle" aria-label="Toggle language">中文</button>

<!-- at the bottom of <body>, after other scripts -->
<script src="/static/js/i18n.js"></script>
```

Pre-paint `<html lang>` in `<head>` to avoid a flash:

```html
<script>
  (function () {
    var saved = localStorage.getItem('lang') === 'zh-Hant' ? 'zh-Hant' : 'en';
    document.documentElement.lang = saved;
  })();
</script>
```

### 2. Drop in `static/js/i18n.js`

```js
(function () {
  // ── 1. Dictionary: exact English → target-language string ─────
  const ZH = {
    'Modbus Stream': 'Modbus 串流',
    'Settings': '設定',
    'Loading…': '載入中…',
    // ...one entry per visible string in the app
  };

  // ── 2. Patterns: regex → function returning translated string ──
  // For templated strings like "Sensor 3", "Last: 14:23:11", etc.
  const PATTERNS = [
    { re: /^Sensor (\d+)$/,     zh: m => `感測器 ${m[1]}` },
    { re: /^Last: (.+)$/,       zh: m => `最後: ${m[1]}` },
    { re: /^(\d[\d,]*) samples$/, zh: m => `${m[1]} 個樣本` },
  ];

  const ATTRS = ['placeholder', 'title', 'aria-label'];
  const TEXT_CACHE = new WeakMap(); // textNode -> original English
  const ATTR_CACHE = new WeakMap(); // element -> { attrName: originalEn }

  function translate(en) {
    if (!en) return en;
    const trimmed = en.trim();
    if (!trimmed) return en;
    const lead = en.match(/^\s*/)[0];
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
    if (cache[attr] === undefined) cache[attr] = el.getAttribute(attr);
    const original = cache[attr];
    if (original == null) return;
    const newVal = lang === 'zh-Hant' ? translate(original) : original;
    if (el.getAttribute(attr) !== newVal) el.setAttribute(attr, newVal);
  }

  function walk(root, lang) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
    let node;
    while ((node = walker.nextNode())) {
      const tag = node.parentNode && node.parentNode.nodeName;
      if (tag === 'SCRIPT' || tag === 'STYLE') continue;
      applyToTextNode(node, lang);
    }
    for (const attr of ATTRS) {
      root.querySelectorAll('[' + attr + ']').forEach(el => applyToAttribute(el, attr, lang));
    }
  }

  const currentLang = () => localStorage.getItem('lang') === 'zh-Hant' ? 'zh-Hant' : 'en';

  let observer = null;
  function withoutObserver(fn) {
    if (observer) observer.disconnect();
    try { fn(); } finally {
      if (observer) observer.observe(document.body, { childList: true, subtree: true, characterData: true });
    }
  }
  const applyAll = () => withoutObserver(() => walk(document.body, currentLang()));

  // Expose t() for JS-built strings: t('Sensor {n}', { n: 3 })
  window.t = function (s, vars) {
    let out = currentLang() === 'zh-Hant' ? translate(s) : s;
    if (vars) for (const k in vars) out = out.replace(new RegExp('\\{' + k + '\\}', 'g'), vars[k]);
    return out;
  };

  window.setLang = function (lang) {
    lang = lang === 'zh-Hant' ? 'zh-Hant' : 'en';
    localStorage.setItem('lang', lang);
    document.documentElement.lang = lang;
    applyAll();
    const btn = document.getElementById('lang-toggle');
    if (btn) btn.textContent = lang === 'zh-Hant' ? 'EN' : '中文';
  };

  function init() {
    document.documentElement.lang = currentLang();
    applyAll();
    const btn = document.getElementById('lang-toggle');
    if (btn) {
      btn.textContent = currentLang() === 'zh-Hant' ? 'EN' : '中文';
      btn.addEventListener('click', () => window.setLang(currentLang() === 'zh-Hant' ? 'en' : 'zh-Hant'));
    }
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();

  // Re-translate after htmx (or fetch-replace) swaps
  document.body.addEventListener('htmx:afterSettle', applyAll);

  // Catch JS-injected text — disconnect observer during our own writes
  // so we don't loop on the very mutations we just caused.
  observer = new MutationObserver(muts => {
    if (currentLang() !== 'zh-Hant') return;
    withoutObserver(() => {
      for (const m of muts) {
        for (const n of m.addedNodes) {
          if (n.nodeType === Node.ELEMENT_NODE) walk(n, 'zh-Hant');
          else if (n.nodeType === Node.TEXT_NODE) applyToTextNode(n, 'zh-Hant');
        }
        if (m.type === 'characterData' && m.target.nodeType === Node.TEXT_NODE) {
          TEXT_CACHE.delete(m.target);
          applyToTextNode(m.target, 'zh-Hant');
        }
      }
    });
  });
  observer.observe(document.body, { childList: true, subtree: true, characterData: true });
})();
```

### 3. Style the toggle button to match the existing nav

```css
.lang-toggle {
  background: transparent;
  border: 1px solid var(--border);
  cursor: pointer;
  font-family: inherit;
  min-width: 3rem;
}
```

### 4. Build the dictionary

Walk through every template and extract user-visible strings. Group by page in the dictionary for maintainability. For each string:

- **Exact match**: drop in `ZH` as `'English exact': '翻譯'`.
- **Templated** (contains numbers, names, timestamps that vary): add to `PATTERNS` with a regex.

When in doubt, add as an exact-match first and graduate to a pattern only if the variant count exceeds ~3.

### 5. Handle JS-built dynamic strings

Anywhere in the codebase that does `el.textContent = 'English string'` or `` `Sensor ${n} ready` ``: the MutationObserver catches these automatically. But if you want zero-flash, wrap with `t()`:

```js
el.textContent = t('Recording…');
el.textContent = t('Sensor {n}: {s} / {t} samples', { n, s, t });
```

## Gotchas (each one bit me; expect them)

1. **MutationObserver feedback loop**: writing to a text node fires a `characterData` mutation, which fires the observer, which writes again. Mitigate by:
   - Comparing before writing (`if (node.nodeValue !== newVal)`)
   - Disconnecting the observer during our own writes (see `withoutObserver` above)
   - Both, for belt-and-suspenders safety.

2. **Cache invalidation**: when JS overwrites `textContent`, the new English string becomes the new source. Delete the cache entry for that node on a `characterData` mutation, then re-apply translation.

3. **Whitespace preservation**: many text nodes are wrapped in significant whitespace (`\n        Hello\n      `). Translating trimmed text and restoring the leading/trailing whitespace prevents layout shift and weird collapses.

4. **`<script>` and `<style>` contents are text nodes too**: skip them in the walker.

5. **`<title>` tag is outside `document.body`**: not touched by this walker. Either set `document.title = t(...)` in JS, or live with it (browser tabs are less critical).

6. **`<option>` elements**: the visible label is the text content (handled). Don't translate `value` attributes — those are form data.

7. **Pre-paint flash**: setting `<html lang>` in a head-script before paint is purely cosmetic for CSS that targets `:lang(zh-Hant)`. It doesn't actually translate text — that happens after `i18n.js` runs.

8. **Cached old version**: when iterating, browsers cache `i18n.js` aggressively. Hard-refresh (Ctrl+Shift+R) or version the URL (`i18n.js?v=2`).

9. **Numbers and punctuation that look like English**: avoid translating strings made up of just digits or symbols. Add an early-return in `translate()` for purely-numeric trimmed content.

## How to verify it works

1. Click the toggle — every visible English string should change to the target language.
2. Click it back — everything restores cleanly (this validates the cache is intact).
3. Navigate to a different page — language persists (localStorage).
4. Trigger a dynamic update (recording progress, websocket message, htmx swap) — the new content appears in the target language.
5. Open devtools Console — no infinite mutation warnings, no errors from `i18n.js`.

## Scaling up

- For two+ extra languages, replace `ZH` with `TRANSLATIONS = { 'zh-Hant': {...}, 'es': {...} }` and the patterns array with `PATTERNS = { 'zh-Hant': [...], 'es': [...] }`. The toggle becomes a `<select>` instead of a button.
- For larger apps, split the dictionary into per-page modules loaded on demand.
- If the dictionary exceeds ~500 entries or you start needing pluralization, switch to a real i18n library — this pattern is intentionally tiny.

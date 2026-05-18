let _lang = "en";
let _translations = {};
let _listeners = [];
let _version = 0;

export async function initI18n(lang) {
  try {
    _translations = (await import(`../locales/${lang}.json`)).default;
    _lang = lang;
    _version++;
    localStorage.setItem("devnexus-lang", lang);
    // 通知所有监听器
    for (const fn of _listeners) {
      try { fn(_version); } catch (e) { console.error(e); }
    }
  } catch (e) {
    console.error("Failed to load language:", lang, e);
  }
}

export function t(key) {
  const keys = key.split(".");
  let val = _translations;
  for (const k of keys) {
    val = val?.[k];
  }
  return val ?? key;
}

export function tFormat(key, vars) {
  let text = t(key);
  if (typeof text === "string") {
    for (const [k, v] of Object.entries(vars)) {
      text = text.replace(`{${k}}`, v);
    }
  }
  return text;
}

export function getLang() {
  return _lang;
}

export function getVersion() {
  return _version;
}

export function onLangChange(fn) {
  _listeners.push(fn);
  return () => {
    _listeners = _listeners.filter(f => f !== fn);
  };
}

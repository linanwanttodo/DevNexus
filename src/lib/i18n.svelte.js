let lang = $state("en");
let translations = $state({});
let version = $state(0);

export async function initI18n(l) {
  try {
    translations = (await import(`../locales/${l}.json`)).default;
    lang = l;
    version++;
    localStorage.setItem("devnexus-lang", lang);
  } catch (e) {
    console.error("Failed to load language:", l, e);
  }
}

export function t(key) {
  const keys = key.split(".");
  let val = translations;
  for (const k of keys) {
    val = val?.[k];
  }
  return val ?? key;
}

export function tFormat(key, vars) {
  let text = t(key);
  if (typeof text === "string") {
    for (const [k, v] of Object.entries(vars)) {
      text = text.replaceAll(`{${k}}`, v);
    }
  }
  return text;
}

export function getLang() {
  return lang;
}

export function getVersion() {
  return version;
}

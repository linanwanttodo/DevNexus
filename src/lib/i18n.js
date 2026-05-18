import { get, writable } from "svelte/store";

const store = writable("en");
let translations = {};

export async function initI18n(lang) {
  try {
    translations = (await import(`../locales/${lang}.json`)).default;
    store.set(lang);
    localStorage.setItem("devnexus-lang", lang);
  } catch (e) {
    console.error("Failed to load language:", lang, e);
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
      text = text.replace(`{${k}}`, v);
    }
  }
  return text;
}

export function getLang() {
  return get(store);
}

export function onLangChange(fn) {
  return store.subscribe(fn);
}

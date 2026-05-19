let _toasts = [];
let _listeners = [];

export function getToasts() {
  return _toasts;
}

export function showToast(message, type = "info", duration = 3000) {
  const id = Date.now() + Math.random();
  const toast = { id, message, type, duration };
  _toasts = [..._toasts, toast];
  for (const fn of _listeners) {
    try { fn(_toasts); } catch (e) { console.error(e); }
  }
  if (duration > 0) {
    setTimeout(() => removeToast(id), duration);
  }
  return id;
}

export function removeToast(id) {
  _toasts = _toasts.filter(t => t.id !== id);
  for (const fn of _listeners) {
    try { fn(_toasts); } catch (e) { console.error(e); }
  }
}

export function onToastChange(fn) {
  _listeners.push(fn);
  return () => {
    _listeners = _listeners.filter(f => f !== fn);
  };
}

let _confirmations = [];
let _listeners = [];

export function getConfirmations() {
  return _confirmations;
}

export function showConfirm(message, title = "Confirm") {
  return new Promise((resolve) => {
    const id = Date.now() + Math.random();
    _confirmations = [..._confirmations, { id, message, title, resolve }];
    for (const fn of _listeners) {
      try { fn(_confirmations); } catch (e) { console.error(e); }
    }
  });
}

export function confirmResponse(id, value) {
  const item = _confirmations.find(c => c.id === id);
  if (item) {
    item.resolve(value);
    _confirmations = _confirmations.filter(c => c.id !== id);
    for (const fn of _listeners) {
      try { fn(_confirmations); } catch (e) { console.error(e); }
    }
  }
}

export function onConfirmChange(fn) {
  _listeners.push(fn);
  return () => {
    _listeners = _listeners.filter(f => f !== fn);
  };
}

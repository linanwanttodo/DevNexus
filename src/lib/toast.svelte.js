let toasts = $state([]);

export function getToasts() {
  return toasts;
}

export function showToast(message, type = "info", duration = 3000) {
  const id = Date.now() + Math.random();
  toasts = [...toasts, { id, message, type, duration }];
  if (duration > 0) {
    setTimeout(() => removeToast(id), duration);
  }
  return id;
}

export function removeToast(id) {
  toasts = toasts.filter(t => t.id !== id);
}

let confirmations = $state([]);
let nextId = 0;

export function getConfirmations() {
  return confirmations;
}

export function showConfirm(message, title = "Confirm") {
  return new Promise((resolve) => {
    const id = `confirm-${++nextId}-${Date.now()}`;
    confirmations = [...confirmations, { id, message, title, resolve }];
  });
}

export function confirmResponse(id, value) {
  const item = confirmations.find(c => c.id === id);
  if (item) {
    item.resolve(value);
    confirmations = confirmations.filter(c => c.id !== id);
  }
}

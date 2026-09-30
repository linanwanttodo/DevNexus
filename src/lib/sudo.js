// src/lib/sudo.js — 全局 sudo 密码输入对话框（Promise API）
// 并发调用按序排队；null 仅表示用户主动取消；空输入由对话框内校验，不 resolve。
// 安全敏感路径：密码仅存活于输入框引用，resolve 后立即清空，不写日志与持久层。
import { ref } from "vue";

export const sudoState = ref(null);

const sudoQueue = [];

function advanceSudoQueue() {
  const next = sudoQueue.shift();
  if (next) {
    sudoState.value = next;
  } else {
    sudoState.value = null;
  }
}

/**
 * @param {string} message 展示给用户的说明文案
 * @returns {Promise<string|null>} 密码；用户取消返回 null
 */
export function promptSudo(message) {
  return new Promise((resolve) => {
    sudoQueue.push({ message, resolve, error: "" });
    if (sudoState.value === null) advanceSudoQueue();
  });
}

/** 向当前对话框展示校验错误（如后端验证失败），用户可重试或取消 */
export function sudoRejectCurrent(message) {
  const s = sudoState.value;
  if (s) s.error = message;
}

/** 结算当前对话框并推进队列；调用后由 SudoDialog 清空输入引用 */
export function sudoResolve(result) {
  const s = sudoState.value;
  if (s) s.resolve(result);
  advanceSudoQueue();
}

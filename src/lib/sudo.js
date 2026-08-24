// src/lib/sudo.js — 全局 sudo 密码输入对话框（Promise API）
// 用法：const pw = await promptSudo("需要管理员权限以继续 DNS 切换")
// 返回用户输入的密码字符串，取消则返回 null。
import { ref } from "vue";

export const sudoState = ref(null);

/**
 * @param {string} message 展示给用户的说明文案
 * @returns {Promise<string|null>}
 */
export function promptSudo(message) {
  return new Promise((resolve) => {
    sudoState.value = { message, resolve, password: "" };
  });
}

export function sudoResolve(result) {
  const s = sudoState.value;
  if (s) {
    s.resolve(result);
    sudoState.value = null;
  }
}
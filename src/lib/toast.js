// src/lib/toast.js — 基于 sonner 的通知封装（保留原 showToast API）
// 特性：顶部居中、X 关闭、自动消失、点击复制（error 时提供“复制”按钮 + 点击文案即复制）
import { toast } from "vue-sonner";

function copyText(text) {
  const t = String(text ?? "");
  if (!t) return;
  // 优先使用 Clipboard API，Tauri 环境下同样可用
  if (navigator?.clipboard?.writeText) {
    navigator.clipboard.writeText(t)
      .then(() => toast.success("已复制", { duration: 1200, closeButton: true }))
      .catch(() => fallbackCopy(t));
  } else {
    fallbackCopy(t);
  }
}

function fallbackCopy(text) {
  try {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    toast.success("已复制", { duration: 1200, closeButton: true });
  } catch {
    // 忽略
  }
}

function attachClickCopy(id, msg) {
  // 点击 toast 任意空白处即可复制报错；按钮（关闭/X、复制）走各自逻辑，不重复触发
  const bind = () => {
    let el = null;
    if (id != null) el = document.querySelector(`[data-sonner-toast][data-sonner-toast-id="${id}"]`);
    if (!el) el = document.querySelector(`[data-sonner-toast][data-toast-id="${id}"]`);
    if (!el) {
      const all = document.querySelectorAll("[data-sonner-toast]");
      el = all.length ? all[all.length - 1] : null;
    }
    if (!el || el.dataset.copyBound === "1") return;
    el.dataset.copyBound = "1";
    el.style.cursor = "pointer";
    el.title = "点击复制报错";
    el.addEventListener("click", (e) => {
      if (e.target.closest("button")) return;
      copyText(msg);
    });
  };
  // toast 挂载为异步，延迟两帧确保 DOM 已就绪
  requestAnimationFrame(() => requestAnimationFrame(bind));
  setTimeout(bind, 120);
}

export function showToast(message, type = "info", duration = 3500) {
  const msg = String(message ?? "");
  // error 适当延长停留，便于阅读与点击复制；默认 3500ms 自动消失
  const d = type === "error" ? Math.max(duration, 5000) : duration;
  const baseOpts = {
    duration: d,
    closeButton: true,
    dismissible: true,
  };

  // error/warning 提供“点击复制”能力：描述提示 + 右侧“复制”按钮 + 点击整条 toast 也复制
  const copyOpts =
    type === "error" || type === "warning"
      ? {
          description: "点击任意处复制报错 · 右侧“复制”亦可",
          action: {
            label: "复制",
            onClick: () => copyText(msg),
          },
          style: { cursor: "pointer" },
        }
      : {};

  const opts = { ...baseOpts, ...copyOpts };

  let id;
  switch (type) {
    case "success":
      id = toast.success(msg, opts);
      break;
    case "error":
      id = toast.error(msg, opts);
      break;
    case "warning":
      id = toast.warning(msg, opts);
      break;
    case "loading":
      id = toast.loading(msg, opts);
      break;
    default:
      id = toast(msg, opts);
  }
  if (type === "error" || type === "warning") attachClickCopy(id, msg);
  return id;
}
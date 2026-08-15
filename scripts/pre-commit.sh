#!/usr/bin/env bash
# DevNexus pre-commit hook：与 CI 的 Code quality checks 保持一致
# 运行：cargo fmt --check + cargo clippy --all-targets -- -D warnings + pnpm check (vite build)
# 任一步失败则阻止提交，避免 CI 反复构建失败。
set -u

cd "$(git rev-parse --show-toplevel)" || exit 1

MANIFEST="src-tauri/Cargo.toml"
FAILED=0

echo "==> [pre-commit] cargo fmt --check"
if ! cargo fmt --check --manifest-path "$MANIFEST"; then
  echo "!! 代码未格式化。运行: cargo fmt --manifest-path $MANIFEST  然后重新提交"
  FAILED=1
fi

echo ""
echo "==> [pre-commit] cargo clippy --all-targets -- -D warnings"
# --all-targets 覆盖测试代码：CI 的 cargo test 会编译 #[cfg(test)]，
# 不加此参数会漏掉仅存在于测试中的告警（如未使用的 import）。
if ! cargo clippy --manifest-path "$MANIFEST" --all-targets -- -D warnings; then
  echo "!! clippy 存在警告（-D warnings 视为错误）。修复后再提交"
  FAILED=1
fi

echo ""
echo "==> [pre-commit] pnpm check (vite build)"
# 本机可能未安装 pnpm：优先 pnpm check，回退到 npx vite build（等价）
if command -v pnpm >/dev/null 2>&1; then
  if ! pnpm check; then
    echo "!! 前端类型/语法检查失败。修复后再提交"
    FAILED=1
  fi
elif command -v npx >/dev/null 2>&1; then
  if ! npx vite build; then
    echo "!! 前端类型/语法检查失败。修复后再提交"
    FAILED=1
  fi
else
  echo "!! 未找到 pnpm 或 npx，跳过前端检查（请手动运行 vite build）"
fi

echo ""
if [ "$FAILED" -ne 0 ]; then
  echo "==> [pre-commit] ❌ 提交被阻止：请修复上述问题"
  exit 1
fi
echo "==> [pre-commit] ✅ 全部检查通过"
exit 0

#!/usr/bin/env bash
# DevNexus pre-commit hook：与 CI 的 Code quality checks 保持一致
# 运行：cargo fmt --check + cargo clippy -- -D warnings + pnpm check (svelte-check)
# 任一步失败则阻止提交，避免 CI 反复构建失败。
set -u

cd "$(git rev-parse --show-toplevel)" || exit 1

FAILED=0

echo "==> [pre-commit] cargo fmt --check"
if ! cargo fmt --check; then
  echo "!! 代码未格式化。运行: cargo fmt  然后重新提交"
  FAILED=1
fi

echo ""
echo "==> [pre-commit] cargo clippy -- -D warnings"
if ! cargo clippy -- -D warnings; then
  echo "!! clippy 存在警告（-D warnings 视为错误）。修复后再提交"
  FAILED=1
fi

echo ""
echo "==> [pre-commit] pnpm check (svelte-check)"
if ! pnpm check; then
  echo "!! 前端类型/语法检查失败。修复后再提交"
  FAILED=1
fi

echo ""
if [ "$FAILED" -ne 0 ]; then
  echo "==> [pre-commit] ❌ 提交被阻止：请修复上述问题"
  exit 1
fi
echo "==> [pre-commit] ✅ 全部检查通过"
exit 0

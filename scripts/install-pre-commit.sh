#!/usr/bin/env bash
# 安装 DevNexus pre-commit hook：把 scripts/pre-commit.sh 链接到 .git/hooks/pre-commit
set -eu

cd "$(git rev-parse --show-toplevel)" || exit 1
HOOKS_DIR="$(git rev-parse --git-path hooks)"
HOOK="$HOOKS_DIR/pre-commit"
SCRIPT="$PWD/scripts/pre-commit.sh"

if [ ! -f "$SCRIPT" ]; then
  echo "错误: 找不到 $SCRIPT"
  exit 1
fi

ln -sf "$SCRIPT" "$HOOK"
chmod +x "$SCRIPT"
echo "✅ pre-commit hook 已安装: $HOOK -> $SCRIPT"
echo "   后续每次 git commit 会自动运行 fmt/clippy/svelte-check，全部通过才允许提交。"

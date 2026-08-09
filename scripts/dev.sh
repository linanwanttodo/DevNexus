#!/bin/bash
# Load nvm if available (handles IDEs that don't source .bashrc/.zshrc)
if [ -f "$HOME/.nvm/nvm.sh" ]; then
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
fi
# 每次启动开发服务器前清理 Vite 缓存，避免旧构建残留导致 UI 不更新。
rm -rf node_modules/.vite 2>/dev/null || true
exec node_modules/.bin/vite --force "$@"

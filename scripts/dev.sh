#!/bin/bash
# Load nvm if available (handles IDEs that don't source .bashrc/.zshrc)
if [ -f "$HOME/.nvm/nvm.sh" ]; then
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
fi
# 构建模式（tauri build 的 beforeBuildCommand）：执行 vite build，构建完成后必须退出。
# 注意不能写成 `vite --force "$@"`：--force 在前会把 build 当作根目录参数，
# 启动的是 dev server 而不是构建，CI 的 Build Tauri 步骤会永久卡住。
if [ "$1" = "build" ]; then
    exec node_modules/.bin/vite build
fi
# 开发模式（tauri dev 的 beforeDevCommand）：每次启动开发服务器前清理 Vite 缓存，
# 避免旧构建残留导致 UI 不更新。
rm -rf node_modules/.vite 2>/dev/null || true
exec node_modules/.bin/vite --force "$@"

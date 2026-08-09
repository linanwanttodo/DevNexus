#!/bin/bash
# 彻底重建并重启 Tauri 开发环境。
# 用途：当你改了前端/后端代码但运行时没生效（HMR 残留、旧进程未退出等），
# 用本脚本彻底杀掉旧进程、清缓存、完整重新编译启动。
#
# 用法：
#   bash scripts/rebuild.sh          # 编译并启动（前台运行，Ctrl+C 退出）—— 用于 RustRover 运行配置
#   bash scripts/rebuild.sh stop     # 仅停止所有相关进程，不启动
#   bash scripts/rebuild.sh daemon   # 后台启动，输出日志到 rebuild.log —— 用于终端

set -e

# 项目根目录（脚本位于 scripts/ 下）
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 加载 nvm（部分 IDE 不 source .bashrc/.zshrc）
if [ -f "$HOME/.nvm/nvm.sh" ]; then
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
fi

APP_NAME="devnexus"
BIN_NAME="DevNexus"
PORT="1420"

echo "==> [1/4] 停止所有相关进程（vite / cargo / tauri 二进制）..."
# 杀掉监听 1420 端口的进程
if command -v lsof >/dev/null 2>&1; then
    PIDS=$(lsof -ti tcp:"$PORT" 2>/dev/null || true)
    [ -n "$PIDS" ] && kill -9 $PIDS 2>/dev/null || true
fi
# 杀掉 vite 进程
pkill -9 -f "node_modules/.bin/vite" 2>/dev/null || true
pkill -9 -f "vite" 2>/dev/null || true
# 杀掉 cargo 构建进程（避免占用 target 锁）
pkill -9 -f "cargo" 2>/dev/null || true
# 杀掉已运行的 Tauri 应用二进制
pkill -9 -f "$BIN_NAME" 2>/dev/null || true
pkill -9 -f "tauri" 2>/dev/null || true
# 给一点时间让进程退出、释放文件锁
sleep 2

# 仅停止模式
if [ "$1" = "stop" ]; then
    echo "==> 已停止所有相关进程。"
    exit 0
fi

echo "==> [2/4] 清理 Vite 缓存与 Tauri 临时产物..."
rm -rf node_modules/.vite 2>/dev/null || true
rm -rf node_modules/.tmp 2>/dev/null || true
rm -rf src-tauri/target/debug/build 2>/dev/null || true

echo "==> [3/4] 确保依赖是最新（pnpm install）..."
if command -v pnpm >/dev/null 2>&1; then
    pnpm install --frozen-lockfile 2>/dev/null || pnpm install
else
    echo "!! 未找到 pnpm，请先安装 pnpm@9+ 后再运行。" >&2
    exit 1
fi

echo "==> [4/4] 完整重新编译并启动 Tauri（pnpm tauri dev）..."

if [ "$1" = "daemon" ]; then
    echo "==> 后台模式：日志写入 rebuild.log"
    nohup pnpm tauri dev > rebuild.log 2>&1 &
    echo "==> 已后台启动，PID=$!。查看日志：tail -f rebuild.log"
    exit 0
else
    # 前台运行（Ctrl+C 退出）
    exec pnpm tauri dev
fi

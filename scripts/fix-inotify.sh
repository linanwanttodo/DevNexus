#!/usr/bin/env bash
# 解决 Linux 下 `cargo tauri dev` 报 "OS file watch limit reached" 的问题。
# 提升 inotify 文件监视上限，写入 /etc/sysctl.d 实现永久生效（重启后保留）。
# 用法: sudo bash scripts/fix-inotify.sh
set -euo pipefail

CONF_FILE="/etc/sysctl.d/99-inotify.conf"

if [ "$(id -u)" -ne 0 ]; then
  echo "!! 需要 root 权限。请运行: sudo bash scripts/fix-inotify.sh"
  exit 1
fi

echo "==> 写入 inotify 上限配置到 $CONF_FILE"
cat > "$CONF_FILE" <<'EOF'
# DevNexus / Tauri dev: 提升 inotify 文件监视上限
# 避免 `cargo tauri dev` 触发 "OS file watch limit reached"
fs.inotify.max_user_watches = 524288
fs.inotify.max_user_instances = 512
fs.inotify.max_queued_events = 1048576
EOF

echo "==> 立即应用"
sysctl --system >/dev/null

echo "==> 当前值:"
echo "    max_user_watches   = $(cat /proc/sys/fs/inotify/max_user_watches)"
echo "    max_user_instances = $(cat /proc/sys/fs/inotify/max_user_instances)"
echo "    max_queued_events  = $(cat /proc/sys/fs/inotify/max_queued_events)"
echo "✅ 完成。现在可以重新运行 cargo tauri dev"

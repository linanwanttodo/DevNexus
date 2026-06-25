#!/usr/bin/env bash
#
# 生成 Tauri v2 updates.json（全平台 × 全 bundle 格式）
# 用法: generate-updates-json.sh <version> <output-dir>
#
# output-dir 下应包含由 CI 收集的 .sig 文件，命名规则：
#   linux-x86_64.sig         — AppImage 签名
#   linux-x86_64.deb.sig     — deb 签名（如果有）
#   darwin-x86_64.sig        — DMG 签名
#   darwin-aarch64.sig       — DMG 签名
#   windows-x86_64.sig       — NSIS 签名
#   windows-x86_64.msi.sig   — MSI 签名（如果有）
#
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "Usage: $0 <version> <output-dir>"
  exit 1
fi

VERSION="$1"
OUTPUT_DIR="$2"
GH_REPO="${GH_REPO:-linanwanttodo/DevNexus}"
GH_TAG="${GH_TAG:-v${VERSION}}"
RELEASE_BASE="https://github.com/${GH_REPO}/releases/download/${GH_TAG}"
PUB_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

# 从签名文件读取并去除换行
read_signature() {
  local file="$1"
  if [ -f "$file" ]; then
    cat "$file" | tr -d '\n'
  else
    echo ""
  fi
}

# 写入 JSON 头
cat > "$OUTPUT_DIR/updates.json" << EOF
{
  "version": "${VERSION}",
  "notes": "",
  "pub_date": "${PUB_DATE}",
  "platforms": {
EOF

first_platform=true

add_entry() {
  local key="$1"
  local sig_file="$2"
  local url="$3"

  if [ -f "$sig_file" ]; then
    local sig
    sig=$(read_signature "$sig_file")
    if [ -n "$sig" ]; then
      $first_platform || echo "," >> "$OUTPUT_DIR/updates.json"
      cat >> "$OUTPUT_DIR/updates.json" << ENTRY
    "${key}": {
      "signature": "${sig}",
      "url": "${url}"
    }
ENTRY
      first_platform=false
      echo "  ✓ ${key}"
    fi
  fi
}

echo "Generating updates.json for v${VERSION}..."

# ── Linux x86_64 ──────────────────────────────────────────────
# 默认匹配条目（Tauri updater 插件用于 AppImage 安装）
add_entry "linux-x86_64" \
  "$OUTPUT_DIR/linux-x86_64.sig" \
  "${RELEASE_BASE}/DevNexus_${VERSION}_amd64.AppImage.tar.gz"

# 显式 AppImage 条目
add_entry "linux-x86_64-appimage" \
  "$OUTPUT_DIR/linux-x86_64.sig" \
  "${RELEASE_BASE}/DevNexus_${VERSION}_amd64.AppImage.tar.gz"

# deb 条目（用于 deb 安装的用户）
add_entry "linux-x86_64-deb" \
  "$OUTPUT_DIR/linux-x86_64.deb.sig" \
  "${RELEASE_BASE}/DevNexus_${VERSION}_amd64.deb"

# ── macOS aarch64 (Apple Silicon) ────────────────────────────
# 默认匹配条目（Tauri updater 使用 .app.tar.gz）
add_entry "darwin-aarch64" \
  "$OUTPUT_DIR/darwin-aarch64.sig" \
  "${RELEASE_BASE}/DevNexus_aarch64.app.tar.gz"

# 显式 app 条目
add_entry "darwin-aarch64-app" \
  "$OUTPUT_DIR/darwin-aarch64.sig" \
  "${RELEASE_BASE}/DevNexus_aarch64.app.tar.gz"

# ── macOS x86_64 (Intel) ─────────────────────────────────────
# 默认匹配条目
add_entry "darwin-x86_64" \
  "$OUTPUT_DIR/darwin-x86_64.sig" \
  "${RELEASE_BASE}/DevNexus_x64.app.tar.gz"

# 显式 app 条目
add_entry "darwin-x86_64-app" \
  "$OUTPUT_DIR/darwin-x86_64.sig" \
  "${RELEASE_BASE}/DevNexus_x64.app.tar.gz"

# ── Windows x86_64 — NSIS ───────────────────────────────────
# 默认匹配条目（NSIS 安装的用户）
add_entry "windows-x86_64" \
  "$OUTPUT_DIR/windows-x86_64.sig" \
  "${RELEASE_BASE}/DevNexus_${VERSION}_x64-setup.exe.nsis.zip"

# 显式 NSIS 条目
add_entry "windows-x86_64-nsis" \
  "$OUTPUT_DIR/windows-x86_64.sig" \
  "${RELEASE_BASE}/DevNexus_${VERSION}_x64-setup.exe.nsis.zip"

# ── Windows x86_64 — MSI ────────────────────────────────────
add_entry "windows-x86_64-msi" \
  "$OUTPUT_DIR/windows-x86_64.msi.sig" \
  "${RELEASE_BASE}/DevNexus_${VERSION}_x64.msi.zip"

# ── 收尾 ─────────────────────────────────────────────────────
cat >> "$OUTPUT_DIR/updates.json" << EOF
  }
}
EOF

ENTRY_COUNT=$(grep -c '"signature"' "$OUTPUT_DIR/updates.json" || echo 0)
echo ""
echo "[updates.json] generated: ${ENTRY_COUNT} platform entries at ${OUTPUT_DIR}/updates.json"

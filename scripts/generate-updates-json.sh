#!/usr/bin/env bash
#
# 生成 Tauri v2 updates.json 文件
# 用法: generate-updates-json.sh <version> <output-dir>
#
# 依赖环境变量（由 GitHub Actions 提供）:
#   GH_REPO:  GitHub 仓库名，如 "linanwanttodo/DevNexus"
#   GH_TAG:   Git tag，如 "v1.0.3"
#
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "Usage: $0 <version> <output-dir>"
  echo "  e.g. $0 1.0.3 ./dist"
  exit 1
fi

VERSION="$1"
OUTPUT_DIR="$2"
GH_REPO="${GH_REPO:-linanwanttodo/DevNexus}"
GH_TAG="${GH_TAG:-v${VERSION}}"
RELEASE_BASE="https://github.com/${GH_REPO}/releases/download/${GH_TAG}"
PUB_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

# 从传入的签名文件读取
read_signature() {
  local file="$1"
  if [ -f "$file" ]; then
    cat "$file" | tr -d '\n'
  else
    echo ""
  fi
}

cat > "$OUTPUT_DIR/updates.json" << EOF
{
  "version": "${VERSION}",
  "notes": "",
  "pub_date": "${PUB_DATE}",
  "platforms": {
EOF

first_platform=true

# Linux x86_64 — AppImage
if [ -f "$OUTPUT_DIR/linux-x86_64.sig" ]; then
  SIG=$(read_signature "$OUTPUT_DIR/linux-x86_64.sig")
  $first_platform || echo "," >> "$OUTPUT_DIR/updates.json"
  cat >> "$OUTPUT_DIR/updates.json" << EOF
    "linux-x86_64": {
      "signature": "${SIG}",
      "url": "${RELEASE_BASE}/DevNexus_${VERSION}_amd64.AppImage.tar.gz"
    }
EOF
  first_platform=false
fi

# macOS x86_64
if [ -f "$OUTPUT_DIR/darwin-x86_64.sig" ]; then
  SIG=$(read_signature "$OUTPUT_DIR/darwin-x86_64.sig")
  $first_platform || echo "," >> "$OUTPUT_DIR/updates.json"
  cat >> "$OUTPUT_DIR/updates.json" << EOF
    "darwin-x86_64": {
      "signature": "${SIG}",
      "url": "${RELEASE_BASE}/DevNexus_${VERSION}_x64.dmg"
    }
EOF
  first_platform=false
fi

# macOS aarch64 (Apple Silicon)
if [ -f "$OUTPUT_DIR/darwin-aarch64.sig" ]; then
  SIG=$(read_signature "$OUTPUT_DIR/darwin-aarch64.sig")
  $first_platform || echo "," >> "$OUTPUT_DIR/updates.json"
  cat >> "$OUTPUT_DIR/updates.json" << EOF
    "darwin-aarch64": {
      "signature": "${SIG}",
      "url": "${RELEASE_BASE}/DevNexus_${VERSION}_aarch64.dmg"
    }
EOF
  first_platform=false
fi

# Windows x86_64 — MSI
if [ -f "$OUTPUT_DIR/windows-x86_64.sig" ]; then
  SIG=$(read_signature "$OUTPUT_DIR/windows-x86_64.sig")
  $first_platform || echo "," >> "$OUTPUT_DIR/updates.json"
  cat >> "$OUTPUT_DIR/updates.json" << EOF
    "windows-x86_64": {
      "signature": "${SIG}",
      "url": "${RELEASE_BASE}/DevNexus_${VERSION}_x64.msi.zip"
    }
EOF
  first_platform=false
fi

cat >> "$OUTPUT_DIR/updates.json" << EOF
  }
}
EOF

echo "[updates.json] generated for v${VERSION} at ${OUTPUT_DIR}/updates.json"

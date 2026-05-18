#!/usr/bin/env python3
"""
图标转换脚本 — 将高清 PNG 转换为各平台所需格式
用法: python3 scripts/generate_icons.py <source.png>

生成:
  - 32x32.png         (Linux)
  - 128x128.png       (Linux)
  - 128x128@2x.png    (macOS Retina, 256x256)
  - icon.icns         (macOS)
  - icon.ico          (Windows, 含多尺寸)
  - icon.png           (通用 512x512)
"""

import sys
import os
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("请先安装 Pillow: pip install Pillow")
    sys.exit(1)

OUTPUT_DIR = Path("src-tauri/icons")

SIZES = {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "icon.png": 512,
}

# Windows .ico 需要包含的尺寸
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


def generate_pngs(source: Image.Image):
    for filename, size in SIZES.items():
        img = source.resize((size, size), Image.LANCZOS)
        if img.mode != "RGBA":
            img = img.convert("RGBA")
        path = OUTPUT_DIR / filename
        img.save(path, "PNG")
        print(f"  ✓ {filename} ({size}x{size})")


def generate_ico(source: Image.Image):
    """生成 .ico 文件 (256x256 BMP 格式，兼容 Windows RC.EXE)"""
    path = OUTPUT_DIR / "icon.ico"
    img = source.resize((256, 256), Image.LANCZOS)
    img = img.convert("RGBA")

    # 手工构建 ICO: BMP 格式头 + DIB 数据
    import struct
    from io import BytesIO

    # 生成 32-bit BMP (BGRA) 数据
    bmp_buf = BytesIO()
    img_rgba = img.tobytes("raw", "BGRA")
    # DIB header: 40 bytes
    dib_header = struct.pack(
        "<IiiHHIIiiII",
        40, 256, 512, 1, 32, 0,
        len(img_rgba), 0, 0, 0, 0,
    )
    bmp_buf.write(dib_header)
    bmp_buf.write(img_rgba)
    bmp_data = bmp_buf.getvalue()

    # ICO header + directory + image
    with open(path, "wb") as f:
        f.write(struct.pack("<HHH", 0, 1, 1))  # ICO header: 1 icon
        # Directory entry
        total_size = 6 + 16 + len(bmp_data)
        f.write(struct.pack(
            "<BBBBHHII",
            0, 0, 1, 0, 1, 32,
            len(bmp_data),
            6 + 16,  # offset
        ))
        f.write(bmp_data)

    print(f"  ✓ icon.ico (256x256 BMP)")


def generate_icns(source: Image.Image):
    """生成 macOS .icns 文件"""
    path = OUTPUT_DIR / "icon.icns"
    # macOS 需要的尺寸
    icon_sizes = {
        "icp4": 16,
        "icp5": 32,
        "icp6": 64,
        "ic07": 128,
        "ic08": 256,
        "ic09": 512,
    }

    frames = []
    for name, size in icon_sizes.items():
        img = source.resize((size, size), Image.LANCZOS)
        if img.mode != "RGBA":
            img = img.convert("RGBA")
        frames.append((name, img))

    # Pillow 10+ 原生支持 ICNS
    frames[0][1].save(path, format="ICNS", append_images=[f[1] for f in frames[1:]])
    print(f"  ✓ icon.icns ({', '.join(f'{n}({s})' for n, s in icon_sizes.items())})")


def main():
    if len(sys.argv) < 2:
        source_path = OUTPUT_DIR / "DevNexus.png"
    else:
        source_path = Path(sys.argv[1])

    if not source_path.exists():
        print(f"错误: 找不到源文件 {source_path}")
        sys.exit(1)

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    source = Image.open(source_path)
    print(f"源文件: {source_path} ({source.size[0]}x{source.size[1]})")
    print(f"输出目录: {OUTPUT_DIR}")

    generate_pngs(source)
    generate_ico(source)
    generate_icns(source)

    print("\n全部完成! 图标已生成到 src-tauri/icons/")


if __name__ == "__main__":
    main()

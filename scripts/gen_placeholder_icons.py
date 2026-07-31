#!/usr/bin/env python3
"""
为 last_port 生成最小占位图标 (C1)。
生成 32x32/128x128/256x256 PNG + 占位 icns/ico (实际只是改名)。
正式图标在 C7 用 tauri icon 命令从源图生成。
"""
import struct
import zlib
import os
import sys

OUT_DIR = sys.argv[1] if len(sys.argv) > 1 else "src-tauri/icons"
os.makedirs(OUT_DIR, exist_ok=True)


def make_png(size: int, color=(46, 160, 67)) -> bytes:
    """生成纯色方形 PNG (RGBA)。"""
    width = height = size
    r, g, b = color
    # 每行: 1 byte filter + width * 4 bytes RGBA
    row = b"\x00" + (bytes([r, g, b, 255]) * width)
    raw = row * height
    compressed = zlib.compress(raw, 9)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", compressed) + chunk(b"IEND", b"")


def main() -> None:
    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
    }
    for name, size in sizes.items():
        path = os.path.join(OUT_DIR, name)
        with open(path, "wb") as f:
            f.write(make_png(size))
        print(f"wrote {path} ({size}x{size})")

    # 占位 .ico / .icns — C7 用 tauri icon 重新生成。
    # 这里简单复制 128 png 作为占位,Tauri 仅在对应平台构建时严格校验。
    import shutil
    shutil.copyfile(
        os.path.join(OUT_DIR, "128x128.png"),
        os.path.join(OUT_DIR, "icon.ico"),
    )
    shutil.copyfile(
        os.path.join(OUT_DIR, "128x128@2x.png"),
        os.path.join(OUT_DIR, "icon.icns"),
    )
    print(f"wrote {OUT_DIR}/icon.ico (placeholder)")
    print(f"wrote {OUT_DIR}/icon.icns (placeholder)")


if __name__ == "__main__":
    main()
#!/usr/bin/env python3
"""
Generate a real application icon for last_port.

Design:
- 1024x1024 PNG (master for tauri icon)
- Rounded rectangle with green/teal gradient (last_port accent color #4ec9b0)
- Center: bold "L" letter (white) with subtle shadow
- Bottom-right: small serial port waveform decoration (3 horizontal bars)
- Renders cleanly at small sizes (16px, 32px) thanks to bold geometric forms

Output: src-tauri/icons/source-icon.png
"""
from PIL import Image, ImageDraw, ImageFilter, ImageFont
import os

OUT_PATH = "src-tauri/icons/source-icon.png"
SIZE = 1024


def find_font(size: int):
    """Try a few common monospace/bold fonts; fall back to default."""
    candidates = [
        # Linux
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        # macOS
        "/System/Library/Fonts/SFCompact.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        # Windows
        "C:/Windows/Fonts/segoeuib.ttf",
        "C:/Windows/Fonts/arialbd.ttf",
    ]
    for path in candidates:
        if os.path.exists(path):
            try:
                return ImageFont.truetype(path, size)
            except Exception:
                continue
    return ImageFont.load_default()


def gradient_color(t: float):
    """Top: brighter teal #4ec9b0, bottom: darker teal #2d7165."""
    top = (78, 201, 176)
    bottom = (45, 113, 101)
    r = int(top[0] + (bottom[0] - top[0]) * t)
    g = int(top[1] + (bottom[1] - top[1]) * t)
    b = int(top[2] + (bottom[2] - top[2]) * t)
    return (r, g, b)


def main() -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Rounded-rectangle background with vertical gradient
    radius = SIZE // 6  # ~170px corner
    bg_layer = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    bg_draw = ImageDraw.Draw(bg_layer)
    for y in range(SIZE):
        t = y / (SIZE - 1)
        bg_draw.line([(0, y), (SIZE, y)], fill=gradient_color(t))
    # Apply rounded mask
    mask = Image.new("L", (SIZE, SIZE), 0)
    mask_draw = ImageDraw.Draw(mask)
    mask_draw.rounded_rectangle((0, 0, SIZE - 1, SIZE - 1), radius=radius, fill=255)
    img.paste(bg_layer, (0, 0), mask)

    draw = ImageDraw.Draw(img)

    # Decorative serial-port waveform at the bottom
    # Three short horizontal bars (TX / RX indicators)
    bar_color = (255, 255, 255, 100)
    bar_x_start = SIZE * 0.62
    bar_x_end = SIZE * 0.86
    bar_y = [SIZE * 0.74, SIZE * 0.81, SIZE * 0.88]
    bar_thickness = SIZE // 60
    for y in bar_y:
        draw.rounded_rectangle(
            (bar_x_start, y, bar_x_end, y + bar_thickness),
            radius=bar_thickness // 2,
            fill=bar_color,
        )

    # Center: bold "L" letter
    font = find_font(int(SIZE * 0.62))
    text = "L"
    # Get bbox to center text
    bbox = draw.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = (SIZE - tw) // 2 - bbox[0]
    ty = (SIZE - th) // 2 - bbox[1] - SIZE * 0.05

    # Shadow layer
    shadow_offset = SIZE // 80
    shadow_layer = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    shadow_draw = ImageDraw.Draw(shadow_layer)
    shadow_draw.text((tx + shadow_offset, ty + shadow_offset), text, font=font, fill=(0, 0, 0, 80))
    shadow_layer = shadow_layer.filter(ImageFilter.GaussianBlur(radius=SIZE // 100))
    img.alpha_composite(shadow_layer)

    # Main letter
    draw.text((tx, ty), text, font=font, fill=(255, 255, 255, 255))

    # Save
    os.makedirs(os.path.dirname(OUT_PATH), exist_ok=True)
    img.convert("RGB").save(OUT_PATH, "PNG", optimize=True)
    print(f"Wrote {OUT_PATH} ({SIZE}x{SIZE})")


if __name__ == "__main__":
    main()
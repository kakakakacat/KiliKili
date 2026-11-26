#!/usr/bin/env python3
"""从透明底源图生成各平台应用图标（#250）。

唯一的源是 assets/app_icon/source_transparent.png（1024²，透明底粉色标记）。
本脚本只负责 flutter_launcher_icons / flutter_native_splash 管不到的产物，
以及它们的输入图；跑完再执行：

    dart run flutter_launcher_icons
    dart run flutter_native_splash:create

依赖 Pillow（`pip install pillow numpy`），不进仓库依赖。

用法：
    python3 tool/app-icon-generate.py extract   # 一次性：从旧深色图标反解透明源
    python3 tool/app-icon-generate.py generate  # 由透明源生成白底图标与启动图
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "assets/app_icon/source_transparent.png"
PINK = np.array([252, 101, 140], dtype=np.float64)
NAVY = np.array([16, 22, 40], dtype=np.float64)  # 旧图标底色 #101628
WHITE = (255, 255, 255)


def extract(dark_icon: Path) -> None:
    """把「粉色画在深蓝底上」的旧图标反混合成透明底。

    每个像素视为 alpha·粉 + (1-alpha)·深蓝，沿两色连线投影求 alpha。
    抗锯齿边缘因此得到连续的 alpha，而不是按阈值抠出锯齿。
    """
    rgb = np.asarray(Image.open(dark_icon).convert("RGB"), dtype=np.float64)
    axis = PINK - NAVY
    alpha = ((rgb - NAVY) @ axis) / (axis @ axis)
    alpha = np.clip(alpha, 0.0, 1.0)
    out = np.empty(rgb.shape[:2] + (4,), dtype=np.uint8)
    out[..., :3] = PINK.astype(np.uint8)
    out[..., 3] = np.round(alpha * 255).astype(np.uint8)
    Image.fromarray(out, "RGBA").save(SOURCE, optimize=True)
    print(f"wrote {SOURCE.relative_to(ROOT)}")


def on_white(source: Image.Image, size: int) -> Image.Image:
    """白底不透明：iOS 不接受带 alpha 的图标，其余平台按所有者决定统一白底。"""
    canvas = Image.new("RGBA", (size, size), WHITE + (255,))
    canvas.alpha_composite(source.resize((size, size), Image.LANCZOS))
    return canvas.convert("RGB")


def macos_tile(source: Image.Image) -> Image.Image:
    """沿用旧 macos.png 的构图：1024 画布、四周 100px 透明边、中间 824 方块。"""
    canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    ImageDraw.Draw(canvas).rectangle((100, 100, 923, 923), fill=WHITE + (255,))
    canvas.alpha_composite(source.resize((824, 824), Image.LANCZOS), (100, 100))
    return canvas


def android12_splash(source: Image.Image) -> Image.Image:
    """Android 12+ 启动画面图标：1152² 画布，内容须落在直径 768 的圆内。

    透明底，不配 icon_background_color：浅色白底、深色 #101628 上都直接
    显示粉色标记，不再是「白底上一块深色方块」。
    """
    box = source.getbbox()
    mark = source.crop(box)
    w, h = mark.size
    # 外接矩形对角线不超过 700，给 768 圆留一点余量。
    scale = 700 / (w * w + h * h) ** 0.5
    mark = mark.resize((round(w * scale), round(h * scale)), Image.LANCZOS)
    canvas = Image.new("RGBA", (1152, 1152), (0, 0, 0, 0))
    canvas.alpha_composite(
        mark, ((1152 - mark.width) // 2, (1152 - mark.height) // 2)
    )
    return canvas


def generate() -> None:
    source = Image.open(SOURCE).convert("RGBA")
    if source.size != (1024, 1024):
        sys.exit(f"{SOURCE} 应为 1024²，实际 {source.size}")

    def save(img: Image.Image, rel: str, **kw) -> None:
        path = ROOT / rel
        img.save(path, optimize=True, **kw)
        print(f"wrote {rel}")

    # flutter_launcher_icons 的输入（iOS / Android 旧式图标）。
    save(on_white(source, 1024), "assets/app_icon/icon.png")
    # flutter_launcher_icons 的 macOS 输入。
    save(macos_tile(source), "assets/app_icon/macos.png")
    # flutter_native_splash 的 android_12 输入。
    save(android12_splash(source), "assets/app_icon/splash_android12.png")

    # 以下是插件不管的产物，与 87464e02e 那次更新覆盖的文件一致。
    w512 = on_white(source, 512)
    for rel in (
        "android/app/src/main/ic_launcher-playstore.png",
        "assets/images/logo/logo.png",  # 关于页；Linux 打包拷它做图标
        "assets/images/logo/desktop/logo_large.png",  # 非 Windows 托盘
    ):
        save(w512, rel)
    ico_sizes = [(s, s) for s in (16, 24, 32, 48, 64, 128, 256)]
    for rel in (
        "windows/runner/resources/app_icon.ico",  # exe 与安装包
        "assets/images/logo/ico/app_icon.ico",  # Windows 托盘
    ):
        save(on_white(source, 256), rel, sizes=ico_sizes)


if __name__ == "__main__":
    match sys.argv[1:]:
        case ["extract"]:
            extract(ROOT / "assets/app_icon/icon.png")
        case ["generate"]:
            generate()
        case _:
            sys.exit(__doc__)

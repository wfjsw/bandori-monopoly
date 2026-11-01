#!/usr/bin/env python3
"""Name-banner SVG generator (sticker-style title over a dotted pill).

Everything is vector. Text, length and colors are parameters.
Default output reproduces the 430x115 MATSUBARA KANON banner. The app does not
use this file for the roster -- it draws the same shape live (webui/src/ui/
NameBanner.tsx, geometry in webui/src/core/nameBanner.ts); this tool is for
offline assets.

Text modes
  default         live <text> (editable in any SVG editor; look depends on installed fonts)
  --title-font / --subtitle-font PATH
                  convert that text to vector outlines with fontTools, so it looks
                  identical everywhere (recommended: M PLUS Rounded 1c ExtraBold for
                  the title, Poppins Bold for the subtitle - both free on Google Fonts)

Examples
  python3 banner.py
  python3 banner.py --width 640 --color "#ff7eb6" --title "星野歩" --subtitle "HOSHINO AYUMU"
  python3 banner.py --title-font MPLUSRounded1c-ExtraBold.ttf --subtitle-font Poppins-Bold.ttf
"""
import argparse
import math
from xml.sax.saxutils import escape

H = 115.0  # banner height (layout below is measured from the 430x115 original)


# ---------------------------------------------------------------- helpers
def star_path(cx, cy, r_out, r_in_ratio=0.5, rot=-90):
    pts = []
    for i in range(10):
        r = r_out if i % 2 == 0 else r_out * r_in_ratio
        a = math.radians(rot + i * 36)
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return "M" + " L".join(f"{x:.2f},{y:.2f}" for x, y in pts) + "Z"


def rounded_star_path(cx, cy, r_out, fatness=0.55, tip_round=2.5, inner_round=1.0, rot=-90):
    """Five-point star with smoothly rounded tips and inner corners (crisp, no blur).

    tip_round / inner_round: distance (px) along each edge that the corner curve uses.
    """
    pts = []
    for i in range(10):
        r = r_out if i % 2 == 0 else r_out * fatness
        a = math.radians(rot + i * 36)
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    n = len(pts)
    segs = []
    for i in range(n):
        p, v, q = pts[i - 1], pts[i], pts[(i + 1) % n]
        d = tip_round if i % 2 == 0 else inner_round

        def toward(a, b):
            L = math.hypot(b[0] - a[0], b[1] - a[1])
            t = min(d / L, 0.49) if L else 0
            return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)

        segs.append((toward(v, p), v, toward(v, q)))
    f = lambda pt: f"{pt[0]:.2f},{pt[1]:.2f}"
    d = f"M{f(segs[0][2])}"
    for a, v, b in segs[1:] + segs[:1]:
        d += f" L{f(a)} Q{f(v)} {f(b)}"
    return d + "Z"


def outline_text(font_path, text, size, spacing, cx, baseline):
    """Return an SVG path 'd' for `text`, horizontally centred on cx."""
    from fontTools.ttLib import TTFont
    from fontTools.pens.svgPathPen import SVGPathPen
    from fontTools.pens.transformPen import TransformPen

    font = TTFont(font_path)
    gs, cmap = font.getGlyphSet(), font.getBestCmap()
    s = size / font["head"].unitsPerEm
    names = [cmap.get(ord(c), ".notdef") for c in text]
    advs = [font["hmtx"][n][0] * s for n in names]
    total = sum(advs) + spacing * (len(text) - 1)
    x, parts = cx - total / 2, []
    for n, adv in zip(names, advs):
        pen = SVGPathPen(gs, ntos=lambda v: f"{v:.2f}".rstrip("0").rstrip("."))
        gs[n].draw(TransformPen(pen, (s, 0, 0, -s, x, baseline)))
        parts.append(pen.getCommands())
        x += adv + spacing
    return " ".join(p for p in parts if p)


# ---------------------------------------------------------------- generator
def make_banner(title="松原花音",
                subtitle="MATSUBARA KANON",
                width=430,
                color="#44ddff",            # pill + title outline
                title_color="#ffffff",      # title letters
                title_outline_color=None,   # defaults to `color`
                rim_color="#ffffff",        # outer rim around title and pill
                text_color="#ffffff",       # subtitle
                star_color="#fdee5b",
                star_size=11.5,             # outer radius (before tip rounding)
                star_round=3.0,             # tip rounding, px along each edge; 0 = sharp
                star_inner_round=1.0,       # rounding of the inner corners
                star_soft=0.0,              # optional edge blur; 0 = crisp
                star_fatness=0.55,          # inner/outer radius ratio; higher = chubbier
                dot_color="#ffffff",
                dot_opacity=0.24,
                dots=True,
                dots_length=150,            # length of dotted zone at right end
                title_size=57,
                title_base=60.5,
                title_spacing=0.3,
                title_weight=700,
                title_outline=14,           # blue outline width (stroke, both sides)
                rim=2.8,                    # white rim thickness
                subtitle_size=19,
                subtitle_spacing=2.3,
                title_font=None,            # .ttf/.otf path -> outlined title
                subtitle_font=None):        # .ttf/.otf path -> outlined subtitle
    W = float(width)
    title_outline_color = title_outline_color or color
    cx = W / 2

    # pill (outer = white rim, inner = colour)
    py, ph = 61.0, 54.0
    pr = ph / 2
    ix, iy, iw, ih = rim, py + rim, W - 2 * rim, ph - 2 * rim
    pill_mid = py + ph / 2

    # subtitle baseline
    sub_base = pill_mid + subtitle_size * 0.357   # Poppins cap-height ~0.698em, +optical nudge

    fam_title = ("'M PLUS Rounded 1c','Rounded Mplus 1c Bold','Rounded Mplus 1c',"
                 "'Hiragino Maru Gothic ProN','Kosugi Maru','Arial Rounded MT Bold',"
                 "'Noto Sans JP','Noto Sans CJK JP',sans-serif")
    fam_sub = "'Poppins','Montserrat','Helvetica Neue',Arial,sans-serif"

    def title_el(extra):
        if title_font:
            d = outline_text(title_font, title, title_size, title_spacing, cx, title_base)
            return f'<path d="{d}" {extra}/>'
        return (f'<text x="{cx + title_spacing / 2:g}" y="{title_base:g}" text-anchor="middle" '
                f'font-family="{fam_title}" font-size="{title_size:g}" font-weight="{title_weight}" '
                f'letter-spacing="{title_spacing:g}" {extra}>{escape(title)}</text>')

    if subtitle_font:
        d = outline_text(subtitle_font, subtitle, subtitle_size, subtitle_spacing, cx, sub_base)
        sub_el = f'<path d="{d}" fill="{text_color}"/>'
    else:
        sub_el = (f'<text x="{cx + subtitle_spacing / 2:g}" y="{sub_base:.2f}" text-anchor="middle" '
                  f'font-family="{fam_sub}" font-size="{subtitle_size:g}" font-weight="700" '
                  f'letter-spacing="{subtitle_spacing:g}" fill="{text_color}">{escape(subtitle)}</text>')

    # dotted zone: staggered lattice, 45-degree cut on the left, anchored to right end
    dot_px, dot_py, dot_r = 17.5, 17.0, 2.5
    anchor_x, anchor_y = W - 149.5, 109.0          # one lattice dot sits here
    pat_x = anchor_x % dot_px
    pat_y = anchor_y % dot_py
    cut_bottom = W - dots_length                    # left edge at pill bottom...
    cut_top = cut_bottom + ih                       # ...slants 45 deg to the right

    # estimated title width: strip under it hides the pill's white border between letters
    est = sum(title_size if ord(c) >= 0x2E80 else title_size * 0.6 for c in title)
    est += title_spacing * (len(title) - 1)
    band = (f'<rect x="{cx - est / 2 + 3:.1f}" y="{py:g}" width="{max(0, est - 6):.1f}" '
            f'height="{rim + 2:g}" fill="{title_outline_color}"/>')

    star_left, star_right = 31.5, W - 29.5    # measured; the original is 1px right-heavy
    star_cy = pill_mid - 0.5
    soft_filter = (f'''    <filter id="soft" filterUnits="userSpaceOnUse" x="0" y="{py:g}" width="{W:g}" height="{ph:g}">
      <feGaussianBlur stdDeviation="{star_soft:g}"/>
    </filter>
''' if star_soft > 0 else '')

    out = [f'''<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{W:g}" height="{H:g}" viewBox="0 0 {W:g} {H:g}" role="img" aria-label="{escape(title)} {escape(subtitle)}">
  <defs>
    <clipPath id="dotClip">
      <polygon points="{cut_top:g},{iy:g} {W:g},{iy:g} {W:g},{iy + ih:g} {cut_bottom:g},{iy + ih:g}"/>
    </clipPath>
    <clipPath id="pillInner">
      <rect x="{ix:g}" y="{iy:g}" width="{iw:g}" height="{ih:g}" rx="{pr - rim:g}"/>
    </clipPath>
    <pattern id="dots" x="{pat_x:g}" y="{pat_y:g}" width="{dot_px:g}" height="{dot_py:g}" patternUnits="userSpaceOnUse">
      <g fill="{dot_color}" fill-opacity="{dot_opacity:g}">
      <circle cx="0" cy="0" r="{dot_r:g}"/><circle cx="{dot_px:g}" cy="0" r="{dot_r:g}"/>
      <circle cx="0" cy="{dot_py:g}" r="{dot_r:g}"/><circle cx="{dot_px:g}" cy="{dot_py:g}" r="{dot_r:g}"/>
      <circle cx="{dot_px / 2:g}" cy="{dot_py / 2:g}" r="{dot_r:g}"/>
      </g>
    </pattern>
{soft_filter}  </defs>

  <!-- 1. white rim: pill outer + title outer stroke (they merge into one sticker shape) -->
  <rect x="0" y="{py:g}" width="{W:g}" height="{ph:g}" rx="{pr:g}" fill="{rim_color}"/>
  {title_el(f'fill="{rim_color}" stroke="{rim_color}" stroke-width="{title_outline + 2 * rim:g}" stroke-linejoin="round" stroke-linecap="round"')}

  <!-- 2. pill body -->
  <rect x="{ix:g}" y="{iy:g}" width="{iw:g}" height="{ih:g}" rx="{pr - rim:g}" fill="{color}"/>
''']
    if dots:
        out.append(f'''  <g clip-path="url(#pillInner)"><g clip-path="url(#dotClip)">
    <rect x="0" y="{iy:g}" width="{W:g}" height="{ih:g}" fill="url(#dots)"/>
  </g></g>
''')
    out.append(f'''
  <!-- 3. stars -->
  <g fill="{star_color}"{' filter="url(#soft)"' if star_soft > 0 else ''}>
    <path d="{rounded_star_path(star_left, star_cy, star_size, star_fatness, star_round, star_inner_round)}"/>
    <path d="{rounded_star_path(star_right, star_cy, star_size, star_fatness, star_round, star_inner_round)}"/>
  </g>

  <!-- 4. subtitle -->
  {sub_el}

  <!-- 5. title: coloured outline, then letters -->
  {band}
  {title_el(f'fill="{title_outline_color}" stroke="{title_outline_color}" stroke-width="{title_outline:g}" stroke-linejoin="round" stroke-linecap="round"')}
  {title_el(f'fill="{title_color}"')}
</svg>
''')
    return "".join(out)


def main():
    ap = argparse.ArgumentParser(description="Generate a name-banner SVG")
    ap.add_argument("--title", default="松原花音")
    ap.add_argument("--subtitle", default="MATSUBARA KANON")
    ap.add_argument("--width", type=float, default=430, help="banner length in px (height is 115)")
    ap.add_argument("--color", default="#44ddff", help="pill and title-outline colour")
    ap.add_argument("--title-color", default="#ffffff")
    ap.add_argument("--title-outline-color", default=None, help="defaults to --color")
    ap.add_argument("--rim-color", default="#ffffff")
    ap.add_argument("--text-color", default="#ffffff", help="subtitle colour")
    ap.add_argument("--star-color", default="#fdee5b")
    ap.add_argument("--star-size", type=float, default=11.5)
    ap.add_argument("--star-round", type=float, default=3.0, help="tip rounding; 0 = sharp")
    ap.add_argument("--star-inner-round", type=float, default=1.0, help="inner-corner rounding")
    ap.add_argument("--star-soft", type=float, default=0.0, help="edge blur; 0 = crisp")
    ap.add_argument("--star-fatness", type=float, default=0.55, help="inner/outer radius ratio")
    ap.add_argument("--dot-color", default="#ffffff")
    ap.add_argument("--dot-opacity", type=float, default=0.24)
    ap.add_argument("--dots-length", type=float, default=150)
    ap.add_argument("--no-dots", action="store_true")
    ap.add_argument("--title-size", type=float, default=57)
    ap.add_argument("--title-base", type=float, default=60.5, help="title baseline y")
    ap.add_argument("--title-weight", type=int, default=700)
    ap.add_argument("--title-spacing", type=float, default=0.3)
    ap.add_argument("--subtitle-size", type=float, default=19)
    ap.add_argument("--subtitle-spacing", type=float, default=2.3)
    ap.add_argument("--title-font", help="font file: outline the title")
    ap.add_argument("--subtitle-font", help="font file: outline the subtitle")
    ap.add_argument("-o", "--output", default="banner.svg")
    a = ap.parse_args()
    svg = make_banner(title=a.title, subtitle=a.subtitle, width=a.width, color=a.color,
                      title_color=a.title_color, title_outline_color=a.title_outline_color,
                      rim_color=a.rim_color, text_color=a.text_color, star_color=a.star_color,
                      star_size=a.star_size, star_round=a.star_round, star_soft=a.star_soft,
                      star_fatness=a.star_fatness, star_inner_round=a.star_inner_round,
                      dot_color=a.dot_color, dot_opacity=a.dot_opacity, dots=not a.no_dots,
                      dots_length=a.dots_length, title_size=a.title_size, title_base=a.title_base,
                      title_spacing=a.title_spacing, title_weight=a.title_weight, subtitle_size=a.subtitle_size,
                      subtitle_spacing=a.subtitle_spacing, title_font=a.title_font,
                      subtitle_font=a.subtitle_font)
    with open(a.output, "w", encoding="utf-8") as f:
        f.write(svg)
    print("wrote", a.output)


if __name__ == "__main__":
    main()

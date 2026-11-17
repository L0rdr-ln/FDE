#!/usr/bin/env python3
"""Draws the FDE icon theme (data/icons/FDE): glossy glass tiles with a white glyph, original artwork.

    python3 tools/make-icons.py        # writes data/icons/FDE/scalable/*/*.svg and index.theme

Each icon is a 128 x 128 SVG. The output is committed, so nothing needs to be built to use it.
"""
import math, os, textwrap

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "icons", "FDE")

# colours of the tiles: (light top, dark bottom)
BLUE = ("#6fc3ff", "#1d6fd1")
SKY = ("#8ad6ff", "#2c93e0")
STEEL = ("#a7bdd6", "#54708f")
TEAL = ("#5fe0df", "#168f9d")
GREEN = ("#8ee24a", "#2e9b22")
ORANGE = ("#ffbe55", "#e06d12")
RED = ("#ff8a6a", "#cc2c1a")
PURPLE = ("#c39bff", "#6a3dc4")
PINK = ("#ff9ccf", "#d23a8a")
GREY = ("#d6dde6", "#7d8996")
YELLOW = ("#ffe56a", "#e1a410")

W = "#ffffff"


def tile(name, colors, glyph, shadow=True):
    top, bot = colors
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
<defs>
<linearGradient id="t" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bot}"/></linearGradient>
<linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".75"/><stop offset="1" stop-color="#fff" stop-opacity=".08"/></linearGradient>
<linearGradient id="r" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".0"/><stop offset="1" stop-color="#fff" stop-opacity=".35"/></linearGradient>
<filter id="s" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur in="SourceAlpha" stdDeviation="2.2"/><feOffset dy="2.5"/><feComponentTransfer><feFuncA type="linear" slope=".45"/></feComponentTransfer><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter>
</defs>
<rect x="9" y="9" width="110" height="110" rx="27" fill="#000" fill-opacity=".18" transform="translate(0 3)"/>
<rect x="8" y="8" width="112" height="112" rx="27" fill="url(#t)"/>
<rect x="8" y="64" width="112" height="56" rx="27" fill="url(#r)"/>
<rect x="8.75" y="8.75" width="110.5" height="110.5" rx="26.5" fill="none" stroke="#fff" stroke-opacity=".55" stroke-width="1.5"/>
<path d="M12 56 C12 26 30 12 64 12 C98 12 116 26 116 56 C96 46 76 44 64 44 C52 44 32 46 12 56 Z" fill="url(#g)"/>
<g filter="url(#s)" fill="{W}" stroke="none">{glyph}</g>
</svg>
'''


def plain(glyph_svg):
    """An icon that is its own shape (folders, dialogs)."""
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
<defs>
<linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".75"/><stop offset="1" stop-color="#fff" stop-opacity=".05"/></linearGradient>
<filter id="s" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur in="SourceAlpha" stdDeviation="2.5"/><feOffset dy="3"/><feComponentTransfer><feFuncA type="linear" slope=".4"/></feComponentTransfer><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter>
</defs>
{glyph_svg}
</svg>
'''


def poly(points):
    return "M" + " L".join(f"{x:.1f} {y:.1f}" for x, y in points) + " Z"


def gear(cx, cy, r_out, r_in, teeth, hole):
    pts = []
    for i in range(teeth * 4):
        a = (i / (teeth * 4)) * 2 * math.pi - math.pi / (teeth * 4)
        phase = i % 4
        r = r_out if phase in (1, 2) else r_in
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    d = poly(pts)
    return f'<path d="{d} M{cx + hole} {cy} A{hole} {hole} 0 1 0 {cx - hole} {cy} A{hole} {hole} 0 1 0 {cx + hole} {cy} Z" fill-rule="evenodd"/>'


def arcs_wifi(cx, cy, radii, width=7):
    out = ""
    for r in radii:
        a = math.radians(42)
        x1, y1 = cx - r * math.sin(a), cy - r * math.cos(a)
        x2, y2 = cx + r * math.sin(a), cy - r * math.cos(a)
        out += f'<path d="M{x1:.1f} {y1:.1f} A{r} {r} 0 0 1 {x2:.1f} {y2:.1f}" fill="none" stroke="{W}" stroke-width="{width}" stroke-linecap="round"/>'
    return out


# ------------------------------------------------------------------ glyphs (128 box, drawn in white)
G = {}
G["terminal"] = f'<path d="M34 42 L58 64 L34 86" fill="none" stroke="{W}" stroke-width="11" stroke-linecap="round" stroke-linejoin="round"/><rect x="64" y="80" width="30" height="10" rx="5"/>'
G["folder_small"] = '<path d="M28 46 h26 l8 9 h38 a6 6 0 0 1 6 6 v30 a6 6 0 0 1 -6 6 h-72 a6 6 0 0 1 -6 -6 v-39 a6 6 0 0 1 6 -6 z"/>'
G["home"] = '<path d="M64 28 L98 58 H88 V92 H72 V70 H56 V92 H40 V58 H30 Z"/>'
G["trash"] = f'<rect x="38" y="42" width="52" height="8" rx="4"/><rect x="54" y="33" width="20" height="9" rx="4"/><path d="M42 54 H86 L83 96 a5 5 0 0 1 -5 4 H50 a5 5 0 0 1 -5 -4 Z"/><path d="M55 62 v28 M64 62 v28 M73 62 v28" stroke="#0003" stroke-width="3.5" stroke-linecap="round"/>'
G["document"] = f'<path d="M36 28 H72 L92 48 V96 a5 5 0 0 1 -5 5 H41 a5 5 0 0 1 -5 -5 Z"/><path d="M72 28 V48 H92 Z" fill="#0003"/><path d="M46 62 H82 M46 74 H82 M46 86 H70" stroke="#0004" stroke-width="5" stroke-linecap="round"/>'
G["globe"] = f'<circle cx="64" cy="64" r="34" fill="none" stroke="{W}" stroke-width="8"/><ellipse cx="64" cy="64" rx="15" ry="34" fill="none" stroke="{W}" stroke-width="6"/><path d="M30 64 H98 M36 46 H92 M36 82 H92" stroke="{W}" stroke-width="5" fill="none"/>'
G["mail"] = f'<rect x="28" y="40" width="72" height="50" rx="8"/><path d="M32 46 L64 70 L96 46" fill="none" stroke="#0005" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>'
G["gear"] = gear(64, 64, 36, 28, 8, 12)
G["lock"] = f'<rect x="36" y="58" width="56" height="42" rx="9"/><path d="M46 58 V48 a18 18 0 0 1 36 0 V58" fill="none" stroke="{W}" stroke-width="9"/><circle cx="64" cy="77" r="6" fill="#0005"/>'
G["logout"] = f'<path d="M38 30 H70 V42 H50 V86 H70 V98 H38 Z"/><path d="M76 50 L102 64 L76 78 V69 H60 V59 H76 Z"/>'
G["moon"] = '<path d="M80 30 A38 38 0 1 0 98 80 A30 30 0 0 1 80 30 Z"/>'
G["reboot"] = f'<path d="M92 54 A30 30 0 1 0 94 76" fill="none" stroke="{W}" stroke-width="10" stroke-linecap="round"/><path d="M96 34 L98 62 L70 56 Z"/>'
G["power"] = f'<path d="M46 44 A34 34 0 1 0 82 44" fill="none" stroke="{W}" stroke-width="10" stroke-linecap="round"/><path d="M64 28 V62" stroke="{W}" stroke-width="10" stroke-linecap="round"/>'
G["wifi"] = arcs_wifi(64, 96, [18, 34, 50], 8) + '<circle cx="64" cy="92" r="7"/>'
G["wired"] = f'<rect x="38" y="30" width="52" height="30" rx="5"/><path d="M48 60 V70 M60 60 V70 M72 60 V70 M84 60 V70" stroke="{W}" stroke-width="6"/><path d="M64 70 V82 M64 82 H40 V96 M64 82 H88 V96" fill="none" stroke="{W}" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"/><rect x="30" y="94" width="20" height="12" rx="3"/><rect x="78" y="94" width="20" height="12" rx="3"/>'
G["speaker"] = '<path d="M28 54 H44 L64 36 V92 L44 74 H28 Z"/>'
def volume(waves, muted=False):
    s = G["speaker"]
    cx, cy = 62, 64
    for i in range(waves):
        r = 20 + i * 15
        a = math.radians(48)
        x1, y1 = cx + r * math.cos(a), cy - r * math.sin(a)
        x2, y2 = cx + r * math.cos(a), cy + r * math.sin(a)
        s += f'<path d="M{x1:.1f} {y1:.1f} A{r} {r} 0 0 1 {x2:.1f} {y2:.1f}" fill="none" stroke="{W}" stroke-width="7" stroke-linecap="round"/>'
    if muted:
        s += f'<path d="M78 50 L100 78 M100 50 L78 78" stroke="{W}" stroke-width="8" stroke-linecap="round"/>'
    return s
G["battery"] = f'<rect x="28" y="48" width="64" height="34" rx="7" fill="none" stroke="{W}" stroke-width="8"/><rect x="94" y="58" width="9" height="14" rx="3"/><path d="M60 52 L48 68 H59 L55 80 L72 62 H61 Z" fill="{W}"/>'
G["calc"] = '<rect x="34" y="28" width="60" height="72" rx="9"/><rect x="42" y="36" width="44" height="16" rx="3" fill="#0004"/>' + "".join(f'<rect x="{42 + c * 15}" y="{60 + r * 14}" width="11" height="9" rx="2.5" fill="#0004"/>' for r in range(3) for c in range(3))
G["camera"] = f'<path d="M30 46 H44 L50 38 H78 L84 46 H98 a6 6 0 0 1 6 6 V90 a6 6 0 0 1 -6 6 H30 a6 6 0 0 1 -6 -6 V52 a6 6 0 0 1 6 -6 Z"/><circle cx="64" cy="70" r="16" fill="#0004"/><circle cx="64" cy="70" r="9" fill="{W}"/>'
G["picture"] = f'<rect x="26" y="34" width="76" height="60" rx="8" fill="none" stroke="{W}" stroke-width="8"/><circle cx="50" cy="55" r="7"/><path d="M32 88 L56 64 L72 80 L84 68 L98 88 Z"/>'
G["note"] = '<path d="M52 34 L92 26 V80 a14 11 0 1 1 -8 -10 V44 L60 49 V90 a14 11 0 1 1 -8 -10 Z"/>'
G["film"] = f'<rect x="28" y="34" width="72" height="60" rx="8" fill="none" stroke="{W}" stroke-width="8"/><path d="M55 50 L80 64 L55 78 Z"/>'
G["game"] = '<path d="M40 44 H88 a18 18 0 0 1 17 24 L98 90 a9 9 0 0 1 -15 3 L76 82 H52 L45 93 a9 9 0 0 1 -15 -3 L23 68 a18 18 0 0 1 17 -24 Z"/><path d="M44 56 V76 M34 66 H54" stroke="#0005" stroke-width="6" stroke-linecap="round"/><circle cx="78" cy="60" r="4.5" fill="#0005"/><circle cx="90" cy="70" r="4.5" fill="#0005"/>'
G["office"] = G["document"].replace('M46 62 H82 M46 74 H82 M46 86 H70', 'M46 86 V74 M56 86 V66 M66 86 V78 M76 86 V60') + ''
G["code"] = f'<path d="M48 42 L26 64 L48 86 M80 42 L102 64 L80 86 M72 36 L56 92" fill="none" stroke="{W}" stroke-width="9" stroke-linecap="round" stroke-linejoin="round"/>'
G["tools"] = '<path d="M30 92 L66 56 a22 22 0 0 1 30 -28 L84 40 L88 48 L96 52 L108 40 a22 22 0 0 1 -28 30 L44 106 Z"/>'
G["window"] = '<rect x="26" y="34" width="76" height="60" rx="9"/><rect x="26" y="34" width="76" height="16" rx="9" fill="#0003"/><circle cx="38" cy="42" r="3.5"/><circle cx="49" cy="42" r="3.5"/>'
G["box"] = f'<path d="M64 28 L98 44 V84 L64 100 L30 84 V44 Z"/><path d="M30 44 L64 60 L98 44 M64 60 V100" fill="none" stroke="#0004" stroke-width="5" stroke-linejoin="round"/>'
G["search"] = f'<circle cx="56" cy="56" r="22" fill="none" stroke="{W}" stroke-width="10"/><path d="M73 73 L96 96" stroke="{W}" stroke-width="12" stroke-linecap="round"/>'
G["help"] = f'<path d="M48 50 a16 16 0 1 1 22 15 q-6 3 -6 10" fill="none" stroke="{W}" stroke-width="10" stroke-linecap="round"/><circle cx="64" cy="94" r="6.5"/>'
G["clock"] = f'<circle cx="64" cy="64" r="36" fill="none" stroke="{W}" stroke-width="8"/><path d="M64 40 V64 L82 74" fill="none" stroke="{W}" stroke-width="8" stroke-linecap="round" stroke-linejoin="round"/>'
G["display"] = '<rect x="26" y="34" width="76" height="50" rx="7"/><path d="M52 94 H76 M64 84 V94" stroke="#fff" stroke-width="8" stroke-linecap="round"/>'
G["keyboard"] = '<rect x="22" y="44" width="84" height="46" rx="8"/>' + "".join(f'<rect x="{30 + c * 13}" y="{52 + r * 13}" width="9" height="8" rx="2" fill="#0004"/>' for r in range(3) for c in range(6))
G["person"] = '<circle cx="64" cy="48" r="16"/><path d="M32 100 a32 28 0 0 1 64 0 Z"/>'
G["palette"] = f'<path d="M64 30 a34 34 0 1 0 0 68 c8 0 9 -7 5 -12 c-4 -5 -1 -12 6 -12 h10 a14 14 0 0 0 14 -14 c0 -18 -16 -30 -35 -30 Z"/><circle cx="46" cy="58" r="6" fill="#0005"/><circle cx="62" cy="46" r="6" fill="#0005"/><circle cx="80" cy="52" r="6" fill="#0005"/>'
G["sun"] = f'<circle cx="64" cy="64" r="20"/>' + "".join(f'<path d="M{64 + 30 * math.cos(a):.1f} {64 + 30 * math.sin(a):.1f} L{64 + 42 * math.cos(a):.1f} {64 + 42 * math.sin(a):.1f}" stroke="{W}" stroke-width="8" stroke-linecap="round"/>' for a in [i * math.pi / 4 for i in range(8)])
G["download"] = f'<path d="M64 30 V68 M46 54 L64 72 L82 54" fill="none" stroke="{W}" stroke-width="11" stroke-linecap="round" stroke-linejoin="round"/><path d="M34 90 H94" stroke="{W}" stroke-width="10" stroke-linecap="round"/>'


def folder(badge=""):
    return plain(f'''<g filter="url(#s)"><path d="M14 34 a8 8 0 0 1 8 -8 h30 l10 11 h44 a8 8 0 0 1 8 8 v8 H14 Z" fill="#d98a14"/>
<path d="M14 46 a8 8 0 0 1 8 -8 h84 a8 8 0 0 1 8 8 v50 a8 8 0 0 1 -8 8 h-84 a8 8 0 0 1 -8 -8 Z" fill="#ffc542"/>
<path d="M14 46 a8 8 0 0 1 8 -8 h84 a8 8 0 0 1 8 8 v4 H14 Z" fill="#fff" fill-opacity=".45"/>
<path d="M22 38 h84 a8 8 0 0 1 8 8 v50 a8 8 0 0 1 -8 8 h-84 a8 8 0 0 1 -8 -8 v-50 a8 8 0 0 1 8 -8 Z" fill="none" stroke="#fff" stroke-opacity=".5" stroke-width="1.5"/>
<g fill="#fff" transform="translate(32 40) scale(.5)">{badge}</g></g>''')


def dialog(kind):
    if kind == "info":
        return plain('<g filter="url(#s)"><circle cx="64" cy="64" r="48" fill="#2d8be6"/><circle cx="64" cy="64" r="47" fill="none" stroke="#fff" stroke-opacity=".6" stroke-width="2"/><path d="M20 52 C24 24 44 14 64 14 C84 14 104 24 108 52 C90 44 76 42 64 42 C52 42 38 44 20 52 Z" fill="url(#g)"/><circle cx="64" cy="40" r="7" fill="#fff"/><rect x="57" y="54" width="14" height="40" rx="6" fill="#fff"/></g>')
    if kind == "warning":
        return plain('<g filter="url(#s)"><path d="M64 14 L116 104 a6 6 0 0 1 -5 9 H17 a6 6 0 0 1 -5 -9 Z" fill="#f2a516" stroke="#fff" stroke-opacity=".6" stroke-width="2" stroke-linejoin="round"/><path d="M64 14 L100 78 C84 70 74 68 64 68 C54 68 44 70 28 78 Z" fill="url(#g)"/><rect x="57" y="44" width="14" height="36" rx="6" fill="#fff"/><circle cx="64" cy="96" r="7.5" fill="#fff"/></g>')
    return plain('<g filter="url(#s)"><circle cx="64" cy="64" r="48" fill="#d9381e"/><circle cx="64" cy="64" r="47" fill="none" stroke="#fff" stroke-opacity=".6" stroke-width="2"/><path d="M20 52 C24 24 44 14 64 14 C84 14 104 24 108 52 C90 44 76 42 64 42 C52 42 38 44 20 52 Z" fill="url(#g)"/><path d="M46 46 L82 82 M82 46 L46 82" stroke="#fff" stroke-width="12" stroke-linecap="round"/></g>')


# (directory, names, svg)
ICONS = []
def add(dirname, names, svg):
    for n in names if isinstance(names, list) else [names]:
        ICONS.append((dirname, n, svg))

add("apps", ["utilities-terminal", "terminal", "org.gnome.Terminal", "foot", "xterm", "uxterm"], tile("t", BLUE, G["terminal"]))
add("apps", ["system-file-manager", "org.gnome.Nautilus", "thunar", "pcmanfm", "file-manager"], tile("fm", ORANGE, G["folder_small"]))
add("apps", ["web-browser", "internet-web-browser", "applications-internet", "firefox", "chromium", "org.mozilla.firefox"], tile("w", SKY, G["globe"]))
add("apps", ["internet-mail", "mail-client", "evolution", "thunderbird"], tile("m", BLUE, G["mail"]))
add("apps", ["accessories-text-editor", "text-editor", "org.gnome.TextEditor", "gedit", "featherpad", "mousepad"], tile("e", STEEL, G["document"]))
add("apps", ["preferences-system", "preferences-desktop", "org.fde.Settings", "fde-settings", "gnome-control-center", "applications-system"], tile("p", STEEL, G["gear"]))
add("apps", ["preferences-desktop-theme", "preferences-desktop-appearance", "applications-graphics-theme"], tile("pt", PURPLE, G["palette"]))
add("apps", ["accessories-calculator", "gnome-calculator", "org.gnome.Calculator"], tile("c", GREY, G["calc"]))
add("apps", ["accessories-screenshot", "applications-screenshooter", "fde-shot"], tile("sc", TEAL, G["camera"]))
add("apps", ["applications-graphics", "image-viewer", "eog", "gimp"], tile("g", PINK, G["picture"]))
add("apps", ["multimedia-audio-player", "applications-multimedia", "rhythmbox", "fde-music"], tile("a", PURPLE, G["note"]))
add("apps", ["multimedia-video-player", "vlc", "totem"], tile("v", RED, G["film"]))
add("apps", ["applications-games"], tile("gm", GREEN, G["game"]))
add("apps", ["applications-office", "libreoffice-writer", "libreoffice-calc", "libreoffice-impress"], tile("o", BLUE, G["office"]))
add("apps", ["applications-development", "utilities-development"], tile("d", GREY, G["code"]))
add("apps", ["applications-utilities", "applications-accessories"], tile("u", STEEL, G["tools"]))
add("apps", ["applications-other", "application-x-executable"], tile("x", GREY, G["window"]))
add("apps", ["system-software-install", "package-x-generic", "org.gnome.Software"], tile("pk", ORANGE, G["box"]))
add("apps", ["system-search", "edit-find", "org.gnome.Search"], tile("sr", BLUE, G["search"]))
add("apps", ["help-browser", "help-contents", "help-about"], tile("h", SKY, G["help"]))
add("apps", ["preferences-system-time", "clock", "org.gnome.clocks"], tile("ck", TEAL, G["clock"]))
add("apps", ["preferences-desktop-display", "video-display", "display"], tile("ds", STEEL, G["display"]))
add("apps", ["preferences-desktop-keyboard", "input-keyboard"], tile("kb", STEEL, G["keyboard"]))
add("apps", ["system-users", "avatar-default", "user-info"], tile("us", BLUE, G["person"]))
add("apps", ["preferences-system-network", "network-workgroup", "nm-device-wireless"], tile("nw", TEAL, G["wifi"]))
add("apps", ["preferences-desktop-sound", "audio-card"], tile("ps", PURPLE, volume(3)))

add("places", ["user-home"], tile("hm", ORANGE, G["home"]))
add("places", ["user-trash", "user-trash-full"], tile("tr", GREY, G["trash"]))
add("places", ["folder", "inode-directory", "folder-open", "folder-remote"], folder())
add("places", ["folder-documents"], folder(G["document"]))
add("places", ["folder-download"], folder(G["download"]))
add("places", ["folder-music"], folder(G["note"]))
add("places", ["folder-pictures"], folder(G["picture"]))
add("places", ["folder-videos"], folder(G["film"]))

add("actions", ["system-lock-screen"], tile("lk", BLUE, G["lock"]))
add("actions", ["system-log-out"], tile("lo", ORANGE, G["logout"]))
add("actions", ["system-suspend", "system-hibernate"], tile("su", PURPLE, G["moon"]))
add("actions", ["system-reboot", "view-refresh"], tile("rb", YELLOW, G["reboot"]))
add("actions", ["system-shutdown"], tile("sd", RED, G["power"]))

add("status", ["network-wireless", "network-wireless-signal-excellent", "network-wireless-signal-good"], tile("ww", TEAL, G["wifi"]))
add("status", ["network-wired", "network-wired-symbolic"], tile("wd", TEAL, G["wired"]))
add("status", ["audio-volume-high"], tile("v3", PURPLE, volume(3)))
add("status", ["audio-volume-medium"], tile("v2", PURPLE, volume(2)))
add("status", ["audio-volume-low"], tile("v1", PURPLE, volume(1)))
add("status", ["audio-volume-muted"], tile("v0", GREY, volume(0, True)))
add("status", ["display-brightness", "display-brightness-symbolic"], tile("br", YELLOW, G["sun"]))
add("status", ["battery", "battery-full", "battery-good", "battery-low"], tile("bt", GREEN, G["battery"]))
add("status", ["dialog-information"], dialog("info"))
add("status", ["dialog-warning"], dialog("warning"))
add("status", ["dialog-error"], dialog("error"))

add("mimetypes", ["text-x-generic", "x-office-document", "application-pdf"], tile("tx", STEEL, G["document"]))
add("mimetypes", ["image-x-generic"], tile("im", PINK, G["picture"]))
add("mimetypes", ["audio-x-generic"], tile("au", PURPLE, G["note"]))
add("mimetypes", ["video-x-generic"], tile("vi", RED, G["film"]))
add("mimetypes", ["application-x-archive", "package-x-generic-archive", "application-zip"], tile("ar", ORANGE, G["box"]))

INDEX = """[Icon Theme]
Name=FDE
Comment=Glossy glass icons for the Frutiger Desktop Environment
Inherits=Adwaita,hicolor
Directories={dirs}

{sections}"""


def main():
    names = {}
    for sub, name, svg in ICONS:
        d = os.path.join(OUT, "scalable", sub)
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, name + ".svg"), "w") as f:
            f.write(svg)
        names[(sub, name)] = 1
    subs = sorted({s for s, _ in names})
    ctx = {"apps": "Applications", "places": "Places", "actions": "Actions", "status": "Status", "mimetypes": "MimeTypes"}
    sections = "".join(f"[scalable/{s}]\nContext={ctx[s]}\nSize=128\nMinSize=16\nMaxSize=512\nType=Scalable\n\n" for s in subs)
    with open(os.path.join(OUT, "index.theme"), "w") as f:
        f.write(INDEX.format(dirs=",".join(f"scalable/{s}" for s in subs), sections=sections))
    print(f"{len(names)} icons in {len(subs)} folders")


if __name__ == "__main__":
    main()

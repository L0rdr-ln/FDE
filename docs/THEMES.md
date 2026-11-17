# Themes

A theme is one small TOML file. FDE comes with the six members of the Frutiger family
(`themes/` in the source):

| id | look |
|---|---|
| `aero` | Frutiger Aero: glossy blue glass, a glass capsule of buttons, the sky and hill wallpaper (default) |
| `frutiger-metro` | the loud one: saturated green, crisp frames, hot-pink and yellow accents |
| `eco` | Frutiger Eco: fresh green glass, sky fading into grass |
| `dorific` | Dorific: the warm one, glossy orange glass, cream and sunset gradients |
| `vector-flourish` | Vector Flourish: light and airy, white and pastel glass |
| `aqua` | Frutiger Aqua: water and aquariums, cyan-teal glass, a wallpaper that deepens like the sea |
| `high-contrast` | High Contrast: black and yellow, opaque, thick borders, no effects (accessibility) |

These are homages drawn from scratch to match how the looks are remembered. They use no artwork
from the original systems, and the wallpapers are plain gradients until real pictures are added: see
`assets/wallpapers/README.md` for the file names and the licence rules. The theme format can do more than these six use, such as capsule and
square buttons, buttons on the left and centred titles, so you can write your own, for example a
Vista-like one (see below).

Choose one in the settings (Appearance > Theme) or in `fde.toml`:

```toml
[appearance]
theme = "metro"
```

`accent`, `transparency`, `blur` and `gloss` in `[appearance]` still win over the theme's
values when they are set. Choosing a theme in the settings removes them, so the theme's own apply.

## Your own theme

Put `NAME.toml` in `~/.config/fde/themes/` (also read: `/etc/fde/themes`, `/usr/share/fde/themes`,
and `$FDE_THEMES`) and use `theme = "NAME"`. Every key is optional; missing ones are Frutiger Aero's.

```toml
name = "My theme"              # shown in the settings
description = "..."
accent = "#4aa8ee"             # the tint of frames, the task bar and menus
transparency = 0.38            # 0 clear .. 1 solid
blur = 3                       # 1..6
gloss = 0.8                    # the shine on the title bar, 0 .. 1
radius = 12                    # corner radius of windows (0 = square)
radius_bottom = -1             # the two bottom corners; -1 = the same as radius
title_align = "left"          # or "center"
border = 5                     # frame width
titlebar = 32
client_tint = [0.85, 0.93, 1.0, 0.10]   # r, g, b, strength over the window's content area
highlight = 0.9                # light line inside the border
reflections = 1.0              # diagonal streaks on the glass, 0 .. 1
glow = 1.0                     # soft light along the inner edge, 0 .. 1
shadow = 1.0                   # drop shadow strength, 0 .. 2
rim = 1.0                      # dark hairline at the edge
title_ink = "#06233d"          # title text colour (focused window)
title_ink_dim = "#465868"      # (other windows)
title_glow = 0.95              # white glow behind the title text, 0 = none

[buttons]
style = "orb"                  # "orb" (round glass), "flat" (glyphs only), "square" (gel squares) or "capsule" (one pill of three, hung from the top edge)
side = "right"                 # or "left": close first, like Aqua
size = 17
gap = 7
margin = 8
close = "#e64f2e"
maximize = "#54ba4a"
minimize = "#3d9be0"

[wallpaper]
kind = "photo"                 # "photo" (the bundled picture), "solid" or "gradient"
file = "my-theme.webp"         # optional picture, looked up in ~/.local/share/fde/wallpapers (and /usr/share/fde/wallpapers, $FDE_WALLPAPERS); the kind is used when it is not there
color = "#1c3a7a"              # solid colour, or the top of the gradient
color2 = "#050b24"             # the bottom of the gradient
```

A wallpaper you choose in the settings (`[wallpaper] path`) replaces the theme's. The task bar,
start menu, lock screen and notifications take their colours from the accent; they look at the
theme again every few seconds, so a change reaches them without a restart.

## Openbox themes

FDE can wear an Openbox theme. Put the theme folder where Openbox keeps themes (`~/.themes/NAME/`,
`~/.local/share/themes/NAME/`, `/usr/share/themes/NAME/`; for Debian and Ubuntu `sudo apt install openbox`
brings Clearlooks, Onyx, Mikachu, Nightmare and more) and choose it in Settings → Appearance (it is listed as
"NAME (Openbox)"), or write

    [appearance]
    theme = "openbox:Onyx"

What is used from the theme (`NAME/openbox-3/themerc` and the `.xbm` bitmaps next to it): the title bar,
label and border textures (solid, gradient in every direction, raised or sunken with bevel, interlaced,
borders), the label text colours and justification, the border width and paddings, the handle at the bottom
with its grips, the buttons (bitmaps `close.xbm`, `max.xbm`, `max_toggled.xbm`, `iconify.xbm`, and the `_hover` and
`_pressed` ones, in the theme's colours for each state), and the colours of the menu (the window menu is drawn
with them). The accent colour of the task bar and start menu is taken from the middle of the title bar.

What is not used: the window icon and the shade, desktop and sticky buttons, on-screen-display colours, the
gradients of the menu (its flat colours are used), and the theme's font. Frames in an Openbox theme are flat
pictures: no blur, no shadow, no glass.

## Other programs and the icon theme

FDE does not only dress its own windows. At the start of a session, and whenever the theme or accent changes,
it writes the theme's colours where GTK4, GTK3 and Qt programs look for them:

| file | what it does |
|---|---|
| `~/.config/gtk-4.0/gtk.css`, `gtk-3.0/gtk.css` | libadwaita's named colours (`accent_bg_color`, `window_bg_color`, ...) and glossy header bars, buttons, entries, switches |
| `~/.config/gtk-4.0/settings.ini`, `gtk-3.0/settings.ini` | the icon theme (FDE) and the cursor theme |
| `~/.config/qt5ct/colors/fde.conf`, `qt6ct/...`, and `qt5ct.conf`, `qt6ct.conf` if there are none | a Qt colour scheme (install `qt5ct` or `qt6ct`; `start-fde` selects it) |

A file is only written when it does not exist yet or when its first lines say `fde-managed`: a file you wrote
yourself is never touched (delete the marker line to take a file over). `fde --apply-style` does it once from a
terminal and says what it did to each file; `appearance.style_apps = false` (or the switch in Settings) turns it off.
Programs read these files when they start, so restart them after a theme change.

The **FDE icon theme** (`data/icons/FDE`, installed to `share/icons/FDE`) has about 130 glossy glass icons for the
common names (terminal, file manager, folders, browser, mail, settings, lock, log out, sleep, restart, shut down,
network, volume, battery, dialogs, file types and the application categories). Programs' own icons win; the FDE
icons come before Adwaita for everything else. `python3 tools/make-icons.py` draws them again (the SVGs are kept in
the repository, so nothing needs building).

The shell uses the icons too: the power buttons of the start menu (lock, log out, sleep, restart, shut down), the
volume, network and battery items of the task bar, and the notification bubbles, which show the icon the program
sent (a dialog icon when it sent none: information, or error for critical ones).

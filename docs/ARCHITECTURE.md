# FDE desktop: architecture

Goal: a complete desktop environment in the Frutiger Aero style (glass, gloss, sky and water),
the way GNOME or KDE are complete: compositor, shell, settings, session, theme, apps.

The rule that makes this possible for a small team: **write the parts that define the look and
the experience, reuse the plumbing every Linux desktop shares.**

## What FDE writes itself

| part | what it is | language / base |
|---|---|---|
| `fde` (compositor) | windows, input, outputs, the glass renderer, animations, layer-shell, session lock, screen capture, `ext-background-effect` (blur on request) | Rust, Smithay |
| `fde-panel`, `fde-lock`, `fde-greeter`, `fde-notify`, `fde-tray`, `fde-shot`, `fde-music` | small hand-drawn layer-shell (and D-Bus, and greetd) programs: task bar with start menu, flyouts and power row, lock screen, login screen, notifications with history, screenshots, background music. Own software drawing, no toolkit: they start instantly and have no dependencies | Rust, Wayland client |
| `fde-settings` | the control panel: appearance, wallpaper, displays, input, sound, network, power, defaults (plan: [SETTINGS.md](SETTINGS.md)) | Rust, GTK4 |
| session | the compositor sets up the session (environment, polkit agent, autostart; docs/SESSION.md); `data/fde.desktop` is the entry for display managers; `fde-greeter` is a greetd greeter (see SESSION.md) | Rust |
| theme | GTK4 and libadwaita-free CSS theme, Qt style, icon theme, cursor theme, sounds, wallpapers: all original artwork | CSS, SVG |
| portals | screen sharing and screenshots through `xdg-desktop-portal-wlr` on FDE's screen capture (config in `data/`); a backend of FDE's own is still to do | config |

## Where things are in `src/`

| files | what |
|---|---|
| `state.rs`, `handlers.rs`, `protocols.rs`, `input.rs`, `windows.rs`, `outputs.rs`, `layers.rs` | the compositor's state and the Wayland protocols it serves; keys, pointer, windows, monitors, panels |
| `render.rs`, `glass/` | the renderer: wallpaper, glass frames (blur, shaders), the mesh program (warped, burning and torn windows), cube, Flip 3D, overview, bubbles and ripples |
| `effects.rs`, `wobble.rs`, `expo.rs` | the geometry of the big animations and the overview (pure functions, tested without a screen) |
| `tiling.rs`, `switcher.rs`, `workspaces.rs`, `placement.rs`, `winmenu.rs` | window management: tiling mode and its layouts, Alt+Tab, workspaces, remembered places, the window menu |
| `theme.rs`, `obtheme.rs`, `config.rs` | themes (TOML and Openbox), the configuration file |
| `panel.rs`, `flyout.rs`, `status.rs`, `notify.rs`, `lockscreen.rs`, `apps.rs`, `icons.rs`, `text.rs` | what the shell programs in `src/bin/` draw and read (no toolkit) |
| `tray.rs` | the system tray: what programs register, their pictures and menus, the state files `fde-tray` writes and the task bar reads |
| `greetd.rs`, `power.rs`, `hotkeys.rs`, `shell.rs`, `security.rs` | the login protocol, power actions, laptop keys, starting the session's programs, who may use what |
| `check.rs`, `selftest.rs` | `fde --check` (what is on this computer) and `fde --selftest` (does this GPU draw every effect right) |

## What FDE reuses (talked to over D-Bus, not rewritten)

NetworkManager (network), PipeWire + WirePlumber (audio), UPower (battery), logind (suspend,
lock, seats), polkit (privileges), udisks2 (disks), BlueZ (bluetooth), xdg-desktop-portal
(sandboxed apps), AccountsService, systemd user units, fontconfig. Apps: `foot` (terminal) at
first, any file manager (Thunar/Nautilus) until `fde-files` exists.

## How the glass reaches every program

Only the compositor can blur "what is behind". So FDE implements the Wayland protocol
`ext-background-effect-v1`: a client tells the compositor *"blur the area behind this part of my
window"* and draws its own translucent pixels there. The shell, GTK4 apps with the FDE theme
and Qt apps can all ask for it, and the compositor's renderer (`src/glass`) does the blur on the
GPU. Windows without the protocol get the glass frame around them (title bar, border), which the
compositor draws itself.

## Layers

```
 apps (GTK4/Qt/Electron/...)  <->  Wayland  <->  fde (compositor, glass renderer)
 fde-shell / fde-settings   <->  Wayland + D-Bus  <->  NetworkManager, PipeWire, UPower, logind...
 fde-session / greeter       <->  greetd, systemd user session, polkit
```

## Honest scope

GNOME and KDE took hundreds of people years. The plan is a coherent MVP in stages, each one
usable on its own, and no part before the one below it works (see the roadmap). The risky
parts are hardware (DRM/KMS on real GPUs, multi-monitor, HiDPI), toolkit integration (blur for
GTK/Qt) and the amount of art (icons, wallpapers, sounds): all of it has to be original, not
Microsoft's.

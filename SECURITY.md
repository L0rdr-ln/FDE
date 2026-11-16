# Security

## What FDE protects

A desktop compositor sees everything: every window's pixels, every key press, the clipboard.
FDE keeps that away from ordinary programs.

| | an ordinary program | FDE's own programs and the trusted ones |
|---|---|---|
| its own windows, keyboard input while it has the focus | yes | yes |
| the clipboard | only while it has the keyboard | the same |
| a picture of the screen (`wlr-screencopy`) | **no**, it does not even see the protocol | yes |
| the list of all windows, and closing / minimizing them (`wlr-foreign-toplevel`) | **no** | yes |
| locking the screen (`ext-session-lock`) | **no** | yes |
| panels, docks, wallpapers (layer-shell `background`, `bottom`, `top`) | yes; the keyboard only when clicked | yes, and may take the keyboard at once |
| an overlay above everything, full-screen windows included (layer-shell `overlay`) | **no**, it is closed | yes |
| pulling itself to the front (`xdg-activation`) | only with a token asked for right after the user did something in the program that has the keyboard, and used within 10 seconds | the same |

**Trusted** means one of FDE's own programs (`fde-panel`, `fde-lock`, `fde-greeter`, `fde-notify`, `fde-shot`,
`fde-settings`, `fde-music`) installed next to the compositor, or a program named in
`[security] trusted_programs` that is installed in a system folder (`/usr`, `/bin`, `/opt`,
`/nix/store` ...), owned by root and writable by nobody else. The default list is `grim`,
`wf-recorder`, `wl-screenrec`, `xdg-desktop-portal-wlr` (screen sharing in browsers), `swaylock`,
`waylock`, `obs`, `waybar`, `nwg-panel`, and the launchers and notification programs `fuzzel`,
`wofi`, `rofi`, `tofi`, `bemenu`, `bemenu-run`, `nwg-drawer`, `wlogout`, `mako`, `dunst`,
`swaync`. The compositor asks the kernel which program is on the
other end of each connection (SO_PEERCRED), so a program cannot claim to be another one by name.
The path the kernel reports is believed only if it names, on the real file system, the very file
the process runs (same device and inode): a program in a sandbox or a mount namespace of its own
cannot dress itself up as `/usr/bin/grim`. On Linux 6.5 and later the compositor also holds a
handle on the connecting process while it looks, so a recycled process number cannot be mistaken for it.

```toml
[security]
trusted_programs = ["grim", "xdg-desktop-portal-wlr"]   # replace the default list
restrict = true                                          # false: every program may (not recommended)
```

### The lock screen

- Only the program that locked the screen can unlock it. A second program asking to lock while the
  screen is locked is refused, and an unlock that does not come from the locking program is
  ignored. (Both used to be possible; there are tests for them now.)
- If the lock program crashes, the screen stays locked: nothing but the lock surface is drawn,
  and the only key that still works is switching consoles.
- The password is checked by PAM's `unix_chkpwd`, as on any Linux desktop, and only by the copy the
  system installed (never one found through `$PATH`). Release builds have no way to swap the check
  for another command (test builds do, for the tests).
- The typed password is overwritten in memory as soon as it has been checked or cleared.

### The login screen

- The compositor behind `fde-greeter` (`FDE_GREETER=1`) has no keyboard shortcuts except switching
  consoles: nobody at the login screen can open a terminal, quit the compositor or start anything.
  It starts no task bar and no autostart programs, and never starts the lock screen.
- The password goes to greetd (PAM) only; the greeter keeps no copy.

### X11 programs

X11 has no protection between programs: under XWayland any X11 program can see the others' windows
and keys. FDE limits what reaches X11: X11 programs can read the clipboard only while an X11 window
has the keyboard, and they never get screen capture of Wayland windows. Other users of the same
computer cannot connect to your XWayland (checked with a second account).

### Small things

- FDE's own programs keep their pictures in anonymous memory (`memfd`), never in a named file in
  `/tmp` another program could replace or read.
- State files (`outputs.toml`, `workspace`, `dnd`) live in `$XDG_RUNTIME_DIR`, which only you can read.
- Notifications are cut to a size a bubble can show (any program on the session bus may send them),
  so a program cannot fill the history or memory with one huge message.
- Pictures that claim absurd sizes (wallpapers over 150 million pixels, icons over 2048 pixels a
  side, SVG icons over 4 MB) are refused before memory is set aside for them.
- A window's own name can choose an icon from the icon themes, never a picture file by path.
- A Wi-Fi network's name is never read as a command option when you connect to it.
- The configuration is written by the settings app to a temporary file and moved into place, and
  a bad value is refused before anything is written.

## What FDE does not protect against

- **Programs running as you, outside a sandbox.** They can read your files, change your
  configuration and start trusted programs with their own settings. That is true on every Linux
  desktop; sandboxes (Flatpak) are what separates programs. FDE does make sure sandboxed programs
  never count as trusted. (Such a program could also open the connection itself and then turn into
  a trusted program, keeping the connection: the same thing by another road.)
- **Panels.** Any program may still place a panel above the windows (layer-shell `top`), because
  task bars from other projects need it. It cannot cover full-screen windows or the lock screen, and
  it gets no typing unless you click it, but it could still draw something that looks like a
  password prompt. Alt+Tab, Alt+L and console switching always work.
- **Full-screen windows** can cover everything, as on any desktop.

## Dependencies

`cargo audit` reports no known vulnerabilities (November 2026). It does warn about libraries FDE
gets through others and that are no longer maintained: `cgmath` (through Smithay; the one unsound
function in it, `swap_columns`, is not used), `ttf-parser` (through the text and SVG libraries) and
`zvariant` / `zbus_names` (through the D-Bus library).

## Reporting a problem

Open an issue, or for something sensitive contact the repository owner privately first.

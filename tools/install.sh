#!/bin/sh
# Installs FDE from a release archive (or a built source tree):
#   sudo ./install.sh            into /usr/local, plus the session entry for the login screen
#   ./install.sh --user          into ~/.local (no login-screen entry; start it with start-fde)
#   sudo ./install.sh --deps     first installs the libraries FDE needs (apt: Debian, Ubuntu)
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
prefix=/usr/local
user=no
for a in "$@"; do
    case "$a" in
        --user) user=yes; prefix="$HOME/.local" ;;
        --deps)
            command -v apt-get >/dev/null || { echo "--deps knows apt only; install the libraries by hand (see README)"; exit 1; }
            apt-get install -y libseat1 libinput10 libgbm1 libegl1 libgles2 libxkbcommon0 libdrm2 libwayland-server0 \
                libgl1-mesa-dri libgtk-4-1 libasound2t64 seatd dbus-user-session xwayland foot \
                xdg-desktop-portal xdg-desktop-portal-wlr xdg-desktop-portal-gtk || \
            apt-get install -y libseat1 libinput10 libgbm1 libegl1 libgles2 libxkbcommon0 libdrm2 libwayland-server0 \
                libgl1-mesa-dri libgtk-4-1 libasound2 seatd dbus-user-session xwayland foot
            ;;
        *) echo "unknown option $a"; exit 2 ;;
    esac
done
# a release archive has bin/; a source tree has target/release/
if [ -d "$here/bin" ]; then bin="$here/bin"; share="$here/share/fde"; data="$here/data"
else root="$(cd "$here/.." && pwd)"; bin="$root/target/release"; share="$root/assets"; data="$root/data"; fi
[ -x "$bin/fde" ] || { echo "no fde in $bin (build first: cargo build --release --workspace)"; exit 1; }

# the compositor and its programs side by side: FDE trusts its own programs only next to itself
mkdir -p "$prefix/bin" "$prefix/share/fde/wallpapers" "$prefix/share/applications"
for p in fde fde-panel fde-lock fde-notify fde-shot fde-settings fde-music; do
    [ -x "$bin/$p" ] && install -m 755 "$bin/$p" "$prefix/bin/$p"
done
install -m 755 "$here/start-fde" "$prefix/bin/start-fde" 2>/dev/null || install -m 755 "$here/../tools/start-fde" "$prefix/bin/start-fde"
if [ -d "$share/wallpapers" ]; then
    find "$share/wallpapers" -maxdepth 1 -type f ! -name '*.md' -exec install -m 644 {} "$prefix/share/fde/wallpapers/" \;
fi
cat >"$prefix/share/applications/fde-settings.desktop" <<ENTRY
[Desktop Entry]
Type=Application
Name=FDE Settings
Exec=$prefix/bin/fde-settings
Categories=Settings;
ENTRY

if [ "$user" = no ]; then
    # the login screen lists FDE as a session
    mkdir -p /usr/share/wayland-sessions
    sed "s|^Exec=.*|Exec=$prefix/bin/start-fde|" "$data/fde.desktop" >/usr/share/wayland-sessions/fde.desktop
    # FDE looks for the wallpapers here
    if [ "$prefix" != /usr ]; then
        mkdir -p /usr/share/fde
        [ -e /usr/share/fde/wallpapers ] || ln -s "$prefix/share/fde/wallpapers" /usr/share/fde/wallpapers
    fi
    mkdir -p /usr/share/xdg-desktop-portal
    install -m 644 "$data/fde-portals.conf" /usr/share/xdg-desktop-portal/fde-portals.conf
    echo "Installed into $prefix. Log out and pick \"FDE\" on the login screen, or run start-fde on a console."
else
    mkdir -p "$HOME/.local/share/fde" "$HOME/.config/xdg-desktop-portal"
    install -m 644 "$data/fde-portals.conf" "$HOME/.config/xdg-desktop-portal/fde-portals.conf"
    echo "Installed into $prefix (make sure $prefix/bin is on your PATH). Start it with start-fde."
fi
echo "Check what FDE finds first:  $prefix/bin/fde --check"

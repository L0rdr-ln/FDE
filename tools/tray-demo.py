#!/usr/bin/python3.12
"""A real tray program (libayatana-appindicator, the library GTK programs, Electron and many others
use), to try FDE's tray with: `python3.12 tools/tray-demo.py` (needs python3-gi and
gir1.2-ayatanaappindicator3-0.1). It prints what is chosen in its menu."""
import sys
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("AyatanaAppIndicator3", "0.1")
from gi.repository import AyatanaAppIndicator3 as AppIndicator, GLib, Gtk


def say(what):
    print(what, flush=True)


indicator = AppIndicator.Indicator.new("fde-tray-demo", "network-wireless", AppIndicator.IndicatorCategory.APPLICATION_STATUS)
indicator.set_title("Tray demo")
indicator.set_status(AppIndicator.IndicatorStatus.ACTIVE)
menu = Gtk.Menu()
open_item = Gtk.MenuItem.new_with_mnemonic("_Open")
open_item.connect("activate", lambda *_: say("activated Open"))
menu.append(open_item)
menu.append(Gtk.SeparatorMenuItem())
mute = Gtk.CheckMenuItem(label="Mute")
mute.set_active(True)
mute.connect("toggled", lambda w: say(f"toggled Mute {w.get_active()}"))
menu.append(mute)
more = Gtk.MenuItem(label="More")
sub = Gtk.Menu()
deep = Gtk.MenuItem(label="Deep")
deep.connect("activate", lambda *_: say("activated Deep"))
sub.append(deep)
more.set_submenu(sub)
menu.append(more)
quit_item = Gtk.MenuItem(label="Quit")
quit_item.connect("activate", lambda *_: (say("quit"), Gtk.main_quit()))
menu.append(quit_item)
menu.show_all()
indicator.set_menu(menu)
say("ready")
if len(sys.argv) > 1 and sys.argv[1] == "--exit-after":
    GLib.timeout_add_seconds(int(sys.argv[2]), Gtk.main_quit)
Gtk.main()

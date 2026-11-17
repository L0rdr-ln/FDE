//! The tray daemon against a REAL tray program: a libayatana-appindicator client (the library GTK
//! programs, Electron and many others use), run under Xvfb on a private session bus. Skipped where
//! python3.12 with GTK and the AppIndicator typelib, Xvfb or dbus-daemon are missing.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn have(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn wait<T>(what: &str, secs: u64, mut f: impl FnMut() -> Option<T>) -> T {
    let t0 = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(t0.elapsed() < Duration::from_secs(secs), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_real_appindicator_program_gets_its_icon_menu_and_clicks_through() {
    let py = "/usr/bin/python3.12";
    let probe = "import gi; gi.require_version('AyatanaAppIndicator3','0.1'); gi.require_version('Gtk','3.0'); from gi.repository import AyatanaAppIndicator3, Gtk";
    if !have(py, &["-c", probe]) || !have("xvfb-run", &["--help"]) || !have("dbus-daemon", &["--version"]) {
        eprintln!("no python3.12 + GTK + AppIndicator, xvfb-run or dbus-daemon here: real tray test skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut bus = Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address=1"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut address = String::new();
    BufReader::new(bus.stdout.take().unwrap()).read_line(&mut address).unwrap();
    let address = address.trim().to_string();
    let tray = env!("CARGO_BIN_EXE_fde-tray");
    let mut daemon = Command::new(tray).env("DBUS_SESSION_BUS_ADDRESS", &address).env("XDG_RUNTIME_DIR", dir.path()).spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800));

    let demo = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/tray-demo.py");
    let mut app = Command::new("xvfb-run")
        .args(["-a", py, demo])
        .env("DBUS_SESSION_BUS_ADDRESS", &address)
        .env("GDK_BACKEND", "x11")
        .env("NO_AT_BRIDGE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let said: Arc<Mutex<Vec<String>>> = Arc::default();
    let said2 = said.clone();
    let out = app.stdout.take().unwrap();
    std::thread::spawn(move || {
        for l in BufReader::new(out).lines().map_while(Result::ok) {
            said2.lock().unwrap().push(l);
        }
    });

    let items_file = dir.path().join("fde/tray/items");
    let entries = wait("the program's icon shows up", 30, || {
        let e = fde::tray::parse_text(&std::fs::read_to_string(&items_file).ok()?);
        (!e.is_empty()).then_some(e)
    });
    let e = &entries[0];
    assert_eq!(e.title, "Tray demo");
    assert_eq!(e.icon, "network-wireless");
    assert!(e.item.path.starts_with("/org/ayatana/NotificationItem/") && !e.menu.is_empty(), "{e:?}");

    let run = |args: &[&str]| Command::new(tray).args(args).env("DBUS_SESSION_BUS_ADDRESS", &address).output().unwrap();
    // the menu, as the task bar reads it
    let out = run(&["menu", &e.item.service, &e.menu]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let rows = fde::tray::menu_from_text(&String::from_utf8(out.stdout).unwrap());
    let shown: Vec<(&str, u8, bool, bool)> = rows.iter().map(|r| (r.label.as_str(), r.depth, r.separator, r.checked)).collect();
    assert_eq!(shown, vec![("Open", 0, false, false), ("", 0, true, false), ("Mute", 0, false, true), ("More", 0, false, false), ("Deep", 1, false, false), ("Quit", 0, false, false)], "{rows:?}");
    // choosing entries reaches the program
    let open = rows.iter().find(|r| r.label == "Open").unwrap().id;
    let deep = rows.iter().find(|r| r.label == "Deep").unwrap().id;
    assert!(run(&["event", &e.item.service, &e.menu, &open.to_string()]).status.success());
    assert!(run(&["event", &e.item.service, &e.menu, &deep.to_string()]).status.success());
    wait("the program heard both choices", 10, || {
        let s = said.lock().unwrap();
        (s.iter().any(|l| l == "activated Open") && s.iter().any(|l| l == "activated Deep")).then_some(())
    });
    // an appindicator program has no Activate: the call fails (the task bar then opens the menu)
    assert!(!run(&["activate", &e.item.service, &e.item.path, "1", "1"]).status.success());
    // quitting the program removes the icon
    let quit = rows.iter().find(|r| r.label == "Quit").unwrap().id;
    assert!(run(&["event", &e.item.service, &e.menu, &quit.to_string()]).status.success());
    wait("the icon is gone after the program quit", 15, || {
        let e = fde::tray::parse_text(&std::fs::read_to_string(&items_file).unwrap_or_default());
        e.is_empty().then_some(())
    });
    let _ = app.kill();
    let _ = app.wait();
    let _ = daemon.kill();
    let _ = daemon.wait();
    let _ = bus.kill();
    let _ = bus.wait();
}

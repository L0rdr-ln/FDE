//! The tray daemon against a private session bus, with mock StatusNotifierItems.

use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use std::collections::HashMap;

use zbus::blocking::connection::Builder;
use zbus::zvariant::{OwnedValue, StructureBuilder, Value};

struct Item {
    title: &'static str,
    icon_name: &'static str,
    status: Arc<Mutex<String>>,
    activations: Arc<Mutex<Vec<(i32, i32)>>>,
    context: Arc<Mutex<Vec<(i32, i32)>>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    #[zbus(property)]
    fn id(&self) -> String {
        "mock".into()
    }
    #[zbus(property)]
    fn title(&self) -> String {
        self.title.into()
    }
    #[zbus(property)]
    fn icon_name(&self) -> String {
        self.icon_name.into()
    }
    #[zbus(property)]
    fn status(&self) -> String {
        self.status.lock().unwrap().clone()
    }
    /// two pictures: 16 x 16 red and 32 x 32 green (ARGB, opaque)
    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        vec![(16, 16, [255u8, 255, 0, 0].repeat(16 * 16)), (32, 32, [255u8, 0, 255, 0].repeat(32 * 32))]
    }
    fn activate(&self, x: i32, y: i32) {
        self.activations.lock().unwrap().push((x, y));
    }
    fn context_menu(&self, x: i32, y: i32) {
        self.context.lock().unwrap().push((x, y));
    }
}

/// A mock com.canonical.dbusmenu: Open, a separator, Mute (checked), a submenu "More" with "Deep".
struct Menu {
    events: Arc<Mutex<Vec<(i32, String)>>>,
}

fn entry(id: i32, props: &[(&str, Value<'static>)], children: Vec<OwnedValue>) -> OwnedValue {
    let dict: HashMap<String, Value<'static>> = props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let st = StructureBuilder::new().add_field(id).append_field(Value::new(dict)).add_field(children).build().unwrap();
    OwnedValue::try_from(Value::Structure(st)).unwrap()
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn about_to_show(&self, _id: i32) -> bool {
        false
    }
    fn get_layout(&self, _parent: i32, _depth: i32, _names: Vec<String>) -> (u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>)) {
        let label = |t: &str| ("label", Value::from(t.to_string()));
        let children = vec![
            entry(1, &[label("_Open")], vec![]),
            entry(2, &[("type", Value::from("separator".to_string()))], vec![]),
            entry(3, &[label("Mute"), ("toggle-type", Value::from("checkmark".to_string())), ("toggle-state", Value::from(1i32))], vec![]),
            entry(4, &[label("More")], vec![entry(5, &[label("Deep")], vec![])]),
            entry(6, &[label("Hidden"), ("visible", Value::from(false))], vec![]),
            entry(7, &[label("Greyed"), ("enabled", Value::from(false))], vec![]),
        ];
        (1, (0, HashMap::new(), children))
    }
    fn event(&self, id: i32, event_id: String, _data: Value<'_>, _timestamp: u32) {
        self.events.lock().unwrap().push((id, event_id));
    }
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }
}

fn wait<T>(what: &str, secs: u64, mut f: impl FnMut() -> Option<T>) -> T {
    let t0 = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(t0.elapsed() < Duration::from_secs(secs), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn programs_register_icons_show_up_get_clicked_and_leave() {
    if Command::new("dbus-daemon").arg("--version").output().is_err() {
        eprintln!("no dbus-daemon here: tray test skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("XDG_RUNTIME_DIR", dir.path());
    let mut bus = Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address=1"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut address = String::new();
    std::io::BufReader::new(bus.stdout.take().unwrap()).read_line(&mut address).unwrap();
    let address = address.trim().to_string();
    std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address);

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_fde-tray")).env("DBUS_SESSION_BUS_ADDRESS", &address).env("XDG_RUNTIME_DIR", dir.path()).spawn().unwrap();
    let conn_for_name = zbus::blocking::Connection::session().unwrap();
    wait("the tray owns the watcher name", 10, || {
        conn_for_name
            .call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "NameHasOwner", &("org.kde.StatusNotifierWatcher",))
            .ok()
            .and_then(|r| r.body().deserialize::<bool>().ok())
            .filter(|b| *b)
    });

    // a program with a pixmap icon, registered by its bus name
    let status = Arc::new(Mutex::new("Active".to_string()));
    let activations = Arc::new(Mutex::new(Vec::new()));
    let context = Arc::new(Mutex::new(Vec::new()));
    let item = Item { title: "Mock App", icon_name: "", status: status.clone(), activations: activations.clone(), context: context.clone() };
    let menu_events = Arc::new(Mutex::new(Vec::new()));
    let program = Builder::session()
        .unwrap()
        .name("org.kde.StatusNotifierItem-4242-1")
        .unwrap()
        .serve_at("/StatusNotifierItem", item)
        .unwrap()
        .serve_at("/MenuBar", Menu { events: menu_events.clone() })
        .unwrap()
        .build()
        .unwrap();
    program
        .call_method(Some("org.kde.StatusNotifierWatcher"), "/StatusNotifierWatcher", Some("org.kde.StatusNotifierWatcher"), "RegisterStatusNotifierItem", &("org.kde.StatusNotifierItem-4242-1",))
        .unwrap();
    let items_file = dir.path().join("fde/tray/items");
    let entries = wait("the item shows up", 10, || {
        let e = fde::tray::parse_text(&std::fs::read_to_string(&items_file).ok()?);
        (!e.is_empty()).then_some(e)
    });
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!((e.item.service.as_str(), e.item.path.as_str(), e.title.as_str(), e.status.as_str()), ("org.kde.StatusNotifierItem-4242-1", "/StatusNotifierItem", "Mock App", "Active"));
    // its picture: the bigger pixmap (32) as a PNG, green
    assert!(e.icon.ends_with(".png") && std::path::Path::new(&e.icon).is_file(), "a PNG of the pixmap: {}", e.icon);
    let decoder = png::Decoder::new(std::fs::File::open(&e.icon).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!((info.width, info.height), (32, 32));
    assert_eq!(&buf[..4], &[0, 255, 0, 255], "opaque green");

    // a click goes to the item
    let ok = Command::new(env!("CARGO_BIN_EXE_fde-tray")).args(["activate", &e.item.service, &e.item.path, "10", "20"]).env("DBUS_SESSION_BUS_ADDRESS", &address).status().unwrap();
    assert!(ok.success());
    wait("the item was activated", 5, || (activations.lock().unwrap().as_slice() == [(10, 20)]).then_some(()));
    let ok = Command::new(env!("CARGO_BIN_EXE_fde-tray")).args(["context", &e.item.service, &e.item.path, "5", "6"]).env("DBUS_SESSION_BUS_ADDRESS", &address).status().unwrap();
    assert!(ok.success());
    wait("the context menu was asked for", 5, || (context.lock().unwrap().as_slice() == [(5, 6)]).then_some(()));
    // the menu: rows as the task bar reads them (submenu opened in place, hidden entry gone)
    let out = Command::new(env!("CARGO_BIN_EXE_fde-tray")).args(["menu", &e.item.service, "/MenuBar"]).env("DBUS_SESSION_BUS_ADDRESS", &address).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let rows = fde::tray::menu_from_text(&String::from_utf8(out.stdout).unwrap());
    let shown: Vec<(&str, u8, bool, bool, bool)> = rows.iter().map(|r| (r.label.as_str(), r.depth, r.enabled, r.checked, r.separator)).collect();
    assert_eq!(
        shown,
        vec![("Open", 0, true, false, false), ("", 0, true, false, true), ("Mute", 0, true, true, false), ("More", 0, true, false, false), ("Deep", 1, true, false, false), ("Greyed", 0, false, false, false)],
        "{rows:?}"
    );
    assert!(rows[3].has_children);
    // choosing a row tells the program
    let ok = Command::new(env!("CARGO_BIN_EXE_fde-tray")).args(["event", &e.item.service, "/MenuBar", "3"]).env("DBUS_SESSION_BUS_ADDRESS", &address).status().unwrap();
    assert!(ok.success());
    wait("the menu event arrived", 5, || (menu_events.lock().unwrap().as_slice() == [(3, "clicked".to_string())]).then_some(()));
    // nonsense is refused, and the daemon is not the one hurt
    assert!(!Command::new(env!("CARGO_BIN_EXE_fde-tray")).args(["activate", "bad name", "/x", "1", "2"]).env("DBUS_SESSION_BUS_ADDRESS", &address).status().unwrap().success());

    // a second one registered by the path of an object on the caller's connection
    let second = Item { title: "Second", icon_name: "network-wireless", status: Arc::new(Mutex::new("Active".into())), activations: Arc::default(), context: Arc::default() };
    let other = Builder::session().unwrap().serve_at("/org/ayatana/NotificationItem/second", second).unwrap().build().unwrap();
    other
        .call_method(Some("org.kde.StatusNotifierWatcher"), "/StatusNotifierWatcher", Some("org.kde.StatusNotifierWatcher"), "RegisterStatusNotifierItem", &("/org/ayatana/NotificationItem/second",))
        .unwrap();
    let both = wait("two items", 10, || {
        let e = fde::tray::parse_text(&std::fs::read_to_string(&items_file).ok()?);
        (e.len() == 2).then_some(e)
    });
    let second_entry = both.iter().find(|e| e.title == "Second").expect("the second item");
    assert_eq!(second_entry.item.path, "/org/ayatana/NotificationItem/second");
    assert!(second_entry.item.service.starts_with(':'), "its unique name: {}", second_entry.item.service);
    assert_eq!(second_entry.icon, "network-wireless", "an icon name is passed on as it is");

    // the watcher says what is registered
    let reply = conn_for_name
        .call_method(Some("org.kde.StatusNotifierWatcher"), "/StatusNotifierWatcher", Some("org.freedesktop.DBus.Properties"), "Get", &("org.kde.StatusNotifierWatcher", "RegisteredStatusNotifierItems"))
        .unwrap();
    let value: zbus::zvariant::OwnedValue = reply.body().deserialize().unwrap();
    let list = Vec::<String>::try_from(value).unwrap();
    assert_eq!(list.len(), 2, "{list:?}");

    // a passive item is hidden, and comes back when it is active again
    *status.lock().unwrap() = "Passive".to_string();
    wait("the passive item is hidden", 10, || {
        let e = fde::tray::parse_text(&std::fs::read_to_string(&items_file).ok()?);
        (e.len() == 1 && e[0].title == "Second").then_some(())
    });
    *status.lock().unwrap() = "Active".to_string();
    wait("it is back", 10, || {
        let e = fde::tray::parse_text(&std::fs::read_to_string(&items_file).ok()?);
        (e.len() == 2).then_some(())
    });

    // a program that quits leaves the tray
    drop(program);
    wait("the first program left", 10, || {
        let e = fde::tray::parse_text(&std::fs::read_to_string(&items_file).ok()?);
        (e.len() == 1 && e[0].title == "Second").then_some(())
    });
    // and its picture file is cleaned up
    let pngs = std::fs::read_dir(dir.path().join("fde/tray")).unwrap().flatten().filter(|f| f.path().extension().is_some_and(|e| e == "png")).count();
    assert_eq!(pngs, 0, "no picture file is left behind");

    let _ = daemon.kill();
    let _ = daemon.wait();
    let _ = bus.kill();
    let _ = bus.wait();
}

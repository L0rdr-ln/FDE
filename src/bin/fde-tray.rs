//! The system tray daemon: the StatusNotifierWatcher (and the only host) of the session bus.
//!
//!   fde-tray                                          run: keep `$XDG_RUNTIME_DIR/fde/tray/items` current
//!   fde-tray activate|secondary|context SERVICE PATH X Y   click an item (what the task bar runs)
//!
//! See `src/tray.rs` for the files.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fde::tray::{self, Entry, ItemRef};
use zbus::blocking::{connection::Builder, Connection};
use zbus::zvariant::OwnedValue;

const ITEM_IFACE: &str = "org.kde.StatusNotifierItem";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";

/// What the watcher object shares with the loop.
#[derive(Default)]
struct Shared {
    /// registrations that came in and have not been looked at yet
    pending: Vec<ItemRef>,
    /// "service/path" of every item now registered
    known: Vec<String>,
}

struct Watcher {
    shared: Arc<Mutex<Shared>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: String, #[zbus(header)] header: zbus::message::Header<'_>) {
        let sender = header.sender().map(|s| s.to_string()).unwrap_or_default();
        if let Some(item) = tray::parse_registration(&service, &sender) {
            let mut s = self.shared.lock().unwrap();
            if s.pending.len() < 200 && !s.pending.contains(&item) {
                s.pending.push(item);
            }
        }
    }

    fn register_status_notifier_host(&self, _service: String) {}

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.shared.lock().unwrap().known.clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// The same interface under the name of the freedesktop spelling (some programs ask for it).
struct WatcherFd(Watcher);

#[zbus::interface(name = "org.freedesktop.StatusNotifierWatcher")]
impl WatcherFd {
    fn register_status_notifier_item(&self, service: String, #[zbus(header)] header: zbus::message::Header<'_>) {
        self.0.register_status_notifier_item(service, header)
    }

    fn register_status_notifier_host(&self, _service: String) {}

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.0.registered_status_notifier_items()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

struct Seen {
    entry: Entry,
}

/// What the item says about itself now (None if it does not answer).
fn fetch(bus: &Connection, item: &ItemRef, out_dir: &std::path::Path, n: usize) -> Option<Entry> {
    let reply = bus
        .call_method(Some(item.service.as_str()), item.path.as_str(), Some("org.freedesktop.DBus.Properties"), "GetAll", &(ITEM_IFACE,))
        .ok()?;
    let props: HashMap<String, OwnedValue> = reply.body().deserialize().ok()?;
    let text = |k: &str| props.get(k).and_then(|v| String::try_from(v.clone()).ok()).unwrap_or_default();
    let status = match text("Status").as_str() {
        "" => "Active".to_string(),
        s => s.to_string(),
    };
    let attention = status == "NeedsAttention";
    // the picture: the icon name, else the pixmap the item sent (as a PNG file)
    let name_key = if attention && !text("AttentionIconName").is_empty() { "AttentionIconName" } else { "IconName" };
    let mut icon = text(name_key);
    let pixmaps: Vec<tray::Pixmap> = props
        .get(if attention && props.contains_key("AttentionIconPixmap") { "AttentionIconPixmap" } else { "IconPixmap" })
        .and_then(|v| Vec::<(i32, i32, Vec<u8>)>::try_from(v.clone()).ok())
        .unwrap_or_default();
    if icon.is_empty() || icon.contains('/') && !icon.ends_with(".png") && !icon.ends_with(".svg") {
        icon.clear();
    }
    if icon.is_empty() {
        if let Some((w, h, argb)) = tray::best_pixmap(&pixmaps, 32) {
            let rgba = tray::argb_to_rgba(argb);
            let path = out_dir.join(format!("{n}-{:016x}.png", tray::fingerprint(&rgba)));
            if path.is_file() || tray::write_png(&path, *w as u32, *h as u32, &rgba).is_ok() {
                icon = path.to_string_lossy().into_owned();
            }
        }
    }
    let title = {
        let t = text("Title");
        if t.is_empty() { text("Id") } else { t }
    };
    let menu = props.get("Menu").and_then(|v| zbus::zvariant::ObjectPath::try_from(v.clone()).ok()).map(|p| p.to_string()).unwrap_or_default();
    let is_menu = props.get("ItemIsMenu").and_then(|v| bool::try_from(v.clone()).ok()).unwrap_or(false);
    Some(Entry { item: item.clone(), title, icon, status, menu, is_menu })
}

fn name_has_owner(bus: &Connection, name: &str) -> bool {
    bus.call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "NameHasOwner", &(name,))
        .ok()
        .and_then(|r| r.body().deserialize::<bool>().ok())
        .unwrap_or(false)
}

const MENU_IFACE: &str = "com.canonical.dbusmenu";

fn unwrap_variant<'a, 'b>(mut v: &'b zbus::zvariant::Value<'a>) -> &'b zbus::zvariant::Value<'a> {
    while let zbus::zvariant::Value::Value(inner) = v {
        v = inner;
    }
    v
}

/// One menu entry (id, properties, children) from what the item sent.
fn node_from_value(v: &zbus::zvariant::Value<'_>) -> Option<tray::Node> {
    use zbus::zvariant::Value;
    let Value::Structure(st) = unwrap_variant(v) else { return None };
    let f = st.fields();
    if f.len() != 3 {
        return None;
    }
    let id = i32::try_from(unwrap_variant(&f[0])).ok()?;
    let Value::Dict(d) = unwrap_variant(&f[1]) else { return None };
    let mut label = String::new();
    let (mut enabled, mut visible, mut separator, mut checked, mut toggle) = (true, true, false, false, false);
    for (k, val) in d.iter() {
        let Ok(key) = <&str>::try_from(unwrap_variant(k)) else { continue };
        let val = unwrap_variant(val);
        match key {
            "label" => label = <&str>::try_from(val).unwrap_or("").to_string(),
            "enabled" => enabled = bool::try_from(val).unwrap_or(true),
            "visible" => visible = bool::try_from(val).unwrap_or(true),
            "type" => separator = <&str>::try_from(val).is_ok_and(|t| t == "separator"),
            "toggle-type" => toggle = <&str>::try_from(val).is_ok_and(|t| !t.is_empty()),
            "toggle-state" => checked = i32::try_from(val).is_ok_and(|n| n == 1),
            _ => {}
        }
    }
    let children = match unwrap_variant(&f[2]) {
        Value::Array(a) => a.iter().filter_map(node_from_value).collect(),
        _ => Vec::new(),
    };
    Some(tray::Node { id, label, enabled, visible, separator, checked: toggle && checked, children })
}

/// `fde-tray menu SERVICE MENUPATH`: print the item's menu as rows (see `tray::menu_to_text`).
fn menu(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let [service, path] = args else { return Err("usage: fde-tray menu SERVICE MENUPATH".into()) };
    let item = tray::parse_registration(&format!("{service}{path}"), "").ok_or("not a menu")?;
    let bus = Builder::session()?.method_timeout(Duration::from_secs(3)).build()?;
    // programs may build the menu only when asked to show it
    let _ = bus.call_method(Some(item.service.as_str()), item.path.as_str(), Some(MENU_IFACE), "AboutToShow", &(0i32,));
    let reply = bus.call_method(Some(item.service.as_str()), item.path.as_str(), Some(MENU_IFACE), "GetLayout", &(0i32, -1i32, Vec::<String>::new()))?;
    let (_revision, (id, _props, children)): (u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>)) = reply.body().deserialize()?;
    let root = tray::Node { id, label: String::new(), enabled: true, visible: true, separator: false, checked: false, children: children.iter().filter_map(|c| node_from_value(c)).collect() };
    print!("{}", tray::menu_to_text(&tray::flatten(&root, 40)));
    Ok(())
}

/// `fde-tray event SERVICE MENUPATH ID`: choose a menu entry.
fn event(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let [service, path, id] = args else { return Err("usage: fde-tray event SERVICE MENUPATH ID".into()) };
    let item = tray::parse_registration(&format!("{service}{path}"), "").ok_or("not a menu")?;
    let bus = Builder::session()?.method_timeout(Duration::from_secs(3)).build()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as u32).unwrap_or(0);
    bus.call_method(Some(item.service.as_str()), item.path.as_str(), Some(MENU_IFACE), "Event", &(id.parse::<i32>()?, "clicked", zbus::zvariant::Value::from(0i32), now))?;
    Ok(())
}

/// `fde-tray activate SERVICE PATH X Y`: click an item.
fn click(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let [method, service, path, x, y] = args else { return Err("usage: fde-tray activate|secondary|context SERVICE PATH X Y".into()) };
    let method = match method.as_str() {
        "activate" => "Activate",
        "secondary" => "SecondaryActivate",
        "context" => "ContextMenu",
        _ => return Err("the action is activate, secondary or context".into()),
    };
    let item = tray::parse_registration(&format!("{service}{path}"), "").ok_or("not an item")?;
    let bus = Builder::session()?.method_timeout(Duration::from_secs(3)).build()?;
    bus.call_method(Some(item.service.as_str()), item.path.as_str(), Some(ITEM_IFACE), method, &(x.parse::<i32>()?, y.parse::<i32>()?))?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("menu") => return menu(&args[1..]),
        Some("event") => return event(&args[1..]),
        Some(_) => return click(&args),
        None => {}
    }
    let shared = Arc::new(Mutex::new(Shared::default()));
    let bus = match Builder::session()?
        .method_timeout(Duration::from_secs(2))
        .name("org.kde.StatusNotifierWatcher")?
        .name("org.freedesktop.StatusNotifierWatcher")?
        .serve_at(WATCHER_PATH, Watcher { shared: shared.clone() })?
        .serve_at(WATCHER_PATH, WatcherFd(Watcher { shared: shared.clone() }))?
        .build()
    {
        Ok(b) => b,
        Err(e) => {
            eprintln!("fde-tray: cannot become the tray (is another one running?): {e}");
            return Ok(());
        }
    };
    let dir = tray::dir().ok_or("XDG_RUNTIME_DIR is not set")?;
    std::fs::create_dir_all(&dir)?;
    let _ = std::fs::remove_file(dir.join("items"));
    let mut items: Vec<Seen> = Vec::new();
    let mut written = String::new();
    let mut last_refresh = Instant::now() - Duration::from_secs(10);
    let mut counter = 0usize;
    loop {
        let pending: Vec<ItemRef> = std::mem::take(&mut shared.lock().unwrap().pending);
        let mut changed = false;
        for item in pending {
            if items.iter().any(|s| s.entry.item == item) {
                continue;
            }
            counter += 1;
            if let Some(entry) = fetch(&bus, &item, &dir, counter) {
                items.push(Seen { entry });
                changed = true;
            }
        }
        if last_refresh.elapsed() >= Duration::from_secs(2) {
            last_refresh = Instant::now();
            // programs that left are dropped; the others are asked again (icons change)
            let mut gone = HashSet::new();
            for (i, s) in items.iter_mut().enumerate() {
                if !name_has_owner(&bus, &s.entry.item.service) {
                    gone.insert(i);
                    continue;
                }
                counter += 1;
                if let Some(fresh) = fetch(&bus, &s.entry.item, &dir, counter) {
                    if fresh != s.entry {
                        s.entry = fresh;
                        changed = true;
                    }
                }
            }
            if !gone.is_empty() {
                let mut i = 0;
                items.retain(|_| {
                    i += 1;
                    !gone.contains(&(i - 1))
                });
                changed = true;
            }
        }
        if changed {
            let entries: Vec<Entry> = items.iter().map(|s| s.entry.clone()).collect();
            let text = tray::to_text(&entries);
            if text != written {
                let tmp = dir.join("items.tmp");
                if std::fs::write(&tmp, &text).and_then(|_| std::fs::rename(&tmp, dir.join("items"))).is_ok() {
                    written = text;
                }
            }
            shared.lock().unwrap().known = entries.iter().map(|e| format!("{}{}", e.item.service, e.item.path)).collect();
            // pictures that no entry refers to any more
            let used: HashSet<String> = entries.iter().map(|e| e.icon.clone()).collect();
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for f in rd.flatten() {
                    let p = f.path();
                    if p.extension().is_some_and(|e| e == "png") && !used.contains(p.to_string_lossy().as_ref()) {
                        let _ = std::fs::remove_file(p);
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

//! `fde --check`: what FDE would find if it started now. Looks only, changes nothing, and can run
//! from any terminal. Paste its output when something does not work.

use std::path::Path;

/// One line of the report: fine, worth a look, or in the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Ok,
    Note,
    Bad,
}

pub struct Report {
    pub lines: Vec<(Mark, String)>,
}

impl Report {
    fn add(&mut self, mark: Mark, text: impl Into<String>) {
        self.lines.push((mark, text.into()));
    }

    pub fn print(&self) {
        for (m, t) in &self.lines {
            let tag = match m {
                Mark::Ok => "  ok ",
                Mark::Note => "  -- ",
                Mark::Bad => "  !! ",
            };
            println!("{tag}{t}");
        }
        let bad = self.lines.iter().filter(|(m, _)| *m == Mark::Bad).count();
        println!();
        if bad == 0 {
            println!("Nothing in the way.");
        } else {
            println!("{bad} problem(s) above (marked !!).");
        }
    }
}

fn read(p: impl AsRef<Path>) -> Option<String> {
    std::fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

/// May we read and write this device file?
fn can_open(p: &Path) -> bool {
    let Ok(c) = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()) else { return false };
    unsafe { libc::access(c.as_ptr(), libc::R_OK | libc::W_OK) == 0 }
}

/// The names of the groups this process is in.
fn my_groups() -> Vec<String> {
    let mut ids = vec![0 as libc::gid_t; 256];
    let n = unsafe { libc::getgroups(ids.len() as i32, ids.as_mut_ptr()) };
    let ids: Vec<u32> = ids.into_iter().take(n.max(0) as usize).collect();
    let table = read("/etc/group").unwrap_or_default();
    table
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            let gid: u32 = f.get(2)?.parse().ok()?;
            ids.contains(&gid).then(|| f[0].to_string())
        })
        .collect()
}

fn in_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
        .unwrap_or(false)
}

/// Look at everything FDE needs. `sys` is normally "/sys" (tests point it elsewhere).
pub fn run_checks(sys: &Path) -> Report {
    let mut r = Report { lines: Vec::new() };
    r.add(Mark::Note, format!("FDE {}", env!("CARGO_PKG_VERSION")));

    // where it would run
    let nested = std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some();
    if nested {
        r.add(Mark::Note, "a desktop is running here: `fde` opens as a window inside it (`fde drm` takes the screen; run that from a console)");
    } else {
        r.add(Mark::Note, "no desktop here: `fde` takes over the screen");
    }
    match std::env::var("XDG_VTNR") {
        Ok(vt) => r.add(Mark::Ok, format!("on console (VT) {vt}")),
        Err(_) if !nested => r.add(Mark::Note, "XDG_VTNR is not set: if `fde` cannot open the seat, log in on a console (Ctrl+Alt+F3) and start it there"),
        Err(_) => {}
    }
    if std::env::var_os("XDG_RUNTIME_DIR").is_none() {
        r.add(Mark::Bad, "XDG_RUNTIME_DIR is not set: log in normally (not with su) so the session sets it up");
    }

    // the seat: who lets us open the GPU and the input devices
    let seatd = Path::new("/run/seatd.sock").exists();
    let logind = Path::new("/run/systemd/seats").exists();
    let groups = my_groups();
    match (seatd, logind) {
        (true, _) => {
            let member = groups.iter().any(|g| g == "_seatd" || g == "seat");
            r.add(
                if member || logind { Mark::Ok } else { Mark::Bad },
                format!("seatd is running{}", if member { " and you are in its group" } else if logind { " (logind too)" } else { ", but you are not in its group (_seatd or seat): sudo usermod -aG _seatd $USER, then log in again" }),
            );
        }
        (false, true) => r.add(Mark::Ok, "logind manages the seat"),
        (false, false) => r.add(Mark::Bad, "neither seatd nor logind is running: sudo systemctl enable --now seatd"),
    }

    // GPUs
    let drm = sys.join("class/drm");
    let mut cards: Vec<String> = std::fs::read_dir(&drm)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    cards.sort();
    let gpus: Vec<&String> = cards.iter().filter(|c| c.starts_with("card") && !c.contains('-')).collect();
    if gpus.is_empty() {
        r.add(Mark::Bad, "no GPU (/sys/class/drm/card*): in a VM, give it a display adapter (VMware: VMSVGA, VirtualBox: VMSVGA)");
    }
    for card in &gpus {
        let driver = std::fs::read_link(drm.join(card).join("device/driver"))
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "unknown driver".into());
        let boot = read(drm.join(card).join("device/boot_vga")).as_deref() == Some("1");
        let dev = Path::new("/dev/dri").join(card);
        let open = can_open(&dev);
        let hint = match driver.as_str() {
            "vmwgfx" => " (VMware: switch on 3D acceleration in the VM settings for smooth glass)",
            "bochs-drm" | "bochs" | "cirrus" | "simpledrm" => " (a basic adapter without 3D: everything is drawn by the CPU, slowly)",
            _ => "",
        };
        r.add(Mark::Ok, format!("GPU {card}: {driver}{}{hint}", if boot { ", the boot screen's" } else { "" }));
        if !open && !seatd && !logind {
            r.add(Mark::Bad, format!("{} cannot be opened by you, and no seat manager can open it for you", dev.display()));
        }
        // its monitors
        let mut any = false;
        for c in cards.iter().filter(|c| c.starts_with(&format!("{card}-"))) {
            let status = read(drm.join(c).join("status")).unwrap_or_default();
            if status == "connected" {
                any = true;
                let mode = read(drm.join(c).join("modes")).and_then(|m| m.lines().next().map(String::from)).unwrap_or_else(|| "no modes".into());
                let name = c.trim_start_matches(&format!("{card}-"));
                r.add(Mark::Ok, format!("  monitor {name}: {mode}"));
            }
        }
        if !any {
            r.add(Mark::Note, format!("  no monitor connected to {card}"));
        }
    }

    // drawing
    match crate::demo::offscreen_renderer() {
        Ok(mut renderer) => {
            let name = renderer
                .with_context(|gl| unsafe {
                    let p = gl.GetString(smithay::backend::renderer::gles::ffi::RENDERER);
                    if p.is_null() { String::new() } else { std::ffi::CStr::from_ptr(p as *const _).to_string_lossy().into_owned() }
                })
                .unwrap_or_default();
            let soft = ["llvmpipe", "softpipe", "swrast", "software"].iter().any(|k| name.to_lowercase().contains(k));
            if soft {
                r.add(Mark::Note, format!("OpenGL ES works, drawn by the CPU ({name}): FDE switches to fast mode; in a VM, 3D acceleration makes it smooth"));
            } else {
                r.add(Mark::Ok, format!("OpenGL ES works: {name}"));
            }
        }
        Err(e) => r.add(Mark::Bad, format!("no OpenGL ES ({e}): install Mesa (libgl1-mesa-dri, libegl1), or in a VM switch on 3D acceleration")),
    }

    // input devices (through the seat manager, or directly)
    let events = std::fs::read_dir("/dev/input").map(|d| d.filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with("event")).count()).unwrap_or(0);
    if events == 0 {
        r.add(Mark::Bad, "no input devices in /dev/input");
    } else {
        r.add(Mark::Ok, format!("{events} input devices"));
    }

    // the configuration
    match crate::config::Config::load() {
        Ok(_) => r.add(Mark::Ok, "the configuration is fine"),
        Err(e) => r.add(Mark::Bad, format!("the configuration has a mistake: {e}")),
    }

    // programs FDE starts or works with
    let term = crate::config::Config::load().map(|c| c.shell.terminal).unwrap_or_else(|_| "foot".into());
    let term_cmd = term.split_whitespace().next().unwrap_or("foot").to_string();
    for (prog, what, needed) in [
        (term_cmd.as_str(), "the terminal (Alt+Return)", true),
        ("Xwayland", "X11 programs", false),
        ("xdg-desktop-portal-wlr", "screen sharing in browsers", false),
        ("grim", "screenshots by other tools", false),
    ] {
        if in_path(prog) {
            r.add(Mark::Ok, format!("{prog}: {what}"));
        } else {
            r.add(if needed { Mark::Bad } else { Mark::Note }, format!("{prog} is not installed: {what}"));
        }
    }
    let here = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf()));
    for sibling in ["fde-panel", "fde-lock", "fde-notify", "fde-settings"] {
        let found = here.as_ref().is_some_and(|d| d.join(sibling).is_file());
        if !found {
            r.add(Mark::Note, format!("{sibling} is not next to fde: build the whole workspace (cargo build --release --workspace)"));
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_names_the_gpu_its_driver_and_the_connected_monitors() {
        let sys = tempfile::tempdir().unwrap();
        let drm = sys.path().join("class/drm");
        let card = drm.join("card0");
        std::fs::create_dir_all(card.join("device")).unwrap();
        let drivers = sys.path().join("bus/pci/drivers/vmwgfx");
        std::fs::create_dir_all(&drivers).unwrap();
        std::os::unix::fs::symlink(&drivers, card.join("device/driver")).unwrap();
        std::fs::write(card.join("device/boot_vga"), "1\n").unwrap();
        for (c, status) in [("card0-Virtual-1", "connected"), ("card0-Virtual-2", "disconnected")] {
            std::fs::create_dir_all(drm.join(c)).unwrap();
            std::fs::write(drm.join(c).join("status"), status).unwrap();
            std::fs::write(drm.join(c).join("modes"), "1920x1080\n1280x720\n").unwrap();
        }
        let report = run_checks(sys.path());
        let text: Vec<&str> = report.lines.iter().map(|(_, t)| t.as_str()).collect();
        assert!(text.iter().any(|t| t.starts_with("GPU card0: vmwgfx, the boot screen's") && t.contains("3D acceleration")), "{text:?}");
        assert!(text.contains(&"  monitor Virtual-1: 1920x1080"), "{text:?}");
        assert!(!text.iter().any(|t| t.contains("Virtual-2")), "a disconnected one is not listed");
    }

    #[test]
    fn no_gpu_is_a_problem() {
        let sys = tempfile::tempdir().unwrap();
        let report = run_checks(sys.path());
        assert!(report.lines.iter().any(|(m, t)| *m == Mark::Bad && t.starts_with("no GPU")));
    }
}

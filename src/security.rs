//! Who may use the protocols that see or control other programs: screen capture, the window list
//! (and closing or minimizing other windows), and locking the screen.
//!
//! A program gets them when it is one of FDE's own (it lives next to the compositor) or when it is
//! named in `[security] trusted_programs` and installed in a system directory (/usr, /bin, /opt ...,
//! owned by root and writable by nobody else). Everything else (an ordinary app, a sandboxed Flatpak, a script in
//! your home directory) does not even see those protocols. `trusted_programs = []` and
//! `restrict = false` are the two ways to change that.
//!
//! This keeps sandboxed and untrusted-by-accident programs out. It is not a wall against a program
//! that already runs as the user outside a sandbox: such a program can start a trusted one itself.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use smithay::reexports::wayland_server::Client;

use crate::state::ClientState;

/// Programs from outside FDE that may capture the screen, list windows or lock the screen.
pub const DEFAULT_TRUSTED: &[&str] = &[
    "grim",
    "wf-recorder",
    "wl-screenrec",
    "xdg-desktop-portal-wlr",
    "swaylock",
    "waylock",
    "obs",
    "waybar",
    "nwg-panel",
    // launchers, menus and notification programs that use overlays or take the keyboard
    "fuzzel",
    "wofi",
    "rofi",
    "tofi",
    "bemenu",
    "bemenu-run",
    "nwg-drawer",
    "wlogout",
    "mako",
    "dunst",
    "swaync",
];

/// The process on the other end of a client connection (from the kernel: SO_PEERCRED).
fn peer_pid(stream: &std::os::unix::net::UnixStream) -> Option<i32> {
    use std::os::fd::AsRawFd;
    let mut cred = libc::ucred { pid: 0, uid: 0, gid: 0 };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let r = unsafe { libc::getsockopt(stream.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, &mut cred as *mut _ as *mut libc::c_void, &mut len) };
    (r == 0 && cred.pid > 0).then_some(cred.pid)
}

/// The program on the other end of a client connection.
pub fn peer_executable(stream: &std::os::unix::net::UnixStream) -> Option<PathBuf> {
    verified_executable(peer_pid(stream)?)
}

/// The program file process `pid` runs, as a path that means the same file *here*.
///
/// `/proc/<pid>/exe` reads as a path inside the process's own view of the file system: a
/// program in a sandbox or a mount namespace of its own can make that say `/usr/bin/grim` while
/// running something else entirely. So the file behind the link (which the kernel resolves to
/// the real file, whatever the process sees) must be the very file that path names for us:
/// the same device and inode. Otherwise there is no answer, and the program is an ordinary one.
pub fn verified_executable(pid: i32) -> Option<PathBuf> {
    let link = format!("/proc/{pid}/exe");
    let path = std::fs::read_link(&link).ok()?;
    let real = std::fs::metadata(&link).ok()?;
    let here = std::fs::metadata(&path).ok()?;
    (real.dev() == here.dev() && real.ino() == here.ino()).then_some(path)
}

/// Where system packages put programs.
const SYSTEM_DIRS: &[&str] = &["/usr/", "/bin/", "/sbin/", "/opt/", "/nix/store/"];

/// In a system folder, and neither the file nor its directory can be changed by anyone but root.
fn installed_by_root(path: &Path) -> bool {
    let in_system = SYSTEM_DIRS.iter().any(|d| path.starts_with(d));
    let safe = |p: &Path| std::fs::metadata(p).map(|m| m.uid() == 0 && m.mode() & 0o022 == 0).unwrap_or(false);
    in_system && safe(path) && path.parent().map(safe).unwrap_or(false)
}

/// May the program at `exe` use the protocols that see or control other programs?
/// `own_dir` is where the compositor itself is installed.
pub fn is_trusted(exe: &Path, own_dir: Option<&Path>, trusted: &[String]) -> bool {
    let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    // FDE's own programs, next to the compositor
    if let (Some(dir), Some(parent)) = (own_dir, exe.parent()) {
        let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        let ours = ["fde-panel", "fde-lock", "fde-greeter", "fde-notify", "fde-shot", "fde-settings", "fde-music"];
        let name = exe.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if parent == dir && ours.contains(&name) {
            return true;
        }
    }
    // named in the configuration and installed where only root can put it
    let name = exe.file_name().and_then(|n| n.to_str()).unwrap_or("");
    trusted.iter().any(|t| t == name) && installed_by_root(&exe)
}

/// The data a new client connection starts with.
pub fn client_state_for(stream: &std::os::unix::net::UnixStream, config: &crate::config::Config) -> ClientState {
    let privileged = if !config.security.restrict {
        true
    } else {
        let own_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
        match peer_executable(stream) {
            Some(exe) => {
                let ok = is_trusted(&exe, own_dir.as_deref(), &config.security.trusted_programs);
                tracing::debug!("client {}: {}", exe.display(), if ok { "trusted" } else { "ordinary" });
                ok
            }
            None => false,
        }
    };
    ClientState { privileged, ..Default::default() }
}

/// May this client see the protocols that see or control other programs?
pub fn privileged(client: &Client) -> bool {
    client.get_data::<ClientState>().map(|d| d.privileged).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fde_programs_next_to_the_compositor_and_root_installed_named_programs_are_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let panel = dir.path().join("fde-panel");
        std::fs::write(&panel, "").unwrap();
        let stranger = dir.path().join("evil");
        std::fs::write(&stranger, "").unwrap();
        assert!(is_trusted(&panel, Some(dir.path()), &[]), "our own task bar");
        assert!(!is_trusted(&stranger, Some(dir.path()), &[]), "another program in the same folder is not ours");
        assert!(!is_trusted(&panel, Some(Path::new("/nonexistent")), &[]), "an fde-panel elsewhere is not ours");
        // a named program counts only where only root can put it
        let named = vec!["env".to_string(), "grim".to_string()];
        if let Ok(env) = std::fs::canonicalize("/usr/bin/env") {
            if installed_by_root(&env) {
                assert!(is_trusted(&env, None, &named));
            }
        }
        let fake = dir.path().join("grim");
        std::fs::write(&fake, "").unwrap();
        assert!(!is_trusted(&fake, None, &named), "a grim in a folder the user can write to is not trusted");
    }

    #[test]
    fn the_program_of_a_process_is_the_file_the_kernel_says_it_runs() {
        // this test process: the path /proc gives names the very file it runs
        let me = verified_executable(std::process::id() as i32).expect("our own program is found");
        assert_eq!(std::fs::canonicalize(me).unwrap(), std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap());
        assert!(verified_executable(i32::MAX).is_none(), "no such process: no answer");
    }

    #[test]
    fn a_program_whose_path_names_another_file_here_is_not_believed() {
        // what a mount namespace does: the process runs one file while its path names another.
        // Stand-in: run a copy of a program, then put a different file at that path.
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("sleeper");
        std::fs::copy("/bin/sleep", &exe).or_else(|_| std::fs::copy("/usr/bin/sleep", &exe)).unwrap();
        let mut child = std::process::Command::new(&exe).arg("30").spawn().unwrap();
        let pid = child.id() as i32;
        assert!(verified_executable(pid).is_some());
        let other = dir.path().join("other");
        std::fs::write(&other, b"something else").unwrap();
        std::fs::rename(&other, &exe).unwrap();
        let seen = verified_executable(pid);
        let _ = child.kill();
        let _ = child.wait();
        assert!(seen.is_none(), "the path now names a different file: {seen:?}");
    }
}

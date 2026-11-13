//! Small status readouts for the task bar: volume (PipeWire or PulseAudio), network and battery. Plain
//! commands and files, nothing resident: the bar looks every couple of seconds.

use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volume {
    pub percent: u32,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Battery {
    pub percent: u32,
    pub charging: bool,
}

/// "Volume: 0.45" or "Volume: 0.45 [MUTED]", what `wpctl get-volume` prints.
pub fn parse_wpctl(out: &str) -> Option<Volume> {
    let rest = out.trim().strip_prefix("Volume:")?.trim();
    let value: f64 = rest.split_whitespace().next()?.parse().ok()?;
    Some(Volume { percent: (value * 100.0).round().max(0.0) as u32, muted: rest.contains("[MUTED]") })
}

/// The first percentage of `pactl get-sink-volume` ("Volume: front-left: 29491 /  45% / ...") and
/// the answer of `pactl get-sink-mute` ("Mute: yes").
pub fn parse_pactl(volume: &str, mute: &str) -> Option<Volume> {
    let percent = volume.split('/').find_map(|part| part.trim().strip_suffix('%')?.trim().parse::<u32>().ok())?;
    Some(Volume { percent, muted: mute.trim().ends_with("yes") })
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    // never wait long on a sound server that does not answer: the task bar draws on this thread
    let out = Command::new("timeout").arg("1").arg(program).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The volume of the default output, if a sound server answers.
pub fn volume() -> Option<Volume> {
    if let Some(v) = output("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]).and_then(|o| parse_wpctl(&o)) {
        return Some(v);
    }
    let vol = output("pactl", &["get-sink-volume", "@DEFAULT_SINK@"])?;
    let mute = output("pactl", &["get-sink-mute", "@DEFAULT_SINK@"]).unwrap_or_default();
    parse_pactl(&vol, &mute)
}

/// Make the volume `delta` percent louder (or quieter, never above 100 %).
pub fn change_volume(delta: i32) {
    let arg = format!("{}%{}", delta.abs(), if delta >= 0 { "+" } else { "-" });
    if Command::new("wpctl").args(["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", &arg]).status().is_err() {
        let _ = Command::new("pactl").args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}{}%", if delta >= 0 { "+" } else { "-" }, delta.abs())]).status();
    }
}

/// Set the volume of the default output to `percent` (0 .. 100).
pub fn set_volume(percent: u32) {
    let value = format!("{:.2}", percent.min(100) as f64 / 100.0);
    if Command::new("wpctl").args(["set-volume", "@DEFAULT_AUDIO_SINK@", &value]).status().is_err() {
        let _ = Command::new("pactl").args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", percent.min(100))]).status();
    }
}

/// Mute or unmute the default output.
pub fn set_mute(muted: bool) {
    let v = if muted { "1" } else { "0" };
    if Command::new("wpctl").args(["set-mute", "@DEFAULT_AUDIO_SINK@", v]).status().is_err() {
        let _ = Command::new("pactl").args(["set-sink-mute", "@DEFAULT_SINK@", v]).status();
    }
}

pub fn toggle_mute() {
    if Command::new("wpctl").args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]).status().is_err() {
        let _ = Command::new("pactl").args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"]).status();
    }
}

/// The first battery under `root` (`/sys/class/power_supply`), if the machine has one.
pub fn battery_in(root: &std::path::Path) -> Option<Battery> {
    let mut dirs: Vec<_> = std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).collect();
    dirs.sort();
    for d in dirs {
        let kind = std::fs::read_to_string(d.join("type")).unwrap_or_default();
        if kind.trim() != "Battery" {
            continue;
        }
        let percent: u32 = std::fs::read_to_string(d.join("capacity")).ok()?.trim().parse().ok()?;
        let status = std::fs::read_to_string(d.join("status")).unwrap_or_default();
        return Some(Battery { percent: percent.min(100), charging: matches!(status.trim(), "Charging" | "Full") });
    }
    None
}

pub fn battery() -> Option<Battery> {
    battery_in(std::path::Path::new("/sys/class/power_supply"))
}

/// How the machine is connected.
#[derive(Debug, Clone, PartialEq)]
pub enum Network {
    /// a Wi-Fi interface is up: the network's name if known, signal 0 .. 100 if known
    Wifi { name: Option<String>, signal: Option<u32> },
    Wired,
    Offline,
}

/// The link quality of `iface` in `/proc/net/wireless` ("wlan0: 0000   54.  -56.  ..."), as a
/// percentage of the usual maximum of 70.
pub fn parse_wireless(text: &str, iface: &str) -> Option<u32> {
    let line = text.lines().find(|l| l.trim_start().starts_with(&format!("{iface}:")))?;
    let quality: f64 = line.split_whitespace().nth(2)?.trim_end_matches('.').parse().ok()?;
    Some(((quality / 70.0) * 100.0).round().clamp(0.0, 100.0) as u32)
}

/// The connected Wi-Fi network in `nmcli -t -f active,ssid dev wifi` output ("yes:Home").
pub fn parse_nmcli_ssid(out: &str) -> Option<String> {
    out.lines().find_map(|l| l.strip_prefix("yes:")).map(|s| s.replace("\\:", ":")).filter(|s| !s.is_empty())
}

/// Look at the interfaces under `net` (`/sys/class/net`): Wi-Fi beats wired, up beats down.
/// `wireless` is the text of `/proc/net/wireless`.
pub fn network_in(net: &std::path::Path, wireless: &str) -> Network {
    let mut names: Vec<String> = std::fs::read_dir(net)
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    let mut wired = false;
    for name in names {
        let dir = net.join(&name);
        // the loopback, and virtual links (bridges for containers, VPN tunnels) do not count
        if name == "lo" || !dir.join("device").exists() {
            continue;
        }
        let up = std::fs::read_to_string(dir.join("operstate")).map(|s| s.trim() == "up").unwrap_or(false);
        if !up {
            continue;
        }
        if dir.join("wireless").exists() || dir.join("phy80211").exists() {
            return Network::Wifi { name: None, signal: parse_wireless(wireless, &name) };
        }
        wired = true;
    }
    if wired {
        Network::Wired
    } else {
        Network::Offline
    }
}

pub fn network() -> Network {
    let wireless = std::fs::read_to_string("/proc/net/wireless").unwrap_or_default();
    match network_in(std::path::Path::new("/sys/class/net"), &wireless) {
        Network::Wifi { signal, .. } => {
            let name = output("nmcli", &["-t", "-f", "active,ssid", "dev", "wifi"]).and_then(|o| parse_nmcli_ssid(&o));
            Network::Wifi { name, signal }
        }
        other => other,
    }
}

pub fn network_label(n: &Network) -> String {
    match n {
        Network::Wifi { name, signal } => {
            let mut s = name.clone().unwrap_or_else(|| "Wi-Fi".to_string());
            if s.chars().count() > 14 {
                s = s.chars().take(13).collect::<String>() + "…";
            }
            match signal {
                Some(p) => format!("{s} {p}%"),
                None => s,
            }
        }
        Network::Wired => "Wired".to_string(),
        Network::Offline => "Offline".to_string(),
    }
}

/// The network settings: NetworkManager's editor if it is installed, else its text interface in
/// the terminal.
pub fn open_network_settings(terminal: &str) {
    if Command::new("nm-connection-editor").spawn().is_ok() {
        return;
    }
    let _ = Command::new(terminal).args(["-e", "nmtui"]).spawn();
}

/// What a status label looks like.
pub fn volume_label(v: Volume) -> String {
    if v.muted {
        "Vol muted".to_string()
    } else {
        format!("Vol {}%", v.percent)
    }
}

pub fn battery_label(b: Battery) -> String {
    format!("Bat {}%{}", b.percent, if b.charging { "+" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volumes_are_read_from_wpctl_and_pactl_output() {
        assert_eq!(parse_wpctl("Volume: 0.45\n"), Some(Volume { percent: 45, muted: false }));
        assert_eq!(parse_wpctl("Volume: 1.00 [MUTED]\n"), Some(Volume { percent: 100, muted: true }));
        assert_eq!(parse_wpctl("nonsense"), None);
        let v = "Volume: front-left: 29491 /  45% / -20.00 dB,   front-right: 29491 /  45% / -20.00 dB\n";
        assert_eq!(parse_pactl(v, "Mute: no\n"), Some(Volume { percent: 45, muted: false }));
        assert_eq!(parse_pactl(v, "Mute: yes\n").unwrap().muted, true);
        assert_eq!(volume_label(Volume { percent: 45, muted: false }), "Vol 45%");
        assert_eq!(volume_label(Volume { percent: 45, muted: true }), "Vol muted");
    }

    #[test]
    fn the_network_is_wifi_wired_or_offline_from_the_interfaces() {
        let dir = tempfile::tempdir().unwrap();
        let iface = |name: &str, state: &str, wifi: bool, physical: bool| {
            let d = dir.path().join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("operstate"), format!("{state}\n")).unwrap();
            if wifi {
                std::fs::create_dir_all(d.join("wireless")).unwrap();
            }
            if physical {
                std::fs::create_dir_all(d.join("device")).unwrap();
            }
        };
        iface("lo", "unknown", false, false);
        iface("docker0", "up", false, false);
        assert_eq!(network_in(dir.path(), ""), Network::Offline, "loopback and a container bridge are not a connection");
        iface("enp0s3", "up", false, true);
        assert_eq!(network_in(dir.path(), ""), Network::Wired);
        iface("wlan0", "down", true, true);
        assert_eq!(network_in(dir.path(), ""), Network::Wired, "Wi-Fi that is down does not count");
        iface("wlan0", "up", true, true);
        let wireless = "Inter-| sta-|   Quality\n face | tus | link level noise\n wlan0: 0000   54.  -56.  -256        0      0      0      0     12        0\n";
        assert_eq!(network_in(dir.path(), wireless), Network::Wifi { name: None, signal: Some(77) });
        assert_eq!(network_label(&Network::Wifi { name: Some("Home".into()), signal: Some(77) }), "Home 77%");
        assert_eq!(network_label(&Network::Wifi { name: Some("A very long network name".into()), signal: None }), "A very long n…");
        assert_eq!(network_label(&Network::Offline), "Offline");
        assert_eq!(parse_nmcli_ssid("no:Neighbour\nyes:Home\n"), Some("Home".into()));
        assert_eq!(parse_nmcli_ssid("no:Neighbour\n"), None);
    }

    #[test]
    fn the_battery_comes_from_the_first_battery_directory() {
        let dir = tempfile::tempdir().unwrap();
        let ac = dir.path().join("AC");
        let bat = dir.path().join("BAT0");
        std::fs::create_dir_all(&ac).unwrap();
        std::fs::create_dir_all(&bat).unwrap();
        std::fs::write(ac.join("type"), "Mains\n").unwrap();
        std::fs::write(bat.join("type"), "Battery\n").unwrap();
        std::fs::write(bat.join("capacity"), "87\n").unwrap();
        std::fs::write(bat.join("status"), "Charging\n").unwrap();
        assert_eq!(battery_in(dir.path()), Some(Battery { percent: 87, charging: true }));
        assert_eq!(battery_label(battery_in(dir.path()).unwrap()), "Bat 87%+");
        std::fs::remove_dir_all(&bat).unwrap();
        assert_eq!(battery_in(dir.path()), None, "a desktop machine has none");
    }

    #[test]
    fn volume_changes_run_wpctl_with_a_cap_at_100_percent() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log");
        let script = dir.path().join("wpctl");
        std::fs::write(&script, format!("#!/bin/sh\necho \"$@\" >> {}\n", log.display())).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!("{}:{}", dir.path().display(), std::env::var("PATH").unwrap_or_default());
        std::env::set_var("PATH", path);
        change_volume(5);
        change_volume(-5);
        toggle_mute();
        set_volume(35);
        set_volume(250);
        set_mute(true);
        let text = std::fs::read_to_string(&log).unwrap();
        assert_eq!(text, "set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%+\nset-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%-\nset-mute @DEFAULT_AUDIO_SINK@ toggle\nset-volume @DEFAULT_AUDIO_SINK@ 0.35\nset-volume @DEFAULT_AUDIO_SINK@ 1.00\nset-mute @DEFAULT_AUDIO_SINK@ 1\n");
    }
}

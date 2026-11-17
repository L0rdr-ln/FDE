use fde::{backend_drm, backend_winit, demo, glass};

use smithay::utils::Rectangle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    // the report alone, without the graphics libraries' own messages
    if matches!(args.get(1).map(String::as_str), Some("--check") | Some("check")) {
        fde::check::run_checks(std::path::Path::new("/sys")).print();
        return Ok(());
    }
    // the theme's colours for GTK and Qt programs, once (what the session does at its start)
    if matches!(args.get(1).map(String::as_str), Some("--apply-style") | Some("apply-style")) {
        let outcomes = fde::appstyle::apply_current();
        if outcomes.is_empty() {
            println!("appearance.style_apps is off (or there is no configuration folder): nothing written");
        }
        for o in outcomes {
            let what = match &o.what {
                fde::appstyle::What::Wrote => "wrote".to_string(),
                fde::appstyle::What::Unchanged => "already right".to_string(),
                fde::appstyle::What::Skipped(why) => format!("left alone: {why}"),
            };
            println!("{}: {what}", o.path.display());
        }
        return Ok(());
    }
    // every effect drawn off-screen on this computer's graphics driver
    if matches!(args.get(1).map(String::as_str), Some("--selftest") | Some("selftest")) {
        let report = fde::selftest::run();
        report.print();
        if report.lines.iter().any(|(m, _)| *m == fde::check::Mark::Bad) {
            std::process::exit(1);
        }
        return Ok(());
    }
    tracing_subscriber::fmt::init();
    match args.get(1).map(String::as_str) {
        Some("glass-demo") => {
            let out = args.get(2).map(String::as_str).unwrap_or("glass-demo.png");
            let mut renderer = demo::offscreen_renderer()?;
            let glass = glass::Glass::new(&mut renderer)?;
            let windows = vec![
                demo::Window {
                    outer: Rectangle::new((70, 120).into(), (440, 280).into()),
                    style: glass::GlassStyle { focus: 0.0, ..Default::default() },
                    client_alpha: 0.35,
                },
                demo::Window {
                    outer: Rectangle::new((330, 200).into(), (480, 290).into()),
                    style: glass::GlassStyle::default(),
                    client_alpha: 0.35,
                },
            ];
            let px = demo::render_scene(&mut renderer, &glass, &windows)?;
            demo::write_png(out, &px, demo::W, demo::H, false)?;
            println!("wrote {out}");
            Ok(())
        }
        Some("winit") => backend_winit::run(),
        Some("drm") => backend_drm::run(),
        // no argument: nested if there is a desktop to open a window in, else on the hardware
        None => {
            if std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some() {
                backend_winit::run()
            } else {
                backend_drm::run()
            }
        }
        _ => {
            eprintln!("usage: fde [winit|drm]        run nested in a window (winit) or on the hardware (drm); default: by environment\n       fde --check           what FDE would find (GPU, monitors, seat, programs); changes nothing\n       fde --apply-style     write the theme's colours for GTK and Qt programs (the session does it at its start)\n       fde --selftest        draw every effect off-screen and check the pixels (does this GPU do it right?)\n       fde glass-demo [out.png]");
            std::process::exit(2);
        }
    }
}

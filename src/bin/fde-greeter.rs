//! The login screen, a greetd greeter: runs inside `fde` started with FDE_GREETER=1 (see
//! docs/SESSION.md), shows the glass card of the lock screen on every monitor, asks for a user name
//! and then for what PAM wants (the password), and has greetd start the session.

use std::io::{Read, Write};
use std::os::fd::AsFd;

use fde::greetd::{self, Step};
use fde::lockscreen;
use fde::text::TextRenderer;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use xkbcommon::xkb;

struct Output {
    surface: wl_surface::WlSurface,
    _layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    size: (i32, i32),
    scale: i32,
    /// the wallpaper at this monitor's size and scale, made once
    background: Option<((i32, i32), Vec<u8>)>,
    file: Option<std::fs::File>,
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    /// typing the user name (shown as typed)
    Name,
    /// typing the answer to something greetd asked; `true`: a secret (dots)
    Answer(bool),
}

struct App {
    outputs: Vec<Output>,
    /// what is asked now
    stage: Stage,
    user: String,
    /// a visible answer (the user name) while typing it
    typed: String,
    message: String,
    redraw: bool,
    submit: bool,
    done: bool,
    xkb_state: Option<xkb::State>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for App {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
delegate_noop!(App: ignore wl_compositor::WlCompositor);
delegate_noop!(App: ignore wl_surface::WlSurface);
/// The user data is the index of the output in `App::outputs`.
impl Dispatch<wl_output::WlOutput, usize> for App {
    fn event(app: &mut Self, _: &wl_output::WlOutput, event: wl_output::Event, idx: &usize, _: &Connection, _: &QueueHandle<Self>) {
        if let wl_output::Event::Scale { factor } = event {
            if let Some(o) = app.outputs.get_mut(*idx) {
                o.scale = factor.max(1);
                app.redraw = true;
            }
        }
    }
}
delegate_noop!(App: ignore wl_shm::WlShm);
delegate_noop!(App: ignore wl_shm_pool::WlShmPool);
delegate_noop!(App: ignore wl_buffer::WlBuffer);
delegate_noop!(App: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, usize> for App {
    fn event(app: &mut Self, s: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, event: zwlr_layer_surface_v1::Event, i: &usize, _: &Connection, _: &QueueHandle<Self>) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, width, height } = event {
            s.ack_configure(serial);
            app.outputs[*i].size = (width as i32, height as i32);
            app.redraw = true;
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for App {
    fn event(_: &mut Self, seat: &wl_seat::WlSeat, event: wl_seat::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(caps) } = event {
            if caps.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(qh, ());
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for App {
    fn event(app: &mut Self, _: &wl_keyboard::WlKeyboard, event: wl_keyboard::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            wl_keyboard::Event::Keymap { format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1), fd, size } => {
                let mut text = String::new();
                let mut file = std::fs::File::from(fd);
                let _ = std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(0));
                let mut bytes = vec![0u8; size as usize];
                if file.read_exact(&mut bytes).is_ok() {
                    text = String::from_utf8_lossy(&bytes).trim_end_matches('\0').to_string();
                }
                let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
                if let Some(keymap) = xkb::Keymap::new_from_string(&ctx, text, xkb::KEYMAP_FORMAT_TEXT_V1, xkb::KEYMAP_COMPILE_NO_FLAGS) {
                    app.xkb_state = Some(xkb::State::new(&keymap));
                }
            }
            wl_keyboard::Event::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                if let Some(st) = &mut app.xkb_state {
                    st.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                }
            }
            wl_keyboard::Event::Key { key, state: WEnum::Value(wl_keyboard::KeyState::Pressed), .. } => {
                let Some(st) = &app.xkb_state else { return };
                let code = xkb::Keycode::new(key + 8);
                let sym = st.key_get_one_sym(code);
                if sym == xkb::Keysym::Return || sym == xkb::Keysym::KP_Enter {
                    app.submit = true;
                } else if sym == xkb::Keysym::BackSpace {
                    app.typed.pop();
                    app.message.clear();
                    app.redraw = true;
                } else if sym == xkb::Keysym::Escape {
                    app.typed.clear();
                    app.message.clear();
                    app.redraw = true;
                } else {
                    let s = st.key_get_utf8(code);
                    if !s.is_empty() && !s.chars().any(|c| c.is_control()) {
                        app.typed.push_str(&s);
                        app.message.clear();
                        app.redraw = true;
                    }
                }
            }
            _ => {}
        }
    }
}

/// What greetd's answer means for the card.
fn apply(app: &mut App, step: Step) {
    app.typed.clear();
    match step {
        Step::Ask { secret, prompt, notes } => {
            app.stage = Stage::Answer(secret);
            app.message = notes.last().cloned().unwrap_or_else(|| prompt.trim().trim_end_matches(':').to_string());
        }
        Step::Authenticated => app.done = true,
        Step::Failed { wrong_secret, message } => {
            app.stage = Stage::Name;
            app.message = if wrong_secret { "Wrong password, try again".into() } else { message };
        }
    }
}

/// The command greetd starts: `start-fde` next to this program, or `FDE_GREETER_CMD`.
fn session_command() -> Vec<String> {
    if let Ok(c) = std::env::var("FDE_GREETER_CMD") {
        return c.split_whitespace().map(String::from).collect();
    }
    let next = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("start-fde"))).filter(|p| p.exists());
    vec![next.map(|p| p.display().to_string()).unwrap_or_else(|| "start-fde".into())]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut greetd = greetd::Client::connect_from_env()?;
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<App>(&conn)?;
    let qh = queue.handle();
    let compositor: wl_compositor::WlCompositor = globals.bind(&qh, 4..=4, ())?;
    let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ())?;
    let layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = globals.bind(&qh, 1..=4, ())?;
    let _seat: wl_seat::WlSeat = globals.bind(&qh, 1..=5, ())?;

    let mut app = App {
        outputs: Vec::new(),
        stage: Stage::Name,
        user: String::new(),
        typed: String::new(),
        message: "Type your user name".into(),
        redraw: false,
        submit: false,
        done: false,
        xkb_state: None,
    };
    for (name, interface, version) in globals.contents().clone_list().iter().map(|g| (g.name, g.interface.clone(), g.version)) {
        if interface == "wl_output" {
            let idx = app.outputs.len();
            let output: wl_output::WlOutput = globals.registry().bind(name, version.min(4), &qh, idx);
            let surface = compositor.create_surface(&qh, ());
            let layer = layer_shell.get_layer_surface(&surface, Some(&output), zwlr_layer_shell_v1::Layer::Overlay, "fde-greeter".into(), &qh, idx);
            layer.set_anchor(zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Bottom | zwlr_layer_surface_v1::Anchor::Left | zwlr_layer_surface_v1::Anchor::Right);
            layer.set_exclusive_zone(-1);
            layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive);
            surface.commit();
            app.outputs.push(Output { surface, _layer: layer, size: (0, 0), scale: 1, background: None, file: None });
        }
    }
    if app.outputs.is_empty() {
        eprintln!("fde-greeter: no outputs");
        return Ok(());
    }

    let config = fde::config::Config::load().unwrap_or_default();
    let (picture, mode) = (fde::wallpaper::for_config(&config), config.wallpaper.mode.clone());
    let mut text = TextRenderer::default();
    loop {
        if app.submit {
            app.submit = false;
            let typed = std::mem::take(&mut app.typed);
            let step = match app.stage {
                Stage::Name if typed.trim().is_empty() => None,
                Stage::Name => {
                    app.user = typed.trim().to_string();
                    Some(greetd.begin(&app.user)?)
                }
                Stage::Answer(_) => Some(greetd.answer(&typed)?),
            };
            if let Some(step) = step {
                let failed = matches!(step, Step::Failed { .. });
                apply(&mut app, step);
                if failed {
                    greetd.cancel()?;
                }
            }
            if app.done {
                match greetd.start(session_command(), vec![])? {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        app.done = false;
                        app.stage = Stage::Name;
                        app.message = e;
                        greetd.cancel()?;
                    }
                }
            }
            app.redraw = true;
        }
        if app.redraw {
            app.redraw = false;
            let (shown, dots) = match app.stage {
                Stage::Name => (if app.typed.is_empty() { "User name".to_string() } else { app.typed.clone() }, 0),
                Stage::Answer(true) => (app.user.clone(), app.typed.chars().count()),
                Stage::Answer(false) => (app.typed.clone(), 0),
            };
            for o in &mut app.outputs {
                let (lw, lh) = o.size;
                if lw <= 0 || lh <= 0 {
                    continue;
                }
                let s = o.scale;
                let (w, h) = (lw * s, lh * s);
                if o.background.as_ref().map(|(sz, _)| *sz != (w, h)).unwrap_or(true) {
                    o.background = Some(((w, h), lockscreen::background(picture.as_ref(), &mode, w, h)));
                }
                let bg = &o.background.as_ref().expect("background").1;
                let mut px = lockscreen::draw(&mut text, bg, w, h, s, &shown, dots, &app.message);
                for p in px.chunks_exact_mut(4) {
                    p.swap(0, 2);
                }
                let mut f = match o.file.take() {
                    Some(f) => f,
                    None => fde::shm::anonymous_file("fde-greeter")?,
                };
                f.set_len(px.len() as u64)?;
                std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(0))?;
                f.write_all(&px)?;
                let pool = shm.create_pool(f.as_fd(), px.len() as i32, &qh, ());
                let buffer = pool.create_buffer(0, w, h, w * 4, wl_shm::Format::Argb8888, &qh, ());
                o.surface.set_buffer_scale(s);
                o.surface.attach(Some(&buffer), 0, 0);
                o.surface.damage_buffer(0, 0, w, h);
                o.surface.commit();
                pool.destroy();
                o.file = Some(f);
            }
        }
        queue.blocking_dispatch(&mut app)?;
    }
}

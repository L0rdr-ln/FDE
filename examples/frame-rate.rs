//! Measures how responsive the compositor is: a window that draws a new frame every time the
//! compositor says the last one was shown, for three seconds, and prints how many it managed.
//! A healthy compositor gives about its refresh rate; a handful means events are getting stuck.
//!
//!     WAYLAND_DISPLAY=wayland-N cargo run --release --example frame-rate
use std::io::Write;
use std::time::{Duration, Instant};

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_buffer, wl_callback, wl_compositor, wl_registry, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default)]
struct App {
    configured: bool,
    frames: u32,
    frame_done: bool,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for App {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
delegate_noop!(App: ignore wl_compositor::WlCompositor);
delegate_noop!(App: ignore wl_surface::WlSurface);
delegate_noop!(App: ignore wl_shm::WlShm);
delegate_noop!(App: ignore wl_shm_pool::WlShmPool);
delegate_noop!(App: ignore wl_buffer::WlBuffer);
delegate_noop!(App: ignore xdg_toplevel::XdgToplevel);

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for App {
    fn event(_: &mut Self, wm: &xdg_wm_base::XdgWmBase, event: xdg_wm_base::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for App {
    fn event(app: &mut Self, s: &xdg_surface::XdgSurface, event: xdg_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_surface::Event::Configure { serial } = event {
            s.ack_configure(serial);
            app.configured = true;
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for App {
    fn event(app: &mut Self, _: &wl_callback::WlCallback, event: wl_callback::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_callback::Event::Done { .. } = event {
            app.frames += 1;
            app.frame_done = true;
        }
    }
}

fn main() {
    let conn = Connection::connect_to_env().expect("connect to WAYLAND_DISPLAY");
    let (globals, mut queue) = registry_queue_init::<App>(&conn).expect("registry");
    let qh = queue.handle();
    let compositor: wl_compositor::WlCompositor = globals.bind(&qh, 4..=4, ()).unwrap();
    let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).unwrap();
    let wm: xdg_wm_base::XdgWmBase = globals.bind(&qh, 1..=1, ()).unwrap();
    let mut app = App::default();
    let surface = compositor.create_surface(&qh, ());
    let xs = wm.get_xdg_surface(&surface, &qh, ());
    let _top = xs.get_toplevel(&qh, ());
    surface.commit();
    while !app.configured {
        queue.blocking_dispatch(&mut app).expect("dispatch");
    }
    let (w, h) = (200, 120);
    let dir = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR");
    let path = format!("{dir}/fde-frame-rate-{}", std::process::id());
    let mut file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    file.write_all(&vec![0xc0u8; (w * h * 4) as usize]).unwrap();
    use std::os::fd::AsFd;
    let pool = shm.create_pool(file.as_fd(), w * h * 4, &qh, ());
    let buffer = pool.create_buffer(0, w, h, w * 4, wl_shm::Format::Argb8888, &qh, ());

    let seconds = 3.0;
    let t0 = Instant::now();
    app.frame_done = true;
    while t0.elapsed() < Duration::from_secs_f64(seconds) {
        if app.frame_done {
            app.frame_done = false;
            surface.frame(&qh, ());
            surface.attach(Some(&buffer), 0, 0);
            surface.damage_buffer(0, 0, w, h);
            surface.commit();
        }
        // wait for the compositor, but not past the end of the measurement
        conn.flush().unwrap();
        if let Some(guard) = queue.prepare_read() {
            let fd = std::os::fd::AsRawFd::as_raw_fd(&guard.connection_fd());
            let mut fds = [libc::pollfd { fd, events: libc::POLLIN, revents: 0 }];
            if unsafe { libc::poll(fds.as_mut_ptr(), 1, 100) } > 0 {
                let _ = guard.read();
            }
        }
        queue.dispatch_pending(&mut app).unwrap();
    }
    println!("{} frames in {seconds} s = {:.0} per second", app.frames, app.frames as f64 / seconds);
}

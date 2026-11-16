//! `fde --selftest`: draws every effect FDE uses (the glass blur and frame, the warped, burning and
//! torn-apart windows, the workspace cube's faces, big textures) off-screen on the graphics
//! driver of this computer, reads the pixels back and says which ones come out right. It is for
//! a first start on new hardware: if something is wrong, this says what.

use std::time::Instant;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, Color32F, ExportMem, Frame, ImportMem, Offscreen, Renderer, Texture};
use smithay::utils::{Buffer as BufferCoord, Physical, Rectangle, Size, Transform};

use crate::check::{Mark, Report};
use crate::glass::{Glass, GlassStyle};

type R<T> = Result<T, Box<dyn std::error::Error>>;

fn bad<T>(what: String) -> R<T> {
    Err(what.into())
}

fn size(w: i32, h: i32) -> Size<i32, BufferCoord> {
    Size::from((w, h))
}

/// A texture of one colour (straight RGBA in, premultiplied inside).
fn solid(r: &mut GlesRenderer, w: i32, h: i32, rgba: [u8; 4]) -> R<GlesTexture> {
    let a = rgba[3] as u32;
    let px: Vec<u8> = (0..w * h).flat_map(|_| [(rgba[0] as u32 * a / 255) as u8, (rgba[1] as u32 * a / 255) as u8, (rgba[2] as u32 * a / 255) as u8, rgba[3]]).collect();
    Ok(r.import_memory(&px, Fourcc::Abgr8888, size(w, h), false)?)
}

/// A texture that is `a` and `b` in squares of `cell` pixels.
fn checker(r: &mut GlesRenderer, w: i32, h: i32, cell: i32) -> R<GlesTexture> {
    let px: Vec<u8> = (0..w * h).flat_map(|i| if ((i % w) / cell + (i / w) / cell) % 2 == 0 { [255, 255, 255, 255] } else { [0, 0, 0, 255] }).collect();
    Ok(r.import_memory(&px, Fourcc::Abgr8888, size(w, h), false)?)
}

/// A target texture filled with `color`.
fn target(r: &mut GlesRenderer, w: i32, h: i32, color: [f32; 4]) -> R<GlesTexture> {
    let mut t: GlesTexture = r.create_buffer(Fourcc::Abgr8888, size(w, h))?;
    {
        let mut tg = r.bind(&mut t)?;
        let mut f = r.render(&mut tg, (w, h).into(), Transform::Normal)?;
        f.clear(Color32F::new(color[0], color[1], color[2], color[3]), &[Rectangle::from_size((w, h).into())])?;
        let _ = f.finish()?;
    }
    Ok(t)
}

/// The pixel at (x, y) of a target texture (R, G, B, A).
fn read(r: &mut GlesRenderer, t: &mut GlesTexture, x: i32, y: i32) -> R<[i32; 4]> {
    let sz = t.size();
    let tg = r.bind(t)?;
    let m = r.copy_framebuffer(&tg, Rectangle::from_size(sz), Fourcc::Abgr8888)?;
    let bytes = r.map_texture(&m)?;
    let i = ((y * sz.w + x) * 4) as usize;
    Ok([bytes[i] as i32, bytes[i + 1] as i32, bytes[i + 2] as i32, bytes[i + 3] as i32])
}

fn close(p: [i32; 4], want: [i32; 3], tol: i32) -> bool {
    (0..3).all(|c| (p[c] - want[c]).abs() <= tol)
}

fn corners(x0: f32, y0: f32, x1: f32, y1: f32) -> [(f32, f32); 4] {
    [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
}

type Check = fn(&mut GlesRenderer, &Glass) -> R<String>;

/// The checks: a name and a function that draws and looks at the pixels (Err says what is wrong).
const CHECKS: &[(&str, Check)] = &[
    ("textures and render targets of 4096 x 4096 (4K monitors, the cube)", check_big),
    ("blur makes a checkerboard grey", check_blur),
    ("glass frame covers what is behind it", check_frame),
    ("a warped window blends with its opacity (minimize, open, cube)", check_mesh_alpha),
    ("a torn-off piece shows only its part of the picture (shatter)", check_piece),
    ("the burn shader eats the picture (close effect)", check_burn),
    ("sprites (bubbles, ripples) import", check_sprites),
];

fn check_big(r: &mut GlesRenderer, _: &Glass) -> R<String> {
    let mut t = target(r, 4096, 4096, [0.0, 1.0, 0.0, 1.0])?;
    let p = read(r, &mut t, 4000, 4000)?;
    if close(p, [0, 255, 0], 3) {
        Ok("works".into())
    } else {
        bad(format!("the far corner came back {p:?}, not green: textures that big are not drawn right"))
    }
}

fn check_blur(r: &mut GlesRenderer, g: &Glass) -> R<String> {
    let (w, h) = (256, 256);
    let src = checker(r, w, h, 2)?;
    let mut scene = target(r, w, h, [0.0, 0.0, 0.0, 1.0])?;
    {
        let mut tg = r.bind(&mut scene)?;
        let mut f = r.render(&mut tg, (w, h).into(), Transform::Normal)?;
        f.render_texture_from_to(&src, Rectangle::from_size((w as f64, h as f64).into()), Rectangle::from_size((w, h).into()), &[Rectangle::from_size((w, h).into())], &[], Transform::Normal, 1.0, None, &[])?;
        let _ = f.finish()?;
    }
    let mut blurred = g.blur(r, &scene, Rectangle::<i32, Physical>::from_size((w, h).into()), 3)?;
    let s = blurred.size();
    let p = read(r, &mut blurred, s.w / 2, s.h / 2)?;
    if close(p, [128, 128, 128], 45) {
        Ok(format!("grey {}", p[0]))
    } else {
        bad(format!("the middle came back {p:?}, not mid-grey: the blur does nothing (or the shaders did not run)"))
    }
}

fn check_frame(r: &mut GlesRenderer, g: &Glass) -> R<String> {
    let (w, h) = (300, 200);
    let backdrop = solid(r, w, h, [255, 255, 255, 255])?;
    let mut scene = target(r, w, h, [1.0, 1.0, 1.0, 1.0])?;
    let style = GlassStyle { tint_frame: [0.0, 0.3, 1.0, 0.9], ..GlassStyle::default() };
    {
        let blurred = g.blur(r, &scene, Rectangle::<i32, Physical>::from_size((w, h).into()), 1)?;
        let bs = blurred.size();
        let mut tg = r.bind(&mut scene)?;
        let mut f = r.render(&mut tg, (w, h).into(), Transform::Normal)?;
        let outer = Rectangle::<i32, Physical>::new((20, 20).into(), (260, 160).into());
        let src = Rectangle::<f64, BufferCoord>::new((20.0 / w as f64 * bs.w as f64, 20.0 / h as f64 * bs.h as f64).into(), (260.0 / w as f64 * bs.w as f64, 160.0 / h as f64 * bs.h as f64).into());
        g.draw_panel_part(&mut f, &blurred, src, outer, &style, 1.0)?;
        let _ = f.finish()?;
    }
    let _ = backdrop;
    // the title bar region of the frame is tinted blue, the far corner is still white
    let frame = read(r, &mut scene, 150, 36)?;
    let outside = read(r, &mut scene, 5, 5)?;
    if close(outside, [255, 255, 255], 4) && frame[2] > frame[0] + 25 {
        Ok(format!("title bar {:?}", &frame[..3]))
    } else {
        bad(format!("title bar {frame:?}, outside {outside:?}: the glass shader draws nothing, or the wrong thing"))
    }
}

fn check_mesh_alpha(r: &mut GlesRenderer, g: &Glass) -> R<String> {
    let (w, h) = (200, 200);
    let red = solid(r, 64, 64, [255, 0, 0, 255])?;
    let mut scene = target(r, w, h, [0.0, 0.0, 1.0, 1.0])?;
    {
        let mut tg = r.bind(&mut scene)?;
        let mut f = r.render(&mut tg, (w, h).into(), Transform::Normal)?;
        g.mesh.draw_part(&mut f, &red, (w, h), &corners(0.0, 0.0, w as f32, h as f32), 2, 0.5, (0.0, 0.0, 1.0, 1.0))?;
        let _ = f.finish()?;
    }
    let p = read(r, &mut scene, w / 2, h / 2)?;
    if close(p, [128, 0, 128], 14) {
        Ok(format!("{:?}", &p[..3]))
    } else {
        bad(format!("red at half opacity over blue came back {p:?}, not purple (128, 0, 128)"))
    }
}

fn check_piece(r: &mut GlesRenderer, g: &Glass) -> R<String> {
    let (w, h) = (200, 200);
    // a picture that is red on the left half and green on the right
    let px: Vec<u8> = (0..64 * 64).flat_map(|i| if i % 64 < 32 { [255, 0, 0, 255] } else { [0, 255, 0, 255] }).collect();
    let tex = r.import_memory(&px, Fourcc::Abgr8888, size(64, 64), false)?;
    let mut scene = target(r, w, h, [0.0, 0.0, 0.0, 1.0])?;
    {
        let mut tg = r.bind(&mut scene)?;
        let mut f = r.render(&mut tg, (w, h).into(), Transform::Normal)?;
        // the right half of the picture on the left half of the target, as a piece
        g.mesh.draw_part(&mut f, &tex, (w, h), &corners(0.0, 0.0, 100.0, 200.0), 2, 1.0, (0.5, 0.0, 1.0, 1.0))?;
        let _ = f.finish()?;
    }
    let inside = read(r, &mut scene, 50, 100)?;
    let outside = read(r, &mut scene, 150, 100)?;
    if close(inside, [0, 255, 0], 8) && close(outside, [0, 0, 0], 4) {
        Ok("right part, nothing outside".into())
    } else {
        bad(format!("inside the piece {inside:?} (green expected), outside {outside:?} (black expected)"))
    }
}

fn check_burn(r: &mut GlesRenderer, g: &Glass) -> R<String> {
    let (w, h) = (200, 200);
    let red = solid(r, 64, 64, [255, 0, 0, 255])?;
    let draw = |r: &mut GlesRenderer, burn: f32| -> R<[i32; 4]> {
        let mut scene = target(r, w, h, [0.0, 0.0, 1.0, 1.0])?;
        {
            let mut tg = r.bind(&mut scene)?;
            let mut f = r.render(&mut tg, (w, h).into(), Transform::Normal)?;
            g.mesh.draw_burn(&mut f, &red, (w, h), &corners(0.0, 0.0, w as f32, h as f32), 2, 1.0, burn)?;
            let _ = f.finish()?;
        }
        read(r, &mut scene, w / 2, h / 2)
    };
    let whole = draw(r, 0.0)?;
    let gone = draw(r, 1.0)?;
    if close(whole, [255, 0, 0], 6) && close(gone, [0, 0, 255], 6) {
        Ok("whole at 0, gone at 1".into())
    } else {
        bad(format!("burn 0 gave {whole:?} (red expected), burn 1 gave {gone:?} (the blue background expected)"))
    }
}

fn check_sprites(r: &mut GlesRenderer, _: &Glass) -> R<String> {
    let n = 128usize;
    r.import_memory(&crate::effects::bubble_sprite(n), Fourcc::Abgr8888, size(n as i32, n as i32), false)?;
    r.import_memory(&crate::effects::ring_sprite(n), Fourcc::Abgr8888, size(n as i32, n as i32), false)?;
    Ok("works".into())
}

/// How long one frame's worth of blur (a full HD screen, three levels) takes here, in ms.
fn blur_frame_ms(r: &mut GlesRenderer, g: &Glass) -> R<f64> {
    let (w, h) = (1920, 1080);
    let scene = target(r, w, h, [0.2, 0.4, 0.8, 1.0])?;
    let region = Rectangle::<i32, Physical>::from_size((w, h).into());
    // warm up (shader compile, allocation), then measure; reading a pixel back waits for the GPU
    let mut last = g.blur(r, &scene, region, 3)?;
    read(r, &mut last, 0, 0)?;
    let runs = 4;
    let t0 = Instant::now();
    for _ in 0..runs {
        last = g.blur(r, &scene, region, 3)?;
    }
    read(r, &mut last, 0, 0)?;
    Ok(t0.elapsed().as_secs_f64() * 1000.0 / runs as f64)
}

/// Run the checks and say what came out.
pub fn run() -> Report {
    let mut rep = Report { lines: Vec::new() };
    let mut renderer = match crate::demo::offscreen_renderer() {
        Ok(r) => r,
        Err(e) => {
            rep.add(Mark::Bad, format!("no OpenGL ES ({e}): install Mesa, or in a VM switch on 3D acceleration; nothing else can be tested"));
            return rep;
        }
    };
    let name = renderer
        .with_context(|gl| unsafe {
            let p = gl.GetString(smithay::backend::renderer::gles::ffi::RENDERER);
            if p.is_null() { String::new() } else { std::ffi::CStr::from_ptr(p as *const _).to_string_lossy().into_owned() }
        })
        .unwrap_or_default();
    let soft = ["llvmpipe", "softpipe", "swrast", "software"].iter().any(|k| name.to_lowercase().contains(k));
    rep.add(if soft { Mark::Note } else { Mark::Ok }, format!("graphics driver: {name}{}", if soft { " (drawn by the CPU: FDE's fast mode will be chosen)" } else { "" }));
    let glass = match Glass::new(&mut renderer) {
        Ok(g) => {
            rep.add(Mark::Ok, "the glass and mesh shaders compile and link");
            g
        }
        Err(e) => {
            rep.add(Mark::Bad, format!("the shaders do not compile on this driver ({e}): glass frames and every effect need them"));
            return rep;
        }
    };
    for (what, check) in CHECKS {
        match check(&mut renderer, &glass) {
            Ok(detail) => rep.add(Mark::Ok, format!("{what}: {detail}")),
            Err(e) => rep.add(Mark::Bad, format!("{what}: {e}")),
        }
    }
    match blur_frame_ms(&mut renderer, &glass) {
        Ok(ms) if ms > 40.0 && !soft => rep.add(Mark::Note, format!("blur of a full HD screen takes {ms:.0} ms here: windows will stutter (set appearance.performance = \"fast\")")),
        Ok(ms) => rep.add(Mark::Ok, format!("blur of a full HD screen: {ms:.0} ms per pass of three levels{}", if soft { " (CPU drawing)" } else { "" })),
        Err(e) => rep.add(Mark::Note, format!("could not time the blur: {e}")),
    }
    rep
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where there is a graphics context (this machine's, or Mesa's software one) every check
    /// passes: the checks are tests of the shaders' own results.
    #[test]
    fn every_check_passes_where_there_is_a_gl_context() {
        let report = run();
        let no_gl = report.lines.first().is_some_and(|(m, t)| *m == Mark::Bad && t.starts_with("no OpenGL ES"));
        if no_gl {
            eprintln!("no GL context here: selftest test skipped");
            return;
        }
        let bad: Vec<&str> = report.lines.iter().filter(|(m, _)| *m == Mark::Bad).map(|(_, t)| t.as_str()).collect();
        assert!(bad.is_empty(), "{bad:#?}");
        assert!(report.lines.len() >= CHECKS.len() + 2);
    }
}

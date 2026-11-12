//! Layer-shell surfaces (wallpaper, panel, launcher, notifications): keyboard focus and what
//! is under the pointer.

use smithay::desktop::{layer_map_for_output, LayerSurface, WindowSurfaceType};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::shell::wlr_layer::{KeyboardInteractivity, Layer};

use crate::state::Fde;

/// Overlays (above everything, full-screen windows included) and taking the keyboard without a
/// click are for trusted programs (`[security]`): any other program could put up a fake password
/// prompt over the whole screen and read what is typed into it. Panels, docks, wallpapers and
/// launchers that are clicked work for every program.
pub fn layer_trusted(surface: &WlSurface) -> bool {
    surface.client().map(|c| crate::security::privileged(&c)).unwrap_or(false)
}

impl Fde {
    /// An overlay from a program that may not have one is closed. True if it was.
    pub fn refuse_untrusted_overlay(&mut self, layer: &LayerSurface, requested: Layer) -> bool {
        if requested != Layer::Overlay || layer_trusted(layer.wl_surface()) {
            return false;
        }
        tracing::warn!("{}: overlays are for trusted programs ([security] trusted_programs); closed", layer.namespace());
        for output in self.all_outputs() {
            let mut map = layer_map_for_output(&output);
            if map.layers().any(|l| l == layer) {
                map.unmap_layer(layer);
                map.arrange();
            }
        }
        layer.layer_surface().send_close();
        true
    }

    /// A layer surface that wants the keyboard exclusively takes it. Called on every commit: the
    /// client sets its interactivity after creating the surface, so it is known only from the
    /// first commit on.
    pub fn focus_layer_if_exclusive(&mut self, layer: &LayerSurface) {
        let wants = layer.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
            && matches!(layer.layer(), Layer::Top | Layer::Overlay)
            && layer_trusted(layer.wl_surface());
        let has_it = self
            .seat
            .get_keyboard()
            .and_then(|k| k.current_focus())
            .map(|f| &f == layer.wl_surface())
            .unwrap_or(false);
        if wants && !has_it && !self.locked {
            self.focus_layer(layer);
        }
    }

    pub fn focus_layer(&mut self, layer: &LayerSurface) {
        let serial = SERIAL_COUNTER.next_serial();
        if let Some(kb) = self.seat.get_keyboard() {
            kb.set_focus(self, Some(layer.wl_surface().clone()), serial);
        }
    }

    /// The layer surface (and the point on its origin, in the shared space) under `p` on the
    /// given layers of the monitor `p` is on.
    pub fn layer_surface_under(
        &self,
        p: Point<f64, Logical>,
        layers: &[Layer],
    ) -> Option<(LayerSurface, WlSurface, Point<i32, Logical>)> {
        let output = self.output_at(p)?;
        let origin = self.output_rect(&output).loc;
        let covered = self.fullscreen_on(&output);
        let map = layer_map_for_output(&output);
        let p = p - origin.to_f64();
        for l in layers {
            if covered && *l == Layer::Top {
                continue; // a fullscreen window hides the panels
            }
            if let Some(ls) = map.layer_under(*l, p) {
                let geo = map.layer_geometry(ls)?;
                let local = p - geo.loc.to_f64();
                if let Some((s, loc)) = ls.surface_under(local, WindowSurfaceType::ALL) {
                    return Some((ls.clone(), s, origin + geo.loc + loc));
                }
            }
        }
        None
    }

    /// The area windows may use on `output`: the monitor minus what panels reserve (in the
    /// shared space).
    pub fn usable_area(&self, output: &smithay::output::Output) -> smithay::utils::Rectangle<i32, Logical> {
        let rect = self.output_rect(output);
        let zone = layer_map_for_output(output).non_exclusive_zone();
        if zone.size.w > 0 && zone.size.h > 0 {
            smithay::utils::Rectangle::new(rect.loc + zone.loc, zone.size)
        } else {
            rect
        }
    }
}

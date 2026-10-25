//! Small hit regions and an expiring availability lease for the custom tray.
//! Dock residents are removed only while a responding tray surface is available.
use crate::types::IntRect;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, Window};
use windows::Win32::Foundation::POINT;

#[derive(Clone)]
struct Surface {
    rects: Vec<IntRect>,
    ready: bool,
    open: bool,
    updated: Instant,
}
static SURFACES: LazyLock<Mutex<HashMap<String, Surface>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const LEASE: Duration = Duration::from_secs(8);

fn live(surface: &Surface) -> bool {
    surface.updated.elapsed() < LEASE
}
fn inside(rect: &IntRect, x: f64, y: f64) -> bool {
    x >= rect.x as f64
        && y >= rect.y as f64
        && x < rect.x as f64 + rect.width as f64
        && y < rect.y as f64 + rect.height as f64
}

#[tauri::command]
pub fn update_tray_surface(window: Window, rects: Vec<IntRect>, ready: bool, open: bool) {
    if !matches!(window.label(), "main" | "dock") {
        return;
    }
    let rects = rects
        .into_iter()
        .take(8)
        .filter(|r| {
            r.width > 0
                && r.height > 0
                && r.width <= 16384
                && r.height <= 16384
                && r.x.unsigned_abs() <= 32768
                && r.y.unsigned_abs() <= 32768
        })
        .collect();
    if let Ok(mut surfaces) = SURFACES.lock() {
        let changed = surfaces
            .get(window.label())
            .is_none_or(|old| old.ready != ready);
        surfaces.insert(
            window.label().to_string(),
            Surface {
                rects,
                ready,
                open,
                updated: Instant::now(),
            },
        );
        drop(surfaces);
        if changed {
            let _ = window.app_handle().emit("tray-surface-changed", ());
        }
    }
}

#[tauri::command]
pub fn tray_surface_available(app: AppHandle) -> bool {
    let surfaces = SURFACES.lock().map(|s| s.clone()).unwrap_or_default();
    surfaces.iter().any(|(label, s)| {
        s.ready
            && live(s)
            && app
                .get_webview_window(label)
                .is_some_and(|w| w.is_visible().unwrap_or(false))
    })
}

pub fn hit(label: &str, point: POINT, origin: (i32, i32), dpi: f64) -> bool {
    let Ok(surfaces) = SURFACES.try_lock() else {
        return false;
    };
    surfaces.get(label).is_some_and(|s| {
        live(s)
            && s.rects.iter().any(|r| {
                inside(
                    r,
                    (point.x - origin.0) as f64 / dpi,
                    (point.y - origin.1) as f64 / dpi,
                )
            })
    })
}

pub fn any_open() -> bool {
    SURFACES
        .try_lock()
        .is_ok_and(|s| s.values().any(|s| s.open && live(s)))
}

pub fn dismiss_outside(app: &AppHandle, point: POINT) {
    let surfaces = SURFACES.lock().map(|s| s.clone()).unwrap_or_default();
    for (label, s) in surfaces {
        if !s.open || !live(&s) {
            continue;
        }
        let within = app.get_webview_window(&label).is_some_and(|w| {
            w.inner_position()
                .is_ok_and(|p| hit(&label, point, (p.x, p.y), w.scale_factor().unwrap_or(1.0)))
        });
        if !within {
            if let Ok(mut all) = SURFACES.lock() {
                if let Some(s) = all.get_mut(&label) {
                    s.open = false;
                }
            }
            let _ = app.emit_to(label, "tray-dismiss", ());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hit_regions_do_not_cover_the_empty_desktop_between_surfaces() {
        let left = IntRect {
            x: 40,
            y: 0,
            width: 140,
            height: 36,
        };
        let right = IntRect {
            x: 1600,
            y: 500,
            width: 220,
            height: 36,
        };
        assert!(inside(&left, 50.0, 20.0));
        assert!(!inside(&left, 180.0, 20.0));
        assert!(![left, right].iter().any(|r| inside(r, 900.0, 20.0)));
    }
    #[test]
    fn an_unresponsive_surface_cannot_hide_the_last_dock_access() {
        let mut s = Surface {
            rects: vec![],
            ready: true,
            open: false,
            updated: Instant::now(),
        };
        assert!(live(&s));
        s.updated -= LEASE;
        assert!(!live(&s));
    }
}

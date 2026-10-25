//! Independent tray HWND with a native region, never dock hover click-through.
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
    if window.label() != "tray" {
        return;
    }
    let rects: Vec<IntRect> = rects
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
    let ready = ready && !rects.is_empty();
    if let Ok(mut surfaces) = SURFACES.lock() {
        let availability_changed = surfaces
            .get(window.label())
            .is_none_or(|old| old.ready != ready);
        let interaction_changed = surfaces
            .get(window.label())
            .is_none_or(|old| old.ready != ready || old.open != open);
        surfaces.insert(
            window.label().to_string(),
            Surface {
                rects: rects.clone(),
                ready,
                open,
                updated: Instant::now(),
            },
        );
        drop(surfaces);
        if interaction_changed {
            crate::diagnostics::record(
                "tray-surface",
                &format!("window={} ready={ready} open={open}", window.label()),
            );
        }
        if availability_changed {
            let _ = window.app_handle().emit("tray-surface-changed", ());
        }
    }
    // A dedicated HWND accepts input over its region at all times. Transparent
    // desktop between the bar and flyout is excluded by SetWindowRgn itself.
    crate::window_region::set_window_region(window.clone(), rects);
    sync_visibility(window.app_handle());
}

pub fn sync_visibility(app: &AppHandle) {
    use crate::state::{CURRENT_FOREGROUND_FULLSCREEN, NATIVE_TASKBAR_HIDDEN, SHUTTING_DOWN};
    use std::sync::atomic::Ordering;
    let Some(window) = app.get_webview_window("tray") else {
        return;
    };
    let has_surface = SURFACES.lock().is_ok_and(|s| {
        s.get("tray")
            .is_some_and(|s| live(s) && !s.rects.is_empty())
    });
    let visible = has_surface
        && crate::utils::dock_enabled()
        && NATIVE_TASKBAR_HIDDEN.load(Ordering::Relaxed)
        && !SHUTTING_DOWN.load(Ordering::Relaxed)
        && !CURRENT_FOREGROUND_FULLSCREEN.load(Ordering::Relaxed)
        && !crate::services::capture_ui_active();
    if !visible {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        }
        return;
    }
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let size = monitor.size();
    let height = ((360.0 * monitor.scale_factor()).ceil() as u32).min(size.height);
    let inset = crate::tray::native_right_inset(monitor.scale_factor(), size.width);
    let wanted = tauri::PhysicalSize::new(size.width - inset, height);
    let position = tauri::PhysicalPosition::new(
        monitor.position().x,
        monitor.position().y + (size.height - height) as i32,
    );
    if window.inner_size().ok() != Some(wanted) {
        let _ = window.set_size(wanted);
    }
    if window.inner_position().ok() != Some(position) {
        let _ = window.set_position(position);
    }
    if !window.is_visible().unwrap_or(false) {
        let _ = window.show();
    }
    if let Ok(hwnd) = window.hwnd() {
        // Unlike main/dock, tao's ignore-cursor flag is never enabled for tray.
        crate::utils::set_click_through(hwnd, false);
        if !crate::state::TRAY_INTERACTION_ACTIVE.load(Ordering::Relaxed) {
            crate::utils::re_assert_topmost(hwnd);
        }
    }
}

#[tauri::command]
pub fn report_tray_input(window: Window, control: String) {
    if window.label() == "tray" && matches!(control.as_str(), "toggle" | "activate" | "context") {
        crate::diagnostics::record("tray-input", &control);
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

    #[test]
    fn dedicated_hwnd_takes_tray_and_flyout_but_not_desktop_gap() {
        use windows::core::w;
        use windows::Win32::Graphics::Gdi::{
            CreateRectRgn, DeleteObject, GetWindowRgn, PtInRegion,
        };
        use windows::Win32::UI::WindowsAndMessaging::*;
        // An invisible test-owned window; no clicks or changes to installed Bloom.
        unsafe {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("Bloom region test"),
                WS_POPUP,
                -32000,
                -32000,
                600,
                400,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let regions = [
                IntRect {
                    x: 360,
                    y: 360,
                    width: 240,
                    height: 40,
                },
                IntRect {
                    x: 360,
                    y: 220,
                    width: 140,
                    height: 120,
                },
            ];
            assert!(crate::window_region::apply_region(hwnd, &regions, 1.0));
            crate::utils::set_click_through(hwnd, false);
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            assert_eq!(style & WS_EX_TRANSPARENT.0, 0);
            assert_ne!(style & WS_EX_LAYERED.0, 0);
            let region = CreateRectRgn(0, 0, 0, 0);
            GetWindowRgn(hwnd, region);
            let bar = PtInRegion(region, 380, 380).as_bool();
            let popup = PtInRegion(region, 380, 240).as_bool();
            let gap = PtInRegion(region, 380, 350).as_bool();
            let desktop = PtInRegion(region, 200, 380).as_bool();
            let _ = DeleteObject(region.into());
            let _ = DestroyWindow(hwnd);
            assert!(bar && popup);
            assert!(!gap && !desktop);
        }
    }
}

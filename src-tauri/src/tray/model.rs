//! Identity, matching and restoration rules, independent of Explorer and UIA.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{POINT, RECT};

pub(super) fn icon_inside(icon: &RECT, bounds: &RECT) -> bool {
    let width = icon.right - icon.left;
    let height = icon.bottom - icon.top;
    if !(1..=128).contains(&width) || !(1..=128).contains(&height) {
        return false;
    }
    let x = icon.left as i64 + width as i64 / 2;
    let y = icon.top as i64 + height as i64 / 2;
    x >= bounds.left as i64
        && x < bounds.right as i64
        && y >= bounds.top as i64
        && y < bounds.bottom as i64
}

/// Translate an Explorer icon back from the taskbar's temporary hidden location.
/// Coordinates already inside the visible taskbar are accepted unchanged.
pub(super) fn visible_icon_rect(icon: RECT, original: RECT, current: RECT) -> Option<RECT> {
    if icon_inside(&icon, &original) {
        return Some(icon);
    }
    if !icon_inside(&icon, &current) {
        return None;
    }
    let dx = original.left.checked_sub(current.left)?;
    let dy = original.top.checked_sub(current.top)?;
    let translated = RECT {
        left: icon.left.checked_add(dx)?,
        right: icon.right.checked_add(dx)?,
        top: icon.top.checked_add(dy)?,
        bottom: icon.bottom.checked_add(dy)?,
    };
    icon_inside(&translated, &original).then_some(translated)
}

pub(super) fn menu_inside_monitor(menu: &RECT, monitor: &RECT) -> bool {
    menu.right > menu.left
        && menu.bottom > menu.top
        && menu.left >= monitor.left
        && menu.top >= monitor.top
        && menu.right <= monitor.right
        && menu.bottom <= monitor.bottom
}

pub(super) fn snapshot_png(bytes: &[u8]) -> Option<String> {
    use base64::Engine;
    // Bound dimensions before decoding an undocumented registry value.
    if bytes.len() < 33
        || bytes.len() > 65536
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
    {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    if !(1..=128).contains(&width) || !(1..=128).contains(&height) {
        return None;
    }
    image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn normalized_path(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

pub(super) fn icon_identity(
    shell: &str,
    key: &str,
    path: &str,
    guid: &str,
    owner: isize,
    uid: u32,
) -> String {
    // A GUID identifies the icon independently of HWND/uID. For legacy icons
    // both HWND and uID matter. Tokens are transient, not saved preferences.
    let (owner, uid) = if guid.is_empty() {
        (owner, uid)
    } else {
        (0, 0)
    };
    serde_json::to_string(&(
        shell,
        key,
        normalized_path(path),
        guid.to_ascii_lowercase(),
        owner,
        uid,
    ))
    .expect("a tuple of strings and numbers is serializable")
}

pub(super) fn parse_order(bytes: &[u8]) -> Result<HashMap<String, usize>, String> {
    let (entries, remainder) = bytes.as_chunks::<8>();
    if !remainder.is_empty() {
        return Err("Windows tray order is invalid.".into());
    }
    let mut result = HashMap::new();
    for (index, bytes) in entries.iter().enumerate() {
        if result
            .insert(u64::from_le_bytes(*bytes).to_string(), index)
            .is_some()
        {
            return Err("Windows tray order contains duplicate icons.".into());
        }
    }
    Ok(result)
}

pub(super) fn normalized_label(label: &str) -> String {
    label
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn menu_y(
    anchor: i32,
    height: i32,
    top: i32,
    bottom: i32,
    dock_top: Option<i32>,
) -> i32 {
    let available_above = anchor - top;
    let desired = if available_above < height && anchor < top + (bottom - top) / 2 {
        anchor + 8 // Notch: open below the pointer rather than over the header.
    } else {
        dock_top.map_or(anchor, |bar| bar.min(anchor)) - height
    };
    desired.clamp(top, (bottom - height).max(top))
}

/// Only move a menu if it is offscreen or would cover Bloom's visible bar.
pub(super) fn menu_position(
    rect: RECT,
    area: RECT,
    bar: Option<RECT>,
    anchor: POINT,
) -> Option<(i32, i32)> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return None;
    }
    let overlaps = bar.is_some_and(|b| {
        rect.left < b.right && rect.right > b.left && rect.top < b.bottom && rect.bottom > b.top
    });
    if menu_inside_monitor(&rect, &area) && !overlaps {
        return None;
    }
    let x = if overlaps { rect.left } else { anchor.x }
        .clamp(area.left, (area.right - width).max(area.left));
    let upper_edge = bar
        .filter(|b| x < b.right && x + width > b.left)
        .map(|b| b.top - 4);
    let y = if overlaps {
        upper_edge.unwrap_or(anchor.y) - height
    } else {
        menu_y(anchor.y, height, area.top, area.bottom, upper_edge)
    }
    .clamp(area.top, (area.bottom - height).max(area.top));
    Some((x, y))
}

pub(super) struct IconLabel {
    pub path: String,
    pub names: Vec<String>,
    // Registry text is only a cross-check, never evidence that an icon is live.
    pub hint: Option<String>,
}

impl IconLabel {
    fn matches(&self, label: &str) -> bool {
        self.names
            .iter()
            .any(|name| name.len() >= 3 && label.contains(name))
            || self.hint.as_deref().is_some_and(|hint| {
                let hint = normalized_label(hint);
                hint.len() >= 3 && label.contains(&hint)
            })
    }
}

pub(super) fn resolve_overflow_button(
    icons: &[IconLabel],
    target: usize,
    labels: &[String],
) -> Result<usize, String> {
    let icon = icons
        .get(target)
        .ok_or("This tray icon is no longer available.")?;
    let labels: Vec<_> = labels.iter().map(|label| normalized_label(label)).collect();
    let shares_process = icons
        .iter()
        .filter(|other| normalized_path(&other.path) == normalized_path(&icon.path))
        .count()
        > 1;
    // A current, unique native label is stronger than an old ordinal. Exact
    // distinct tooltips also distinguish two icons owned by the same app.
    let hint = icon
        .hint
        .as_deref()
        .map(normalized_label)
        .unwrap_or_default();
    let exact_hint_unique = hint.len() >= 3
        && icons
            .iter()
            .filter(|other| {
                other
                    .hint
                    .as_deref()
                    .is_some_and(|h| normalized_label(h) == hint)
            })
            .count()
            == 1;
    let exact: Vec<_> = labels
        .iter()
        .enumerate()
        .filter(|(_, label)| exact_hint_unique && **label == hint)
        .map(|(i, _)| i)
        .collect();
    if exact.len() == 1 {
        return Ok(exact[0]);
    }
    let unique: Vec<_> = labels
        .iter()
        .enumerate()
        .filter(|(_, label)| {
            icon.matches(label)
                && !icons
                    .iter()
                    .enumerate()
                    .any(|(i, other)| i != target && other.matches(label))
        })
        .map(|(i, _)| i)
        .collect();
    if !shares_process && unique.len() == 1 {
        return Ok(unique[0]);
    }
    if labels.len() != icons.len() {
        return Err("Windows tray icons changed. Open the native tray or try again.".into());
    }
    for (button, label) in labels.iter().enumerate() {
        if label.is_empty() {
            continue;
        }
        let names: Vec<_> = icons
            .iter()
            .enumerate()
            .filter(|(_, icon)| icon.matches(label))
            .map(|(i, _)| i)
            .collect();
        let hints: Vec<_> = icons
            .iter()
            .enumerate()
            .filter(|(_, icon)| {
                icon.hint
                    .as_deref()
                    .is_some_and(|hint| normalized_label(hint) == *label)
            })
            .map(|(i, _)| i)
            .collect();
        if (names.len() == 1 && names[0] != button) || (hints.len() == 1 && hints[0] != button) {
            return Err("Windows tray icon order changed. Please try again.".into());
        }
    }
    if shares_process {
        // Distinct exact labels returned above. Never guess by ordinal here.
        return Err(
            "Windows cannot distinguish these app icons. Use the native tray to select the icon."
                .into(),
        );
    }
    Ok(target)
}

pub(super) struct OperationGuard<'a>(&'a AtomicBool);
impl<'a> OperationGuard<'a> {
    pub fn begin(busy: &'a AtomicBool) -> Result<Self, String> {
        if busy.swap(true, Ordering::SeqCst) {
            return Err("Another tray action is active. Close its menu and try again.".into());
        }
        Ok(Self(busy))
    }
}
impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Restoration {
    None,
    HideOriginal,
    HideCurrent,
    ShowCurrent,
}

pub(super) fn restoration(
    was_hidden: bool,
    hidden: bool,
    dock: bool,
    shutdown: bool,
    same_shell: bool,
) -> Restoration {
    if !was_hidden {
        Restoration::None
    } else if !hidden || !dock || shutdown {
        Restoration::ShowCurrent
    } else if same_shell {
        Restoration::HideOriginal
    } else {
        Restoration::HideCurrent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_icon_coordinates_survive_taskbar_handoff() {
        let bar = RECT {
            left: 0,
            top: 1400,
            right: 2560,
            bottom: 1440,
        };
        let icon = RECT {
            left: 2320,
            top: 1408,
            right: 2344,
            bottom: 1432,
        };
        let shifted_bar = RECT {
            left: -10000,
            top: -10000,
            right: -7440,
            bottom: -9960,
        };
        let shifted_icon = RECT {
            left: -7680,
            top: -9992,
            right: -7656,
            bottom: -9968,
        };
        assert_eq!(
            visible_icon_rect(shifted_icon, bar, shifted_bar),
            Some(icon)
        );
        assert_eq!(visible_icon_rect(icon, bar, shifted_bar), Some(icon));
        let unrelated = RECT {
            left: 100,
            top: 100,
            right: 124,
            bottom: 124,
        };
        assert_eq!(visible_icon_rect(unrelated, bar, shifted_bar), None);
    }

    #[test]
    fn visible_app_menus_keep_their_native_position() {
        let monitor = RECT {
            left: 0,
            top: 0,
            right: 2560,
            bottom: 1440,
        };
        let visible = RECT {
            left: 2200,
            top: 1000,
            right: 2520,
            bottom: 1400,
        };
        let offscreen = RECT {
            left: -10000,
            top: -10000,
            right: -9680,
            bottom: -9600,
        };
        assert!(menu_inside_monitor(&visible, &monitor));
        assert!(!menu_inside_monitor(&offscreen, &monitor));
    }

    #[test]
    fn a_hidden_icons_chevron_rectangle_does_not_identify_a_popup_button() {
        let popup = RECT {
            left: 2200,
            top: 1200,
            right: 2400,
            bottom: 1360,
        };
        let chevron = RECT {
            left: 2200,
            top: 1408,
            right: 2224,
            bottom: 1432,
        };
        assert!(!icon_inside(&chevron, &popup));
        let live = RECT {
            left: 2210,
            top: 1210,
            right: 2234,
            bottom: 1234,
        };
        assert!(icon_inside(&live, &popup));
    }

    #[test]
    fn artwork_rejects_corrupt_and_oversized_registry_values() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let mut png = bytes.into_inner();
        assert!(snapshot_png(&png)
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(snapshot_png(&png[..32]).is_none());
        png[16..20].copy_from_slice(&100000u32.to_be_bytes());
        assert!(snapshot_png(&png).is_none());
        assert!(snapshot_png(&[0; 65537]).is_none());
    }

    #[test]
    fn menus_fit_below_the_notch_and_above_the_bottom_dock_on_negative_monitors() {
        assert_eq!(menu_y(30, 240, 0, 1080, Some(1030)), 38);
        assert_eq!(menu_y(1050, 240, 0, 1080, Some(1030)), 790);
        assert_eq!(menu_y(-1060, 240, -1080, 0, Some(-50)), -1052);
        assert_eq!(menu_y(10, 2000, 0, 1080, None), 0);
    }

    #[test]
    fn menu_is_moved_only_when_offscreen_or_over_the_actual_tray() {
        let screen = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let bar = RECT {
            left: 1640,
            top: 1020,
            right: 1850,
            bottom: 1080,
        };
        let anchor = POINT { x: 1700, y: 1050 };
        let overlap = RECT {
            left: 1600,
            top: 800,
            right: 1840,
            bottom: 1040,
        };
        assert_eq!(
            menu_position(overlap, screen, Some(bar), anchor),
            Some((1600, 776))
        );
        let clear = RECT {
            left: 1200,
            top: 800,
            right: 1500,
            bottom: 1070,
        };
        assert_eq!(menu_position(clear, screen, Some(bar), anchor), None);
        let offscreen = RECT {
            left: 1800,
            top: 1000,
            right: 2100,
            bottom: 1200,
        };
        assert_eq!(
            menu_position(offscreen, screen, Some(bar), anchor),
            Some((1620, 816))
        );
    }

    fn label(path: &str, name: &str, hint: Option<&str>) -> IconLabel {
        IconLabel {
            path: path.into(),
            names: vec![normalized_label(name)],
            hint: hint.map(str::to_string),
        }
    }

    #[test]
    fn identity_distinguishes_icons_owner_and_shell_restarts() {
        let first = icon_identity("100:200:300", "1", "C:\\Apps\\app.exe", "", 7, 1);
        for other in [
            icon_identity("101:201:301", "1", "C:\\Apps\\app.exe", "", 7, 1),
            icon_identity("100:200:300", "1", "C:\\Apps\\app.exe", "", 8, 1),
            icon_identity("100:200:300", "1", "C:\\Apps\\app.exe", "", 7, 2),
            icon_identity("100:200:300", "1", "C:\\Other\\app.exe", "", 7, 1),
        ] {
            assert_ne!(first, other);
        }
    }

    #[test]
    fn guid_identity_ignores_legacy_fields_and_normalizes_paths() {
        assert_eq!(
            icon_identity("s", "k", "C:/Apps/App.exe", "GUID", 7, 1),
            icon_identity("s", "k", "c:\\apps\\app.exe", "guid", 8, 2)
        );
        assert_ne!(
            icon_identity("s", "k", "app", "guid-a", 0, 0),
            icon_identity("s", "k", "app", "guid-b", 0, 0)
        );
    }

    #[test]
    fn invalid_and_duplicate_orders_are_rejected() {
        assert!(parse_order(&[0; 7]).is_err());
        assert!(parse_order(&[0; 16]).is_err());
        let bytes = [1u64.to_le_bytes(), 5u64.to_le_bytes()].concat();
        assert_eq!(parse_order(&bytes).unwrap().get("5"), Some(&1));
    }

    #[test]
    fn reordered_native_buttons_are_not_invoked_by_old_position() {
        let icons = [label("a", "Alpha", None), label("b", "Beta", None)];
        assert_eq!(
            resolve_overflow_button(&icons, 1, &["Alpha".into(), "Beta".into()]),
            Ok(1)
        );
        assert_eq!(
            resolve_overflow_button(&icons, 1, &["Beta".into(), "Alpha".into()]),
            Ok(0)
        );
    }

    #[test]
    fn unknown_extra_icons_require_one_unambiguous_target() {
        let icons = [label("a", "Alpha", None), label("b", "Beta", None)];
        assert_eq!(
            resolve_overflow_button(&icons, 1, &["Other".into(), "Alpha".into(), "Beta".into()]),
            Ok(2)
        );
        assert!(resolve_overflow_button(
            &icons,
            1,
            &["Beta 1".into(), "Alpha".into(), "Beta 2".into()]
        )
        .is_err());
    }

    #[test]
    fn slack_icons_with_distinct_current_tooltips_survive_a_count_change() {
        let icons = [
            label("slack.exe", "Slack", Some("Slack - Workspace A")),
            label("slack.exe", "Slack", Some("Slack - Workspace B")),
        ];
        assert_eq!(
            resolve_overflow_button(
                &icons,
                1,
                &[
                    "Other app".into(),
                    "Slack - Workspace B".into(),
                    "Slack - Workspace A".into()
                ]
            ),
            Ok(1)
        );
        assert!(resolve_overflow_button(
            &icons,
            1,
            &["Other app".into(), "Slack".into(), "Slack".into()]
        )
        .is_err());
        assert!(resolve_overflow_button(
            &icons,
            1,
            &[
                "Slack - Workspace B".into(),
                "Slack - Workspace B".into(),
                "Other app".into()
            ]
        )
        .is_err());
    }

    #[test]
    fn a_distinct_native_tooltip_resolves_an_extra_icon_without_using_its_old_position() {
        let icons = [
            label("drive.exe", "GoogleDriveFS", Some("Google Drive")),
            label("onedrive.exe", "OneDrive", Some("OneDrive")),
        ];
        assert_eq!(
            resolve_overflow_button(
                &icons,
                0,
                &[
                    "Other".into(),
                    "OneDrive - Synced".into(),
                    "Google Drive".into()
                ]
            ),
            Ok(2)
        );
        assert!(resolve_overflow_button(
            &icons,
            0,
            &[
                "Google Drive".into(),
                "Google Drive".into(),
                "OneDrive".into()
            ]
        )
        .is_err());
    }

    #[test]
    fn multiple_icons_of_one_app_need_distinct_labels_agreeing_with_order() {
        let icons = [
            label("same.exe", "App", Some("Work")),
            label("same.exe", "App", Some("Personal")),
        ];
        assert_eq!(
            resolve_overflow_button(&icons, 1, &["Work".into(), "Personal".into()]),
            Ok(1)
        );
        assert_eq!(
            resolve_overflow_button(&icons, 1, &["Personal".into(), "Work".into()]),
            Ok(0)
        );
        assert!(resolve_overflow_button(&icons, 1, &["".into(), "".into()]).is_err());
        let ambiguous = [
            label("same.exe", "App", None),
            label("same.exe", "App", None),
        ];
        assert!(resolve_overflow_button(&ambiguous, 0, &["App".into(), "App".into()]).is_err());
    }

    #[test]
    fn operation_is_exclusive_and_can_retry_after_failure() {
        let busy = AtomicBool::new(false);
        {
            let _guard = OperationGuard::begin(&busy).unwrap();
            assert!(OperationGuard::begin(&busy).is_err());
            assert!(busy.load(Ordering::SeqCst));
        }
        assert!(OperationGuard::begin(&busy).is_ok());
        assert!(!busy.load(Ordering::SeqCst));
    }

    #[test]
    fn exit_and_disabled_dock_are_never_hidden_by_late_cleanup() {
        assert_eq!(
            restoration(true, true, true, false, true),
            Restoration::HideOriginal
        );
        assert_eq!(
            restoration(true, true, true, false, false),
            Restoration::HideCurrent
        );
        assert_eq!(
            restoration(true, true, false, false, true),
            Restoration::ShowCurrent
        );
        assert_eq!(
            restoration(true, true, true, true, true),
            Restoration::ShowCurrent
        );
        assert_eq!(
            restoration(true, false, true, false, true),
            Restoration::ShowCurrent
        );
        assert_eq!(
            restoration(false, false, false, false, true),
            Restoration::None
        );
    }
}

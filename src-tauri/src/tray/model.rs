//! Identity, matching and restoration rules, independent of Explorer and UIA.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

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
            .any(|name| !name.is_empty() && label.contains(name))
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
    if labels.len() != icons.len() {
        // Unknown native icons can shift every ordinal. Only a unique label
        // belonging to exactly one known icon can be used instead.
        let matches: Vec<_> = labels
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
        if !shares_process && matches.len() == 1 {
            return Ok(matches[0]);
        }
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
        let hint = icon
            .hint
            .as_deref()
            .map(normalized_label)
            .unwrap_or_default();
        let unique = !hint.is_empty()
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
        if !unique || labels[target] != hint {
            return Err("Windows cannot distinguish these app icons. Use the native tray to select the icon.".into());
        }
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
        assert!(resolve_overflow_button(&icons, 1, &["Beta".into(), "Alpha".into()]).is_err());
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
    fn multiple_icons_of_one_app_need_distinct_labels_agreeing_with_order() {
        let icons = [
            label("same.exe", "App", Some("Work")),
            label("same.exe", "App", Some("Personal")),
        ];
        assert_eq!(
            resolve_overflow_button(&icons, 1, &["Work".into(), "Personal".into()]),
            Ok(1)
        );
        assert!(resolve_overflow_button(&icons, 1, &["Personal".into(), "Work".into()]).is_err());
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

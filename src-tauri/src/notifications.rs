//! Windows notifications after the user opts in inside Bloom.
//! Without package identity, an already-allowed listener has worked on one
//! tested system, but requesting new OS access remains unverified.
use serde::Serialize;
use windows::{
    ApplicationModel::Package,
    Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
    UI::Notifications::{
        KnownNotificationBindings,
        Management::{UserNotificationListener, UserNotificationListenerAccessStatus},
        NotificationKinds, UserNotification,
    },
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    id: u32,
    app: String,
    app_id: Option<String>,
    title: String,
    body: String,
    created_ms: i64,
}

#[derive(Serialize)]
pub struct NotificationSnapshot {
    status: &'static str,
    items: Vec<NotificationItem>,
}

impl NotificationSnapshot {
    fn state(status: &'static str) -> Self {
        Self {
            status,
            items: Vec::new(),
        }
    }
}

fn item(value: UserNotification) -> windows::core::Result<NotificationItem> {
    let id = value.Id()?;
    // A broken or unavailable app identity must not hide other notifications.
    let app_info = value.AppInfo().ok();
    let app = app_info
        .as_ref()
        .and_then(|app| app.DisplayInfo().ok())
        .and_then(|display| display.DisplayName().ok())
        .map(|name| name.to_string())
        .unwrap_or_default();
    let app_id = app_info
        .and_then(|app| app.AppUserModelId().ok())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty());
    let ticks = value.CreationTime()?.UniversalTime;
    let created_ms = ticks / 10_000 - 11_644_473_600_000;
    let mut lines = Vec::new();
    if let Ok(elements) = value
        .Notification()
        .and_then(|n| n.Visual())
        .and_then(|v| v.GetBinding(&KnownNotificationBindings::ToastGeneric()?))
        .and_then(|b| b.GetTextElements())
    {
        for index in 0..elements.Size().unwrap_or(0).min(8) {
            if let Ok(text) = elements.GetAt(index).and_then(|e| e.Text()) {
                let line: String = text.to_string().chars().take(500).collect();
                if !line.trim().is_empty() {
                    lines.push(line);
                }
            }
        }
    }
    Ok(NotificationItem {
        id,
        app: app.chars().take(150).collect(),
        app_id,
        title: lines.first().cloned().unwrap_or_default(),
        body: lines
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>()
            .join(" · "),
        created_ms,
    })
}

fn blocked_state(
    status: windows::core::Result<UserNotificationListenerAccessStatus>,
    has_identity: bool,
) -> Option<&'static str> {
    match status {
        Ok(UserNotificationListenerAccessStatus::Allowed) => None,
        Ok(UserNotificationListenerAccessStatus::Unspecified) if has_identity => Some("permission"),
        Ok(UserNotificationListenerAccessStatus::Denied) if has_identity => Some("denied"),
        Ok(
            UserNotificationListenerAccessStatus::Unspecified
            | UserNotificationListenerAccessStatus::Denied,
        ) => Some("unavailable"),
        _ => Some("error"),
    }
}

fn user_opted_in(value: Option<&str>) -> bool {
    value == Some("true")
}

fn snapshot() -> NotificationSnapshot {
    let listener = match UserNotificationListener::Current() {
        Ok(listener) => listener,
        Err(_) => return NotificationSnapshot::state("unavailable"),
    };
    let has_identity = Package::Current().is_ok();
    if let Some(status) = blocked_state(listener.GetAccessStatus(), has_identity) {
        return NotificationSnapshot::state(status);
    }
    let values = match listener
        .GetNotificationsAsync(NotificationKinds::Toast)
        .and_then(|v| v.get())
    {
        Ok(values) => values,
        Err(_) => return NotificationSnapshot::state("error"),
    };
    // Check after the request too: Windows can silently return an empty list if
    // access was revoked while the query was in flight.
    if let Some(status) = blocked_state(listener.GetAccessStatus(), has_identity) {
        return NotificationSnapshot::state(status);
    }
    let mut items = Vec::new();
    let size = match values.Size() {
        Ok(size) => size,
        Err(_) => return NotificationSnapshot::state("error"),
    };
    for index in 0..size {
        if let Ok(value) = values.GetAt(index).and_then(item) {
            items.push(value);
        }
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.created_ms));
    if let Some(status) = blocked_state(listener.GetAccessStatus(), has_identity) {
        return NotificationSnapshot::state(status);
    }
    NotificationSnapshot {
        status: "allowed",
        items,
    }
}

#[tauri::command]
pub async fn notification_snapshot(app: tauri::AppHandle) -> NotificationSnapshot {
    if !user_opted_in(crate::utils::get_setting_str(&app, "bloom-notifications-enabled").as_deref())
    {
        return NotificationSnapshot::state("disabled");
    }
    tauri::async_runtime::spawn_blocking(|| {
        let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
        let result = snapshot();
        if initialized {
            unsafe { RoUninitialize() };
        }
        result
    })
    .await
    .unwrap_or_else(|_| NotificationSnapshot::state("error"))
}

#[tauri::command]
pub async fn request_notification_access(app: tauri::AppHandle) -> Result<(), String> {
    if !user_opted_in(crate::utils::get_setting_str(&app, "bloom-notifications-enabled").as_deref())
    {
        return Err("disabled".to_string());
    }
    let (send, receive) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        if Package::Current().is_err() {
            let _ = send.send(Err("unavailable".to_string()));
            return;
        }
        let operation = UserNotificationListener::Current().and_then(|l| l.RequestAccessAsync());
        match operation {
            Ok(operation) => {
                tauri::async_runtime::spawn_blocking(move || {
                    let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
                    let result = operation
                        .get()
                        .map(|_| ())
                        .map_err(|_| "permission".to_string());
                    if initialized {
                        unsafe { RoUninitialize() };
                    }
                    let _ = send.send(result);
                });
            }
            Err(_) => {
                let _ = send.send(Err("permission".to_string()));
            }
        }
    })
    .map_err(|_| "unavailable".to_string())?;
    receive.await.map_err(|_| "unavailable".to_string())?
}

fn management_listener() -> Result<UserNotificationListener, String> {
    let listener = UserNotificationListener::Current().map_err(|_| "unavailable".to_string())?;
    if let Some(status) = blocked_state(listener.GetAccessStatus(), Package::Current().is_ok()) {
        return Err(status.to_string());
    }
    Ok(listener)
}

async fn run_notification_action<F>(operation: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
        let result = operation();
        if initialized {
            unsafe { RoUninitialize() };
        }
        result
    })
    .await
    .map_err(|_| "error".to_string())?
}

fn opted_in(app: &tauri::AppHandle) -> Result<(), String> {
    if user_opted_in(crate::utils::get_setting_str(app, "bloom-notifications-enabled").as_deref()) {
        Ok(())
    } else {
        Err("disabled".to_string())
    }
}

#[tauri::command]
pub async fn remove_notifications(app: tauri::AppHandle, ids: Vec<u32>) -> Result<(), String> {
    opted_in(&app)?;
    if ids.is_empty() {
        return Ok(());
    }
    run_notification_action(move || {
        let listener = management_listener()?;
        for id in ids {
            // The list can change between the UI snapshot and this command.
            if listener.GetNotification(id).is_ok() {
                listener
                    .RemoveNotification(id)
                    .map_err(|_| "error".to_string())?;
            }
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn clear_notifications(app: tauri::AppHandle) -> Result<(), String> {
    opted_in(&app)?;
    run_notification_action(move || {
        let listener = management_listener()?;
        let notifications = listener
            .GetNotificationsAsync(NotificationKinds::Toast)
            .and_then(|operation| operation.get())
            .map_err(|_| "error".to_string())?;
        for notification in notifications {
            listener
                .RemoveNotification(notification.Id().map_err(|_| "error".to_string())?)
                .map_err(|_| "error".to_string())?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn open_notification_app(app: tauri::AppHandle, id: u32) -> Result<(), String> {
    opted_in(&app)?;
    run_notification_action(move || {
        use windows::{
            core::{HSTRING, PCWSTR},
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        };

        let listener = management_listener()?;
        let app_id = listener
            .GetNotification(id)
            .and_then(|notification| notification.AppInfo())
            .and_then(|info| info.AppUserModelId())
            .map_err(|_| "unavailable".to_string())?
            .to_string();
        if app_id.is_empty()
            || !app_id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-_!".contains(character))
        {
            return Err("unavailable".to_string());
        }
        let target = HSTRING::from(format!("shell:AppsFolder\\{app_id}"));
        let result = unsafe {
            ShellExecuteW(
                None,
                windows::core::w!("open"),
                PCWSTR(target.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as usize <= 32 {
            Err("unavailable".to_string())
        } else {
            Ok(())
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_allowed_listener_is_usable_even_without_package_identity() {
        assert_eq!(
            blocked_state(Ok(UserNotificationListenerAccessStatus::Allowed), false),
            None
        );
    }

    #[test]
    fn notifications_require_explicit_bloom_opt_in() {
        assert!(!user_opted_in(None));
        assert!(!user_opted_in(Some("false")));
        assert!(!user_opted_in(Some("invalid")));
        assert!(user_opted_in(Some("true")));
    }

    #[test]
    fn absent_identity_never_promises_an_untested_permission_prompt() {
        assert_eq!(
            blocked_state(Ok(UserNotificationListenerAccessStatus::Unspecified), false),
            Some("unavailable")
        );
        assert_eq!(
            blocked_state(Ok(UserNotificationListenerAccessStatus::Denied), false),
            Some("unavailable")
        );
        assert_eq!(
            blocked_state(Ok(UserNotificationListenerAccessStatus::Unspecified), true),
            Some("permission")
        );
        assert_eq!(
            blocked_state(Ok(UserNotificationListenerAccessStatus::Denied), true),
            Some("denied")
        );
    }
}

#[tauri::command]
pub fn open_notification_privacy_settings() {
    use windows::Win32::{
        Foundation::HWND,
        UI::{Shell::ShellExecuteA, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    unsafe {
        let _ = ShellExecuteA(
            Some(HWND(std::ptr::null_mut())),
            windows::core::PCSTR(c"open".as_ptr() as *const u8),
            windows::core::PCSTR(c"ms-settings:privacy-notifications".as_ptr() as *const u8),
            windows::core::PCSTR::null(),
            windows::core::PCSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

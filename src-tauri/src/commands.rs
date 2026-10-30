use crate::services::unregister_appbar_native;
use crate::state::*;
use crate::types::{AudioSessionInfo, IntRect, VolumeChangeEvent};
use crate::utils::*;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, Window};
use windows::core::Interface;
const MAX_SETTINGS_IMPORT_BYTES: u64 = 1024 * 1024;

/// Documentation only. No OAuth client, personal calendar or token is read.
#[tauri::command]
pub fn open_calendar_setup_guide() -> Result<(), String> {
    tauri_plugin_opener::open_url(
        "https://github.com/Asius/bloom/blob/main/GOOGLE_CALENDAR.md",
        None::<&str>,
    )
    .map_err(|_| "No se pudo abrir la guía de Google Calendar".to_string())
}

#[tauri::command]
pub fn setup_shelf_drop(app: AppHandle) {
    let first = app.clone();
    let _ = app.run_on_main_thread(move || crate::shelf::setup(&first));
    // WebView2 can create or recreate its render child after first paint;
    // re-register once it has settled so the drop target tracks the live window.
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
        let late = app.clone();
        let _ = app.run_on_main_thread(move || crate::shelf::setup(&late));
    });
}

#[tauri::command]
pub async fn shelf_thumbnail(path: String) -> crate::shelf_preview::ShelfPreview {
    tauri::async_runtime::spawn_blocking(move || crate::shelf_preview::preview(&path))
        .await
        .unwrap_or(crate::shelf_preview::ShelfPreview {
            thumb: None,
            text: None,
            folder: false,
        })
}

#[tauri::command]
pub async fn shelf_open(path: String) -> Result<(), String> {
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn shelf_prune(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect()
}

#[tauri::command]
pub fn shelf_drag_icon() -> Option<String> {
    use base64::Engine;

    let page = image::Rgba([246, 246, 246, 255]);
    let fold = image::Rgba([198, 198, 198, 255]);
    let line = image::Rgba([152, 152, 152, 255]);
    let mut img = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 0]));

    for y in 6..58u32 {
        for x in 14..50u32 {
            let in_fold = x >= 38 && y <= 18 && (x - 38) + (y - 6) >= 12;
            img.put_pixel(x, y, if in_fold { fold } else { page });
        }
    }
    for (i, y) in [22u32, 30, 38, 46].into_iter().enumerate() {
        let end = if i == 3 { 36 } else { 42 };
        for x in 20..end {
            img.put_pixel(x, y, line);
        }
    }

    let mut png_bytes: Vec<u8> = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_bytes),
        image::ImageFormat::Png,
    )
    .ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    Some(format!("data:image/png;base64,{}", b64))
}

#[tauri::command]
pub async fn update_notch_rect(rect: IntRect) {
    if let Ok(mut r) = NOTCH_RECT.lock() {
        *r = Some(rect);
    }
}

#[tauri::command]
pub fn set_window_height(window: Window, height: f64) {
    if let Ok(scale_factor) = window.scale_factor() {
        if let Ok(physical_size) = window.inner_size() {
            let logical_width = physical_size.width as f64 / scale_factor;
            let _ = window.set_size(tauri::LogicalSize::new(logical_width, height));
        }
    }
}

#[tauri::command]
pub fn resize_settings_window(app: AppHandle, width: f64, height: f64) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.set_size(tauri::LogicalSize::new(width, height));
    }
}

#[tauri::command]
pub fn set_ignore_cursor_events(window: Window, ignore: bool) {
    let _ = window.set_ignore_cursor_events(ignore);
}

#[tauri::command]
pub async fn change_notch_mode(app: AppHandle, mode: String) {
    if let Some(main_win) = app.get_webview_window("main") {
        if mode == "fixed" {
            crate::services::apply_main_appbar_mode(&app, true);
        } else {
            let _ = main_win.show();
            // Locked helper so a concurrent display/power event cannot
            // interleave between the mode decision and the AppBar call.
            crate::services::apply_main_appbar_mode(&app, false);

            let main_clone = main_win.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if !MAIN_APPBAR_REGISTERED.load(Ordering::Relaxed) {
                    if let Ok(hwnd) = main_clone.hwnd() {
                        re_assert_topmost(hwnd);
                    }
                }
            });
        }
        // Reposition window to span the full primary monitor so CSS justify-content:center works
        if let Ok(Some(monitor)) = main_win.primary_monitor() {
            let m_pos = monitor.position();
            let m_size = monitor.size();
            let scale = monitor.scale_factor();
            let bloom_scale = crate::utils::get_notch_scale(&app);
            let target_height = (480.0 * bloom_scale * scale) as u32;
            let _ = main_win.set_position(tauri::PhysicalPosition::new(m_pos.x, m_pos.y));
            let _ = main_win.set_size(tauri::PhysicalSize::new(m_size.width, target_height));
        }

        let current = CURRENT_NOTCH_OVERLAP.load(Ordering::Relaxed);
        if current != -1 {
            let _ = app.emit("notch-overlap", current == 1);
        }
    }
}

#[tauri::command]
pub async fn sync_appbar(app: AppHandle) {
    crate::services::reposition_island(&app);
}

#[tauri::command]
pub fn hide_native_osd() {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            FindWindowA, IsWindowVisible, ShowWindow, SW_HIDE,
        };
        let class1 = windows::core::PCSTR(c"NativeHWNDHost".as_ptr() as *const u8);
        if let Ok(hwnd) = FindWindowA(class1, windows::core::PCSTR::null()) {
            if IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        if let Ok(hwnd) = win.hwnd() {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{
                    SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
                };
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    if let Some(win) = app.get_webview_window("overlay") {
        let _ = win.hide();
    }
}

#[tauri::command]
pub fn set_splash_fullscreen(app: AppHandle, fullscreen: bool) {
    crate::state::OVERLAY_IN_SPLASH.store(fullscreen, Ordering::Relaxed);
    if let Some(win) = app.get_webview_window("overlay") {
        if fullscreen {
            let _ = win.hide();
            if let Ok(Some(monitor)) = win.primary_monitor() {
                let size = monitor.size();
                let pos = monitor.position();
                let _ = win.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
                let _ = win.set_size(tauri::PhysicalSize::new(size.width, size.height));
            }
            let _ = win.show();
        } else {
            let _ = win.hide();
            crate::services::sync_overlays(&app);
        }
    }
}

#[tauri::command]
pub fn sync_overlay_position(app: AppHandle) {
    crate::services::sync_overlays(&app);
}

#[tauri::command]
pub fn open_sound_settings() {
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::ShellExecuteA;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let _ = ShellExecuteA(
            Some(HWND(std::ptr::null_mut())),
            windows::core::PCSTR(c"open".as_ptr() as *const u8),
            windows::core::PCSTR(c"ms-settings:sound".as_ptr() as *const u8),
            windows::core::PCSTR::null(),
            windows::core::PCSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

#[tauri::command]
pub fn media_play_pause() {
    if let Some(sender) = COMMAND_SENDER.get() {
        let _ = sender.send(crate::types::SystemCommand::MediaPlayPause);
    }
}

#[tauri::command]
pub fn media_next() {
    if let Some(sender) = COMMAND_SENDER.get() {
        let _ = sender.send(crate::types::SystemCommand::MediaNext);
    }
}

#[tauri::command]
pub fn media_previous() {
    if let Some(sender) = COMMAND_SENDER.get() {
        let _ = sender.send(crate::types::SystemCommand::MediaPrevious);
    }
}

#[tauri::command]
pub fn media_seek(position_ms: f64) {
    if let Some(sender) = COMMAND_SENDER.get() {
        let _ = sender.send(crate::types::SystemCommand::MediaSeek(position_ms as i64));
    }
}

#[tauri::command]
pub fn open_media_source_app() {
    unsafe {
        use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
        use windows::Win32::Foundation::{HWND, LPARAM};
        use windows::Win32::System::Threading::OpenProcess;
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, FindWindowW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
            SetForegroundWindow, ShowWindow, SW_RESTORE,
        };

        let mgr = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|op| op.get())
        {
            Ok(m) => m,
            Err(_) => return,
        };
        let session = match mgr.GetCurrentSession() {
            Ok(s) => s,
            Err(_) => return,
        };
        let app_id = match session.SourceAppUserModelId() {
            Ok(id) => id.to_string(),
            Err(_) => return,
        };
        if app_id.is_empty() {
            return;
        }

        // First try: FindWindow with the AppUserModelId directly (works for UWP)
        let h_app_id = windows::core::HSTRING::from(&app_id);
        if let Ok(hwnd) = FindWindowW(None, windows::core::PCWSTR(h_app_id.as_ptr())) {
            if !hwnd.is_invalid() {
                if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                let _ = SetForegroundWindow(hwnd);
                return;
            }
        }

        // Second try: find window by process name extracted from app_id
        let process_name = app_id.split('.').next().unwrap_or(&app_id).to_lowercase();

        struct EnumData {
            process_name: String,
            found_hwnd: Option<HWND>,
        }

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
            let data = &mut *(lparam.0 as *mut EnumData);
            if !IsWindowVisible(hwnd).as_bool() {
                return windows::core::BOOL(1);
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return windows::core::BOOL(1);
            }

            if let Ok(proc) = OpenProcess(
                windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            ) {
                let mut buf = [0u16; 260];
                let mut size = buf.len() as u32;
                let ok = windows::Win32::System::Threading::QueryFullProcessImageNameW(
                    proc,
                    windows::Win32::System::Threading::PROCESS_NAME_FORMAT(0),
                    windows::core::PWSTR(buf.as_mut_ptr()),
                    &mut size,
                );
                let _ = windows::Win32::Foundation::CloseHandle(proc);
                if ok.is_ok() {
                    let path = String::from_utf16_lossy(&buf[..size as usize]);
                    let file_name = path
                        .rsplit('\\')
                        .next()
                        .unwrap_or("")
                        .to_lowercase()
                        .replace(".exe", "");
                    if file_name == data.process_name {
                        data.found_hwnd = Some(hwnd);
                        return windows::core::BOOL(0);
                    }
                }
            }
            windows::core::BOOL(1)
        }

        let mut data = EnumData {
            process_name,
            found_hwnd: None,
        };
        let callback: unsafe extern "system" fn(HWND, LPARAM) -> windows::core::BOOL =
            enum_callback;
        let _ = EnumWindows(Some(callback), LPARAM(&mut data as *mut EnumData as isize));

        if let Some(hwnd) = data.found_hwnd {
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[tauri::command]
pub fn set_volume(volume: f32) {
    if let Some(sender) = COMMAND_SENDER.get() {
        let _ = sender.send(crate::types::SystemCommand::SetVolume(volume));
    }
}

pub(crate) unsafe fn process_image_path(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    let res = QueryFullProcessImageNameW(
        handle,
        PROCESS_NAME_WIN32,
        windows::core::PWSTR(buf.as_mut_ptr()),
        &mut size,
    );
    let _ = CloseHandle(handle);
    res.ok()?;
    Some(String::from_utf16_lossy(&buf[..size as usize]))
}

pub(crate) unsafe fn friendly_process_name(path: &str) -> String {
    let cache = PROCESS_NAME_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(name) = guard.get(path) {
            return name.clone();
        }
    }
    let name = friendly_process_name_uncached(path);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(path.to_string(), name.clone());
    }
    name
}

unsafe fn friendly_process_name_uncached(path: &str) -> String {
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let filename = windows::core::PCWSTR(wide.as_ptr());

    'description: {
        let size = GetFileVersionInfoSizeW(filename, None);
        if size == 0 {
            break 'description;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(filename, None, size, data.as_mut_ptr() as *mut _).is_err() {
            break 'description;
        }

        // FileDescription lives under a language/codepage pair taken from the
        // translation table; the first entry is what Explorer shows.
        let mut trans_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut trans_len = 0u32;
        if !VerQueryValueW(
            data.as_ptr() as *const _,
            windows::core::w!("\\VarFileInfo\\Translation"),
            &mut trans_ptr,
            &mut trans_len,
        )
        .as_bool()
            || trans_len < 4
            || trans_ptr.is_null()
        {
            break 'description;
        }
        // Translation entries are (LANGID, codepage) u16 pairs. Require a full
        // four-byte pair, then read it byte-wise so the pointer can't be
        // dereferenced short or with an alignment assumption.
        let translation = std::slice::from_raw_parts(trans_ptr as *const u8, 4);
        let lang = u16::from_le_bytes([translation[0], translation[1]]);
        let codepage = u16::from_le_bytes([translation[2], translation[3]]);

        let query = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription");
        let query_wide: Vec<u16> = query.encode_utf16().chain(std::iter::once(0)).collect();
        let mut value_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut value_len = 0u32;
        if !VerQueryValueW(
            data.as_ptr() as *const _,
            windows::core::PCWSTR(query_wide.as_ptr()),
            &mut value_ptr,
            &mut value_len,
        )
        .as_bool()
            || value_len == 0
            || value_ptr.is_null()
        {
            break 'description;
        }

        let chars = std::slice::from_raw_parts(value_ptr as *const u16, value_len as usize);
        let description = String::from_utf16_lossy(chars)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        if !description.is_empty() {
            return description;
        }
    }

    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|stem| {
            let mut chars = stem.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => stem.to_string(),
            }
        })
        .unwrap_or_else(|| "Unknown app".to_string())
}

#[tauri::command]
pub async fn get_audio_sessions() -> Result<Vec<AudioSessionInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| unsafe {
        use std::collections::HashSet;
        use windows::Win32::Media::Audio::{
            AudioSessionStateActive, AudioSessionStateExpired, ISimpleAudioVolume,
        };

        let own_pid = std::process::id();
        let mut seen: HashSet<u32> = HashSet::new();
        let mut found: Vec<(AudioSessionInfo, bool)> = Vec::new();

        for_each_audio_session(|control| {
            // Only S_OK marks the system-sounds session; S_FALSE is an ordinary app.
            if control.IsSystemSoundsSession() == windows::Win32::Foundation::S_OK {
                return;
            }
            let Ok(pid) = control.GetProcessId() else {
                return;
            };
            if pid == 0 || pid == own_pid || !seen.insert(pid) {
                return;
            }
            let state = control.GetState().unwrap_or_default();
            if state == AudioSessionStateExpired {
                seen.remove(&pid);
                return;
            }
            let Ok(volume_ctl) = control.cast::<ISimpleAudioVolume>() else {
                return;
            };
            let process_path = process_image_path(pid);
            let name = match process_path.as_deref() {
                Some(path) => friendly_process_name(path),
                None => format!("Process {pid}"),
            };
            found.push((
                AudioSessionInfo {
                    pid,
                    name,
                    process_path,
                    volume: volume_ctl.GetMasterVolume().unwrap_or(1.0),
                    is_muted: volume_ctl.GetMute().map(|m| m.as_bool()).unwrap_or(false),
                },
                state == AudioSessionStateActive,
            ));
        })?;

        found.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()))
        });
        Ok(found.into_iter().map(|(info, _)| info).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_app_volume(pid: u32, volume: f32) -> Result<(), String> {
    let volume = volume.clamp(0.0, 1.0);
    tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::Media::Audio::ISimpleAudioVolume;

        for_each_audio_session(|control| {
            if control.GetProcessId().ok() != Some(pid) {
                return;
            }
            if let Ok(volume_ctl) = control.cast::<ISimpleAudioVolume>() {
                let _ = volume_ctl.SetMasterVolume(volume, std::ptr::null());
                if volume > 0.0 {
                    let _ = volume_ctl.SetMute(false, std::ptr::null());
                }
            }
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_app_mute(pid: u32, muted: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::Media::Audio::ISimpleAudioVolume;

        for_each_audio_session(|control| {
            if control.GetProcessId().ok() != Some(pid) {
                return;
            }
            if let Ok(volume_ctl) = control.cast::<ISimpleAudioVolume>() {
                let _ = volume_ctl.SetMute(muted, std::ptr::null());
            }
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn set_volume_mixer_rect(x: f64, y: f64, width: f64, height: f64) {
    if let Ok(mut rect) = VOLUME_MIXER_RECT.lock() {
        *rect = Some((x, y, width, height));
    }
}

#[tauri::command]
pub fn clear_volume_mixer_rect() {
    if let Ok(mut rect) = VOLUME_MIXER_RECT.lock() {
        *rect = None;
    }
}

pub fn release_appbars(handle: &AppHandle) {
    if let Some(w) = handle.get_webview_window("main") {
        if let Ok(hwnd) = w.hwnd() {
            unregister_appbar_native(hwnd);
        }
    }
    MAIN_APPBAR_REGISTERED.store(false, Ordering::Relaxed);
}

pub fn quit_island(handle: &AppHandle) {
    crate::diagnostics::record("shutdown", "releasing island reservation");
    SHUTTING_DOWN.store(true, Ordering::Relaxed);
    release_appbars(handle);
    crate::services::set_native_osd_suppressed(false);
    // Destroy all webview windows before exiting so Chromium's UnregisterClass for
    // Chrome_WidgetWin_0 finds no open windows (avoids the harmless Error=1412 log).
    for (_, w) in handle.webview_windows() {
        let _ = w.destroy();
    }
    handle.exit(0);
}

#[tauri::command]
pub async fn quit_bloom(handle: AppHandle) {
    quit_island(&handle);
}

#[tauri::command]
pub async fn restart_bloom(handle: AppHandle) {
    crate::diagnostics::record("restart", "releasing island reservation");
    SHUTTING_DOWN.store(true, Ordering::Relaxed);
    release_appbars(&handle);
    if let Some(w) = handle.get_webview_window("settings") {
        let _ = w.destroy();
    }
    crate::services::set_native_osd_suppressed(false);
    close_single_instance_handles();
    handle.restart();
}

fn re_register_appbars(app: &AppHandle, _settings: &HashMap<String, serde_json::Value>) {
    crate::services::reposition_island(app);
}

fn affects_surface_scale(key: &str) -> bool {
    key == "bloom-notch-scale"
}

#[tauri::command]
pub fn save_setting(app: AppHandle, key: String, value: serde_json::Value) -> Result<(), String> {
    let path = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("settings.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let settings = crate::utils::save_setting_file(&path, key.clone(), value)?;

    // Broadcast so all windows sync — emit the key as-is (bloom-prefixed)
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({ "key": &key, "value": &settings[&key] }),
    );

    if affects_surface_scale(&key) {
        re_register_appbars(&app, &settings);
    }
    Ok(())
}

#[tauri::command]
pub fn load_settings(app: AppHandle) -> Result<HashMap<String, serde_json::Value>, String> {
    let path = match app.path().app_config_dir() {
        Ok(p) => p.join("settings.json"),
        Err(_) => return Ok(HashMap::new()),
    };
    crate::utils::read_settings_file(&path)
}

#[tauri::command]
pub fn settings_file_readable(app: AppHandle) -> bool {
    app.path().app_config_dir().ok().is_some_and(|dir| {
        let path = dir.join("settings.json");
        path.exists() && crate::utils::read_settings_file(&path).is_ok()
    })
}

#[tauri::command]
pub fn get_volume() -> f32 {
    crate::state::CURRENT_VOLUME.load(std::sync::atomic::Ordering::Relaxed) as f32 / 100.0
}

#[tauri::command]
pub async fn get_volume_state() -> Result<VolumeChangeEvent, String> {
    tauri::async_runtime::spawn_blocking(|| unsafe {
        use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
        use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
        };

        let com_initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let result = (|| -> Result<VolumeChangeEvent, String> {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(
                &windows::Win32::Media::Audio::MMDeviceEnumerator,
                None,
                CLSCTX_ALL,
            )
            .map_err(|e| e.to_string())?;
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| e.to_string())?;
            let endpoint: IAudioEndpointVolume = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| e.to_string())?;
            let volume = endpoint
                .GetMasterVolumeLevelScalar()
                .map_err(|e| e.to_string())?;
            let is_muted = endpoint.GetMute().map(|m| m.as_bool()).unwrap_or(false);
            CURRENT_VOLUME.store((volume * 100.0) as u32, Ordering::Relaxed);
            Ok(VolumeChangeEvent { volume, is_muted })
        })();
        if com_initialized {
            CoUninitialize();
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_cpu_usage() -> Result<u32, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST_IDLE: AtomicU64 = AtomicU64::new(0);
    static LAST_KERNEL: AtomicU64 = AtomicU64::new(0);
    static LAST_USER: AtomicU64 = AtomicU64::new(0);

    unsafe {
        let mut idle = windows::Win32::Foundation::FILETIME::default();
        let mut kernel = windows::Win32::Foundation::FILETIME::default();
        let mut user = windows::Win32::Foundation::FILETIME::default();
        windows::Win32::System::Threading::GetSystemTimes(
            Some(&mut idle),
            Some(&mut kernel),
            Some(&mut user),
        )
        .map_err(|e| format!("GetSystemTimes failed: {e}"))?;

        let idle_u64 = (idle.dwHighDateTime as u64) << 32 | idle.dwLowDateTime as u64;
        let kernel_u64 = (kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64;
        let user_u64 = (user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64;

        let prev_idle = LAST_IDLE.swap(idle_u64, Ordering::Relaxed);
        let prev_kernel = LAST_KERNEL.swap(kernel_u64, Ordering::Relaxed);
        let prev_user = LAST_USER.swap(user_u64, Ordering::Relaxed);

        let idle_delta = idle_u64.saturating_sub(prev_idle);
        let kernel_delta = kernel_u64.saturating_sub(prev_kernel);
        let user_delta = user_u64.saturating_sub(prev_user);
        let total = kernel_delta + user_delta;

        if total == 0 {
            return Ok(0);
        }
        let usage = ((total - idle_delta) * 100) / total;
        Ok(usage as u32)
    }
}

#[tauri::command]
pub fn get_ram_usage() -> Result<f32, String> {
    unsafe {
        let mut mem = windows::Win32::System::SystemInformation::MEMORYSTATUSEX::default();
        mem.dwLength =
            std::mem::size_of::<windows::Win32::System::SystemInformation::MEMORYSTATUSEX>() as u32;
        windows::Win32::System::SystemInformation::GlobalMemoryStatusEx(&mut mem)
            .map_err(|e| format!("GlobalMemoryStatusEx failed: {e}"))?;
        Ok(mem.dwMemoryLoad as f32)
    }
}

#[tauri::command]
pub fn get_disk_space() -> Result<u64, String> {
    unsafe {
        let mut free_bytes = 0u64;
        let mut total_bytes = 0u64;
        let mut total_free = 0u64;
        windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            windows::core::w!("C:\\"),
            Some(&mut free_bytes),
            Some(&mut total_bytes),
            Some(&mut total_free),
        )
        .map_err(|e| format!("GetDiskFreeSpaceEx failed: {e}"))?;
        Ok(free_bytes / (1024 * 1024 * 1024))
    }
}

#[tauri::command]
pub fn get_network_speed() -> Result<(u64, u64), String> {
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    static LAST_SAMPLE: OnceLock<Mutex<Option<(Instant, u64, u64)>>> = OnceLock::new();

    let (mut total_in, mut total_out) = (0u64, 0u64);
    unsafe {
        // GetIfTable2 allocates the table and writes its address here.
        let mut out = std::mem::MaybeUninit::<
            *mut windows::Win32::NetworkManagement::IpHelper::MIB_IF_TABLE2,
        >::uninit();
        let ret = windows::Win32::NetworkManagement::IpHelper::GetIfTable2(out.as_mut_ptr());
        if !ret.is_ok() {
            return Err(format!("GetIfTable2 failed: {ret:?}"));
        }
        let Some(table) = std::ptr::NonNull::new(out.assume_init()) else {
            return Err("GetIfTable2 returned no table".to_string());
        };
        let table_ptr = table.as_ptr();
        // `Table` is declared with one element but holds `NumEntries` rows, so
        // read them through a pointer to the whole allocation, not the array.
        let rows = std::slice::from_raw_parts(
            std::ptr::addr_of!((*table_ptr).Table)
                .cast::<windows::Win32::NetworkManagement::IpHelper::MIB_IF_ROW2>(),
            table.as_ref().NumEntries as usize,
        );
        for row in rows {
            total_in = total_in.saturating_add(row.InOctets);
            total_out = total_out.saturating_add(row.OutOctets);
        }
        windows::Win32::NetworkManagement::IpHelper::FreeMibTable(table_ptr as *const _);
    }

    let now = Instant::now();
    let mut last = LAST_SAMPLE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|e| e.to_string())?;
    let speed = if let Some((previous, sent, recv)) = *last {
        let elapsed_ms = now.duration_since(previous).as_millis().max(1);
        let rate = |current: u64, previous: u64| {
            ((current.saturating_sub(previous) as u128 * 1000) / elapsed_ms).min(u64::MAX as u128)
                as u64
        };
        (rate(total_out, sent), rate(total_in, recv))
    } else {
        (0, 0)
    };
    *last = Some((now, total_out, total_in));
    Ok(speed)
}

#[tauri::command]
pub fn get_windows_accent_color() -> Option<String> {
    unsafe {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_BINARY,
            REG_DWORD,
        };

        // 1. Try reading AccentPalette from Explorer\Accent for the true system accent color
        let subkey =
            windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Accent");
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_READ, &mut hkey).is_ok() {
            let mut value_type = REG_BINARY;
            let mut palette = [0u8; 32];
            let mut value_size = 32u32;
            let val_name = windows::core::w!("AccentPalette");
            if RegQueryValueExW(
                hkey,
                val_name,
                None,
                Some(&mut value_type),
                Some(palette.as_mut_ptr()),
                Some(&mut value_size),
            )
            .is_ok()
            {
                let _ = RegCloseKey(hkey);
                if value_size >= 15 {
                    // bytes 12, 13, 14 are R, G, B of the active accent color
                    let r = palette[12];
                    let g = palette[13];
                    let b = palette[14];
                    return Some(format!("#{:02x}{:02x}{:02x}", r, g, b));
                }
            } else {
                let _ = RegCloseKey(hkey);
            }
        }

        // 2. Fallback to DWM\AccentColor
        let subkey = windows::core::w!("Software\\Microsoft\\Windows\\DWM");
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_READ, &mut hkey).is_ok() {
            let mut value_type = REG_DWORD;
            let mut color_val = 0u32;
            let mut value_size = std::mem::size_of::<u32>() as u32;
            let val_name = windows::core::w!("AccentColor");
            if RegQueryValueExW(
                hkey,
                val_name,
                None,
                Some(&mut value_type),
                Some(&mut color_val as *mut u32 as *mut u8),
                Some(&mut value_size),
            )
            .is_ok()
            {
                let _ = RegCloseKey(hkey);
                // color_val is AABBGGRR (ABGR format)
                let r = (color_val & 0xff) as u8;
                let g = ((color_val >> 8) & 0xff) as u8;
                let b = ((color_val >> 16) & 0xff) as u8;
                return Some(format!("#{:02x}{:02x}{:02x}", r, g, b));
            }
            let _ = RegCloseKey(hkey);
        }
    }
    None
}

#[tauri::command]
pub fn get_system_accent_color() -> Result<String, String> {
    // Try reading registry first for exact Windows accent color
    if let Some(color) = get_windows_accent_color() {
        return Ok(color);
    }

    // Fallback to DwmGetColorizationColor
    unsafe {
        let mut color = 0u32;
        let mut opaque = windows::core::BOOL(0);
        if windows::Win32::Graphics::Dwm::DwmGetColorizationColor(&mut color, &mut opaque).is_ok() {
            let r = ((color >> 16) & 0xff) as u8;
            let g = ((color >> 8) & 0xff) as u8;
            let b = (color & 0xff) as u8;
            let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
            Ok(hex)
        } else {
            Err("Failed to query colorization color".into())
        }
    }
}

#[tauri::command]
pub async fn export_settings_to_file(
    app: AppHandle,
    window: Window,
) -> Result<Option<String>, String> {
    if window.label() != "settings" {
        return Err("Settings export is only available from the Settings window".into());
    }
    let source = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("settings.json");
    if !source.exists() {
        return Err("No settings file found".into());
    }
    let content = std::fs::read_to_string(&source).map_err(|e| e.to_string())?;
    // Validate it's valid JSON before offering it for export.
    crate::utils::read_settings_file(&source)?;

    let picked = tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title("Export Bloom Settings")
            .set_file_name("bloom-settings.json")
            .add_filter("JSON", &["json"])
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(file_path) = picked else {
        return Ok(None);
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn import_settings_from_file(
    app: AppHandle,
    window: Window,
) -> Result<Option<String>, String> {
    if window.label() != "settings" {
        return Err("Settings import is only available from the Settings window".into());
    }
    let picked = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || {
            use tauri_plugin_dialog::DialogExt;
            app.dialog()
                .file()
                .set_parent(&window)
                .set_title("Import Bloom Settings")
                .add_filter("JSON", &["json"])
                .blocking_pick_file()
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(file_path) = picked else {
        return Ok(None);
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;
    let file = std::fs::File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
    let size = file
        .metadata()
        .map_err(|e| format!("Failed to inspect file: {}", e))?
        .len();
    if size > MAX_SETTINGS_IMPORT_BYTES {
        return Err("Settings file exceeds the 1 MiB import limit".into());
    }
    let content = read_settings_import(file)?;
    apply_imported_settings(&app, &content)?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

fn read_settings_import(reader: impl std::io::Read) -> Result<String, String> {
    use std::io::Read;

    // Bound the read as well as checking metadata, in case the file grows.
    let mut bytes = Vec::new();
    reader
        .take(MAX_SETTINGS_IMPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    if bytes.len() as u64 > MAX_SETTINGS_IMPORT_BYTES {
        return Err("Settings file exceeds the 1 MiB import limit".into());
    }
    String::from_utf8(bytes).map_err(|e| format!("Settings file is not valid UTF-8: {}", e))
}

fn apply_imported_settings(app: &AppHandle, settings: &str) -> Result<(), String> {
    let mut imported: HashMap<String, serde_json::Value> =
        serde_json::from_str(settings.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("Invalid JSON: {}", e))?;

    let path = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("settings.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::island_settings::normalize(&mut imported);
    let _guard = SETTINGS_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    crate::island_settings::migrate_file(&path)?;
    let previous = crate::utils::read_settings_file(&path)?;
    crate::utils::write_settings_file(&path, &imported)?;
    crate::utils::replace_settings_cache(imported.clone());
    drop(_guard);

    // Broadcast changes for every imported key so all windows re-sync
    for (key, value) in &imported {
        let _ = app.emit(
            "settings-changed",
            serde_json::json!({ "key": key, "value": value }),
        );
    }

    // Keys the imported file doesn't have must leave each window's localStorage
    // mirror too, or they come back from it on the next start. Same sentinels
    // as reset_settings are kept.
    let keep = ["bloom-first-run", "bloom-app-version"];
    for key in previous.keys().filter(|key| {
        key.starts_with("bloom-") && !keep.contains(&key.as_str()) && !imported.contains_key(*key)
    }) {
        let _ = app.emit(
            "settings-external-changed",
            serde_json::json!({ "key": key, "value": null }),
        );
    }

    re_register_appbars(app, &imported);

    Ok(())
}

#[tauri::command]
pub fn reset_settings(app: AppHandle) -> Result<(), String> {
    let path = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("settings.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Keep non-Bloom keys (the file is user-editable) and the first-run/version
    // sentinels (lifecycle markers, not preferences — keeping them avoids
    // replaying the splash after a reset). Everything else falls back to
    // defaults in both the backend and the frontends.
    let keep = ["bloom-first-run", "bloom-app-version"];
    let _guard = SETTINGS_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    crate::island_settings::migrate_file(&path)?;
    let mut settings = crate::utils::read_settings_file(&path)?;
    let removed: Vec<String> = settings
        .keys()
        .filter(|key| key.starts_with("bloom-") && !keep.contains(&key.as_str()))
        .cloned()
        .collect();
    for key in &removed {
        settings.remove(key);
    }

    crate::utils::write_settings_file(&path, &settings)?;
    crate::utils::replace_settings_cache(settings);
    drop(_guard);

    // Mirror the external-edit watcher so every open window drops the removed
    // keys from localStorage. The caller restarts Bloom right after, but this
    // keeps the reset correct even if a window lingers.
    for key in removed {
        let _ = app.emit(
            "settings-external-changed",
            serde_json::json!({ "key": key, "value": null }),
        );
    }
    Ok(())
}

pub fn setup_settings_watcher(app: AppHandle) {
    use tauri::Manager;

    let config_dir = match app.path().app_config_dir() {
        Ok(p) => p,
        Err(_) => return,
    };
    let settings_path = config_dir.join("settings.json");

    // Initialize SETTINGS_CACHE if not yet set (backup for race with init_settings_cache)
    let _ = crate::state::SETTINGS_CACHE.set(std::sync::Mutex::new(HashMap::new()));

    std::thread::spawn(move || {
        use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, ReadDirectoryChangesW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY,
            FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, OPEN_EXISTING,
        };
        use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};
        use windows::Win32::System::IO::OVERLAPPED;

        unsafe {
            let dir_path: Vec<u16> = config_dir
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let dir_handle = match CreateFileW(
                windows::core::PCWSTR(dir_path.as_ptr()),
                FILE_LIST_DIRECTORY.0,
                windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                    | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE
                    | windows::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            ) {
                Ok(h) if h != INVALID_HANDLE_VALUE => h,
                _ => return,
            };

            let h_event = match CreateEventW(None, false, false, None) {
                Ok(e) => e,
                Err(_) => {
                    let _ = CloseHandle(dir_handle);
                    return;
                }
            };

            let mut notify_buffer = [0u8; 4096];
            let mut bytes_returned = 0u32;
            let mut overlapped = OVERLAPPED {
                hEvent: h_event,
                ..Default::default()
            };

            loop {
                let success = ReadDirectoryChangesW(
                    dir_handle,
                    notify_buffer.as_mut_ptr() as *mut _,
                    notify_buffer.len() as u32,
                    false,
                    FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_FILE_NAME,
                    Some(&mut bytes_returned),
                    Some(&mut overlapped),
                    None,
                );

                if success.is_err() {
                    break;
                }

                WaitForSingleObject(h_event, INFINITE);

                std::thread::sleep(std::time::Duration::from_millis(200));

                // Keep the file read and cache comparison in the same
                // transaction as writes from the four webviews. Otherwise a
                // delayed watcher read can roll the cache back after a save.
                let changes = {
                    let Ok(_file_guard) = SETTINGS_FILE_LOCK.lock() else {
                        continue;
                    };
                    crate::utils::read_settings_file(&settings_path)
                        .ok()
                        .map(|new_settings| {
                            let mut cache = crate::state::SETTINGS_CACHE
                                .get_or_init(|| std::sync::Mutex::new(HashMap::new()))
                                .lock()
                                .unwrap();

                            let mut changed = Vec::new();
                            for (key, value) in &new_settings {
                                if cache.get(key) != Some(value) {
                                    changed.push((key.clone(), value.clone()));
                                }
                            }

                            let removed: Vec<String> = cache
                                .keys()
                                .filter(|k| !new_settings.contains_key(*k))
                                .cloned()
                                .collect();

                            *cache = new_settings.clone();
                            (changed, removed)
                        })
                };
                if let Some((changed, removed)) = changes {
                    let geometry_changed =
                        changed.iter().any(|(key, _)| affects_surface_scale(key))
                            || removed.iter().any(|key| affects_surface_scale(key));
                    // Both file and cache locks are released before emitting.
                    for (key, value) in &changed {
                        let _ = app.emit(
                            "settings-external-changed",
                            serde_json::json!({ "key": key, "value": value }),
                        );
                    }
                    for key in removed {
                        let _ = app.emit(
                            "settings-external-changed",
                            serde_json::json!({ "key": key, "value": null }),
                        );
                    }
                    if geometry_changed {
                        let settings = crate::state::SETTINGS_CACHE
                            .get()
                            .and_then(|cache| cache.lock().ok().map(|value| value.clone()))
                            .unwrap_or_default();
                        re_register_appbars(&app, &settings);
                    }
                }
            }

            let _ = CloseHandle(h_event);
            let _ = CloseHandle(dir_handle);
        }
    });
}

#[tauri::command]
pub fn get_system_transparency() -> Option<bool> {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut value = 1u32;
    let mut size = 4u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            windows::core::w!("EnableTransparency"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut std::ffi::c_void),
            Some(&mut size),
        )
        .is_ok()
        .then_some(value != 0)
    }
}

#[tauri::command]
pub async fn configure_native_material(window: Window, enabled: bool) -> Result<String, String> {
    if window.label() != "settings" {
        return Ok("unsupported-surface".into());
    }
    let allowed = enabled && get_system_transparency() != Some(false);
    let (tx, rx) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            use windows::Win32::Graphics::Dwm::{
                DwmSetWindowAttribute, DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW,
                DWMWA_SYSTEMBACKDROP_TYPE,
            };
            let result = target
                .hwnd()
                .map_err(|error| error.to_string())
                .and_then(|hwnd| {
                    let kind = if allowed {
                        DWMSBT_TRANSIENTWINDOW
                    } else {
                        DWMSBT_NONE
                    };
                    unsafe {
                        DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_SYSTEMBACKDROP_TYPE,
                            &kind as *const _ as _,
                            std::mem::size_of_val(&kind) as u32,
                        )
                    }
                    .map_err(|error| error.to_string())
                });
            let status = match result {
                Ok(()) if allowed => "accepted",
                Ok(()) => "disabled",
                Err(_) => "unavailable",
            };
            crate::diagnostics::material_status(status);
            // A successful HRESULT confirms DWM accepted the request, not the visual result.
            let _ = tx.send(status.to_string());
        })
        .map_err(|error| error.to_string())?;
    rx.await.map_err(|error| error.to_string())
}

/// Icons for the expanded audio mixer only; no launcher catalog or disk cache.
#[tauri::command]
pub async fn get_app_icon(path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
        use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON};
        use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
        let initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        let mut info = SHFILEINFOW::default();
        let found = SHGetFileInfoW(
            windows::core::PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        );
        let image = if found != 0 && !info.hIcon.is_invalid() {
            let image = icon_to_base64(info.hIcon);
            let _ = DestroyIcon(info.hIcon);
            image
        } else {
            None
        };
        if initialized {
            CoUninitialize();
        }
        image
    })
    .await
    .ok()
    .flatten()
}

unsafe fn for_each_audio_session<F: FnMut(&windows::Win32::Media::Audio::IAudioSessionControl2)>(
    mut f: F,
) -> Result<(), String> {
    use windows::Win32::Media::Audio::{
        eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
        DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    let com_initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
    let result = (|| -> Result<(), String> {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(
            &windows::Win32::Media::Audio::MMDeviceEnumerator,
            None,
            CLSCTX_ALL,
        )
        .map_err(|e| e.to_string())?;
        // Windows' volume mixer lists sessions per output device, and per-app
        // output routing (Windows 11) can put an app on a non-default device,
        // so scan every active render endpoint instead of only the default one.
        let devices = enumerator
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|e| e.to_string())?;
        let device_count = devices.GetCount().map_err(|e| e.to_string())?;
        for i in 0..device_count {
            let Ok(device) = devices.Item(i) else {
                continue;
            };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                continue;
            };
            let Ok(sessions) = manager.GetSessionEnumerator() else {
                continue;
            };
            let Ok(count) = sessions.GetCount() else {
                continue;
            };
            for j in 0..count {
                let Ok(control) = sessions.GetSession(j) else {
                    continue;
                };
                let Ok(control) = control.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                f(&control);
            }
        }
        Ok(())
    })();
    if com_initialized {
        CoUninitialize();
    }
    result
}

#[tauri::command]
pub async fn set_notch_hovered(app: AppHandle, hovered: bool) {
    NOTCH_IS_HOVERED.store(hovered, Ordering::Relaxed);
    // Re-evaluate right away. During a native drag the webview never sees the
    // pointer leave, so waiting for the next mouse move can leave the notch
    // (and its edge-hover state) stuck expanded in peek/smart mode.
    crate::services::refresh_main_interaction(&app);
}

#[tauri::command]
pub async fn set_notch_visible(visible: bool) {
    NOTCH_IS_VISIBLE.store(visible, Ordering::Relaxed);
}

#[cfg(test)]
mod settings_import_tests {
    use super::{read_settings_import, MAX_SETTINGS_IMPORT_BYTES};
    use std::io::{Cursor, Read};

    #[test]
    fn accepts_utf8_settings_at_the_size_limit() {
        let mut content = String::from("{\"bloom-weather-city\":\"Montréal\"}");
        content.extend(std::iter::repeat_n(
            ' ',
            MAX_SETTINGS_IMPORT_BYTES as usize - content.len(),
        ));
        assert_eq!(read_settings_import(content.as_bytes()).unwrap(), content);
    }

    #[test]
    fn rejects_oversized_input_without_reading_the_rest() {
        let mut reader = Cursor::new(vec![b' '; MAX_SETTINGS_IMPORT_BYTES as usize + 100]);
        let error = read_settings_import(&mut reader).unwrap_err();
        assert!(error.contains("1 MiB"));
        assert_eq!(reader.position(), MAX_SETTINGS_IMPORT_BYTES + 1);
    }

    #[test]
    fn rejects_invalid_utf8_and_read_errors() {
        assert!(read_settings_import(&[0xff][..])
            .unwrap_err()
            .contains("UTF-8"));
        struct FailedReader;
        impl Read for FailedReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("read failed"))
            }
        }
        assert!(read_settings_import(FailedReader)
            .unwrap_err()
            .contains("read failed"));
    }
}

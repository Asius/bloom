#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod diagnostics;
mod google_auth;
mod google_events;
mod island_settings;
mod legacy_taskbar;
mod services;
mod shelf;
mod shelf_preview;
mod state;
mod types;
mod updater;
mod utils;
mod window_region;
use crate::{commands::*, services::*, state::*};
use tauri::Manager;
fn main() {
    // An old watchdog can launch the upgraded binary. Never start a second UI.
    if std::env::args().nth(1).as_deref() == Some("--taskbar-watchdog") {
        return;
    }
    // Single-instance enforcement
    unsafe {
        use windows::Win32::Foundation::{CloseHandle, GetLastError};
        use windows::Win32::System::Threading::{
            CreateEventW, CreateMutexW, OpenEventW, SetEvent, SYNCHRONIZATION_ACCESS_RIGHTS,
        };

        let mutex_name: Vec<u16> = "BloomSingleInstance"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let event_name: Vec<u16> = "BloomOpenSettings"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let h_mutex = CreateMutexW(None, true, windows::core::PCWSTR(mutex_name.as_ptr())).ok();
        let err = GetLastError();

        if err.0 == 183 {
            // Another instance is already running — signal it to open settings
            if let Ok(h_event) = OpenEventW(
                SYNCHRONIZATION_ACCESS_RIGHTS(0x00100002),
                false,
                windows::core::PCWSTR(event_name.as_ptr()),
            ) {
                let _ = SetEvent(h_event);
                let _ = CloseHandle(h_event);
            }
            if let Some(h) = h_mutex {
                let _ = CloseHandle(h);
            }
            return;
        }

        if let Ok(h_event) = CreateEventW(
            None,
            false,
            false,
            windows::core::PCWSTR(event_name.as_ptr()),
        ) {
            let _ = SINGLE_INSTANCE_EVENT_HANDLE.set(h_event.0 as isize);
        }
        if let Some(h) = h_mutex {
            let _ = SINGLE_INSTANCE_MUTEX_HANDLE.set(h.0 as isize);
        }
    }

    diagnostics::initialize();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .invoke_handler(tauri::generate_handler![
            google_auth::google_calendar_connect,
            google_auth::google_calendar_connected,
            google_auth::google_calendar_disconnect,
            google_events::google_calendar_events,
            open_calendar_setup_guide,
            setup_shelf_drop,
            shelf_thumbnail,
            shelf_open,
            shelf_prune,
            shelf_drag_icon,
            update_notch_rect,
            set_notch_hovered,
            set_notch_visible,
            window_region::set_window_region,
            set_window_height,
            resize_settings_window,
            set_ignore_cursor_events,
            change_notch_mode,
            sync_appbar,
            hide_native_osd,
            open_settings_window,
            hide_overlay,
            set_splash_fullscreen,
            sync_overlay_position,
            open_sound_settings,
            media_play_pause,
            media_next,
            media_previous,
            media_seek,
            open_media_source_app,
            set_volume,
            get_audio_sessions,
            set_app_volume,
            set_app_mute,
            set_volume_mixer_rect,
            clear_volume_mixer_rect,
            quit_bloom,
            restart_bloom,
            save_setting,
            load_settings,
            settings_file_readable,
            get_volume,
            get_volume_state,
            get_cpu_usage,
            get_ram_usage,
            get_disk_space,
            get_network_speed,
            get_windows_accent_color,
            get_system_accent_color,
            export_settings_to_file,
            import_settings_from_file,
            reset_settings,
            get_system_transparency,
            configure_native_material,
            get_app_icon,
            updater::check_for_updates,
            updater::install_update,
            updater::get_update_state
        ])
        .setup(|app| {
            // Recovery is conditional on an old marker. Normal island sessions
            // never write taskbar state or start a taskbar watchdog.
            legacy_taskbar::recover(app.handle());
            utils::init_settings_cache(app.handle());
            let update_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                updater::run_startup_check(update_handle).await;
            });
            let window = app
                .get_webview_window("main")
                .ok_or("Missing island window")?;
            let _ = window.set_ignore_cursor_events(true);
            let rect_window = window.clone();
            let update_rect = move || {
                if let (Ok(position), Ok(size)) =
                    (rect_window.outer_position(), rect_window.outer_size())
                {
                    if let Ok(mut rect) = MAIN_WINDOW_RECT.lock() {
                        *rect = Some((position, size));
                    }
                }
            };
            update_rect();
            let handle = app.handle().clone();
            window.on_window_event(move |event| match event {
                tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {
                    update_rect();
                    sync_overlays(&handle);
                }
                tauri::WindowEvent::ScaleFactorChanged { .. } => {
                    reposition_island(&handle);
                }
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    quit_island(&handle);
                }
                _ => {}
            });
            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.set_ignore_cursor_events(true);
                let _ = overlay.show();
                let _ = overlay.hide();
            }
            sync_overlays(app.handle());
            watch_webview_processes(app.handle());
            setup_mouse_hook(app.handle().clone());
            setup_display_change_monitor(app.handle().clone());
            let _ = COMMAND_SENDER.set(setup_system_worker(app.handle().clone()));
            setup_audio_visualization(app.handle().clone());
            setup_settings_watcher(app.handle().clone());
            diagnostics::record("services_ready", "island, audio and settings initialized");
            if let Some(&event) = SINGLE_INSTANCE_EVENT_HANDLE.get() {
                let handle = app.handle().clone();
                std::thread::spawn(move || unsafe {
                    use windows::Win32::{
                        Foundation::{HANDLE, WAIT_OBJECT_0},
                        System::Threading::{WaitForSingleObject, INFINITE},
                    };
                    while WaitForSingleObject(HANDLE(event as *mut _), INFINITE) == WAIT_OBJECT_0 {
                        open_settings_window(handle.clone());
                    }
                });
            }
            if let Some(settings) = app.get_webview_window("settings") {
                let target = settings.clone();
                settings.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = target.hide();
                    }
                });
            }
            // A normal notification icon inside Windows' own tray, not a replacement UI.
            use tauri::{
                menu::{Menu, MenuItem},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
            };
            let settings = MenuItem::with_id(app, "settings", "Open Settings", true, None::<&str>)?;
            let diagnostics = MenuItem::with_id(
                app,
                "diagnostics",
                "Copy Bloom diagnostics",
                true,
                None::<&str>,
            )?;
            let restart = MenuItem::with_id(app, "restart", "Restart Bloom", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Bloom", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings, &diagnostics, &restart, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().ok_or("Missing icon")?.clone())
                .tooltip("Bloom Island")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "settings" => open_settings_window(app.clone()),
                    "quit" => quit_island(app),
                    "restart" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            restart_bloom(handle).await;
                        });
                    }
                    "diagnostics" => {
                        use tauri_plugin_clipboard_manager::ClipboardExt;
                        let _ = app
                            .clipboard()
                            .write_text(crate::diagnostics::support_report(app));
                    }
                    _ => {}
                })
                .on_tray_icon_event(|icon, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        open_settings_window(icon.app_handle().clone());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Could not initialize Bloom Island");
    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            SHUTTING_DOWN.store(true, std::sync::atomic::Ordering::Relaxed);
            release_appbars(app);
            set_native_osd_suppressed(false);
        }
    });
}

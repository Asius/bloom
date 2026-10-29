use base64::{engine::general_purpose, Engine as _};
use tauri::Manager;
use windows::Win32::Foundation::{HGLOBAL, HWND};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppPBGRA,
    IWICImagingFactory, WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::StructuredStorage::{CreateStreamOnHGlobal, GetHGlobalFromStream};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::WindowsAndMessaging::HICON;

pub unsafe fn icon_to_base64(hicon: HICON) -> Option<String> {
    let factory: IWICImagingFactory =
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
    let bitmap = factory.CreateBitmapFromHICON(hicon).ok()?;
    wic_bitmap_to_base64(&factory, &bitmap)
}

pub unsafe fn hbitmap_to_base64(hbitmap: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<String> {
    let factory: IWICImagingFactory =
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
    let bitmap = factory
        .CreateBitmapFromHBITMAP(
            hbitmap,
            windows::Win32::Graphics::Gdi::HPALETTE::default(),
            windows::Win32::Graphics::Imaging::WICBitmapUsePremultipliedAlpha,
        )
        .ok()?;
    wic_bitmap_to_base64(&factory, &bitmap)
}

unsafe fn wic_bitmap_to_base64(
    factory: &IWICImagingFactory,
    bitmap: &windows::Win32::Graphics::Imaging::IWICBitmapSource,
) -> Option<String> {
    let stream = CreateStreamOnHGlobal(HGLOBAL(std::ptr::null_mut()), true).ok()?;
    let encoder = factory
        .CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())
        .ok()?;
    encoder.Initialize(&stream, WICBitmapEncoderNoCache).ok()?;

    let mut frame = None;
    encoder
        .CreateNewFrame(&mut frame, std::ptr::null_mut())
        .ok()?;
    let frame = frame?;
    frame.Initialize(None).ok()?;

    let (mut width, mut height) = (0u32, 0u32);
    bitmap.GetSize(&mut width, &mut height).ok()?;
    frame.SetSize(width, height).ok()?;

    let mut format = GUID_WICPixelFormat32bppPBGRA;
    frame.SetPixelFormat(&mut format).ok()?;

    frame.WriteSource(bitmap, std::ptr::null()).ok()?;
    frame.Commit().ok()?;
    encoder.Commit().ok()?;

    let hglobal = GetHGlobalFromStream(&stream).ok()?;
    let ptr = windows::Win32::System::Memory::GlobalLock(hglobal);
    let size = windows::Win32::System::Memory::GlobalSize(hglobal);

    let data = std::slice::from_raw_parts(ptr as *const u8, size);
    let base64_str = general_purpose::STANDARD.encode(data);

    let _ = windows::Win32::System::Memory::GlobalUnlock(hglobal);

    Some(format!("data:image/png;base64,{}", base64_str))
}

pub fn get_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn read_settings_file(
    path: &std::path::Path,
) -> Result<std::collections::HashMap<String, serde_json::Value>, String> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let mut values = serde_json::from_str(content.trim_start_matches('\u{feff}'))
                .map_err(|e| format!("Invalid settings file: {e}"))?;
            crate::island_settings::normalize(&mut values);
            Ok(values)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
        Err(e) => Err(format!("Could not read settings file: {e}")),
    }
}

pub fn atomic_write_file(path: &std::path::Path, content: &[u8]) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let temp = path.with_file_name(format!(
        "{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("bloom"),
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(e) = std::fs::write(&temp, content) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("Could not write {}: {e}", temp.display()));
    }
    if let Err(e) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("Could not replace {}: {e}", path.display()));
    }
    Ok(())
}

pub fn write_settings_file(
    path: &std::path::Path,
    settings: &std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    let content = serde_json::to_vec(settings).map_err(|e| e.to_string())?;
    atomic_write_file(path, &content)
}

pub fn save_setting_file(
    path: &std::path::Path,
    key: String,
    value: serde_json::Value,
) -> Result<std::collections::HashMap<String, serde_json::Value>, String> {
    let _guard = crate::state::SETTINGS_FILE_LOCK
        .lock()
        .map_err(|e| e.to_string())?;
    crate::island_settings::migrate_file(path)?;
    if crate::island_settings::retired_key(&key) {
        return Err("This preference belongs to the retired taskbar replacement".into());
    }
    let mut settings = read_settings_file(path)?;
    settings.insert(key, value);
    write_settings_file(path, &settings)?;
    replace_settings_cache(settings.clone());
    Ok(settings)
}

pub fn init_settings_cache(app: &tauri::AppHandle) {
    use crate::state::SETTINGS_CACHE;
    let Ok(_guard) = crate::state::SETTINGS_FILE_LOCK.lock() else {
        return;
    };
    let cache =
        SETTINGS_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(path) = app
        .path()
        .app_config_dir()
        .ok()
        .map(|p| p.join("settings.json"))
    {
        if let Err(error) = crate::island_settings::migrate_file(&path) {
            crate::diagnostics::record("settings-migration", &error);
        }
        if let Ok(settings) = read_settings_file(&path) {
            if let Ok(mut cache) = cache.lock() {
                *cache = settings;
            }
        }
    }
}

pub fn replace_settings_cache(new_settings: std::collections::HashMap<String, serde_json::Value>) {
    let cache = crate::state::SETTINGS_CACHE
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(mut cache) = cache.lock() {
        *cache = new_settings;
    }
}

pub fn get_setting_str(_app: &tauri::AppHandle, key: &str) -> Option<String> {
    let cache = crate::state::SETTINGS_CACHE.get()?;
    let guard = cache.lock().ok()?;
    guard.get(key)?.as_str().map(|s| s.to_string())
}

pub fn set_click_through(hwnd: HWND, ignore: bool) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE,
    };
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let wanted = click_through_style(current, ignore);
        if wanted == current {
            return false;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted as isize);
        true
    }
}

fn click_through_style(style: u32, ignore: bool) -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::{WS_EX_LAYERED, WS_EX_TRANSPARENT};
    let layered = style | WS_EX_LAYERED.0;
    if ignore {
        layered | WS_EX_TRANSPARENT.0
    } else {
        layered & !WS_EX_TRANSPARENT.0
    }
}

pub fn re_assert_topmost(hwnd: HWND) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSENDCHANGING, SWP_NOSIZE, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW,
        };
        // Set topmost without activating or notifying the window
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOSENDCHANGING,
        );
        // Re-stamp NOACTIVATE + TOOLWINDOW — HWND_TOPMOST can cause these to be reset
        // by the shell on some Windows builds
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as usize;
        let new_ex = ex | WS_EX_NOACTIVATE.0 as usize | WS_EX_TOOLWINDOW.0 as usize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex as isize);
    }
}

pub fn get_notch_scale(_app: &tauri::AppHandle) -> f64 {
    crate::state::SETTINGS_CACHE
        .get()
        .and_then(|cache| cache.lock().ok())
        .and_then(|cache| {
            cache.get("bloom-notch-scale").and_then(|value| {
                value
                    .as_f64()
                    .or_else(|| value.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
            })
        })
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.8, 1.3))
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST_SETTINGS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    #[test]
    fn click_through_keeps_the_window_layered() {
        const LAYERED: u32 = 0x0008_0000;
        const TRANSPARENT: u32 = 0x0000_0020;
        const NOACTIVATE: u32 = 0x0800_0000;
        // tao's interactive style: neither bit. Taking clicks adds layered.
        assert_eq!(click_through_style(NOACTIVATE, false), NOACTIVATE | LAYERED);
        // Click-through adds both; other bits are kept.
        assert_eq!(
            click_through_style(NOACTIVATE, true),
            NOACTIVATE | LAYERED | TRANSPARENT
        );
        // Back to taking clicks drops only the transparent bit.
        assert_eq!(
            click_through_style(NOACTIVATE | LAYERED | TRANSPARENT, false),
            NOACTIVATE | LAYERED
        );
    }

    fn test_settings_path() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "bloom-settings-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).expect("create test directory");
        dir.join("settings.json")
    }

    #[test]
    fn settings_bom_is_read_and_a_damaged_file_is_never_overwritten() {
        let _guard = TEST_SETTINGS_LOCK.lock().unwrap();
        let path = test_settings_path();
        std::fs::write(&path, "\u{feff}{\"bloom-existing\":\"true\"}").unwrap();
        let settings = super::read_settings_file(&path).unwrap();
        assert_eq!(settings["bloom-existing"], "true");

        super::save_setting_file(&path, "bloom-new".into(), "false".into()).unwrap();
        let settings = super::read_settings_file(&path).unwrap();
        assert_eq!(settings.len(), 2);

        std::fs::write(&path, "{invalid").unwrap();
        assert!(super::save_setting_file(&path, "bloom-another".into(), "true".into()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{invalid");
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn simultaneous_setting_saves_keep_every_key() {
        let _guard = TEST_SETTINGS_LOCK.lock().unwrap();
        let path = test_settings_path();
        let workers: Vec<_> = (0..12)
            .map(|index| {
                let path = path.clone();
                std::thread::spawn(move || {
                    super::save_setting_file(
                        &path,
                        format!("bloom-key-{index}"),
                        serde_json::json!(index.to_string()),
                    )
                    .unwrap();
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(super::read_settings_file(&path).unwrap().len(), 12);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn replace_settings_cache_before_init_does_not_panic() {
        let _guard = TEST_SETTINGS_LOCK.lock().unwrap();
        use super::replace_settings_cache;
        use crate::state::SETTINGS_CACHE;

        // Mirrors a webview invoking save_setting before the setup hook has
        // initialized the cache.
        let mut settings = std::collections::HashMap::new();
        settings.insert("bloom-test".to_string(), serde_json::json!("true"));
        replace_settings_cache(settings);

        let cache = SETTINGS_CACHE.get().expect("cache initialized");
        let guard = cache.lock().expect("cache lock");
        assert_eq!(
            guard.get("bloom-test").and_then(|value| value.as_str()),
            Some("true")
        );
    }
}

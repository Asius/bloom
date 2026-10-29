//! Upgrade recovery only. The island never hides/replaces Explorer's taskbar.
//! Remove the old marker after restoring it; no writer, watchdog or shell observer.
use tauri::Manager;
use windows::{
    core::{s, BOOL, PCSTR},
    Win32::{
        Foundation::{HWND, LPARAM},
        UI::{Shell::*, WindowsAndMessaging::*},
    },
};

pub fn recover(app: &tauri::AppHandle) {
    let Ok(directory) = app.path().app_config_dir() else {
        return;
    };
    let marker = directory.join("taskbar_hidden.flag");
    let Ok(contents) = std::fs::read_to_string(&marker) else {
        return;
    };
    let state = saved_state(&contents);
    unsafe {
        let Ok(hwnd) = FindWindowA(s!("Shell_TrayWnd"), PCSTR::null()) else {
            return;
        };
        let mut data = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            ..Default::default()
        };
        if let Some(state) = state {
            data.lParam = LPARAM(state as isize);
            SHAppBarMessage(ABM_SETSTATE, &mut data);
        }
        if SHAppBarMessage(ABM_GETTASKBARPOS, &mut data) != 0
            && data.rc.right > data.rc.left
            && data.rc.bottom > data.rc.top
        {
            let _ = SetWindowPos(
                hwnd,
                None,
                data.rc.left,
                data.rc.top,
                data.rc.right - data.rc.left,
                data.rc.bottom - data.rc.top,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        unsafe extern "system" fn restore_secondary(hwnd: HWND, context: LPARAM) -> BOOL {
            let mut class = [0u8; 128];
            let len = GetClassNameA(hwnd, &mut class);
            if &class[..len as usize] == b"Shell_SecondaryTrayWnd" {
                let saved = &*(context.0 as *const Vec<(isize, windows::Win32::Foundation::RECT)>);
                if let Some((_, rect)) = saved.iter().find(|(handle, _)| *handle == hwnd.0 as isize)
                {
                    use windows::Win32::Graphics::Gdi::{MonitorFromRect, MONITOR_DEFAULTTONULL};
                    if !MonitorFromRect(rect, MONITOR_DEFAULTTONULL).is_invalid() {
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            rect.left,
                            rect.top,
                            rect.right - rect.left,
                            rect.bottom - rect.top,
                            SWP_NOACTIVATE | SWP_NOZORDER,
                        );
                    }
                }
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            BOOL(1)
        }
        let saved = saved_secondaries(&contents);
        let _ = EnumWindows(Some(restore_secondary), LPARAM(&saved as *const _ as isize));
    }
    if std::fs::remove_file(marker).is_ok() {
        crate::diagnostics::record(
            "upgrade-recovery",
            "native taskbar restored from legacy marker",
        );
    }
}

fn saved_state(contents: &str) -> Option<u32> {
    contents
        .lines()
        .find_map(|line| {
            line.strip_prefix("abstate=")
                .and_then(|value| value.parse::<u32>().ok())
        })
        .filter(|state| *state <= 3)
}

fn saved_secondaries(contents: &str) -> Vec<(isize, windows::Win32::Foundation::RECT)> {
    contents
        .lines()
        .filter_map(|line| {
            let mut values = line.strip_prefix("secondary=")?.split(',');
            let handle = values.next()?.parse().ok()?;
            let rect = windows::Win32::Foundation::RECT {
                left: values.next()?.parse().ok()?,
                top: values.next()?.parse().ok()?,
                right: values.next()?.parse().ok()?,
                bottom: values.next()?.parse().ok()?,
            };
            (values.next().is_none() && rect.right > rect.left && rect.bottom > rect.top)
                .then_some((handle, rect))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_restore_never_invents_or_accepts_invalid_taskbar_settings() {
        assert_eq!(saved_state("1"), None);
        assert_eq!(saved_state("abstate=-1"), None);
        assert_eq!(saved_state("abstate=99"), None);
        assert_eq!(saved_state("abstate=3\npid=1"), Some(3));
    }
    #[test]
    fn secondary_migration_accepts_negative_monitors_and_discards_invalid_rectangles() {
        let saved = saved_secondaries("secondary=15,-1920,1032,0,1080\nsecondary=17,0,0,-1,1\nsecondary=18,0,0,1,1,unexpected");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].0, 15);
        assert_eq!(saved[0].1.left, -1920);
    }
}

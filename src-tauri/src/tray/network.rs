//! Windows owns network status and its quick-settings surface.
use serde::Serialize;
use tauri::Window;

#[derive(Serialize)]
pub struct NetworkStatus {
    connected: bool,
    internet: bool,
    wifi: bool,
}

#[tauri::command]
pub async fn tray_network_status(window: Window) -> Result<NetworkStatus, String> {
    if window.label() != "tray" {
        return Err("Only the tray can request network status.".into());
    }
    tauri::async_runtime::spawn_blocking(|| unsafe {
        use windows::Win32::{
            Networking::NetworkListManager::{INetworkListManager, NetworkListManager},
            System::Com::{CoCreateInstance, CLSCTX_ALL},
        };
        let _com = super::ComApartment::new()?;
        let manager: INetworkListManager =
            CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let connected = manager.IsConnected().map_err(|e| e.to_string())?.as_bool();
        let internet = manager
            .IsConnectedToInternet()
            .map_err(|e| e.to_string())?
            .as_bool();
        // The default profile distinguishes Wi-Fi from a wired connection;
        // no profile is normal when the machine is offline.
        let wifi =
            windows::Networking::Connectivity::NetworkInformation::GetInternetConnectionProfile()
                .and_then(|profile| profile.IsWlanConnectionProfile())
                .unwrap_or(false);
        Ok(NetworkStatus {
            connected,
            internet,
            wifi,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn open_tray_network(window: Window, settings: bool) -> Result<(), String> {
    if window.label() != "tray" {
        return Err("Only the tray can open network controls.".into());
    }
    tauri::async_runtime::spawn_blocking(move || unsafe {
        if settings {
            use windows::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
            let result = ShellExecuteW(
                None,
                windows::core::w!("open"),
                windows::core::w!("ms-settings:network"),
                None,
                None,
                SW_SHOWNORMAL,
            );
            return if result.0 as isize > 32 {
                Ok(())
            } else {
                Err("Windows could not open network settings.".into())
            };
        }
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        // Do not turn a modifier held by the user into a different shortcut.
        for key in [VK_LWIN, VK_RWIN, VK_CONTROL, VK_SHIFT, VK_MENU] {
            if (GetAsyncKeyState(key.0 as i32) as u16 & 0x8000) != 0 {
                return Err("Release modifier keys and try opening Quick Settings again.".into());
            }
        }
        let key = |vk, flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        let inputs = [
            key(VK_LWIN, Default::default()),
            key(VK_A, Default::default()),
            key(VK_A, KEYEVENTF_KEYUP),
            key(VK_LWIN, KEYEVENTF_KEYUP),
        ];
        if SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) != inputs.len() as u32 {
            let releases = [key(VK_A, KEYEVENTF_KEYUP), key(VK_LWIN, KEYEVENTF_KEYUP)];
            SendInput(&releases, std::mem::size_of::<INPUT>() as i32);
            return Err("Windows could not open Quick Settings.".into());
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

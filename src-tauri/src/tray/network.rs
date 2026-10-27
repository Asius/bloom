//! Windows owns network status and its quick-settings surface.
use serde::Serialize;
use tauri::Window;

#[derive(Serialize)]
pub struct NetworkStatus {
    connected: bool,
    internet: bool,
    wifi: Option<bool>,
}

fn is_wifi(wlan: Option<bool>, adapter_type: Option<u32>) -> Option<bool> {
    if wlan == Some(true) {
        return Some(true);
    }
    match adapter_type {
        Some(6) => Some(false), // ethernetCsmacd
        Some(71) => Some(true), // ieee80211
        _ => None,
    }
}

fn read_status() -> Result<NetworkStatus, String> {
    unsafe {
        use windows::Win32::{
            Networking::NetworkListManager::{INetworkListManager, NetworkListManager},
            System::{
                Com::{CoCreateInstance, CLSCTX_ALL},
                WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
            },
        };
        // WinRT initialization also initializes COM for Network List Manager.
        RoInitialize(RO_INIT_MULTITHREADED).map_err(|e| e.to_string())?;
        struct Runtime;
        impl Drop for Runtime {
            fn drop(&mut self) {
                unsafe {
                    RoUninitialize();
                }
            }
        }
        let _runtime = Runtime;
        let manager: INetworkListManager =
            CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let connected = manager.IsConnected().map_err(|e| e.to_string())?.as_bool();
        let internet = manager
            .IsConnectedToInternet()
            .map_err(|e| e.to_string())?
            .as_bool();
        // A non-WLAN profile can also be a VPN, cellular or virtual adapter.
        // Only label it Ethernet when the adapter's IANA type actually says so.
        let wifi =
            windows::Networking::Connectivity::NetworkInformation::GetInternetConnectionProfile()
                .ok()
                .and_then(|profile| {
                    let wlan = profile.IsWlanConnectionProfile().ok();
                    let adapter_type = profile
                        .NetworkAdapter()
                        .and_then(|a| a.IanaInterfaceType())
                        .ok();
                    is_wifi(wlan, adapter_type)
                });
        Ok(NetworkStatus {
            connected,
            internet,
            wifi,
        })
    }
}

#[tauri::command]
pub async fn tray_network_status(window: Window) -> Result<NetworkStatus, String> {
    if window.label() != "tray" {
        return Err("Only the tray can request network status.".into());
    }
    tauri::async_runtime::spawn_blocking(read_status)
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    #[test]
    fn network_type_does_not_mislabel_vpn_or_unknown_adapters_as_ethernet() {
        assert_eq!(super::is_wifi(Some(true), None), Some(true));
        assert_eq!(super::is_wifi(Some(false), Some(6)), Some(false));
        assert_eq!(super::is_wifi(None, Some(71)), Some(true));
        assert_eq!(super::is_wifi(Some(false), Some(131)), None);
        assert_eq!(super::is_wifi(None, None), None);
    }
    #[test]
    #[ignore = "read-only Windows network API smoke test; run explicitly"]
    fn windows_reports_network_state() {
        let status = super::read_status().expect("Windows network APIs must respond");
        assert!(!status.internet || status.connected);
        println!(
            "connected={} internet={} wifi={:?}",
            status.connected, status.internet, status.wifi
        );
    }
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

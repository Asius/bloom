//! Battery readings for specific wireless receivers used by the owner.
//!
//! Protocol references: BatteryTray by qu1ncky (MIT), Barracuda Battery by
//! Mehdi Bouchami (MIT), and Halo Battery by HeyOkay (MIT). See notices.
//! All requests query status; no firmware or peripheral settings are changed.
use hidapi::{DeviceInfo, HidApi, HidDevice};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::WebviewWindow;
use windows::core::{Interface, HSTRING};
use windows::Devices::Enumeration::{DeviceInformation, DeviceInformationKind};
use windows::Foundation::IPropertyValue;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows_collections::IIterable;

static POLLING: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PeripheralReading {
    id: String,
    name: String,
    kind: String,
    connected: Option<bool>,
    percent: Option<u8>,
    approximate: bool,
    charging: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PeripheralSnapshot {
    devices: Vec<PeripheralReading>,
    bluetooth_available: bool,
}

fn unknown(id: &'static str, name: &'static str, kind: &'static str) -> PeripheralReading {
    PeripheralReading {
        id: id.into(),
        name: name.into(),
        kind: kind.into(),
        connected: None,
        percent: None,
        approximate: false,
        charging: None,
    }
}

fn valid_percent(value: u8) -> Option<u8> {
    (value <= 100).then_some(value)
}

// HID++ 2.0 response parsers return None for invalid data instead of guessing.
fn logitech_battery(feature: u16, p: &[u8]) -> Option<(Option<u8>, bool, bool)> {
    if p.len() < 3 {
        return None;
    }
    match feature {
        0x1004 => {
            let charging = matches!(p[2], 1..=4);
            if p[0] > 0 {
                return Some((valid_percent(p[0]), charging, false));
            }
            // Some Logitech devices report only a coarse level, not a percentage.
            let value = match p[1] {
                8 => Some(90),
                4 => Some(50),
                2 => Some(20),
                1 => Some(5),
                _ => None,
            };
            Some((value, charging, value.is_some()))
        }
        0x1000 => Some((
            if p[0] == 0 { None } else { valid_percent(p[0]) },
            matches!(p[2], 1..=4),
            false,
        )),
        _ => None,
    }
}

/// A request matches only the selected slot, feature and function. Hardware
/// paths and serial numbers never leave the backend or appear in logs.
fn logitech_call(
    device: &HidDevice,
    slot: u8,
    feature: u8,
    function: u8,
    args: &[u8],
) -> Option<Vec<u8>> {
    let op = (function << 4) | 0x0c;
    let mut msg = [0u8; 20];
    msg[..4].copy_from_slice(&[0x11, slot, feature, op]);
    let len = args.len().min(16);
    msg[4..4 + len].copy_from_slice(&args[..len]);
    if device.write(&msg).ok()? != msg.len() {
        return None;
    }
    let end = Instant::now() + Duration::from_millis(700);
    while Instant::now() < end {
        let mut input = [0u8; 64];
        let n = device.read_timeout(&mut input, 80).ok()?;
        if n < 7 || input[1] != slot {
            continue;
        }
        if input[2] == feature && input[3] == op {
            return Some(input[4..n].to_vec());
        }
        if (input[2] == 0xff || input[2] == 0x8f) && input[3] == feature && input[4] == op {
            return None;
        }
    }
    None
}

fn logitech_device(api: &HidApi, pid: u16, mut out: PeripheralReading) -> PeripheralReading {
    // 0xff00:0002 is the long HID++ collection. Limit access to the two
    // observed receiver models; do not enumerate other Logitech devices.
    if !api
        .device_list()
        .any(|i| i.vendor_id() == 0x046d && i.product_id() == pid)
    {
        out.connected = Some(false);
        return out;
    }
    let Some(info) = api.device_list().find(|i| {
        i.vendor_id() == 0x046d
            && i.product_id() == pid
            && i.usage_page() == 0xff00
            && i.usage() == 2
    }) else {
        return out;
    };
    let Ok(device) = info.open_device(api) else {
        return out;
    };
    // These Lightspeed receivers expose exactly one paired-device slot.
    for slot in [1] {
        // Never infer the peripheral identity solely from the receiver ID.
        let Some(name_index) =
            logitech_call(&device, slot, 0, 0, &[0, 5]).and_then(|v| v.first().copied())
        else {
            continue;
        };
        if name_index == 0 {
            continue;
        }
        let Some(name_len) =
            logitech_call(&device, slot, name_index, 0, &[]).and_then(|v| v.first().copied())
        else {
            continue;
        };
        if name_len == 0 || name_len > 48 {
            continue;
        }
        let mut name = Vec::new();
        while name.len() < name_len as usize {
            let Some(part) = logitech_call(&device, slot, name_index, 1, &[name.len() as u8])
            else {
                break;
            };
            let n = part.len().min(name_len as usize - name.len());
            if n == 0 {
                break;
            }
            name.extend_from_slice(&part[..n]);
        }
        let name = String::from_utf8_lossy(&name).to_ascii_uppercase();
        let expected = if out.id == "logitech-mouse" {
            name.contains("PRO") && !name.contains("KEYBOARD")
        } else {
            name.contains("G613")
        };
        if !expected {
            continue;
        }
        out.connected = Some(true);
        for feature in [0x1004u16, 0x1000] {
            let Some(index) = logitech_call(&device, slot, 0, 0, &feature.to_be_bytes())
                .and_then(|v| v.first().copied())
            else {
                continue;
            };
            if index == 0 {
                continue;
            }
            let Some(params) = logitech_call(
                &device,
                slot,
                index,
                if feature == 0x1004 { 1 } else { 0 },
                &[],
            ) else {
                continue;
            };
            if let Some((level, charging, approximate)) = logitech_battery(feature, &params) {
                out.connected = Some(true);
                out.percent = level;
                out.approximate = approximate;
                out.charging = Some(charging);
                return out;
            }
        }
        return out;
    }
    out
}

fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for b in data {
        crc ^= (*b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn frame_response(input: &[u8], group: u8, seq: u8) -> Option<Vec<u8>> {
    if input.len() < 2 || input[0] != 2 {
        return None;
    }
    let end = 2usize.checked_add(input[1] as usize)?;
    if end > input.len() {
        return None;
    }
    let mut pos = 2;
    while pos + 10 <= end {
        if input[pos..pos + 2] != [0x50, 0x49] {
            return None;
        }
        let size = 10 + u16::from_le_bytes([input[pos + 8], input[pos + 9]]) as usize;
        if pos + size > end {
            return None;
        }
        let frame = &input[pos..pos + size];
        if frame.len() >= 13 && frame[10] == group && frame[11] == (0x80 | seq) {
            return Some(frame.to_vec());
        }
        pos += size;
    }
    None
}

fn razer_call(device: &HidDevice, group: u8, seq: u8, body: &[u8]) -> Option<Vec<u8>> {
    let mut frame = [0u8; 64];
    if body.len() > 55 {
        return None;
    }
    frame[..5].copy_from_slice(&[2, 0x80, (4 + body.len()) as u8, 0, 0]);
    frame[5..9].copy_from_slice(&[0x50, 0x41, group, seq]);
    frame[9..9 + body.len()].copy_from_slice(body);
    let crc = crc16(&frame).to_le_bytes();
    frame[3..5].copy_from_slice(&crc);
    let mut accepted = false;
    for _ in 0..5 {
        if device.write(&frame).ok()? != frame.len() {
            return None;
        }
        let mut status = [0u8; 64];
        status[0] = 2;
        if device.get_input_report(&mut status).ok()? < 2 {
            return None;
        }
        if status[1] == b'O' {
            accepted = true;
            break;
        }
        if status[1] != b'F' {
            return None;
        }
        thread::sleep(Duration::from_millis(50));
    }
    if !accepted {
        return None;
    }
    let deadline = Instant::now() + Duration::from_millis(800);
    while Instant::now() < deadline {
        let mut data = [0u8; 64];
        let n = device.read_timeout(&mut data, 100).ok()?;
        if let Some(result) = frame_response(&data[..n], group, seq) {
            return Some(result);
        }
    }
    None
}

fn razer_percent(mv: u16) -> u8 {
    let curve = [
        3300u16, 3680, 3750, 3790, 3830, 3870, 3910, 3960, 4050, 4122, 4160,
    ];
    if mv <= curve[0] {
        return 0;
    }
    for i in 1..curve.len() {
        if mv < curve[i] {
            return ((i - 1) * 10 + ((mv - curve[i - 1]) * 10 / (curve[i] - curve[i - 1])) as usize)
                as u8;
        }
    }
    100
}

fn razer_device(api: &HidApi, mut out: PeripheralReading) -> PeripheralReading {
    if !api
        .device_list()
        .any(|i| i.vendor_id() == 0x1532 && i.product_id() == 0x0550)
    {
        out.connected = Some(false);
        return out;
    }
    let candidates: Vec<&DeviceInfo> = api
        .device_list()
        .filter(|i| i.vendor_id() == 0x1532 && i.product_id() == 0x0550 && i.usage_page() == 0xff00)
        .collect();
    for info in candidates {
        let Ok(device) = info.open_device(api) else {
            continue;
        };
        let Some(reply) = razer_call(&device, 0x0e, 1, &[2, 0xe1, 1]) else {
            continue;
        };
        if reply[12] != 0 {
            continue;
        }
        let mut samples = Vec::new();
        for seq in 2..=8 {
            if seq > 2 {
                thread::sleep(Duration::from_millis(350));
            }
            if let Some(r) = razer_call(&device, 0x06, seq, &[1, 0, 0x31]) {
                if r.len() >= 15 && r[12] == 0 {
                    let mv = u16::from_le_bytes([r[13], r[14]]);
                    if (2500..=4500).contains(&mv) {
                        samples.push(mv);
                    }
                }
            }
        }
        // Always attempt to return the dongle to local mode after a query.
        let _ = razer_call(&device, 0x0e, 9, &[2, 0xe1, 0]);
        if samples.len() < 5 {
            continue;
        }
        samples.sort_unstable();
        let low = samples[1];
        let high = samples[samples.len() - 2];
        let charging = high - low >= 12 || low >= 4180;
        out.connected = Some(true);
        // Voltage while charging cannot reliably represent the charge level.
        out.percent = if charging {
            None
        } else {
            Some(razer_percent(low))
        };
        out.approximate = !charging;
        // The voltage heuristic can exclude an unreliable percentage but
        // cannot prove charging; leave the actual charging state unknown.
        out.charging = None;
        return out;
    }
    out
}

// Read cached Windows AEP properties only; never pair, toggle a radio or read
// device addresses into logs. AEPs missing confirmed connection are omitted
// because Windows may retain their last battery reading after disconnect.
fn bluetooth_devices() -> Option<Vec<PeripheralReading>> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let result = bluetooth_snapshot().ok().flatten();
    if initialized {
        unsafe {
            CoUninitialize();
        }
    }
    result
}

fn bluetooth_snapshot() -> windows::core::Result<Option<Vec<PeripheralReading>>> {
    let keys: IIterable<HSTRING> = vec![
        HSTRING::from("System.Devices.Aep.IsConnected"),
        HSTRING::from("System.Devices.BatteryLife"),
        HSTRING::from("System.Devices.Aep.ContainerId"),
    ]
    .into();
    let filter = HSTRING::from("(System.Devices.Aep.IsPaired:=System.StructuredQueryType.Boolean#True) AND ((System.Devices.Aep.ProtocolId:=\"{bb7bb05e-5972-42b5-94fc-76eaa7084d49}\") OR (System.Devices.Aep.ProtocolId:=\"{e0cbf06c-cd8b-4647-bb8a-263b43f0f974}\"))");
    let op = DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(
        &filter,
        &keys,
        DeviceInformationKind::AssociationEndpoint,
    )?;
    let deadline = Instant::now() + Duration::from_secs(4);
    while op.Status()?.0 == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    if op.Status()?.0 != 1 {
        let _ = op.Cancel();
        return Ok(None);
    }
    let devices = op.GetResults()?;
    let mut seen = HashMap::<String, usize>::new();
    let mut results: Vec<PeripheralReading> = Vec::new();
    for device in &devices {
        if results.len() >= 24 {
            break;
        }
        let Ok(properties) = device.Properties() else {
            continue;
        };
        let value = |key: &str| {
            properties
                .Lookup(&HSTRING::from(key))
                .ok()?
                .cast::<IPropertyValue>()
                .ok()
        };
        if !value("System.Devices.Aep.IsConnected")
            .and_then(|p| p.GetBoolean().ok())
            .unwrap_or(false)
        {
            continue;
        }
        let name = device.Name().unwrap_or_default().to_string();
        if name.trim().is_empty() {
            continue;
        }
        let identity = value("System.Devices.Aep.ContainerId")
            .and_then(|p| p.GetGuid().ok())
            .map(|v| format!("{v:?}"))
            .or_else(|| device.Id().ok().map(|v| v.to_string()))
            .unwrap_or_default();
        if identity.is_empty() {
            continue;
        }
        let battery = value("System.Devices.BatteryLife")
            .and_then(|p| {
                p.GetUInt8()
                    .ok()
                    .or_else(|| p.GetUInt32().ok().and_then(|v| u8::try_from(v).ok()))
            })
            .and_then(valid_percent);
        merge_bluetooth(&mut results, &mut seen, identity, name, battery);
    }
    Ok(Some(results))
}

fn merge_bluetooth(
    results: &mut Vec<PeripheralReading>,
    seen: &mut HashMap<String, usize>,
    identity: String,
    name: String,
    percent: Option<u8>,
) {
    if let Some(index) = seen.get(&identity) {
        if results[*index].percent.is_none() && percent.is_some() {
            results[*index].percent = percent;
        }
        return;
    }
    let digest = Sha256::digest(identity.as_bytes());
    let id = digest[..12]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    seen.insert(identity, results.len());
    results.push(PeripheralReading {
        id: format!("bluetooth-{id}"),
        name: name.chars().take(120).collect(),
        kind: "other".into(),
        connected: Some(true),
        percent,
        approximate: false,
        charging: None,
    });
}

#[tauri::command]
pub(crate) async fn peripheral_batteries(
    window: WebviewWindow,
) -> Result<PeripheralSnapshot, String> {
    if window.label() != "main" {
        return Err("Acceso no disponible".into());
    }
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = POLLING.try_lock().map_err(|_| "Consulta ya en curso")?;
        let api = HidApi::new().map_err(|_| "No se pudieron consultar los dispositivos")?;
        let mut devices = vec![
            logitech_device(
                &api,
                0xc539,
                unknown("logitech-mouse", "Logitech G PRO Wireless", "mouse"),
            ),
            logitech_device(
                &api,
                0xc53d,
                unknown("logitech-keyboard", "Logitech G613", "keyboard"),
            ),
            razer_device(
                &api,
                unknown("razer-headset", "Razer Barracuda X", "headphones"),
            ),
        ];
        let bluetooth = bluetooth_devices();
        let bluetooth_available = bluetooth.is_some();
        if let Some(items) = bluetooth {
            devices.extend(items);
        }
        Ok(PeripheralSnapshot {
            devices,
            bluetooth_available,
        })
    })
    .await
    .map_err(|_| "No se pudieron consultar los dispositivos".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "reads local Windows Bluetooth state; not a hardware compatibility test"]
    fn windows_bluetooth_snapshot_completes_without_changing_devices() {
        assert!(bluetooth_devices().is_some());
    }
    #[test]
    fn bluetooth_merges_two_endpoints_and_keeps_the_available_reading() {
        let mut results = Vec::new();
        let mut seen = HashMap::new();
        merge_bluetooth(
            &mut results,
            &mut seen,
            "private-address".into(),
            "Headphones".into(),
            None,
        );
        merge_bluetooth(
            &mut results,
            &mut seen,
            "private-address".into(),
            "Headphones".into(),
            Some(42),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].percent, Some(42));
        assert!(!results[0].id.contains("private-address"));
    }
    #[test]
    fn percent_respects_reported_and_coarse_values() {
        assert_eq!(
            logitech_battery(0x1000, &[42, 20, 0]),
            Some((Some(42), false, false))
        );
        assert_eq!(
            logitech_battery(0x1004, &[0, 2, 0]),
            Some((Some(20), false, true))
        );
        assert_eq!(
            logitech_battery(0x1004, &[110, 0, 0]),
            Some((None, false, false))
        );
    }
    #[test]
    fn razer_frames_are_bounded_and_match_the_request() {
        let frame = [2, 13, 0x50, 0x49, 1, 0, 0, 0, 0, 0, 3, 0, 6, 0x82, 0];
        assert!(frame_response(&frame, 6, 2).is_some());
        assert!(frame_response(&frame, 6, 3).is_none());
        assert!(frame_response(&frame[..12], 6, 2).is_none());
        assert_eq!(razer_percent(3960), 70);
    }
}

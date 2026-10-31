//! Read-only access to G HUB's local, undocumented device service.
use crate::peripherals::PeripheralReading;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use tauri::WebviewWindow;
use tungstenite::{client::IntoClientRequest, Message};

#[tauri::command]
pub(crate) async fn ghub_batteries(
    window: WebviewWindow,
) -> Result<Option<Vec<PeripheralReading>>, String> {
    if window.label() != "main" {
        return Err("Acceso no disponible".into());
    }
    tauri::async_runtime::spawn_blocking(snapshot)
        .await
        .map_err(|_| "No se pudieron consultar los dispositivos".into())
}

fn read_response(socket: &mut tungstenite::WebSocket<TcpStream>, path: &str) -> Option<Value> {
    let deadline = Instant::now() + Duration::from_secs(2);
    for _ in 0..32 {
        if Instant::now() >= deadline {
            return None;
        }
        let Message::Text(text) = socket.read().ok()? else {
            continue;
        };
        let response: Value = serde_json::from_str(&text).ok()?;
        if response.get("path").and_then(Value::as_str) == Some(path) {
            return Some(response);
        }
    }
    None
}

fn request(socket: &mut tungstenite::WebSocket<TcpStream>, path: &str) -> Option<Value> {
    socket
        .send(Message::Text(
            json!({"msgId":"bloom-readonly", "verb":"GET", "path":path})
                .to_string()
                .into(),
        ))
        .ok()?;
    read_response(socket, path)
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

pub(crate) fn snapshot() -> Option<Vec<PeripheralReading>> {
    let address = SocketAddr::from(([127, 0, 0, 1], 9010));
    let stream = TcpStream::connect_timeout(&address, Duration::from_millis(400)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .ok()?;
    let mut req = "ws://127.0.0.1:9010".into_client_request().ok()?;
    req.headers_mut()
        .insert("Sec-WebSocket-Protocol", "json".parse().ok()?);
    let mut config = tungstenite::protocol::WebSocketConfig::default();
    config.max_message_size = Some(1024 * 1024);
    config.max_frame_size = Some(1024 * 1024);
    let (mut socket, _) =
        tungstenite::client::client_with_config(req, stream, Some(config)).ok()?;
    let list = request(&mut socket, "/devices/list")?;
    let entries = list.get("payload")?.get("deviceInfos")?.as_array()?;
    let mut devices = Vec::new();
    let battery_deadline = Instant::now() + Duration::from_secs(4);
    for item in entries.iter().take(32) {
        if item.get("connectionType").and_then(Value::as_str) != Some("WIRELESS")
            || item
                .pointer("/capabilities/hasBatteryStatus")
                .and_then(Value::as_bool)
                != Some(true)
        {
            continue;
        }
        let Some(id) = item
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| safe_id(id))
        else {
            continue;
        };
        let name = item
            .get("extendedDisplayName")
            .or_else(|| item.get("deviceModel"))
            .and_then(Value::as_str)
            .unwrap_or("Dispositivo Logitech")
            .chars()
            .take(120)
            .collect::<String>();
        let state = item.get("state").and_then(Value::as_str);
        let connected = match state {
            Some("ACTIVE") => Some(true),
            Some("NOT_CONNECTED") => Some(false),
            _ => None,
        };
        let digest = Sha256::digest(
            format!(
                "{}:{}",
                item.get("deviceModel")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                name
            )
            .as_bytes(),
        );
        let stable = digest[..12]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let kind = match item.get("deviceType").and_then(Value::as_str) {
            Some("MOUSE") => "mouse",
            Some("KEYBOARD") => "keyboard",
            Some("HEADSET") => "headphones",
            _ => "other",
        };
        let battery = if connected == Some(true) && Instant::now() < battery_deadline {
            request(&mut socket, &format!("/battery/{id}/state"))
        } else {
            None
        };
        let payload = battery.as_ref().and_then(|v| v.get("payload"));
        let percent = payload
            .and_then(|p| p.get("percentage"))
            .and_then(Value::as_u64)
            .filter(|n| *n <= 100)
            .map(|n| n as u8);
        let charging = payload
            .and_then(|p| p.get("charging"))
            .and_then(Value::as_bool);
        devices.push(PeripheralReading {
            id: format!("ghub-{stable}"),
            name,
            kind: kind.into(),
            connected,
            percent,
            approximate: false,
            charging,
        });
    }
    Some(devices)
}

#[cfg(test)]
mod tests {
    use super::safe_id;
    #[test]
    fn only_safe_local_device_paths_are_accepted() {
        assert!(safe_id("abc-123_4"));
        assert!(!safe_id("../state"));
        assert!(!safe_id(""));
    }
    #[test]
    #[ignore = "requires a running local Logitech G HUB"]
    fn local_ghub_snapshot_is_readable() {
        let devices = super::snapshot().expect("G HUB did not provide an inventory");
        assert!(
            !devices.is_empty(),
            "G HUB returned no supported wireless devices"
        );
        assert!(
            devices
                .iter()
                .any(|d| d.connected == Some(true) && d.percent.is_some()),
            "G HUB returned no active battery readings"
        );
    }
}

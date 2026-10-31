//! Google OAuth for a desktop client. Tokens never cross the Tauri boundary.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, Window};

const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events.readonly";
static LOGIN: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
struct FileConfig {
    installed: Client,
}
#[derive(Deserialize, Serialize)]
struct Client {
    client_id: String,
    client_secret: String,
}
#[derive(Deserialize, Serialize)]
struct TokenResponse {
    refresh_token: Option<String>,
    access_token: String,
    scope: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Stored {
    client: Client,
    refresh_token: String,
}

fn storage(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|p| p.join("google-calendar.dat"))
        .map_err(|_| "No se pudo acceder al almacenamiento local".into())
}

fn random_string() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| "No se pudo crear una clave segura")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn secure_http_client() -> Result<reqwest::blocking::Client, String> {
    // The updater uses rustls-no-provider; login can run before the updater.
    // Keep an already-installed provider and explicitly install ring otherwise.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "No se pudo preparar la conexión segura".into())
}

fn client_from_json(data: &[u8]) -> Result<Client, String> {
    let config: FileConfig = serde_json::from_slice(data)
        .map_err(|_| "El archivo no es un cliente OAuth de escritorio válido")?;
    if !config
        .installed
        .client_id
        .ends_with(".apps.googleusercontent.com")
        || config.installed.client_id.len() > 300
        || config.installed.client_secret.is_empty()
        || config.installed.client_secret.len() > 300
    {
        return Err("El cliente OAuth de escritorio está incompleto".into());
    }
    Ok(config.installed)
}

fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB},
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            windows::core::PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| "Windows no pudo proteger la cuenta")?;
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
        Ok(result)
    }
}

fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::{
            CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| "Windows no pudo recuperar la conexión protegida")?;
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
        Ok(result)
    }
}

fn parse_callback(request: &str, expected: &str) -> Option<Result<String, String>> {
    let mut parts = request.lines().next()?.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let path = parts.next()?;
    if !path.starts_with("/oauth/callback?") {
        return None;
    }
    let url = url::Url::parse(&format!("http://127.0.0.1{path}")).ok()?;
    let pairs: Vec<_> = url.query_pairs().collect();
    let values = |key: &str| {
        pairs
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.to_string())
            .collect::<Vec<_>>()
    };
    let states = values("state");
    if states.len() != 1 || states[0] != expected {
        return None;
    }
    let codes = values("code");
    if codes.len() == 1 && !codes[0].is_empty() && codes[0].len() <= 4096 {
        return Some(Ok(codes[0].clone()));
    }
    if !values("error").is_empty() {
        return Some(Err("Se canceló o rechazó la autorización".into()));
    }
    Some(Err("Respuesta de autorización inválida".into()))
}

fn authorize(app: AppHandle, window: Window) -> Result<bool, String> {
    let _guard = LOGIN
        .try_lock()
        .map_err(|_| "Ya hay una conexión en curso")?;
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Selecciona el cliente OAuth de escritorio de Google")
        .add_filter("JSON", &["json"])
        .blocking_pick_file();
    let Some(picked) = picked else {
        return Ok(false);
    };
    let path = picked.into_path().map_err(|_| "Archivo inválido")?;
    let file = std::fs::File::open(path).map_err(|_| "No se pudo abrir el archivo")?;
    let mut data = Vec::new();
    file.take(65537)
        .read_to_end(&mut data)
        .map_err(|_| "No se pudo leer el archivo")?;
    if data.len() > 65536 {
        return Err("El archivo de cliente es demasiado grande".into());
    }
    let client = client_from_json(&data)?;
    let verifier = random_string()?;
    let state = random_string()?;
    let challenge = pkce_challenge(&verifier);
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|_| "No se pudo abrir el retorno local")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "No se pudo preparar el retorno local")?;
    let redirect = format!(
        "http://127.0.0.1:{}/oauth/callback",
        listener
            .local_addr()
            .map_err(|_| "Puerto local no disponible")?
            .port()
    );
    let mut auth = url::Url::parse(AUTH).map_err(|_| "Error de configuración")?;
    auth.query_pairs_mut()
        .append_pair("client_id", &client.client_id)
        .append_pair("redirect_uri", &redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    tauri_plugin_opener::open_url(auth.as_str(), None::<&str>)
        .map_err(|_| "No se pudo abrir el navegador")?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let code = loop {
        if Instant::now() >= deadline {
            return Err("Tiempo de autorización agotado. Inténtalo de nuevo.".into());
        }
        match listener.accept() {
            Ok((mut stream, peer)) if peer.ip().is_loopback() => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                while request.len() < 8192 {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            request.extend_from_slice(&buffer[..n]);
                            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                    }
                }
                let reply = parse_callback(&String::from_utf8_lossy(&request), &state);
                let message = if reply.as_ref().is_some_and(|r| r.is_ok()) {
                    "Autorizacion recibida. Puedes volver a Bloom."
                } else {
                    "Respuesta no valida o autorizacion cancelada."
                };
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", message.len(), message);
                if let Some(result) = reply {
                    break result?;
                }
            }
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(_) => return Err("Falló el retorno local".into()),
        }
    };
    let http = secure_http_client()?;
    let response = http
        .post(TOKEN)
        .form(&[
            ("code", code.as_str()),
            ("client_id", client.client_id.as_str()),
            ("client_secret", client.client_secret.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .map_err(|_| "No se pudo conectar con Google")?;
    if !response.status().is_success() {
        return Err(
            "Google no aceptó la autorización. Revisa la configuración y vuelve a intentarlo."
                .into(),
        );
    }
    let mut body = Vec::new();
    response
        .take(65537)
        .read_to_end(&mut body)
        .map_err(|_| "No se pudo leer la respuesta de Google")?;
    if body.len() > 65536 {
        return Err("La respuesta de Google excede el límite permitido".into());
    }
    let token: TokenResponse =
        serde_json::from_slice(&body).map_err(|_| "Google devolvió una respuesta inválida")?;
    if token.access_token.is_empty()
        || token
            .scope
            .as_ref()
            .is_some_and(|scope| !scope.split_whitespace().any(|item| item == SCOPE))
    {
        return Err("Google no concedió el permiso de lectura solicitado".into());
    }
    let refresh_token = token
        .refresh_token
        .filter(|s| !s.is_empty())
        .ok_or("Google no entregó autorización para mantener la conexión")?;
    let encrypted = protect(
        &serde_json::to_vec(&Stored {
            client,
            refresh_token,
        })
        .map_err(|_| "No se pudo guardar la conexión")?,
    )?;
    let path = storage(&app)?;
    std::fs::create_dir_all(path.parent().ok_or("Ruta inválida")?)
        .map_err(|_| "No se pudo preparar el almacenamiento")?;
    // Write a temporary file before replacing the old connection; never persist plaintext.
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, encrypted).map_err(|_| "No se pudo guardar la conexión protegida")?;
    if std::fs::rename(&temp, &path).is_err() {
        let _ = std::fs::remove_file(&temp);
        return Err("No se pudo completar el guardado de la conexión".into());
    }
    Ok(true)
}

#[tauri::command]
pub async fn google_calendar_connect(app: AppHandle, window: Window) -> Result<bool, String> {
    if window.label() != "main" {
        return Err("Acción no disponible en esta ventana".into());
    }
    if storage(&app)?.exists() {
        return Err("Ya existe una conexión; desconéctala antes de conectar otra cuenta".into());
    }
    let notify = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || authorize(app, window))
        .await
        .map_err(|_| "No se pudo iniciar la conexión")?;
    if matches!(result, Ok(true)) {
        let _ = notify.emit("google-calendar-state", ());
    }
    result
}

#[tauri::command]
pub fn google_calendar_connected(app: AppHandle, window: Window) -> Result<bool, String> {
    if window.label() != "main" {
        return Err("Acción no disponible en esta ventana".into());
    }
    let path = storage(&app)?;
    if !path.exists() {
        return Ok(false);
    }
    let file = std::fs::File::open(path).map_err(|_| "No se pudo leer la conexión protegida")?;
    let mut encrypted = Vec::new();
    file.take(65537)
        .read_to_end(&mut encrypted)
        .map_err(|_| "No se pudo leer la conexión protegida")?;
    if encrypted.len() > 65536 {
        return Err("La conexión guardada es inválida".into());
    }
    let plaintext = unprotect(&encrypted)?;
    let stored: Stored =
        serde_json::from_slice(&plaintext).map_err(|_| "La conexión guardada es inválida")?;
    Ok(!stored.refresh_token.is_empty())
}

#[tauri::command]
pub fn google_calendar_disconnect(app: AppHandle, window: Window) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Acción no disponible en esta ventana".into());
    }
    let path = storage(&app)?;
    let temp = path.with_extension("tmp");
    if path.exists() {
        std::fs::remove_file(path).map_err(|_| "No se pudo eliminar la conexión local")?;
    }
    if temp.exists() {
        std::fs::remove_file(temp)
            .map_err(|_| "No se pudo eliminar la copia temporal protegida")?;
    }
    let _ = app.emit("google-calendar-state", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_requires_correct_state_and_exact_path() {
        assert!(
            parse_callback("GET /oauth/callback?code=abc&state=wrong HTTP/1.1", "right").is_none()
        );
        assert!(parse_callback("GET /other?code=abc&state=right HTTP/1.1", "right").is_none());
        assert_eq!(
            parse_callback("GET /oauth/callback?code=abc&state=right HTTP/1.1", "right")
                .unwrap()
                .unwrap(),
            "abc"
        );
        assert!(parse_callback(
            "GET /oauth/callback?code=abc&state=right&state=right HTTP/1.1",
            "right"
        )
        .is_none());
        assert!(parse_callback(
            "GET /oauth/callback?error=access_denied&state=right HTTP/1.1",
            "right"
        )
        .unwrap()
        .is_err());
        assert!(parse_callback(
            "GET /oauth/callback?code=a&code=b&state=right HTTP/1.1",
            "right"
        )
        .unwrap()
        .is_err());
    }
    #[test]
    fn pkce_matches_rfc_7636_example() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn refuses_non_desktop_clients() {
        assert!(client_from_json(br#"{"web":{"client_id":"x","client_secret":"y"}}"#).is_err());
    }
    #[test]
    fn windows_protection_round_trip() {
        let protected = protect(b"example token").unwrap();
        assert!(!protected
            .windows(13)
            .any(|window| window == b"example token"));
        assert_eq!(unprotect(&protected).unwrap(), b"example token");
        let mut damaged = protected;
        let last = damaged.len() - 1;
        damaged[last] ^= 0xff;
        assert!(unprotect(&damaged).is_err());
    }
    #[test]
    fn secure_http_client_initializes() {
        assert!(secure_http_client().is_ok());
    }
}

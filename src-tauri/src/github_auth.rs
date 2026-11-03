//! GitHub App Device Flow, DPAPI-protected credentials and repository selection.
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, Window};
use tauri_plugin_updater::UpdaterExt;

const CLIENT_ID: &str = "Iv23ctoA6xuYRnqst4Nd";
const ACCOUNT_FILE: &str = "github-account.dat";
static DEVICE: Mutex<Option<DeviceSession>> = Mutex::new(None);
static TOKEN_REFRESH: Mutex<()> = Mutex::new(());

#[derive(Clone)]
struct DeviceSession {
    code: String,
    interval: Duration,
    next_poll: Instant,
    expires_at: Instant,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Repository {
    pub id: u64,
    pub full_name: String,
    pub private: bool,
    #[serde(default)]
    pub selected: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    user_id: u64,
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    login: String,
    selected: Vec<Repository>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    user_code: String,
    verification_uri: String,
    interval: u64,
    expires_in: u64,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn app_data_file(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|_| "No se pudo abrir el almacenamiento local de Bloom".to_string())?
        .join(ACCOUNT_FILE))
}

fn save_account(app: &AppHandle, account: &Account) -> Result<(), String> {
    let serialized = serde_json::to_vec(account)
        .map_err(|_| "No se pudo preparar la cuenta de GitHub".to_string())?;
    let protected = crate::google_auth::protect_data(&serialized)?;
    let path = app_data_file(app)?;
    std::fs::create_dir_all(path.parent().ok_or("Ruta de almacenamiento no válida")?)
        .map_err(|_| "No se pudo preparar el almacenamiento seguro".to_string())?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, protected)
        .map_err(|_| "No se pudo guardar el token protegido".to_string())?;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let from = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let to = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(from.as_ptr()),
            PCWSTR(to.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|_| "No se pudo completar el guardado protegido".to_string())
}

fn load_account(app: &AppHandle) -> Result<Account, String> {
    let path = app_data_file(app)?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "Conecta tu cuenta de GitHub para continuar".to_string())?
        .take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| "No se pudo leer la cuenta de GitHub".to_string())?;
    if bytes.len() > 65_536 {
        return Err("Los datos protegidos de GitHub exceden el tamaño esperado".into());
    }
    let plaintext = crate::google_auth::unprotect_data(&bytes)?;
    serde_json::from_slice(&plaintext)
        .map_err(|_| "Los datos protegidos de GitHub no son válidos".into())
}

pub(crate) fn secure_http_client() -> Result<reqwest::blocking::Client, String> {
    crate::google_auth::secure_http_client()
}

fn refresh_account(app: &AppHandle, account: Account) -> Result<Account, String> {
    if account.expires_at > now() + 120 {
        return Ok(account);
    }
    let _guard = TOKEN_REFRESH
        .lock()
        .map_err(|_| "No se pudo coordinar la renovación de la cuenta GitHub".to_string())?;
    let mut account = load_account(app)?;
    if account.expires_at > now() + 120 {
        return Ok(account);
    }
    let response: serde_json::Value = secure_http_client()?
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", account.refresh_token.as_str()),
        ])
        .send()
        .map_err(|_| "No se pudo renovar la conexión con GitHub".to_string())?
        .json()
        .map_err(|_| "GitHub devolvió una respuesta de renovación no válida".to_string())?;
    let new_access = response["access_token"]
        .as_str()
        .ok_or("La autorización GitHub expiró; vuelve a conectar la cuenta")?;
    let new_refresh = response["refresh_token"]
        .as_str()
        .ok_or("GitHub no devolvió un token de renovación; vuelve a conectar")?;
    account.access_token = new_access.to_string();
    account.refresh_token = new_refresh.to_string();
    account.expires_at = now() + response["expires_in"].as_i64().unwrap_or(28_800);
    save_account(app, &account)?;
    Ok(account)
}

pub(crate) fn access_token_for_selected_repository(
    app: &AppHandle,
    repository: &str,
) -> Result<Option<String>, String> {
    let account = match load_account(app) {
        Ok(account) => account,
        Err(_) => return Ok(None),
    };
    if !account
        .selected
        .iter()
        .any(|selected| selected.full_name.eq_ignore_ascii_case(repository))
    {
        return Ok(None);
    }
    Ok(Some(refresh_account(app, account)?.access_token))
}

fn fetch_authorized_repositories(account: &Account) -> Result<Vec<Repository>, String> {
    let client = secure_http_client()?;
    let installations: serde_json::Value = client
        .get("https://api.github.com/user/installations?per_page=100")
        .bearer_auth(&account.access_token)
        .header("User-Agent", "Bloom-desktop")
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|_| "No se pudieron consultar las instalaciones autorizadas".to_string())?
        .error_for_status()
        .map_err(|_| {
            "No se encontró una instalación de Bloom autorizada en esta cuenta".to_string()
        })?
        .json()
        .map_err(|_| "La lista de instalaciones de GitHub no es válida".to_string())?;
    let installations = installations["installations"]
        .as_array()
        .ok_or("GitHub no devolvió instalaciones")?;
    let mut repositories = Vec::new();
    for installation in installations {
        let Some(id) = installation["id"].as_u64() else {
            continue;
        };
        if installation["app_id"].as_u64() != Some(5_588_585) {
            continue;
        }
        for page in 1..=10 {
            let response: serde_json::Value = client
                .get(format!("https://api.github.com/user/installations/{id}/repositories?per_page=100&page={page}"))
                .bearer_auth(&account.access_token)
                .header("User-Agent", "Bloom-desktop")
                .header("Accept", "application/vnd.github+json")
                .send()
                .map_err(|_| "No se pudieron consultar los repositorios autorizados".to_string())?
                .error_for_status()
                .map_err(|_| "GitHub no permite leer los repositorios de esta instalación".to_string())?
                .json()
                .map_err(|_| "La lista de repositorios de GitHub no es válida".to_string())?;
            let page_repositories = response["repositories"]
                .as_array()
                .ok_or("GitHub no devolvió los repositorios autorizados")?;
            if page_repositories.is_empty() {
                break;
            }
            for repo in page_repositories {
                let (Some(id), Some(full_name)) = (repo["id"].as_u64(), repo["full_name"].as_str())
                else {
                    continue;
                };
                if repositories.iter().any(|item: &Repository| item.id == id) {
                    continue;
                }
                repositories.push(Repository {
                    id,
                    full_name: full_name.to_string(),
                    private: repo["private"].as_bool().unwrap_or(false),
                    selected: account.selected.iter().any(|item| item.id == id),
                });
            }
            if page_repositories.len() < 100 || repositories.len() >= 1000 {
                break;
            }
        }
    }
    repositories.sort_by_key(|repo| repo.full_name.to_lowercase());
    Ok(repositories)
}

fn require_main(window: &Window) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Ventana no válida".into())
    }
}

#[tauri::command]
pub fn github_auth_status(app: AppHandle, window: Window) -> Result<serde_json::Value, String> {
    require_main(&window)?;
    match load_account(&app) {
        Ok(account) => Ok(serde_json::json!({
            "connected": true,
            "login": account.login,
            "userId": account.user_id,
            "selected": account.selected,
        })),
        Err(_) => Ok(serde_json::json!({
            "connected": false,
            "login": null,
            "selected": [],
        })),
    }
}

#[tauri::command]
pub async fn github_device_start(app: AppHandle, window: Window) -> Result<DeviceStart, String> {
    require_main(&window)?;
    if app_data_file(&app)?.exists() {
        return Err("Desconecta la cuenta actual antes de iniciar otra".into());
    }
    let result = tauri::async_runtime::spawn_blocking(|| {
        let response: serde_json::Value = secure_http_client()?
            .post("https://github.com/login/device/code")
            .header("Accept", "application/json")
            .form(&[("client_id", CLIENT_ID), ("scope", "offline_access")])
            .send()
            .map_err(|_| "No se pudo conectar con GitHub".to_string())?
            .error_for_status()
            .map_err(|_| "GitHub rechazó iniciar Device Flow".to_string())?
            .json()
            .map_err(|_| "GitHub devolvió una respuesta Device Flow no válida".to_string())?;
        let code = response["device_code"]
            .as_str()
            .ok_or("GitHub no entregó el código del dispositivo")?
            .to_string();
        let interval = response["interval"].as_u64().unwrap_or(5).max(5);
        let expires_in = response["expires_in"].as_u64().unwrap_or(900);
        let output = DeviceStart {
            user_code: response["user_code"]
                .as_str()
                .ok_or("GitHub no entregó el código visible")?
                .to_string(),
            verification_uri: response["verification_uri"]
                .as_str()
                .unwrap_or("https://github.com/login/device")
                .to_string(),
            interval,
            expires_in,
        };
        Ok::<_, String>((code, output))
    })
    .await
    .map_err(|_| "No se pudo iniciar el flujo de GitHub".to_string())??;
    *DEVICE
        .lock()
        .map_err(|_| "No se pudo preparar el inicio de sesión".to_string())? =
        Some(DeviceSession {
            code: result.0,
            interval: Duration::from_secs(result.1.interval),
            next_poll: Instant::now() + Duration::from_secs(result.1.interval),
            expires_at: Instant::now() + Duration::from_secs(result.1.expires_in),
        });
    if result.1.verification_uri != "https://github.com/login/device" {
        return Err("GitHub devolvió una dirección de verificación inesperada".into());
    }
    tauri_plugin_opener::open_url(&result.1.verification_uri, None::<&str>)
        .map_err(|_| "No se pudo abrir github.com/login/device".to_string())?;
    Ok(result.1)
}

#[tauri::command]
pub async fn github_device_poll(
    app: AppHandle,
    window: Window,
) -> Result<serde_json::Value, String> {
    require_main(&window)?;
    let session = DEVICE
        .lock()
        .map_err(|_| "No se pudo consultar el inicio de sesión".to_string())?
        .clone()
        .ok_or("Inicia primero el acceso a GitHub")?;
    if Instant::now() >= session.expires_at {
        *DEVICE
            .lock()
            .map_err(|_| "No se pudo cancelar Device Flow")? = None;
        return Err("El código de GitHub expiró; inicia sesión de nuevo".into());
    }
    if Instant::now() < session.next_poll {
        return Ok(
            serde_json::json!({ "state": "pending", "waitSeconds": session.next_poll.duration_since(Instant::now()).as_secs() + 1 }),
        );
    }
    let device_code = session.code.clone();
    let response = tauri::async_runtime::spawn_blocking(move || {
        let response: serde_json::Value = secure_http_client()?
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", CLIENT_ID),
                ("device_code", device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .map_err(|_| "No se pudo consultar la autorización de GitHub".to_string())?
            .json()
            .map_err(|_| "GitHub devolvió una respuesta Device Flow no válida".to_string())?;
        Ok::<_, String>(response)
    })
    .await
    .map_err(|_| "No se pudo consultar la autorización de GitHub".to_string())??;

    if let Some(error) = response["error"].as_str() {
        match error {
            "authorization_pending" => {
                if let Ok(mut device) = DEVICE.lock() {
                    if let Some(active) = device.as_mut() {
                        active.next_poll = Instant::now() + active.interval;
                    }
                }
                return Ok(serde_json::json!({ "state": "pending" }));
            }
            "slow_down" => {
                if let Ok(mut device) = DEVICE.lock() {
                    if let Some(active) = device.as_mut() {
                        active.interval += Duration::from_secs(5);
                        active.next_poll = Instant::now() + active.interval;
                    }
                }
                return Ok(serde_json::json!({ "state": "slow_down" }));
            }
            "access_denied" | "expired_token" | "bad_verification_code" => {
                if let Ok(mut device) = DEVICE.lock() {
                    *device = None;
                }
                return Err(if error == "access_denied" {
                    "Autorización cancelada en GitHub".into()
                } else {
                    "El código de GitHub expiró; inicia sesión de nuevo".into()
                });
            }
            _ => {
                return Err(response["error_description"]
                    .as_str()
                    .unwrap_or("Falló Device Flow")
                    .into())
            }
        }
    }

    let access = response["access_token"]
        .as_str()
        .ok_or("GitHub no entregó un token de acceso")?
        .to_string();
    let identity = tauri::async_runtime::spawn_blocking({
        let access = access.clone();
        move || {
            secure_http_client()?
                .get("https://api.github.com/user")
                .bearer_auth(&access)
                .header("User-Agent", "Bloom-desktop")
                .header("Accept", "application/vnd.github+json")
                .send()
                .map_err(|_| "No se pudo validar la cuenta de GitHub".to_string())?
                .error_for_status()
                .map_err(|_| "GitHub rechazó validar la cuenta".to_string())?
                .json::<serde_json::Value>()
                .map_err(|_| "El perfil de GitHub no es válido".to_string())
        }
    })
    .await
    .map_err(|_| "No se pudo validar la cuenta de GitHub".to_string())??;
    let login = identity["login"]
        .as_str()
        .ok_or("GitHub no identificó la cuenta")?
        .to_string();
    let user_id = identity["id"]
        .as_u64()
        .ok_or("GitHub no devolvió el identificador estable de la cuenta")?;
    let refresh = response["refresh_token"]
        .as_str()
        .ok_or("GitHub no concedió renovación; vuelve a iniciar Device Flow")?
        .to_string();
    let account = Account {
        user_id,
        access_token: access,
        refresh_token: refresh,
        expires_at: now() + response["expires_in"].as_i64().unwrap_or(28_800),
        login: login.clone(),
        selected: Vec::new(),
    };
    save_account(&app, &account)?;
    if let Ok(mut device) = DEVICE.lock() {
        *device = None;
    }
    let _ = app.emit("github-auth-state", ());
    Ok(serde_json::json!({ "state": "connected", "login": login }))
}

#[tauri::command]
pub async fn github_repositories(
    app: AppHandle,
    window: Window,
) -> Result<Vec<Repository>, String> {
    require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let account = refresh_account(&app, load_account(&app)?)?;
        fetch_authorized_repositories(&account)
    })
    .await
    .map_err(|_| "No se pudieron consultar los repositorios de GitHub".to_string())?
}

#[tauri::command]
pub async fn github_release_verification(
    app: AppHandle,
    window: Window,
) -> Result<serde_json::Value, String> {
    require_main(&window)?;
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(180))
        .version_comparator(|_, _| true)
        .build()
        .map_err(|error| format!("No se pudo preparar el verificador: {error}"))?;
    let update = updater
        .check()
        .await
        .map_err(|error| format!("No se pudo consultar o validar latest: {error}"))?
        .ok_or("GitHub latest no publicó una actualización para esta plataforma")?;
    let version = update.version.clone();
    let url = update.download_url.to_string();
    if !url.starts_with("https://github.com/Asius/bloom/releases/download/")
        || !url.ends_with(".exe")
    {
        return Err("El canal latest apunta a un instalador inesperado".into());
    }
    let signature_available = !update.signature.trim().is_empty();
    if !signature_available {
        return Err("El canal latest no incluye firma del instalador".into());
    }
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|error| format!("La firma del instalador no pasó la verificación: {error}"))?;
    Ok(serde_json::json!({
        "channel": "latest",
        "version": version,
        "manifest": true,
        "installer": true,
        "signature": true,
        "verified": true,
        "bytes": bytes.len(),
        "detail": "El manifiesto latest respondió y la firma Minisign del instalador se verificó sin instalarlo.",
    }))
}

#[tauri::command]
pub async fn github_select_repositories(
    app: AppHandle,
    window: Window,
    repositories: Vec<Repository>,
) -> Result<(), String> {
    require_main(&window)?;
    if repositories.len() > 50 {
        return Err("Puedes seguir hasta 50 repositorios".into());
    }
    if repositories
        .iter()
        .any(|repo| repo.id == 0 || repo.full_name.len() > 201)
    {
        return Err("La selección contiene un repositorio no válido".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut account = refresh_account(&app, load_account(&app)?)?;
        let authorized = fetch_authorized_repositories(&account)?;
        account.selected = repositories
            .into_iter()
            .map(|requested| {
                authorized
                    .iter()
                    .find(|repo| repo.id == requested.id && repo.full_name == requested.full_name)
                    .cloned()
                    .ok_or_else(|| {
                        "La selección incluye un repositorio que GitHub no autorizó".to_string()
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        save_account(&app, &account)
    })
    .await
    .map_err(|_| "No se pudo guardar la selección de repositorios".to_string())?
}

#[tauri::command]
pub fn github_disconnect(app: AppHandle, window: Window) -> Result<(), String> {
    require_main(&window)?;
    let path = app_data_file(&app)?;
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|_| "No se pudo quitar el token GitHub de este equipo".to_string())?;
    }
    if let Ok(mut device) = DEVICE.lock() {
        *device = None;
    }
    let _ = app.emit("github-auth-state", ());
    Ok(())
}

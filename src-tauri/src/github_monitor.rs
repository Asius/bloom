//! Read-only GitHub Actions snapshots for selected public or authorized repositories.
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct Snapshot {
    repository: String,
    runs: Vec<Value>,
}

fn valid_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        && value != "."
        && value != ".."
}

#[tauri::command]
pub async fn github_monitor_snapshot(
    app: tauri::AppHandle,
    repository: String,
) -> Result<Snapshot, String> {
    let parts: Vec<_> = repository.split('/').collect();
    if parts.len() != 2 || !parts.iter().all(|part| valid_part(part)) {
        return Err("Introduce un repositorio con el formato propietario/nombre.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let token = crate::github_auth::access_token_for_selected_repository(&app, &repository)?;
        let client = crate::google_auth::secure_http_client()?;
        let mut request = client
            .get(format!(
                "https://api.github.com/repos/{repository}/actions/runs?per_page=50"
            ))
            .header("User-Agent", "Bloom-desktop")
            .header("Accept", "application/vnd.github+json");
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|_| "No se pudo conectar con GitHub.".to_string())?;
        if !response.status().is_success() {
            return Err(format!(
                "GitHub respondió con {}. Comprueba el repositorio o el límite de consultas.",
                response.status().as_u16()
            ));
        }
        let body: Value = response
            .json()
            .map_err(|_| "GitHub devolvió datos no válidos.".to_string())?;
        let runs = body
            .get("workflow_runs")
            .and_then(Value::as_array)
            .cloned()
            .ok_or("GitHub no devolvió ejecuciones.")?;
        Ok(Snapshot { repository, runs })
    })
    .await
    .map_err(|_| "No se pudo consultar GitHub.".to_string())?
}

#[tauri::command]
pub async fn github_monitor_jobs(
    app: tauri::AppHandle,
    repository: String,
    run_id: u64,
) -> Result<Value, String> {
    let parts: Vec<_> = repository.split('/').collect();
    if parts.len() != 2 || !parts.iter().all(|part| valid_part(part)) || run_id == 0 {
        return Err("Repositorio o ejecución no válidos.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let token = crate::github_auth::access_token_for_selected_repository(&app, &repository)?;
        let client = crate::google_auth::secure_http_client()?;
        let mut request = client
            .get(format!(
                "https://api.github.com/repos/{repository}/actions/runs/{run_id}/jobs?per_page=100"
            ))
            .header("User-Agent", "Bloom-desktop")
            .header("Accept", "application/vnd.github+json");
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|_| "No se pudo conectar con GitHub.".to_string())?;
        if !response.status().is_success() {
            return Err(format!(
                "GitHub respondió con {}.",
                response.status().as_u16()
            ));
        }
        response
            .json()
            .map_err(|_| "GitHub devolvió datos no válidos.".to_string())
    })
    .await
    .map_err(|_| "No se pudieron consultar los pasos.".to_string())?
}

#[tauri::command]
pub fn github_monitor_open(repository: String) -> Result<(), String> {
    let parts: Vec<_> = repository.split('/').collect();
    if parts.len() != 2 || !parts.iter().all(|part| valid_part(part)) {
        return Err("Repositorio no válido.".into());
    }
    tauri_plugin_opener::open_url(
        format!("https://github.com/{repository}/actions"),
        None::<&str>,
    )
    .map_err(|_| "No se pudo abrir GitHub.".into())
}

#[cfg(test)]
mod tests {
    use super::valid_part;

    #[test]
    fn repository_path_cannot_escape_the_github_api_path() {
        for invalid in ["", ".", "..", "x/y", "x?y", "x#y", "x%2fy"] {
            assert!(!valid_part(invalid), "{invalid}");
        }
        assert!(valid_part("Asius"));
        assert!(valid_part("bloom-example_1"));
    }
}

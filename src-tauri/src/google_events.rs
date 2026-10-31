//! Bounded, read-only queries for the primary calendar; no event or token logging.
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Read};
use tauri::{AppHandle, Window};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const EVENTS_URL: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Deserialize)]
struct AccessToken {
    access_token: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Moment {
    date: Option<String>,
    date_time: Option<String>,
}
#[derive(Deserialize, Serialize)]
pub(crate) struct Event {
    id: String,
    summary: Option<String>,
    status: Option<String>,
    start: Option<Moment>,
    end: Option<Moment>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    #[serde(default)]
    items: Vec<Event>,
    next_page_token: Option<String>,
}

fn validate_range(start: &str, end: &str) -> Result<(), String> {
    let a =
        chrono::DateTime::parse_from_rfc3339(start).map_err(|_| "Intervalo de fechas inválido")?;
    let b =
        chrono::DateTime::parse_from_rfc3339(end).map_err(|_| "Intervalo de fechas inválido")?;
    let seconds = (b - a).num_seconds();
    if seconds <= 0 || seconds > 80 * 3600 {
        return Err("Intervalo de consulta fuera de límites".into());
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(
    response: reqwest::blocking::Response,
) -> Result<T, String> {
    let mut data = Vec::new();
    response
        .take(LIMIT + 1)
        .read_to_end(&mut data)
        .map_err(|_| "No se pudo leer la respuesta de Google")?;
    if data.len() as u64 > LIMIT {
        return Err("La respuesta de Google es demasiado grande".into());
    }
    serde_json::from_slice(&data)
        .map_err(|_| "Google devolvió datos que Bloom no pudo interpretar".into())
}

fn collect_pages(
    mut get_page: impl FnMut(Option<&str>) -> Result<Page, String>,
) -> Result<Vec<Event>, String> {
    let mut result = Vec::new();
    let mut next: Option<String> = None;
    let mut seen = HashSet::new();
    for _ in 0..8 {
        let page = get_page(next.as_deref())?;
        result.extend(page.items.into_iter().filter_map(|mut event| {
            if event.status.as_deref() == Some("cancelled")
                || event.start.is_none()
                || event.end.is_none()
            {
                return None;
            }
            if let Some(summary) = event.summary.as_mut() {
                *summary = summary.chars().take(500).collect();
            }
            Some(event)
        }));
        match page.next_page_token {
            None => return Ok(result),
            Some(page) if !page.is_empty() && page.len() <= 2048 && seen.insert(page.clone()) => {
                next = Some(page)
            }
            _ => return Err("Google devolvió una página de eventos inválida".into()),
        }
    }
    Err("Hay demasiados eventos para mostrar en este día".into())
}

fn fetch_events(app: AppHandle, start: String, end: String) -> Result<Vec<Event>, String> {
    validate_range(&start, &end)?;
    let _guard = crate::google_auth::lock_connection()?;
    let stored = crate::google_auth::load_stored(&app)?;
    let http = crate::google_auth::secure_http_client()?;
    let response = http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", stored.client.client_id.as_str()),
            ("client_secret", stored.client.client_secret.as_str()),
            ("refresh_token", stored.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .map_err(|_| "No se pudo contactar a Google. Comprueba tu conexión.")?;
    if response.status().as_u16() == 400 || response.status().as_u16() == 401 {
        return Err("La autorización ya no es válida. Elimina la conexión local y vuelve a conectar Google Calendar.".into());
    }
    if !response.status().is_success() {
        return Err("Google no pudo renovar la autorización. Inténtalo de nuevo.".into());
    }
    let token: AccessToken = read_json(response)?;
    if token.access_token.is_empty() || token.access_token.len() > 8192 {
        return Err("Google devolvió una autorización inválida".into());
    }
    collect_pages(|next| {
        let mut url = url::Url::parse(EVENTS_URL).map_err(|_| "No se pudo preparar la consulta")?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("timeMin", &start)
                .append_pair("timeMax", &end)
                .append_pair("singleEvents", "true")
                .append_pair("orderBy", "startTime")
                .append_pair("maxResults", "250")
                .append_pair("fields", "nextPageToken,items(id,summary,status,start,end)");
            if let Some(page) = next {
                query.append_pair("pageToken", page);
            }
        }
        let response = http
            .get(url)
            .bearer_auth(&token.access_token)
            .send()
            .map_err(|_| "No se pudieron consultar los eventos. Comprueba tu conexión.")?;
        if response.status().as_u16() == 401 {
            return Err("Google rechazó la autorización. Vuelve a conectar tu cuenta.".into());
        }
        if !response.status().is_success() {
            return Err(
                "Google Calendar no pudo entregar los eventos. Comprueba la API y los permisos."
                    .into(),
            );
        }
        read_json(response)
    })
}

#[tauri::command]
pub async fn google_calendar_events(
    app: AppHandle,
    window: Window,
    start: String,
    end: String,
) -> Result<Vec<Event>, String> {
    if window.label() != "main" {
        return Err("Acción no disponible en esta ventana".into());
    }
    tauri::async_runtime::spawn_blocking(move || fetch_events(app, start, end))
        .await
        .map_err(|_| "No se pudo consultar Google Calendar")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unbounded_or_reversed_ranges() {
        assert!(validate_range("2026-10-30T00:00:00Z", "2026-11-01T00:00:00Z").is_ok());
        assert!(validate_range("2026-10-30T00:00:00Z", "2026-11-10T00:00:00Z").is_err());
        assert!(validate_range("2026-10-30", "2026-10-31").is_err());
        assert!(validate_range("2026-11-01T00:00:00Z", "2026-10-30T00:00:00Z").is_err());
    }
    #[test]
    fn collects_pages_and_rejects_repeated_tokens() {
        let first = r#"{"items":[{"id":"a","start":{"date":"2026-10-01"},"end":{"date":"2026-10-02"}}],"nextPageToken":"next"}"#;
        let second =
            r#"{"items":[{"id":"b","start":{"date":"2026-10-02"},"end":{"date":"2026-10-03"}}]}"#;
        let items = collect_pages(|next| {
            Ok(serde_json::from_str(if next.is_none() { first } else { second }).unwrap())
        })
        .unwrap();
        assert_eq!(
            items.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert!(collect_pages(|_| Ok(serde_json::from_str(first).unwrap())).is_err());
    }
}

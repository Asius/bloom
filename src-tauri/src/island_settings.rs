//! One-time migration from the retired taskbar replacement. Never delete user files.
use serde_json::Value;
use std::{collections::HashMap, path::Path};

pub fn retired_key(key: &str) -> bool {
    key.starts_with("bloom-dock-")
        || key.starts_with("bloom-tray-")
        || matches!(
            key,
            "bloom-start-icon" | "bloom-scale" | "bloom-scale-linked"
        )
}

pub fn normalize(values: &mut HashMap<String, Value>) {
    if values.contains_key("bloom-scale") || values.contains_key("bloom-scale-linked") {
        let linked = values.get("bloom-scale-linked").and_then(Value::as_str) != Some("false")
            && values.get("bloom-scale-linked").and_then(Value::as_bool) != Some(false);
        let previous = if linked {
            values.get("bloom-scale")
        } else {
            values
                .get("bloom-notch-scale")
                .or_else(|| values.get("bloom-scale"))
        };
        let scale = previous
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| v.as_f64())
            })
            .filter(|v| v.is_finite())
            .unwrap_or(1.0)
            .clamp(0.8, 1.3);
        values.insert("bloom-notch-scale".into(), Value::String(scale.to_string()));
    }
    values.retain(|key, _| !retired_key(key));
}

/// Production callers hold SETTINGS_FILE_LOCK across migration and the following save.
pub fn migrate_file(path: &Path) -> Result<(), String> {
    let original = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let text = std::str::from_utf8(&original).map_err(|e| e.to_string())?;
    let old: HashMap<String, Value> =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let mut migrated = old.clone();
    normalize(&mut migrated);
    if migrated == old {
        return Ok(());
    }
    use std::io::Write;
    let backup = path.with_file_name("settings-before-island.json");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(backup)
    {
        Ok(mut file) => file.write_all(&original).map_err(|e| e.to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    crate::utils::write_settings_file(path, &migrated)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_preserves_effective_notch_size_and_personal_preferences() {
        let mut settings = serde_json::from_value(serde_json::json!({"bloom-scale":"1.2", "bloom-notch-scale":"0.9", "bloom-scale-linked":"true", "bloom-dock-enabled":"true", "bloom-tray-location":"dock", "bloom-theme-mode":"light", "custom-key":17})).unwrap();
        normalize(&mut settings);
        assert_eq!(settings["bloom-notch-scale"], "1.2");
        assert_eq!(settings["bloom-theme-mode"], "light");
        assert_eq!(settings["custom-key"], 17);
        assert!(!settings.keys().any(|k| retired_key(k)));
        let once = settings.clone();
        normalize(&mut settings);
        assert_eq!(settings, once);
    }
    #[test]
    fn independent_notch_scale_wins_and_invalid_scale_is_safe() {
        let mut settings = serde_json::from_value(serde_json::json!({"bloom-scale":"1.3", "bloom-scale-linked":"false", "bloom-notch-scale":"0.9"})).unwrap();
        normalize(&mut settings);
        assert_eq!(settings["bloom-notch-scale"], "0.9");
        settings.insert("bloom-scale".into(), Value::String("NaN".into()));
        settings.remove("bloom-scale-linked");
        normalize(&mut settings);
        assert_eq!(settings["bloom-notch-scale"], "1");
    }
    #[test]
    fn migration_keeps_one_original_backup_and_rejects_damaged_json() {
        let directory = std::env::temp_dir().join(format!(
            "bloom-island-migration-{}-{}",
            std::process::id(),
            crate::utils::get_now_ms()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("settings.json");
        let original = br#"{"bloom-dock-enabled":"true","bloom-theme-mode":"dark"}"#;
        std::fs::write(&path, original).unwrap();
        migrate_file(&path).unwrap();
        migrate_file(&path).unwrap();
        assert_eq!(
            std::fs::read(directory.join("settings-before-island.json")).unwrap(),
            original
        );
        std::fs::write(&path, b"invalid JSON").unwrap();
        assert!(migrate_file(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid JSON");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(directory.join("settings-before-island.json")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}

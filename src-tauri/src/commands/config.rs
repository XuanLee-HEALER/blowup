use crate::config::{load_config, Config};

#[tauri::command]
pub fn get_config() -> Result<Config, String> {
    Ok(load_config())
}

/// key format: "section.field"  e.g. "tmdb.api_key"
#[tauri::command]
pub fn set_config_key(key: String, value: String) -> Result<(), String> {
    use crate::config::config_path;
    use toml_edit::{DocumentMut, Item, Value};

    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;

    let mut parts = key.splitn(2, '.');
    let section = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Invalid key: '{key}'"))?;
    let field = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Invalid key: '{key}'"))?;

    doc[section][field] = Item::Value(Value::from(value));
    std::fs::write(&path, doc.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

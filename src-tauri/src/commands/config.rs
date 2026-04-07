use crate::config::{Config, app_data_dir, load_config, save_config};

#[tauri::command]
pub fn get_config() -> Result<Config, String> {
    Ok(load_config())
}

#[tauri::command]
pub fn save_config_cmd(new_config: Config) -> Result<(), String> {
    save_config(&new_config)
}

#[tauri::command]
pub fn get_cache_path() -> String {
    app_data_dir()
        .join("credits_cache.json")
        .to_string_lossy()
        .into_owned()
}

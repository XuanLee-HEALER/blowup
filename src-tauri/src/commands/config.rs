use crate::config::{load_config, save_config, Config};

#[tauri::command]
pub fn get_config() -> Result<Config, String> {
    Ok(load_config())
}

#[tauri::command]
pub fn save_config_cmd(new_config: Config) -> Result<(), String> {
    save_config(&new_config)
}

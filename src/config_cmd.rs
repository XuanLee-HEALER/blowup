use crate::config::config_path;
use crate::error::ConfigCmdError;
use toml_edit::DocumentMut;

/// 所有合法的 config key（section, field）
const KNOWN_KEYS: &[(&str, &str)] = &[
    ("tools", "aria2c"),
    ("tools", "alass"),
    ("search", "rate_limit_secs"),
    ("subtitle", "default_lang"),
    ("omdb", "api_key"),
    ("opensubtitles", "api_key"),
];

/// 解析 "section.field" 格式的 key
fn parse_key(key: &str) -> Result<(&str, &str), ConfigCmdError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(ConfigCmdError::InvalidKeyFormat(key.to_string()));
    }
    let section = parts[0];
    let field = parts[1];
    if !KNOWN_KEYS.contains(&(section, field)) {
        return Err(ConfigCmdError::UnknownKey(key.to_string()));
    }
    Ok((section, field))
}

fn read_doc() -> Result<DocumentMut, ConfigCmdError> {
    let path = config_path();
    if !path.exists() {
        return Ok(DocumentMut::new());
    }
    let content = std::fs::read_to_string(&path).map_err(ConfigCmdError::Io)?;
    content
        .parse::<DocumentMut>()
        .map_err(|e| ConfigCmdError::TomlParse(e.to_string()))
}

fn write_doc(doc: &DocumentMut) -> Result<(), ConfigCmdError> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(ConfigCmdError::Io)?;
    }
    std::fs::write(&path, doc.to_string()).map_err(ConfigCmdError::Io)?;
    Ok(())
}

pub fn set_config(key: &str, value: &str) -> Result<(), ConfigCmdError> {
    let (section, field) = parse_key(key)?;
    let mut doc = read_doc()?;
    doc[section][field] = toml_edit::value(value);
    write_doc(&doc)?;
    println!("✓ Set {}", key);
    Ok(())
}

pub fn get_config(key: &str) -> Result<(), ConfigCmdError> {
    let (section, field) = parse_key(key)?;
    let doc = read_doc()?;
    let val = doc
        .get(section)
        .and_then(|s| s.get(field))
        .and_then(|v| v.as_str())
        .unwrap_or("(not set)");
    println!("{}", val);
    Ok(())
}

pub fn list_config() -> Result<(), ConfigCmdError> {
    let doc = read_doc()?;

    // 按 section 分组打印
    let sections = ["tools", "search", "subtitle", "omdb", "opensubtitles"];
    for section in &sections {
        println!("[{}]", section);
        let section_keys: Vec<&str> = KNOWN_KEYS
            .iter()
            .filter(|(s, _)| s == section)
            .map(|(_, f)| *f)
            .collect();
        for field in section_keys {
            let val = doc
                .get(section)
                .and_then(|s| s.get(field))
                .and_then(|v| v.as_str())
                .unwrap_or("(not set)");
            println!("  {:<20} = {}", field, val);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_valid() {
        let (section, field) = parse_key("omdb.api_key").unwrap();
        assert_eq!(section, "omdb");
        assert_eq!(field, "api_key");
    }

    #[test]
    fn parse_key_no_dot_returns_error() {
        let err = parse_key("omdbapi_key").unwrap_err();
        assert!(matches!(err, ConfigCmdError::InvalidKeyFormat(_)));
    }

    #[test]
    fn parse_key_unknown_returns_error() {
        let err = parse_key("foo.bar").unwrap_err();
        assert!(matches!(err, ConfigCmdError::UnknownKey(_)));
    }

    #[test]
    fn set_and_get_in_memory() {
        // 测试 toml_edit 的 set/get 逻辑，不写磁盘
        let mut doc = DocumentMut::new();
        doc["omdb"]["api_key"] = toml_edit::value("test_key_123");
        let val = doc["omdb"]["api_key"].as_str().unwrap();
        assert_eq!(val, "test_key_123");
    }
}

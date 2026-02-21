use crate::config::config_path;
use crate::error::ConfigCmdError;
use toml_edit::DocumentMut;

#[derive(Clone, Copy, PartialEq, Debug)]
enum KeyType {
    Str,
    U64,
}

/// 所有合法的 config key（section, field, type）
const KNOWN_KEYS: &[(&str, &str, KeyType)] = &[
    ("tools", "aria2c", KeyType::Str),
    ("tools", "alass", KeyType::Str),
    ("search", "rate_limit_secs", KeyType::U64),
    ("subtitle", "default_lang", KeyType::Str),
    ("tmdb", "api_key", KeyType::Str),
    ("opensubtitles", "api_key", KeyType::Str),
];

/// 解析 "section.field" 格式的 key
fn parse_key(key: &str) -> Result<(&str, &str, KeyType), ConfigCmdError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(ConfigCmdError::InvalidKeyFormat(key.to_string()));
    }
    let section = parts[0];
    let field = parts[1];
    match KNOWN_KEYS
        .iter()
        .find(|(s, f, _)| *s == section && *f == field)
    {
        Some((_, _, kt)) => Ok((section, field, *kt)),
        None => Err(ConfigCmdError::UnknownKey(key.to_string())),
    }
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

fn item_to_string(item: &toml_edit::Item) -> Option<String> {
    if let Some(s) = item.as_str() {
        return Some(s.to_string());
    }
    if let Some(n) = item.as_integer() {
        return Some(n.to_string());
    }
    None
}

pub fn set_config(key: &str, value: &str) -> Result<(), ConfigCmdError> {
    let (section, field, key_type) = parse_key(key)?;
    let mut doc = read_doc()?;
    match key_type {
        KeyType::U64 => {
            let n: i64 = value.parse().map_err(|_| {
                ConfigCmdError::InvalidKeyFormat(format!("{} must be a number", key))
            })?;
            doc[section][field] = toml_edit::value(n);
        }
        KeyType::Str => {
            doc[section][field] = toml_edit::value(value);
        }
    }
    write_doc(&doc)?;
    println!("✓ Set {}", key);
    Ok(())
}

pub fn get_config(key: &str) -> Result<(), ConfigCmdError> {
    let (section, field, _) = parse_key(key)?;
    let doc = read_doc()?;
    let val = doc
        .get(section)
        .and_then(|s| s.get(field))
        .and_then(item_to_string)
        .unwrap_or_else(|| "(not set)".to_string());
    println!("{}", val);
    Ok(())
}

pub fn list_config() -> Result<(), ConfigCmdError> {
    let doc = read_doc()?;

    // Collect unique sections in insertion order
    let mut sections: Vec<&str> = vec![];
    for (s, _, _) in KNOWN_KEYS {
        if !sections.contains(s) {
            sections.push(s);
        }
    }

    for section in &sections {
        println!("[{}]", section);
        let section_keys: Vec<(&str, KeyType)> = KNOWN_KEYS
            .iter()
            .filter(|(s, _, _)| s == section)
            .map(|(_, f, kt)| (*f, *kt))
            .collect();
        for (field, _) in section_keys {
            let val = doc
                .get(section)
                .and_then(|s| s.get(field))
                .and_then(item_to_string)
                .unwrap_or_else(|| "(not set)".to_string());
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
        let (section, field, kt) = parse_key("tmdb.api_key").unwrap();
        assert_eq!(section, "tmdb");
        assert_eq!(field, "api_key");
        assert_eq!(kt, KeyType::Str);
    }

    #[test]
    fn parse_key_no_dot_returns_error() {
        let err = parse_key("tmdbapi_key").unwrap_err();
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
        doc["tmdb"]["api_key"] = toml_edit::value("test_key_123");
        let val = doc["tmdb"]["api_key"].as_str().unwrap();
        assert_eq!(val, "test_key_123");
    }

    #[test]
    fn item_to_string_handles_integer() {
        let mut doc = DocumentMut::new();
        doc["search"]["rate_limit_secs"] = toml_edit::value(5_i64);
        let item = &doc["search"]["rate_limit_secs"];
        let result = item_to_string(item);
        assert_eq!(result, Some("5".to_string()));
    }

    #[test]
    fn item_to_string_handles_string() {
        let mut doc = DocumentMut::new();
        doc["tmdb"]["api_key"] = toml_edit::value("my_key");
        let item = &doc["tmdb"]["api_key"];
        let result = item_to_string(item);
        assert_eq!(result, Some("my_key".to_string()));
    }
}

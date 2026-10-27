//! Read–merge–write for structured emulator configs (TOML, YAML, JSON).
//!
//! Emulators keep their own state in these files (RPCS3's console PSID,
//! Xenia's migration date, xemu's controller GUIDs), so we only ever set the
//! keys we own and leave everything else exactly as found.

use anyhow::Context;
use std::{fs, path::Path};

/// A typed config value. Types matter: shadPS4 aborts on a string where it
/// expects a bool, and Xenia resets mistyped keys.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Str(String),
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(v.to_string())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Str(v)
    }
}

/// `table` is the path of nested tables/maps; `key` is set inside the last one.
pub struct Entry {
    pub table: &'static [&'static str],
    pub key: &'static str,
    pub value: Value,
}

pub fn entry(table: &'static [&'static str], key: &'static str, value: impl Into<Value>) -> Entry {
    Entry {
        table,
        key,
        value: value.into(),
    }
}

pub fn merge_toml(path: &Path, entries: &[Entry]) -> anyhow::Result<()> {
    use toml_edit::{value, DocumentMut, Item, Table};

    let text = fs::read_to_string(path).unwrap_or_default();
    // A broken file is fatal to the emulator; start clean rather than keep it.
    let mut doc: DocumentMut = text.parse().unwrap_or_default();
    for e in entries {
        let mut table = doc.as_table_mut();
        for name in e.table {
            let item = table
                .entry(name)
                .or_insert_with(|| Item::Table(Table::new()));
            if !item.is_table() {
                *item = Item::Table(Table::new());
            }
            table = item.as_table_mut().expect("just made a table");
        }
        table[e.key] = match &e.value {
            Value::Bool(b) => value(*b),
            Value::Int(i) => value(*i),
            Value::Str(s) => value(s.as_str()),
        };
    }
    write(path, doc.to_string())
}

pub fn merge_yaml(path: &Path, entries: &[Entry]) -> anyhow::Result<()> {
    use serde_yaml_ng::{Mapping, Value as Y};

    let text = fs::read_to_string(path).unwrap_or_default();
    let mut root = match serde_yaml_ng::from_str::<Y>(&text) {
        Ok(Y::Mapping(m)) => Y::Mapping(m),
        _ => Y::Mapping(Mapping::new()),
    };
    for e in entries {
        let mut node = &mut root;
        for name in e.table {
            let map = node.as_mapping_mut().expect("tables are mappings");
            let child = map
                .entry(Y::from(*name))
                .or_insert_with(|| Y::Mapping(Mapping::new()));
            if !child.is_mapping() {
                *child = Y::Mapping(Mapping::new());
            }
            node = child;
        }
        let value = match &e.value {
            Value::Bool(b) => Y::Bool(*b),
            Value::Int(i) => Y::Number((*i).into()),
            Value::Str(s) => Y::String(s.clone()),
        };
        node.as_mapping_mut()
            .expect("tables are mappings")
            .insert(Y::from(e.key), value);
    }
    write(path, serde_yaml_ng::to_string(&root)?)
}

pub fn merge_json(path: &Path, entries: &[Entry]) -> anyhow::Result<()> {
    use serde_json::{Map, Value as J};

    let text = fs::read_to_string(path).unwrap_or_default();
    let mut root = match serde_json::from_str::<J>(&text) {
        Ok(J::Object(m)) => J::Object(m),
        _ => J::Object(Map::new()),
    };
    for e in entries {
        let mut node = &mut root;
        for name in e.table {
            let map = node.as_object_mut().expect("tables are objects");
            let child = map.entry(*name).or_insert_with(|| J::Object(Map::new()));
            if !child.is_object() {
                *child = J::Object(Map::new());
            }
            node = child;
        }
        let value = match &e.value {
            Value::Bool(b) => J::Bool(*b),
            Value::Int(i) => J::from(*i),
            Value::Str(s) => J::String(s.clone()),
        };
        node.as_object_mut()
            .expect("tables are objects")
            .insert(e.key.to_string(), value);
    }
    write(path, serde_json::to_string_pretty(&root)?)
}

fn write(path: &Path, text: String) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text).with_context(|| format!("{} yazılamadı", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str, contents: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("lumen-formats-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn toml_keeps_foreign_keys_and_comments() {
        let path = temp(
            "xemu.toml",
            "# user note\n[input.bindings]\nport1 = 'abc-guid'\n\n[display.quality]\nsurface_scale = 1\n",
        );
        merge_toml(
            &path,
            &[
                entry(&["display", "quality"], "surface_scale", 3),
                entry(&["display", "window"], "fullscreen_on_startup", true),
                entry(&["display"], "renderer", "VULKAN"),
            ],
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let doc: toml_edit::DocumentMut = text.parse().unwrap();
        assert!(text.contains("# user note"));
        assert_eq!(doc["input"]["bindings"]["port1"].as_str(), Some("abc-guid"));
        assert_eq!(
            doc["display"]["quality"]["surface_scale"].as_integer(),
            Some(3)
        );
        assert_eq!(
            doc["display"]["window"]["fullscreen_on_startup"].as_bool(),
            Some(true)
        );
        assert_eq!(doc["display"]["renderer"].as_str(), Some("VULKAN"));
    }

    #[test]
    fn yaml_keeps_console_identity() {
        let path = temp(
            "config.yml",
            "System:\n  Console PSID: 1234ABCD\nVideo:\n  Renderer: OpenGL\n",
        );
        merge_yaml(
            &path,
            &[
                entry(&["Video"], "Renderer", "Vulkan"),
                entry(&["Video", "Vulkan"], "Exclusive Fullscreen Mode", "Disable"),
                entry(&["Video"], "Resolution Scale", 150),
            ],
        )
        .unwrap();
        let doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["System"]["Console PSID"].as_str(), Some("1234ABCD"));
        assert_eq!(doc["Video"]["Renderer"].as_str(), Some("Vulkan"));
        assert_eq!(doc["Video"]["Resolution Scale"].as_i64(), Some(150));
        assert_eq!(
            doc["Video"]["Vulkan"]["Exclusive Fullscreen Mode"].as_str(),
            Some("Disable")
        );
    }

    #[test]
    fn json_writes_native_types() {
        let path = temp(
            "config.json",
            r#"{"General":{"home_dir":"C:/saves"},"Debug":{"config_version":"x"}}"#,
        );
        merge_json(
            &path,
            &[
                entry(&["GPU"], "full_screen", true),
                entry(&["GPU"], "window_width", 1920),
            ],
        )
        .unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["GPU"]["full_screen"], serde_json::Value::Bool(true));
        assert_eq!(doc["GPU"]["window_width"], serde_json::json!(1920));
        assert_eq!(doc["General"]["home_dir"], "C:/saves");
        assert_eq!(doc["Debug"]["config_version"], "x");
    }

    #[test]
    fn broken_files_are_replaced_not_fatal() {
        let path = temp("bad.toml", "[[[ nope");
        merge_toml(&path, &[entry(&["general"], "show_welcome", false)]).unwrap();
        let doc: toml_edit::DocumentMut = fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(doc["general"]["show_welcome"].as_bool(), Some(false));
    }
}

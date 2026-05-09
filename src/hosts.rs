//! Persistent device alias registry — ~/.config/denki/hosts.json
//!
//! Maps friendly names to IPs and protocol type so commands auto-route
//! to the correct transport (Kasa XOR on port 9999, or KLAP on port 80).
//!
//! File format (v2):
//!   {"floor lamp": {"ip": "192.168.7.254", "protocol": "klap"}, ...}
//!
//! Backward compat (v1 plain strings are read as Kasa):
//!   {"office bulb": "192.168.4.65"}

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Kasa,
    Klap,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Kasa => write!(f, "kasa"),
            Protocol::Klap => write!(f, "klap"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub ip: String,
    pub protocol: Protocol,
}

fn hosts_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()))
        .join("denki")
        .join("hosts.json")
}

fn load_map(path: &Path) -> Result<BTreeMap<String, HostEntry>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let data = std::fs::read_to_string(path)?;

    // Try v2 format first
    if let Ok(map) = serde_json::from_str::<BTreeMap<String, HostEntry>>(&data) {
        return Ok(map);
    }

    // Fall back: v1 plain-string values → Kasa protocol
    let v1: BTreeMap<String, String> = serde_json::from_str(&data).unwrap_or_default();
    Ok(v1
        .into_iter()
        .map(|(k, ip)| (k, HostEntry { ip, protocol: Protocol::Kasa }))
        .collect())
}

fn save_map(path: &Path, map: &BTreeMap<String, HostEntry>) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(map)?)?;
    Ok(())
}

/// Normalize a name: lowercase, collapse non-alphanumeric to spaces.
pub fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Look up a name — exact match first, then substring.
/// Returns the HostEntry if exactly one match is found.
pub fn lookup(name: &str) -> Option<HostEntry> {
    let map = load_map(&hosts_path()).ok()?;
    let needle = normalize(name);

    // Exact match
    for (k, v) in &map {
        if normalize(k) == needle {
            return Some(v.clone());
        }
    }

    // Substring match (only if unambiguous)
    let hits: Vec<_> = map
        .values()
        .zip(map.keys())
        .filter(|(_, k)| normalize(k).contains(&needle))
        .map(|(v, _)| v.clone())
        .collect();
    if hits.len() == 1 {
        return Some(hits[0].clone());
    }
    None
}

/// Save (or overwrite) a name→entry alias.
pub fn set(name: &str, ip: &str, protocol: Protocol) -> Result<()> {
    let path = hosts_path();
    let mut map = load_map(&path)?;
    map.insert(name.to_string(), HostEntry { ip: ip.to_string(), protocol });
    save_map(&path, &map)
}

/// Remove an alias by exact name. Returns true if it existed.
pub fn remove(name: &str) -> Result<bool> {
    let path = hosts_path();
    let mut map = load_map(&path)?;
    let removed = map.remove(name).is_some();
    if removed {
        save_map(&path, &map)?;
    }
    Ok(removed)
}

/// List all saved aliases sorted by name.
pub fn list() -> Result<Vec<(String, HostEntry)>> {
    Ok(load_map(&hosts_path())?.into_iter().collect())
}

pub fn path_display() -> String {
    hosts_path().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Each test gets a unique temp path so parallel tests don't interfere.
    fn temp_path() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("denki-hosts-{}-{}.json", std::process::id(), n))
    }

    #[test]
    fn normalize_lowercases_and_collapses_whitespace() {
        assert_eq!(normalize("Office Bulb"), "office bulb");
        assert_eq!(normalize("Coat-Rack Lights"), "coat rack lights");
        assert_eq!(normalize("  MULTIPLE   SPACES  "), "multiple spaces");
        assert_eq!(normalize("123Abc!@#"), "123abc");
    }

    #[test]
    fn load_returns_empty_map_when_file_missing() {
        let path = temp_path(); // does not exist
        let map = load_map(&path).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn save_and_load_round_trips_v2_format() {
        let path = temp_path();
        let mut map = BTreeMap::new();
        map.insert(
            "floor lamp".to_string(),
            HostEntry { ip: "192.168.1.10".to_string(), protocol: Protocol::Kasa },
        );
        map.insert(
            "tapo plug".to_string(),
            HostEntry { ip: "192.168.7.254".to_string(), protocol: Protocol::Klap },
        );
        save_map(&path, &map).unwrap();

        let loaded = load_map(&path).unwrap();
        assert_eq!(loaded["floor lamp"].ip, "192.168.1.10");
        assert_eq!(loaded["floor lamp"].protocol, Protocol::Kasa);
        assert_eq!(loaded["tapo plug"].ip, "192.168.7.254");
        assert_eq!(loaded["tapo plug"].protocol, Protocol::Klap);
    }

    #[test]
    fn load_v1_plain_strings_as_kasa() {
        let path = temp_path();
        std::fs::write(&path, r#"{"office bulb": "192.168.4.65"}"#).unwrap();

        let map = load_map(&path).unwrap();
        assert_eq!(map["office bulb"].ip, "192.168.4.65");
        assert_eq!(map["office bulb"].protocol, Protocol::Kasa);
    }

    #[test]
    fn saved_file_is_valid_json() {
        let path = temp_path();
        let mut map = BTreeMap::new();
        map.insert(
            "desk lamp".to_string(),
            HostEntry { ip: "10.0.0.5".to_string(), protocol: Protocol::Kasa },
        );
        save_map(&path, &map).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&raw).is_ok());
    }
}

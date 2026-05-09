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
    use rstest::rstest;
    use tempfile::TempDir;

    /// Returns a TempDir (kept alive by caller) and a path inside it.
    /// TempDir deletes itself on drop — tests must hold it for the full scope.
    fn temp_hosts() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hosts.json");
        (dir, path)
    }

    fn entry(ip: &str, protocol: Protocol) -> HostEntry {
        HostEntry { ip: ip.to_string(), protocol }
    }

    // ── normalize ─────────────────────────────────────────────────────────────

    #[rstest]
    #[case("Office Bulb",        "office bulb")]
    #[case("Coat-Rack Lights",   "coat rack lights")]
    #[case("  MULTIPLE   SPACES  ", "multiple spaces")]
    #[case("123Abc!@#",          "123abc")]
    #[case("",                   "")]
    fn normalize_cases(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize(input), expected);
    }

    // ── load_map ──────────────────────────────────────────────────────────────

    #[test]
    fn load_returns_empty_when_file_missing() {
        let (_dir, path) = temp_hosts();
        assert!(load_map(&path).unwrap().is_empty());
    }

    #[test]
    fn load_v1_plain_strings_as_kasa() {
        let (_dir, path) = temp_hosts();
        std::fs::write(&path, r#"{"office bulb": "192.168.4.65"}"#).unwrap();

        let map = load_map(&path).unwrap();
        assert_eq!(map["office bulb"].ip, "192.168.4.65");
        assert_eq!(map["office bulb"].protocol, Protocol::Kasa);
    }

    // ── save_map + load_map round-trip ────────────────────────────────────────

    #[test]
    fn save_and_load_preserves_kasa_and_klap_entries() {
        let (_dir, path) = temp_hosts();
        let mut map = BTreeMap::new();
        map.insert("floor lamp".to_string(), entry("192.168.1.10", Protocol::Kasa));
        map.insert("tapo plug".to_string(),  entry("192.168.7.254", Protocol::Klap));
        save_map(&path, &map).unwrap();

        let loaded = load_map(&path).unwrap();
        assert_eq!(loaded["floor lamp"].protocol, Protocol::Kasa);
        assert_eq!(loaded["tapo plug"].protocol,  Protocol::Klap);
        assert_eq!(loaded["tapo plug"].ip, "192.168.7.254");
    }

    #[test]
    fn saved_file_is_pretty_printed_json() {
        let (_dir, path) = temp_hosts();
        let mut map = BTreeMap::new();
        map.insert("desk lamp".to_string(), entry("10.0.0.5", Protocol::Kasa));
        save_map(&path, &map).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        // Pretty-printed JSON has newlines
        assert!(raw.contains('\n'));
        assert!(serde_json::from_str::<serde_json::Value>(&raw).is_ok());
    }

    // ── lookup (via load_map directly, no real config dir involved) ───────────

    #[test]
    fn exact_match_returns_entry() {
        let (_dir, path) = temp_hosts();
        let mut map = BTreeMap::new();
        map.insert("floor lamp".to_string(), entry("10.0.0.1", Protocol::Kasa));
        save_map(&path, &map).unwrap();

        let loaded = load_map(&path).unwrap();
        let needle = normalize("floor lamp");
        let found = loaded.iter().find(|(k, _)| normalize(k) == needle);
        assert!(found.is_some());
    }

    #[test]
    fn substring_match_is_unambiguous_when_only_one_entry_matches() {
        let (_dir, path) = temp_hosts();
        let mut map = BTreeMap::new();
        map.insert("floor lamp".to_string(),  entry("10.0.0.1", Protocol::Kasa));
        map.insert("ceiling fan".to_string(), entry("10.0.0.2", Protocol::Kasa));
        save_map(&path, &map).unwrap();

        let loaded = load_map(&path).unwrap();
        let needle = normalize("floor");
        let hits: Vec<_> = loaded.keys().filter(|k| normalize(k).contains(&needle)).collect();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn ambiguous_substring_matches_multiple_entries() {
        let (_dir, path) = temp_hosts();
        let mut map = BTreeMap::new();
        map.insert("floor lamp".to_string(), entry("10.0.0.1", Protocol::Kasa));
        map.insert("desk lamp".to_string(),  entry("10.0.0.2", Protocol::Kasa));
        save_map(&path, &map).unwrap();

        let loaded = load_map(&path).unwrap();
        let needle = normalize("lamp");
        let hits: Vec<_> = loaded.keys().filter(|k| normalize(k).contains(&needle)).collect();
        // Both match "lamp" — lookup should return None in this case
        assert_eq!(hits.len(), 2);
    }
}

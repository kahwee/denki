//! Persistent device alias registry — ~/.config/denki/hosts.json
//!
//! Maps friendly names to IP addresses so users don't need to remember IPs.
//! Supports both legacy Kasa (XOR) and Tapo (KLAP) devices.
//!
//! File format: {"living room lamp": "192.168.7.254", ...}

use anyhow::Result;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn hosts_path() -> PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()))
        .join("denki");
    base.join("hosts.json")
}

fn load_map() -> Result<BTreeMap<String, String>> {
    let path = hosts_path();
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

fn save_map(map: &BTreeMap<String, String>) -> Result<()> {
    let path = hosts_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(map)?)?;
    Ok(())
}

/// Normalize a name for comparison: lowercase, collapse whitespace.
pub fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Look up a name in the hosts file. Returns the IP if found.
pub fn lookup(name: &str) -> Option<String> {
    let map = load_map().ok()?;
    let needle = normalize(name);
    // exact match first
    for (k, v) in &map {
        if normalize(k) == needle {
            return Some(v.clone());
        }
    }
    // prefix/substring match
    let hits: Vec<_> = map.iter()
        .filter(|(k, _)| normalize(k).contains(&needle))
        .collect();
    if hits.len() == 1 {
        return Some(hits[0].1.clone());
    }
    None
}

/// Save (or overwrite) a name→IP alias.
pub fn set(name: &str, ip: &str) -> Result<()> {
    let mut map = load_map()?;
    map.insert(name.to_string(), ip.to_string());
    save_map(&map)
}

/// Remove an alias by name.
pub fn remove(name: &str) -> Result<bool> {
    let mut map = load_map()?;
    let removed = map.remove(name).is_some();
    if removed {
        save_map(&map)?;
    }
    Ok(removed)
}

/// List all saved aliases as (name, ip) pairs, sorted by name.
pub fn list() -> Result<Vec<(String, String)>> {
    let map = load_map()?;
    Ok(map.into_iter().collect())
}

/// Return the path to the hosts file for display purposes.
pub fn path_display() -> String {
    hosts_path().display().to_string()
}

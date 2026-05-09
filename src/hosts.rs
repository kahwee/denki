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
use std::path::PathBuf;

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

fn load_map() -> Result<BTreeMap<String, HostEntry>> {
    let path = hosts_path();
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let data = std::fs::read_to_string(&path)?;

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

fn save_map(map: &BTreeMap<String, HostEntry>) -> Result<()> {
    let path = hosts_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(map)?)?;
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
    let map = load_map().ok()?;
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
    let mut map = load_map()?;
    map.insert(name.to_string(), HostEntry { ip: ip.to_string(), protocol });
    save_map(&map)
}

/// Remove an alias by exact name. Returns true if it existed.
pub fn remove(name: &str) -> Result<bool> {
    let mut map = load_map()?;
    let removed = map.remove(name).is_some();
    if removed {
        save_map(&map)?;
    }
    Ok(removed)
}

/// List all saved aliases sorted by name.
pub fn list() -> Result<Vec<(String, HostEntry)>> {
    Ok(load_map()?.into_iter().collect())
}

pub fn path_display() -> String {
    hosts_path().display().to_string()
}

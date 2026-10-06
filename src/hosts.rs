//! Device alias registry — ~/.config/denki/hosts.json
//!
//! v2 format: {"floor lamp": {"ip": "192.168.7.254", "protocol": "klap"}, ...}
//! v1 compat: plain string values are read as Kasa protocol.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::IpAddr;
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostEntry {
    pub ip: String,
    pub protocol: Protocol,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
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

    let map = match serde_json::from_str::<BTreeMap<String, HostEntry>>(&data) {
        Ok(map) => map,
        Err(v2_err) => match serde_json::from_str::<BTreeMap<String, String>>(&data) {
            Ok(v1) => v1
                .into_iter()
                .map(|(k, ip)| {
                    (
                        k,
                        HostEntry {
                            ip,
                            protocol: Protocol::Kasa,
                            device_id: None,
                        },
                    )
                })
                .collect(),
            Err(v1_err) => {
                anyhow::bail!(format_map_parse_error(
                    path.display().to_string(),
                    v2_err,
                    v1_err
                ))
            }
        },
    };
    Ok(map)
}

fn format_map_parse_error(
    path: String,
    v2_error: serde_json::Error,
    v1_error: serde_json::Error,
) -> String {
    format!(
        "{} is malformed and cannot be loaded.\n\
         Expected either:\n\
         - v2: {{\"alias\": {{\"ip\":\"...\",\"protocol\":\"kasa|klap\"}}, ...}}\n\
         - v1: {{\"alias\": \"ip\", ...}}\n\
         Parse details:\n\
         - v2 parse: {v2_error}\n\
         - v1 parse: {v1_error}",
        path
    )
}

fn normalize_alias_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Alias name cannot be empty");
    }
    Ok(trimmed.to_string())
}

fn validate_alias_ip(ip: &str) -> Result<()> {
    ip.parse::<IpAddr>()
        .map(|_| ())
        .map_err(|_| anyhow::anyhow!("Invalid IP address for alias: \"{ip}\""))
}

fn find_normalized_alias_collision(
    map: &BTreeMap<String, HostEntry>,
    name: &str,
) -> Option<String> {
    let incoming = normalize(name);
    map.iter()
        .find(|(existing, _)| normalize(existing) == incoming && existing.as_str() != name)
        .map(|(existing, _)| existing.clone())
}

fn save_map(path: &Path, map: &BTreeMap<String, HostEntry>) -> Result<()> {
    crate::storage::atomic_write(path, &serde_json::to_vec_pretty(map)?)
}

pub fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Exact match first, then substring. Returns Err if the query is ambiguous.
pub fn lookup(name: &str) -> anyhow::Result<Option<HostEntry>> {
    let map = load_map(&hosts_path())?;
    lookup_in(name, &map)
}

/// Exact-match or substring matches for group operations.
pub fn lookup_many(name: &str) -> anyhow::Result<Vec<(String, HostEntry)>> {
    let map = load_map(&hosts_path())?;
    lookup_many_in(name, &map)
}

fn lookup_many_in(
    name: &str,
    map: &std::collections::BTreeMap<String, HostEntry>,
) -> anyhow::Result<Vec<(String, HostEntry)>> {
    let needle = normalize(name);
    if needle.is_empty() {
        return Ok(vec![]);
    }

    let exact: Vec<(String, HostEntry)> = map
        .iter()
        .filter(|(alias, _)| normalize(alias) == needle)
        .map(|(alias, entry)| (alias.to_string(), entry.clone()))
        .collect();
    if !exact.is_empty() {
        return Ok(exact);
    }

    Ok(map
        .iter()
        .filter(|(alias, _)| normalize(alias).contains(&needle))
        .map(|(alias, entry)| (alias.to_string(), entry.clone()))
        .collect())
}

fn lookup_in(
    name: &str,
    map: &std::collections::BTreeMap<String, HostEntry>,
) -> anyhow::Result<Option<HostEntry>> {
    let needle = normalize(name);

    for (k, v) in map {
        if normalize(k) == needle {
            return Ok(Some(v.clone()));
        }
    }

    if needle.is_empty() {
        return Ok(None);
    }

    let hits: Vec<(&String, &HostEntry)> = map
        .iter()
        .filter(|(k, _)| normalize(k).contains(&needle))
        .collect();

    match hits.len() {
        0 => Ok(None),
        1 => Ok(Some(hits[0].1.clone())),
        _ => {
            let names: Vec<&str> = hits.iter().map(|(k, _)| k.as_str()).collect();
            anyhow::bail!("\"{}\" is ambiguous; matches: {}", name, names.join(", "))
        }
    }
}

pub fn set(name: &str, ip: &str, protocol: Protocol) -> Result<()> {
    let name = normalize_alias_name(name)?;
    validate_alias_ip(ip)?;
    update_map(&hosts_path(), |map| {
        if let Some(existing) = find_normalized_alias_collision(map, &name) {
            anyhow::bail!(
                "Alias \"{name}\" is too similar to existing alias \"{existing}\". Use a more specific name or remove the existing alias first."
            );
        }
        map.insert(
            name.to_string(),
            HostEntry {
                ip: ip.to_string(),
                protocol,
                device_id: None,
            },
        );
        Ok(())
    })
}

pub fn remove(name: &str) -> Result<bool> {
    update_map(&hosts_path(), |map| Ok(map.remove(name).is_some()))
}

/// Serialize read-modify-write against other processes. Readers see whole snapshots.
fn update_map<T>(
    path: &Path,
    update: impl FnOnce(&mut BTreeMap<String, HostEntry>) -> Result<T>,
) -> Result<T> {
    let _lock = crate::storage::FileLock::acquire(path)?;
    let mut map = load_map(path)?;
    let previous = map.clone();
    let result = update(&mut map)?;
    if map != previous {
        save_map(path, &map)?;
    }
    Ok(result)
}

pub fn list() -> Result<Vec<(String, HostEntry)>> {
    Ok(load_map(&hosts_path())?.into_iter().collect())
}

pub fn path_display() -> String {
    hosts_path().display().to_string()
}

/// Load the full host map from disk.
pub fn load() -> Result<std::collections::BTreeMap<String, HostEntry>> {
    load_map(&hosts_path())
}

/// Explicitly replace the full registry under a lock. For a previously loaded
/// snapshot use `save_if_unchanged` to reject concurrent edits instead.
pub fn save(map: &std::collections::BTreeMap<String, HostEntry>) -> Result<()> {
    let path = hosts_path();
    let _lock = crate::storage::FileLock::acquire(&path)?;
    save_map(&path, map)
}

/// Commit a scan snapshot without holding a lock during network discovery.
/// Concurrent edits are preserved; the caller must rerun discovery on conflict.
pub fn save_if_unchanged(
    previous: &BTreeMap<String, HostEntry>,
    map: &BTreeMap<String, HostEntry>,
) -> Result<()> {
    save_if_unchanged_at(&hosts_path(), previous, map)
}

fn save_if_unchanged_at(
    path: &Path,
    previous: &BTreeMap<String, HostEntry>,
    map: &BTreeMap<String, HostEntry>,
) -> Result<()> {
    update_map(path, |current| {
        if current != previous {
            return Err(crate::error::error(
                "registry_conflict",
                "Aliases changed during discovery; no scan updates were saved. Run denki scan again.",
            ));
        }
        *current = map.clone();
        Ok(())
    })
}

/// Insert a new Kasa alias only when neither its IP nor normalized name is saved.
/// Returns false for blank names or collisions, preserving existing aliases.
pub fn save_if_new_in(
    name: &str,
    ip: &str,
    map: &mut std::collections::BTreeMap<String, HostEntry>,
) -> bool {
    let name = name.trim();
    let normalized = normalize(name);
    if normalized.is_empty()
        || map.values().any(|v| v.ip == ip)
        || map.keys().any(|existing| normalize(existing) == normalized)
    {
        return false;
    }
    map.insert(
        name.to_string(),
        HostEntry {
            ip: ip.to_string(),
            protocol: Protocol::Kasa,
            device_id: None,
        },
    );
    true
}

/// Reconcile an observed identity, never guessing from a colliding name.
pub fn reconcile_in(
    name: &str,
    ip: &str,
    protocol: Protocol,
    device_id: Option<&str>,
    map: &mut BTreeMap<String, HostEntry>,
) -> Result<bool> {
    let id = device_id.filter(|id| !id.trim().is_empty());
    if let Some(id) = id {
        let matches: Vec<String> = map
            .iter()
            .filter(|(_, e)| e.protocol == protocol && e.device_id.as_deref() == Some(id))
            .map(|(n, _)| n.clone())
            .collect();
        if !matches.is_empty() {
            let mut changed = false;
            for name in matches {
                let entry = map.get_mut(&name).unwrap();
                changed |= entry.ip != ip;
                entry.ip = ip.into();
            }
            return Ok(changed);
        }
    }
    for entry in map
        .values()
        .filter(|e| e.ip == ip && e.protocol == protocol)
    {
        if let Some(expected) = entry.device_id.as_deref()
            && id != Some(expected)
        {
            return Err(crate::error::error(
                "identity_mismatch",
                "Saved address now belongs to a different device or its identity is missing",
            ));
        }
    }
    let mut changed = false;
    let mut same_address = false;
    for entry in map
        .values_mut()
        .filter(|e| e.ip == ip && e.protocol == protocol)
    {
        same_address = true;
        if entry.device_id.is_none()
            && let Some(id) = id
        {
            entry.device_id = Some(id.into());
            changed = true;
        }
    }
    if same_address {
        return Ok(changed);
    }
    let normalized = normalize(name);
    if normalized.is_empty() || map.keys().any(|n| normalize(n) == normalized) {
        return Ok(false);
    }
    map.insert(
        name.trim().into(),
        HostEntry {
            ip: ip.into(),
            protocol,
            device_id: id.map(str::to_owned),
        },
    );
    Ok(true)
}

/// Check stored identities before using an address. Legacy entries bind on scan.
pub fn verify_identity(ip: &str, protocol: Protocol, actual: Option<&str>) -> Result<()> {
    let map = load()?;
    for entry in map
        .values()
        .filter(|e| e.ip == ip && e.protocol == protocol)
    {
        if let Some(expected) = &entry.device_id
            && actual != Some(expected.as_str())
        {
            return Err(crate::error::error(
                "identity_mismatch",
                "Device identity does not match the saved alias. Run denki scan to reconcile addresses.",
            ));
        }
    }
    Ok(())
}

/// Return the alias name for a given IP, if one exists in `map`.
pub fn lookup_by_ip_in(
    ip: &str,
    map: &std::collections::BTreeMap<String, HostEntry>,
) -> Option<String> {
    map.iter().find(|(_, v)| v.ip == ip).map(|(k, _)| k.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tempfile::TempDir;

    fn temp_hosts() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hosts.json");
        (dir, path)
    }

    fn entry(ip: &str, protocol: Protocol) -> HostEntry {
        HostEntry {
            ip: ip.to_string(),
            protocol,
            device_id: None,
        }
    }

    #[rstest]
    #[case("Office Bulb", "office bulb")]
    #[case("Coat-Rack Lights", "coat rack lights")]
    #[case("  MULTIPLE   SPACES  ", "multiple spaces")]
    #[case("123Abc!@#", "123abc")]
    #[case("", "")]
    fn normalize_cases(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize(input), expected);
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let (_dir, path) = temp_hosts();
        assert!(load_map(&path).unwrap().is_empty());
    }

    #[test]
    fn load_errors_on_corrupt_json() {
        let (_dir, path) = temp_hosts();
        std::fs::write(&path, "this is not json at all").unwrap();
        let result = load_map(&path);
        assert!(result.is_err(), "corrupt file should return an error");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("malformed") || msg.contains("Invalid"),
            "error should mention malformed input: {msg}"
        );
    }

    #[test]
    fn load_errors_on_wrong_json_shape() {
        let (_dir, path) = temp_hosts();
        // Valid JSON but neither v2 objects nor v1 plain strings
        std::fs::write(&path, r#"{"key": 42}"#).unwrap();
        let result = load_map(&path);
        assert!(result.is_err(), "wrong-shaped JSON should return an error");
    }

    #[test]
    fn normalize_alias_name_trims_whitespace() {
        assert_eq!(normalize_alias_name("  Desk Lamp  ").unwrap(), "Desk Lamp");
    }

    #[test]
    fn normalize_alias_name_rejects_empty() {
        assert!(normalize_alias_name("   ").is_err());
    }

    #[test]
    fn normalize_alias_name_rejects_empty_string() {
        assert!(normalize_alias_name("").is_err());
    }

    #[test]
    fn find_normalized_alias_collision_detects_similarity() {
        let mut map = BTreeMap::new();
        map.insert("Desk Lamp".to_string(), entry("10.0.0.1", Protocol::Kasa));
        let hit = find_normalized_alias_collision(&map, "desk    lamp");
        assert_eq!(hit.as_deref(), Some("Desk Lamp"));
    }

    #[test]
    fn lookup_many_returns_exact_match_first() {
        let mut map = BTreeMap::new();
        map.insert(
            "Office Color 1".to_string(),
            entry("192.168.4.65", Protocol::Kasa),
        );
        map.insert("Office".to_string(), entry("192.168.4.12", Protocol::Kasa));
        map.insert(
            "Living Room Lamp".to_string(),
            entry("192.168.4.20", Protocol::Kasa),
        );

        let hits = lookup_many_in("Office", &map).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "Office");
    }

    #[test]
    fn lookup_many_returns_substring_matches() {
        let mut map = BTreeMap::new();
        map.insert(
            "Office Color 1".to_string(),
            entry("192.168.4.65", Protocol::Kasa),
        );
        map.insert(
            "Office Color 2".to_string(),
            entry("192.168.4.25", Protocol::Kasa),
        );
        map.insert(
            "Living Room Lamp".to_string(),
            entry("192.168.4.20", Protocol::Kasa),
        );

        let hits = lookup_many_in("office", &map).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().any(|(name, _)| name == "Office Color 1"));
        assert!(hits.iter().any(|(name, _)| name == "Office Color 2"));
    }

    #[test]
    fn validate_alias_ip_rejects_junk() {
        assert!(validate_alias_ip("not-an-ip").is_err());
    }

    #[test]
    fn validate_alias_ip_accepts_ipv4() {
        assert!(validate_alias_ip("192.168.1.42").is_ok());
    }

    #[test]
    fn load_v1_plain_strings_as_kasa() {
        let (_dir, path) = temp_hosts();
        std::fs::write(&path, r#"{"office bulb": "192.168.4.65"}"#).unwrap();

        let map = load_map(&path).unwrap();
        assert_eq!(map["office bulb"].ip, "192.168.4.65");
        assert_eq!(map["office bulb"].protocol, Protocol::Kasa);
    }

    #[test]
    fn save_and_load_preserves_kasa_and_klap_entries() {
        let (_dir, path) = temp_hosts();
        let mut map = BTreeMap::new();
        map.insert(
            "floor lamp".to_string(),
            entry("192.168.1.10", Protocol::Kasa),
        );
        map.insert(
            "tapo plug".to_string(),
            entry("192.168.7.254", Protocol::Klap),
        );
        save_map(&path, &map).unwrap();

        let loaded = load_map(&path).unwrap();
        assert_eq!(loaded["floor lamp"].protocol, Protocol::Kasa);
        assert_eq!(loaded["tapo plug"].protocol, Protocol::Klap);
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

    #[test]
    fn exact_match_returns_entry() {
        let mut map = BTreeMap::new();
        map.insert("floor lamp".to_string(), entry("10.0.0.1", Protocol::Kasa));
        let found = lookup_in("floor lamp", &map).unwrap();
        assert_eq!(found.unwrap().ip, "10.0.0.1");
    }

    #[test]
    fn ambiguous_match_returns_err_with_both_names() {
        let mut map = BTreeMap::new();
        map.insert("floor lamp".to_string(), entry("10.0.0.1", Protocol::Kasa));
        map.insert("desk lamp".to_string(), entry("10.0.0.2", Protocol::Kasa));

        let err = lookup_in("lamp", &map).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ambiguous"), "{msg}");
        assert!(msg.contains("matches:"), "{msg}");
        assert!(msg.contains("floor lamp"), "{msg}");
        assert!(msg.contains("desk lamp"), "{msg}");
    }

    #[test]
    fn unambiguous_substring_returns_entry() {
        let mut map = BTreeMap::new();
        map.insert("floor lamp".to_string(), entry("10.0.0.1", Protocol::Kasa));
        map.insert("ceiling fan".to_string(), entry("10.0.0.2", Protocol::Kasa));

        let found = lookup_in("floor", &map).unwrap();
        assert_eq!(found.unwrap().ip, "10.0.0.1");
    }

    #[test]
    fn unknown_name_returns_none() {
        let mut map = BTreeMap::new();
        map.insert("floor lamp".to_string(), entry("10.0.0.1", Protocol::Kasa));

        assert!(lookup_in("kitchen", &map).unwrap().is_none());
    }

    #[test]
    fn auto_save_adds_new_device() {
        let mut map = BTreeMap::new();
        let saved = save_if_new_in("Coat Rack", "192.168.7.203", &mut map);
        assert!(saved);
        assert_eq!(map["Coat Rack"].ip, "192.168.7.203");
    }

    #[test]
    fn auto_save_preserves_existing_alias() {
        let mut map = BTreeMap::new();
        map.insert("hummer".to_string(), entry("192.168.4.36", Protocol::Kasa));

        // Sysinfo reports "Hummer" but IP is already saved as "hummer" — skip.
        let saved = save_if_new_in("Hummer", "192.168.4.36", &mut map);
        assert!(!saved);
        assert!(map.contains_key("hummer"));
        assert!(!map.contains_key("Hummer"));
    }

    #[rstest]
    #[case("Desk Lamp")]
    #[case("desk-lamp")]
    #[case("  DESK   LAMP  ")]
    fn auto_save_preserves_colliding_alias(#[case] incoming: &str) {
        let mut map = BTreeMap::new();
        map.insert("Desk Lamp".into(), entry("192.0.2.1", Protocol::Klap));
        assert!(!save_if_new_in(incoming, "192.0.2.2", &mut map));
        assert_eq!(map.len(), 1);
        assert_eq!(map["Desk Lamp"].ip, "192.0.2.1");
        assert_eq!(map["Desk Lamp"].protocol, Protocol::Klap);
    }

    #[rstest]
    #[case("   ")]
    #[case("---")]
    fn auto_save_rejects_normalized_empty_names(#[case] name: &str) {
        let mut map = BTreeMap::new();
        assert!(!save_if_new_in(name, "192.0.2.1", &mut map));
        assert!(map.is_empty());
    }

    #[test]
    fn auto_save_skips_blank_sysinfo_name() {
        let mut map = BTreeMap::new();
        let saved = save_if_new_in("", "192.168.4.99", &mut map);
        assert!(!saved);
        assert!(map.is_empty());
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use serde_json::json;
    fn known(ip: &str, id: Option<&str>) -> HostEntry {
        HostEntry {
            ip: ip.into(),
            protocol: Protocol::Kasa,
            device_id: id.map(str::to_owned),
        }
    }
    #[test]
    fn identity_reconciles_changed_and_swapped_addresses_preserving_names() {
        let mut map = BTreeMap::from([
            ("custom a".into(), known("192.0.2.1", Some("a"))),
            ("custom b".into(), known("192.0.2.2", Some("b"))),
        ]);
        assert!(
            reconcile_in(
                "factory a",
                "192.0.2.2",
                Protocol::Kasa,
                Some("a"),
                &mut map
            )
            .unwrap()
        );
        assert!(
            reconcile_in(
                "factory b",
                "192.0.2.1",
                Protocol::Kasa,
                Some("b"),
                &mut map
            )
            .unwrap()
        );
        assert_eq!(map["custom a"].ip, "192.0.2.2");
        assert_eq!(map["custom b"].ip, "192.0.2.1");
        assert_eq!(map.len(), 2);
        assert!(
            !reconcile_in(
                "factory a",
                "192.0.2.2",
                Protocol::Kasa,
                Some("a"),
                &mut map
            )
            .unwrap()
        );
    }
    #[test]
    fn legacy_entries_learn_at_current_address_but_never_move_by_name() {
        let mut map = BTreeMap::from([("custom".into(), known("192.0.2.1", None))]);
        assert!(!reconcile_in("custom", "192.0.2.2", Protocol::Kasa, Some("a"), &mut map).unwrap());
        assert_eq!(map["custom"].ip, "192.0.2.1");
        assert!(reconcile_in("factory", "192.0.2.1", Protocol::Kasa, Some("a"), &mut map).unwrap());
        assert_eq!(map["custom"].device_id.as_deref(), Some("a"));
    }
    #[test]
    fn conflicting_or_missing_identity_never_changes_any_entry() {
        for actual in [Some("different"), None, Some("")] {
            let mut map = BTreeMap::from([
                ("unbound".into(), known("192.0.2.1", None)),
                ("bound".into(), known("192.0.2.1", Some("original"))),
            ]);
            let before = serde_json::to_value(&map).unwrap();
            assert!(
                reconcile_in("factory", "192.0.2.1", Protocol::Kasa, actual, &mut map).is_err()
            );
            assert_eq!(serde_json::to_value(&map).unwrap(), before);
        }
    }
    #[test]
    fn same_identity_on_different_protocol_cannot_move_alias() {
        let mut map = BTreeMap::from([("desk".into(), known("192.0.2.1", Some("a")))]);
        assert!(!reconcile_in("desk", "192.0.2.2", Protocol::Klap, Some("a"), &mut map).unwrap());
        assert_eq!(map["desk"].ip, "192.0.2.1");
    }
    #[test]
    fn identity_round_trips_and_old_v2_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        std::fs::write(
            &path,
            json!({"desk":{"ip":"192.0.2.1","protocol":"kasa"}}).to_string(),
        )
        .unwrap();
        let mut map = load_map(&path).unwrap();
        assert!(map["desk"].device_id.is_none());
        reconcile_in("factory", "192.0.2.1", Protocol::Kasa, Some("a"), &mut map).unwrap();
        save_map(&path, &map).unwrap();
        assert_eq!(
            load_map(&path).unwrap()["desk"].device_id.as_deref(),
            Some("a")
        );
    }
}

#[cfg(test)]
mod transaction_tests {
    use super::*;
    fn entry(ip: &str) -> HostEntry {
        HostEntry {
            ip: ip.into(),
            protocol: Protocol::Kasa,
            device_id: None,
        }
    }
    #[test]
    fn stale_scan_cannot_overwrite_an_added_changed_or_removed_alias() {
        for kind in ["added", "changed", "removed"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("hosts.json");
            let before = BTreeMap::from([("desk".into(), entry("192.0.2.1"))]);
            save_map(&path, &before).unwrap();
            let mut scanned = before.clone();
            scanned.get_mut("desk").unwrap().device_id = Some("synthetic-id".into());
            update_map(&path, |map| {
                match kind {
                    "added" => {
                        map.insert("other".into(), entry("192.0.2.2"));
                    }
                    "changed" => {
                        map.get_mut("desk").unwrap().ip = "192.0.2.3".into();
                    }
                    _ => {
                        map.remove("desk");
                    }
                }
                Ok(())
            })
            .unwrap();
            let current = std::fs::read(&path).unwrap();
            let error = save_if_unchanged_at(&path, &before, &scanned).unwrap_err();
            assert_eq!(crate::error::code(&error), "registry_conflict");
            assert_eq!(std::fs::read(&path).unwrap(), current);
        }
    }
    #[test]
    fn unchanged_snapshot_commits_and_failed_transaction_preserves_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let before = BTreeMap::new();
        let after = BTreeMap::from([("desk".into(), entry("192.0.2.1"))]);
        save_if_unchanged_at(&path, &before, &after).unwrap();
        assert_eq!(load_map(&path).unwrap(), after);
        let result: Result<()> = update_map(&path, |map| {
            map.clear();
            anyhow::bail!("validation failed")
        });
        assert!(result.is_err());
        assert_eq!(load_map(&path).unwrap(), after);
        // An error also releases the lock.
        update_map(&path, |map| {
            map.clear();
            Ok(())
        })
        .unwrap();
        assert!(load_map(&path).unwrap().is_empty());
    }
}

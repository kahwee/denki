// Device name/IP resolution and protocol guards for the CLI.

use crate::{hosts, strip};
use anyhow::{bail, Result};
use colored::Colorize;
use std::net::IpAddr;

#[derive(Debug)]
pub struct Resolved {
    pub ip: String,
    pub protocol: hosts::Protocol,
    pub saved_name: Option<String>,
}

fn not_found(input: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "No device named \"{input}\" was found in saved aliases.\n\
         \n\
         If you just ran `denki scan`, use the device IP directly:\n\
         \x20 denki <command> 192.168.x.x\n\
         \n\
         To save a friendly name for next time:\n\
         \x20 denki alias \"<name>\" <ip>\n\
         \n\
         To review saved names:\n\
         \x20 denki aliases"
    )
}

/// Resolve a name or IP, printing "Using alias…" if matched.
pub async fn resolve(input: &str) -> Result<Resolved> {
    let r = resolve_quiet(input).await?;
    if let Some(name) = &r.saved_name {
        println!("{}", format!("Using alias \"{name}\" [{}]", r.ip).dimmed());
    }
    Ok(r)
}

/// Like `resolve` but suppresses the "Using alias…" line.
/// Used by `info` so the detail header isn't preceded by a redundant name echo.
pub async fn resolve_quiet(input: &str) -> Result<Resolved> {
    if input.parse::<IpAddr>().is_ok() {
        return Ok(Resolved {
            ip: input.to_string(),
            protocol: hosts::Protocol::Kasa,
            saved_name: None,
        });
    }
    if let Some(entry) = hosts::lookup(input)? {
        return Ok(Resolved {
            ip: entry.ip,
            protocol: entry.protocol,
            saved_name: Some(input.to_string()),
        });
    }
    Err(not_found(input))
}

/// Resolve a 1-based outlet number to the matching StripChild.
pub fn resolve_outlet(s: &strip::Strip, outlet: u8) -> Result<&strip::StripChild> {
    let idx = (outlet - 1) as usize;
    s.children.get(idx).ok_or_else(|| {
        anyhow::anyhow!(
            "outlet {} does not exist (strip has {} outlets)",
            outlet,
            s.children.len()
        )
    })
}

/// Fail with a clear message if the resolved device is not a Kasa device.
/// KLAP (Tapo) devices don't support the Kasa XOR protocol.
pub fn require_kasa(r: &Resolved, cmd: &str) -> Result<()> {
    if r.protocol != hosts::Protocol::Kasa {
        bail!("`{cmd}` requires Kasa protocol — save the alias without --klap");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolve_raw_ip_returns_kasa_protocol() {
        let r = resolve("192.168.1.1").await.unwrap();
        assert_eq!(r.ip, "192.168.1.1");
        assert!(matches!(r.protocol, hosts::Protocol::Kasa));
        assert!(r.saved_name.is_none());
    }

    #[tokio::test]
    async fn resolve_unknown_name_error_mentions_ip_and_alias() {
        let err = resolve("ZZZ_no_such_device_99999").await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("192.168.x.x") || msg.contains("IP") || msg.contains("ip"),
            "error should mention using an IP: {msg}"
        );
        assert!(
            msg.contains("alias"),
            "error should mention saving an alias: {msg}"
        );
        assert!(
            msg.contains("denki aliases"),
            "error should suggest listing aliases: {msg}"
        );
    }

    #[tokio::test]
    async fn resolve_unknown_name_error_quotes_the_input() {
        let err = resolve("My Nonexistent Lamp").await.unwrap_err();
        assert!(
            err.to_string().contains("My Nonexistent Lamp"),
            "error should quote the unrecognized input: {err}"
        );
    }

    #[test]
    fn resolve_outlet_returns_child_at_1_based_index() {
        let strip = make_strip(3);
        let child = resolve_outlet(&strip, 1).unwrap();
        assert_eq!(child.alias, "Outlet 1");
        let child = resolve_outlet(&strip, 3).unwrap();
        assert_eq!(child.alias, "Outlet 3");
    }

    #[test]
    fn resolve_outlet_out_of_range_errors() {
        let strip = make_strip(2);
        let err = resolve_outlet(&strip, 3).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("outlet 3"), "{msg}");
        assert!(msg.contains("2 outlets"), "{msg}");
    }

    #[test]
    fn require_kasa_allows_kasa_protocol() {
        let r = kasa("1.2.3.4");
        assert!(require_kasa(&r, "energy").is_ok());
    }

    #[test]
    fn require_kasa_rejects_klap_protocol() {
        let r = klap("1.2.3.4");
        let err = require_kasa(&r, "energy").unwrap_err();
        assert!(err.to_string().contains("`energy`"), "{err}");
    }

    fn kasa(ip: &str) -> Resolved {
        Resolved {
            ip: ip.into(),
            protocol: hosts::Protocol::Kasa,
            saved_name: None,
        }
    }

    fn klap(ip: &str) -> Resolved {
        Resolved {
            ip: ip.into(),
            protocol: hosts::Protocol::Klap,
            saved_name: None,
        }
    }

    fn make_strip(n: usize) -> strip::Strip {
        use serde_json::json;
        let children: Vec<serde_json::Value> = (1..=n)
            .map(
                |i| json!({"id": format!("ABC{i:02}"), "state": 0, "alias": format!("Outlet {i}")}),
            )
            .collect();
        let json = json!({
            "system": { "get_sysinfo": {
                "alias": "Test Strip", "model": "HS300(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0", "rssi": -40, "feature": "TIM",
                "children": children
            }}
        });
        strip::parse(&json).unwrap()
    }
}

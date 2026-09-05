use anyhow::{Result, bail};
use colored::Colorize;

use crate::hosts;

fn format_alias_rows(list: &[(String, hosts::HostEntry)]) -> String {
    if list.is_empty() {
        return format!(
            "No saved aliases. Use `denki alias <name> <ip> [--klap]` to add one.\nFile: {}",
            hosts::path_display()
        );
    }

    let mut out = String::new();
    out.push_str(&format!(
        "{:<30} {:<18} {}\n",
        "Name".bold(),
        "IP".bold(),
        "Protocol".bold()
    ));
    out.push_str(&format!("{}\n", "─".repeat(58).dimmed()));
    for (name, entry) in list {
        out.push_str(&format!(
            "{:<30} {:<18} {}\n",
            name, entry.ip, entry.protocol
        ));
    }
    out.push_str(&format!(
        "({} aliases in {})",
        list.len(),
        hosts::path_display()
    ));
    out
}

pub fn handle_alias(name: &str, ip: &str, klap: bool) -> Result<()> {
    let protocol = if klap {
        hosts::Protocol::Klap
    } else {
        hosts::Protocol::Kasa
    };
    hosts::set(name, ip, protocol)?;
    let tag = if klap {
        " (klap)".dimmed()
    } else {
        "".normal()
    };
    println!("Saved: {} → {}{}", name.bold(), ip, tag);
    Ok(())
}

pub fn handle_unalias(name: &str) -> Result<()> {
    if hosts::remove(name)? {
        println!("Removed alias \"{name}\"");
    } else {
        bail!("No alias named \"{name}\" found");
    }
    Ok(())
}

pub fn handle_aliases() -> Result<()> {
    let list = hosts::list()?;
    println!("{}", format_alias_rows(&list));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(ip: &str, protocol: hosts::Protocol) -> hosts::HostEntry {
        hosts::HostEntry {
            ip: ip.to_string(),
            protocol,
        }
    }

    #[test]
    fn format_alias_rows_handles_empty_list() {
        let rendered = format_alias_rows(&[]);
        assert!(rendered.contains("No saved aliases"), "{rendered}");
        assert!(rendered.contains("denki alias"), "{rendered}");
    }

    #[test]
    fn format_alias_rows_includes_table_content() {
        let rendered = format_alias_rows(&[(
            "Lamp".to_string(),
            entry("192.168.1.10", hosts::Protocol::Kasa),
        )]);
        assert!(rendered.contains("Lamp"), "{rendered}");
        assert!(rendered.contains("192.168.1.10"), "{rendered}");
        assert!(rendered.contains("kasa"), "{rendered}");
    }
}

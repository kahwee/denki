use crate::bulb::LightingEffectState;
use colored::Colorize;

use crate::commands;
use crate::devices;
use crate::ops;

pub const BUILTIN_EFFECTS: &[&str] = &[
    "Off",
    "Aurora",
    "Bubbling Cauldron",
    "Candy Cane",
    "Christmas",
    "Flicker",
    "Hanukkah",
    "Haunted Mansion",
    "Icicle",
    "Lightning",
    "Ocean",
    "Rainbow",
    "Raindrop",
    "Spring",
    "Valentines",
];

pub fn resolve(name: &str) -> Option<&'static str> {
    let needle = crate::hosts::normalize(name);
    if needle.is_empty() {
        return None;
    }

    let mut exact = BUILTIN_EFFECTS
        .iter()
        .copied()
        .filter(|effect| crate::hosts::normalize(effect) == needle);
    if let Some(effect) = exact.next() {
        if exact.next().is_none() {
            return Some(effect);
        }
    }

    let mut fuzzy = BUILTIN_EFFECTS
        .iter()
        .copied()
        .filter(|effect| crate::hosts::normalize(effect).contains(&needle));
    if let Some(effect) = fuzzy.next() {
        if fuzzy.next().is_none() {
            return Some(effect);
        }
    }

    None
}

pub fn print_catalog(current: &LightingEffectState) {
    let state = if current.enable == 1 { "On" } else { "Off" };
    println!("Current effect: {} ({state})", current.name.bold());
    println!("{}", "Built-in effects:".bold());
    for effect in BUILTIN_EFFECTS {
        if effect.eq_ignore_ascii_case("off") {
            println!("  {effect}");
        } else if effect.eq_ignore_ascii_case(&current.name) {
            println!("  {effect}  {}", "(selected)".dimmed());
        } else {
            println!("  {effect}");
        }
    }
}

pub async fn handle_effects_command(host: &str) -> anyhow::Result<()> {
    let (r, _, kind) = commands::kasa_sysinfo(host, "effects").await?;
    devices::can_get_effects(&kind)?;
    print_catalog(&ops::lightstrip_current_effect(&r.ip).await?);
    Ok(())
}

pub async fn handle_effect_command(host: &str, name: &str) -> anyhow::Result<()> {
    let (r, _, kind) = commands::kasa_sysinfo(host, "effect").await?;
    devices::can_get_effects(&kind)?;

    if name.eq_ignore_ascii_case("off") {
        ops::lightstrip_disable_effect(&r.ip).await?;
        println!("Effect -> {}", "Off".dimmed());
        return Ok(());
    }

    let resolved = resolve(name).ok_or_else(|| {
        anyhow::anyhow!(
            "No light-strip effect named \"{name}\" found. Available effects: {}",
            BUILTIN_EFFECTS.join(", ")
        )
    })?;
    let current = ops::lightstrip_current_effect(&r.ip).await?;
    ops::lightstrip_set_effect(&r.ip, &current, resolved).await?;
    println!("Effect -> {}", resolved.bold());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_matches_exact_and_partial_names() {
        assert_eq!(resolve("Rainbow"), Some("Rainbow"));
        assert_eq!(resolve("bubbl"), Some("Bubbling Cauldron"));
        assert_eq!(resolve("Bubbling Cauldron"), Some("Bubbling Cauldron"));
    }

    #[test]
    fn resolve_rejects_unknown_or_ambiguous_names() {
        assert_eq!(resolve(""), None);
        assert_eq!(resolve("x"), None);
        assert_eq!(resolve("rain"), None);
        assert_eq!(resolve("ca"), None);
    }
}

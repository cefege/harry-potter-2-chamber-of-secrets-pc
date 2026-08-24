//! Console execution path and keybinding resolution.
//!
//! Scripted cues (`FlyTo`, camera moves, quit) reach the engine as console
//! commands through the same chain as bound keys:
//! key event → `[Engine.Input]` binding pipeline → alias expansion →
//! [`crate::sim::Engine::exec_console`].

use std::collections::HashMap;

use crate::input::{key_from_name, name_for_key};

/// Whether the exec chain recognized a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleOutcome {
    Handled,
    Unhandled,
}

/// Keybindings plus alias table from an `[Engine.Input]` config section.
pub struct Bindings {
    /// Alias name → command pipeline (`Aliases[N]=(Command=…,Alias=…)`).
    aliases: HashMap<String, String>,
}

impl Bindings {
    /// Collect `Aliases[…]` entries out of a parsed `[Engine.Input]`
    /// section. Per-key bindings stay in the config and resolve on demand
    /// so user overrides apply without re-parsing.
    pub fn parse(entries: &[&str]) -> Self {
        let mut aliases = HashMap::new();
        for entry in entries {
            let Some(open) = entry.find('(') else {
                continue;
            };
            let Some(close) = entry.rfind(')') else {
                continue;
            };
            let body = &entry[open + 1..close];
            let mut command = None;
            let mut alias = None;
            // Values may be quoted (`Alias="Fire"`) or bare (`Alias=Fire`);
            // the stock configs mix both spellings.
            fn unquote(field: &str, key: &str) -> Option<String> {
                let rest = field.trim().strip_prefix(key)?;
                let rest = rest.strip_prefix('"').unwrap_or(rest);
                let trimmed = rest.strip_suffix('"').unwrap_or(rest);
                Some(trimmed.to_string())
            }
            for part in body.split(',') {
                if let Some(value) = unquote(part, "Command=") {
                    command = Some(value);
                } else if let Some(value) = unquote(part, "Alias=") {
                    alias = Some(value);
                }
            }
            if let (Some(command), Some(alias)) = (command, alias) {
                aliases.insert(alias, command);
            }
        }
        Self { aliases }
    }

    /// Split one key's binding pipeline into segments.
    ///
    /// Mirrors `UInput::Process`: the `<Key>=<pipeline>` entry splits on
    /// `|`; each segment either names an alias or is a literal console
    /// command.
    pub fn pipeline_for<'a>(&'a self, raw: &'a str) -> Vec<&'a str> {
        raw.split('|')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .collect()
    }

    /// Expand an alias name to its command text.
    pub fn alias(&self, name: &str) -> Option<&str> {
        self.aliases.get(name).map(String::as_str)
    }
}

/// Expand one pipeline segment through the alias table; unaliased
/// segments pass through verbatim.
pub fn expand_segment(bindings: &Bindings, segment: &str) -> Vec<String> {
    match bindings.alias(segment) {
        Some(command) => bindings
            .pipeline_for(command)
            .into_iter()
            .map(str::to_string)
            .collect(),
        None => vec![segment.to_string()],
    }
}

/// The `[Engine.Input]` config key that carries this key's binding, when
/// the key has a spellable name (synthesized `IK_<n>` values never do).
pub fn binding_key_name(key: u8) -> Option<String> {
    if key == crate::input::key::NONE {
        return None;
    }
    let name = name_for_key(key);
    key_from_name(&name)?;
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALIASES: [&str; 3] = [
        "Aliases[0]=(Command=\"Button bFire | Fire\",Alias=Fire)",
        "Aliases[2]=(Command=\"Axis aBaseY  Speed=+300.0\",Alias=MoveForward)",
        "Aliases[8]=(Command=\"Jump | Axis aUp Speed=+300.0 | Button bBroomAction\",Alias=Jump)",
    ];

    #[test]
    fn parses_alias_entries() {
        let bindings = Bindings::parse(&ALIASES);
        assert_eq!(bindings.alias("Fire"), Some("Button bFire | Fire"));
        assert_eq!(
            bindings.alias("MoveForward"),
            Some("Axis aBaseY  Speed=+300.0")
        );
        assert_eq!(bindings.alias("Missing"), None);
    }

    #[test]
    fn pipelines_split_and_expand() {
        let bindings = Bindings::parse(&ALIASES);
        assert_eq!(
            bindings.pipeline_for("Jump | Button bExtra"),
            ["Jump", "Button bExtra"]
        );
        assert_eq!(
            expand_segment(&bindings, "MoveForward"),
            ["Axis aBaseY  Speed=+300.0".to_string()]
        );
        // Unaliased segments pass through.
        assert_eq!(expand_segment(&bindings, "FLUSH"), ["FLUSH".to_string()]);
    }

    #[test]
    fn key_names_round_trip() {
        assert_eq!(binding_key_name(75).as_deref(), Some("K"));
        assert_eq!(binding_key_name(17).as_deref(), Some("Ctrl"));
        assert_eq!(binding_key_name(236).as_deref(), Some("MouseWheelUp"));
        assert_eq!(binding_key_name(0), None);
    }
}

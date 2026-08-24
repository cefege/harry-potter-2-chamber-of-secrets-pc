//! INI text helpers over `hp-ini` for the launcher store's strict
//! full-value parsers (`Integer`, `Number`, `Boolean` from
//! `HP2LauncherStore.cpp`).
//!
//! The engine's typed readers differ from the launcher's: the launcher
//! requires whole-value parses and treats unknown spellings as absent so
//! callers fall back to model defaults. These helpers pin that contract.

pub use hp_ini::IniFile as IniDoc;

/// Parses an INI document applying every pinned hp-ini quirk.
pub fn parse_ini(bytes: &[u8]) -> IniDoc {
    IniDoc::parse(bytes)
}

/// Case-insensitive newest-wins raw lookup.
pub fn get<'a>(doc: &'a IniDoc, section: &str, key: &str) -> Option<&'a str> {
    doc.get(section, key)
}

/// `Boolean`: true/on/yes/1 vs false/off/no/0, case-insensitive; anything
/// else is unrecognized.
pub fn parse_boolean(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Some(true),
        "false" | "off" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// Lookup + [`parse_boolean`].
pub fn get_bool(doc: &IniDoc, section: &str, key: &str) -> Option<bool> {
    get(doc, section, key).and_then(parse_boolean)
}

/// `Integer`: optional sign then digits only, whole value consumed.
pub fn parse_integer(text: &str) -> Option<i32> {
    let text = text.trim();
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<i32>().ok()
}

/// Lookup + [`parse_number`].
pub fn get_number(doc: &IniDoc, section: &str, key: &str) -> Option<f64> {
    parse_number(get(doc, section, key)?)
}

/// Lookup + [`parse_integer`] with a range check.
pub fn get_integer_ranged(
    doc: &IniDoc,
    section: &str,
    key: &str,
    range: std::ops::RangeInclusive<i32>,
) -> Option<i32> {
    parse_integer(get(doc, section, key)?).filter(|value| range.contains(value))
}

/// Plain lookup + strict integer parse.
pub fn get_integer(doc: &IniDoc, section: &str, key: &str) -> Option<i32> {
    parse_integer(get(doc, section, key)?)
}

/// `Number`: C-locale whole-value float parse (accepts inf/nan spellings
/// like `strtod`; callers range-check).
pub fn parse_number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok()
}

/// Lookup + [`parse_number`] with an inclusive range check.
pub fn get_ranged_float(
    doc: &IniDoc,
    section: &str,
    key: &str,
    minimum: f64,
    maximum: f64,
) -> Option<f64> {
    parse_number(get(doc, section, key)?)
        .filter(|value| value.is_finite() && (minimum..=maximum).contains(value))
}

/// Lookup + discrete membership check; returns the accepted value itself.
pub fn get_discrete_float(doc: &IniDoc, section: &str, key: &str, accepted: &[f64]) -> Option<f64> {
    let parsed = parse_number(get(doc, section, key)?)?;
    accepted.iter().copied().find(|value| *value == parsed)
}

/// Lookup + strict integer + discrete membership check.
pub fn get_discrete_integer(
    doc: &IniDoc,
    section: &str,
    key: &str,
    accepted: &[i32],
) -> Option<i32> {
    let parsed = parse_integer(get(doc, section, key)?)?;
    accepted.iter().copied().find(|value| *value == parsed)
}

/// Canonical double rendering used by commits: shortest round-trip form
/// (`DoubleText`, e.g. `1`, `0.4`, `0.85`, `0.53`).
pub fn double_text(value: f64) -> String {
    format!("{value}")
}

/// `BoolText`: canonical persisted booleans.
pub fn bool_text(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_spellings_match_the_native_store() {
        assert_eq!(parse_boolean("True"), Some(true));
        assert_eq!(parse_boolean("on"), Some(true));
        assert_eq!(parse_boolean(" YES "), Some(true));
        assert_eq!(parse_boolean("No"), Some(false));
        assert_eq!(parse_boolean("OFF"), Some(false));
        assert_eq!(parse_boolean("perhaps"), None);
        assert_eq!(parse_boolean(""), None);
    }

    #[test]
    fn number_rejects_partial_and_garbage_parses() {
        assert_eq!(parse_number("60.000000"), Some(60.0));
        assert_eq!(parse_number("nan").map(f64::is_nan), Some(true));
        assert_eq!(parse_number("invalid"), None);
        assert_eq!(parse_number("12abc"), None);
        assert_eq!(parse_number("  0.75  "), Some(0.75));
    }

    #[test]
    fn integer_is_strict_and_signed() {
        assert_eq!(parse_integer("+42"), Some(42));
        assert_eq!(parse_integer("-7"), Some(-7));
        assert_eq!(parse_integer("12x"), None);
        assert_eq!(parse_integer(""), None);
        assert_eq!(parse_integer("99999999999"), None);
    }
}

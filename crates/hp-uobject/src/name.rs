//! M1 — interning name pool.
//!
//! Mirrors the observed UE1 `FName` behavior our packages rely on:
//! - lookups are case-insensitive;
//! - the first-seen spelling is preserved as the canonical text;
//! - a name splits into a base string plus an optional object number
//!   (`Trailing_3`), compared and interned by base text alone;
//! - indices are stable: seeding from a package's name table keeps every
//!   entry at its serialized index, and newly interned names append after
//!   the seeded range so re-serialization reproduces table order.

use std::collections::HashMap;

/// Index reserved for the null name (`NAME_None`), matching the engine
/// convention that name index 0 means "no name".
pub const NAME_NONE: u32 = 0;

/// A pooled-name reference: pool index plus object number, exactly the two
/// halves a serialized `FName` carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Name {
    pub index: u32,
    pub number: i32,
}

impl Name {
    pub const fn none() -> Self {
        Self {
            index: NAME_NONE,
            number: NO_NUMBER,
        }
    }

    pub fn is_none(&self) -> bool {
        self.index == NAME_NONE && self.number == NO_NUMBER
    }
}

/// Object number meaning "no number attached".
pub const NO_NUMBER: i32 = -1;

/// Case-folded key for a base name. Package names are Latin-1; only ASCII
/// letters fold (the same rule the stock tables were written under), other
/// bytes pass through untouched so non-ASCII names stay distinct.
pub(crate) fn fold_key(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_uppercase() { b + 32 } else { b })
        .map(|b| b as char)
        .collect()
}

/// Split a displayed name into its base text and object number.
///
/// A trailing `_N` counts as a number only when `N` is one or more digits
/// immediately after the final underscore and the base stays non-empty.
pub fn split_name_number(display: &str) -> (&str, i32) {
    if let Some((base, digits)) = display.rsplit_once('_')
        && !base.is_empty()
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
    {
        // Overflowing numbers keep the whole text as the base name.
        if let Ok(number) = digits.parse::<i32>() {
            return (base, number);
        }
    }
    (display, NO_NUMBER)
}

/// Join a base name and object number back into display form.
pub fn join_name_number(base: &str, number: i32) -> String {
    if number == NO_NUMBER {
        base.to_string()
    } else {
        format!("{base}_{number}")
    }
}

/// Interning pool with stable indices.
#[derive(Debug, Default, Clone)]
pub struct NamePool {
    /// Canonical (first-seen) texts by index.
    entries: Vec<String>,
    /// Folded base text -> index of the first-seen spelling.
    lookup: HashMap<String, u32>,
}

impl NamePool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed the pool from a package name table, preserving each entry's
    /// serialized index and spelling. Later seeds must continue the index
    /// sequence (multi-package loads share one pool); a gap or duplicate is
    /// rejected loudly rather than silently reordered.
    pub fn seed<I, S>(&mut self, texts: I) -> Result<(), SeedError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for text in texts {
            let text = text.as_ref();
            let expected = self.entries.len() as u32;
            match self.find_index(text) {
                Some(existing) => {
                    if existing != expected {
                        return Err(SeedError::IndexMismatch {
                            text: text.to_string(),
                            expected,
                            found: existing,
                        });
                    }
                }
                None => {
                    self.push(text);
                }
            }
        }
        Ok(())
    }

    fn push(&mut self, text: &str) -> u32 {
        let index = self.entries.len() as u32;
        self.entries.push(text.to_string());
        self.lookup.insert(fold_key(text), index);
        index
    }

    /// Intern a base name (case-insensitively), returning its index.
    pub fn intern(&mut self, base: &str) -> u32 {
        match self.find_index(base) {
            Some(index) => index,
            None => self.push(base),
        }
    }

    /// Intern a full display name, splitting off any `_N` number suffix.
    /// Returns `(index, number)` without storing the number.
    pub fn intern_split(&mut self, display: &str) -> (u32, i32) {
        let (base, number) = split_name_number(display);
        (self.intern(base), number)
    }

    /// Look up a base name's index without interning.
    pub fn find_index(&self, base: &str) -> Option<u32> {
        self.lookup.get(&fold_key(base)).copied()
    }

    /// Canonical text for an index.
    pub fn text(&self, index: u32) -> Option<&str> {
        self.entries.get(index as usize).map(String::as_str)
    }

    /// Display form of a pooled reference (base text plus `_N` suffix).
    pub fn display(&self, name: Name) -> Option<String> {
        self.text(name.index)
            .map(|base| join_name_number(base, name.number))
    }

    /// Number of pooled entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries in serialization order (canonical spellings).
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(String::as_str)
    }

    /// Case-insensitive equality of two references to this pool.
    pub fn equals(&self, a: Name, b: Name) -> bool {
        a.index == b.index
    }
}

/// Seeding violated the stable-index contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeedError {
    /// An already-pooled name appeared again at a different position.
    IndexMismatch {
        text: String,
        expected: u32,
        found: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_case_insensitive_and_keeps_first_case() {
        let mut pool = NamePool::new();
        let first = pool.intern("Health");
        assert_eq!(pool.intern("health"), first);
        assert_eq!(pool.intern("HEALTH"), first);
        assert_eq!(pool.text(first), Some("Health"));
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn distinct_names_do_not_collide() {
        let mut pool = NamePool::new();
        let a = pool.intern("Actor");
        let b = pool.intern("ACTORS");
        assert_ne!(a, b);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn non_ascii_names_stay_distinct() {
        let mut pool = NamePool::new();
        let a = pool.intern("Éclair");
        let b = pool.intern("éclair");
        // Only ASCII folds: these are distinct bases.
        assert_ne!(a, b);
    }

    #[test]
    fn name_number_split_round_trips() {
        assert_eq!(split_name_number("Harry_3"), ("Harry", 3));
        assert_eq!(split_name_number("Harry"), ("Harry", NO_NUMBER));
        assert_eq!(split_name_number("My_Actor_12"), ("My_Actor", 12));
        // Non-numeric tails stay part of the base.
        assert_eq!(split_name_number("Default__"), ("Default__", NO_NUMBER));
        assert_eq!(split_name_number("_5"), ("_5", NO_NUMBER));
        assert_eq!(
            split_name_number("Big_99999999999999"),
            ("Big_99999999999999", NO_NUMBER)
        );

        assert_eq!(join_name_number("Harry", 3), "Harry_3");
        assert_eq!(join_name_number("Harry", NO_NUMBER), "Harry");
    }

    #[test]
    fn intern_split_separates_base_and_number() {
        let mut pool = NamePool::new();
        let (idx, number) = pool.intern_split("Epilogue_7");
        let (idx2, number2) = pool.intern_split("epilogue_9");
        assert_eq!(idx, idx2, "same base interns once regardless of number");
        assert_eq!(number, 7);
        assert_eq!(number2, 9);
        assert_eq!(
            pool.display(Name { index: idx, number }),
            Some("Epilogue_7".to_string())
        );
    }

    #[test]
    fn seeded_indices_match_package_tables_and_appends_continue() {
        let mut pool = NamePool::new();
        pool.seed(["None", "ByteProperty", "ObjectProperty"])
            .unwrap();
        assert_eq!(pool.find_index("objectproperty"), Some(2));
        assert_eq!(pool.text(0), Some("None"));
        assert_eq!(pool.text(2), Some("ObjectProperty"));

        let appended = pool.intern("Engine");
        assert_eq!(appended, 3, "new names continue the seeded sequence");

        // Seeding is position-locked: any already-pooled name appearing at
        // a different position than the current append point errors loudly
        // instead of silently reordering the table.
        let err = pool.seed(["None", "Engine"]).unwrap_err();
        assert_eq!(
            err,
            SeedError::IndexMismatch {
                text: "None".to_string(),
                expected: 4,
                found: 0
            }
        );
    }

    #[test]
    fn iteration_preserves_serialization_order() {
        let mut pool = NamePool::new();
        pool.seed(["Alpha", "beta"]).unwrap();
        pool.intern("GAMMA");
        let ordered: Vec<&str> = pool.iter().collect();
        assert_eq!(ordered, vec!["Alpha", "beta", "GAMMA"]);
    }

    #[test]
    fn none_reference_behaves() {
        assert!(Name::none().is_none());
        let mut pool = NamePool::new();
        pool.seed(["None"]).unwrap();
        let none_idx = pool.find_index("NONE").unwrap();
        assert_eq!(none_idx, NAME_NONE);
        let named = Name {
            index: pool.intern("Harry"),
            number: NO_NUMBER,
        };
        assert!(!named.is_none());
    }
}

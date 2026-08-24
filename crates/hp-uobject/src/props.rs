//! The tagged-property bag: dynamic, name-keyed storage attached to every
//! instance object. Script code addresses properties by pool name, so the
//! store holds `(name index, value)` pairs — no fixed structs anywhere.

use crate::value::PropValue;

/// Dynamic property storage for one object.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropStore {
    entries: Vec<(u32, PropValue)>,
}

impl PropStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fetch by base-name pool index (names are interned case-insensitively,
    /// so index equality already compares without case).
    pub fn get(&self, name_index: u32) -> Option<&PropValue> {
        self.entries
            .iter()
            .find(|(n, _)| *n == name_index)
            .map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, name_index: u32) -> Option<&mut PropValue> {
        self.entries
            .iter_mut()
            .find(|(n, _)| *n == name_index)
            .map(|(_, v)| v)
    }

    /// Insert or replace; at most one entry per name.
    pub fn set(&mut self, name_index: u32, value: PropValue) {
        match self.get_mut(name_index) {
            Some(slot) => *slot = value,
            None => self.entries.push((name_index, value)),
        }
    }

    pub fn remove(&mut self, name_index: u32) -> Option<PropValue> {
        let pos = self.entries.iter().position(|(n, _)| *n == name_index)?;
        Some(self.entries.remove(pos).1)
    }

    /// Entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &PropValue)> + '_ {
        self.entries.iter().map(|(n, v)| (*n, v))
    }

    /// Entries sorted by name index — the deterministic order snapshots use.
    pub fn iter_sorted(&self) -> impl Iterator<Item = (u32, &PropValue)> + '_ {
        let mut indices: Vec<usize> = (0..self.entries.len()).collect();
        indices.sort_by_key(|&i| self.entries[i].0);
        indices
            .into_iter()
            .map(move |i| (self.entries[i].0, &self.entries[i].1))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

//! Reactive state: values that tell the UI when they changed.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Owns the revision counter every [`Signal`] created from it bumps on change.
#[derive(Clone, Default)]
pub struct Store {
    revision: Rc<Cell<u64>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increases whenever any signal of this store changes value.
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// Marks the UI stale without changing a signal (for example after external state changed).
    pub fn invalidate(&self) {
        self.revision.set(self.revision.get() + 1);
    }

    pub fn signal<T: PartialEq + Clone>(&self, value: T) -> Signal<T> {
        Signal {
            value: Rc::new(RefCell::new(value)),
            revision: self.revision.clone(),
        }
    }
}

/// A shared, observable value. Cloning shares the value.
pub struct Signal<T> {
    value: Rc<RefCell<T>>,
    revision: Rc<Cell<u64>>,
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            revision: self.revision.clone(),
        }
    }
}

impl<T: PartialEq + Clone> Signal<T> {
    pub fn get(&self) -> T {
        self.value.borrow().clone()
    }

    /// Stores `value`; bumps the revision only if it differs, so equal writes never cause a rebuild.
    pub fn set(&self, value: T) {
        let changed = *self.value.borrow() != value;
        if changed {
            *self.value.borrow_mut() = value;
            self.revision.set(self.revision.get() + 1);
        }
    }

    pub fn update(&self, change: impl FnOnce(&mut T)) {
        let mut next = self.get();
        change(&mut next);
        self.set(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_changes_bump_the_revision() {
        let store = Store::new();
        let count = store.signal(1);
        assert_eq!(store.revision(), 0);
        count.set(1);
        assert_eq!(store.revision(), 0);
        count.set(2);
        assert_eq!(store.revision(), 1);
        count.update(|n| *n += 1);
        assert_eq!((count.get(), store.revision()), (3, 2));
    }

    #[test]
    fn clones_share_the_value() {
        let store = Store::new();
        let a = store.signal(String::from("x"));
        let b = a.clone();
        b.set("y".into());
        assert_eq!(a.get(), "y");
    }
}

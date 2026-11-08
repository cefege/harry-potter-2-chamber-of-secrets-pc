//! Engine-owned keyboard overlay. Content is supplied independently of settings policy.
//!
//! A provider publishes a snapshot with stable action IDs using `set_content` and
//! drains activations with `take_action`. It owns all setting reads, writes and
//! persistence. The built-in snapshot makes the unwired state explicit.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action { pub id: String, pub label: String, pub enabled: bool }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content { pub title: String, pub status: Vec<String>, pub actions: Vec<Action> }
impl Default for Content {
    fn default() -> Self { Self { title: "Settings".into(), status: vec!["Settings are not connected yet.".into(), "F10 or Escape: close".into()], actions: vec![] } }
}
#[derive(Default)]
struct Menu { open: bool, toggle_down: bool, selected: usize, content: Content, pending: VecDeque<String>, forwarded: HashSet<i32>, owned: HashSet<i32>, orphaned: HashSet<i32> }
impl Menu {
    fn event(&mut self, key: i32, state: i32) -> bool {
        const PRESS: i32 = 1; const RELEASE: i32 = 3;
        if state == PRESS && self.orphaned.remove(&key) { self.owned.remove(&key); }
        if state == RELEASE { self.orphaned.remove(&key); }
        if key == 121 {
            if state == RELEASE { self.toggle_down = false; self.owned.remove(&key); return true; }
            if state == PRESS && !self.toggle_down { self.toggle_down = true; self.open = !self.open; self.owned.insert(key); }
            return true;
        }
        if state == RELEASE {
            if self.forwarded.remove(&key) { return false; }
            if self.owned.remove(&key) { return true; }
            // Unmatched releases must not reach release-bound gameplay actions.
            return true;
        }
        if state == PRESS {
            if self.forwarded.contains(&key) { return self.open; }
            if self.owned.contains(&key) { return true; }
            if !self.open { self.forwarded.insert(key); return false; }
            self.owned.insert(key);
        } else if !self.open { return false; }
        if state == PRESS {
            match key {
                27 => self.open = false,
                38 if self.selected > 0 => self.selected -= 1,
                40 if self.selected + 1 < self.content.actions.len() => self.selected += 1,
                13 => if let Some(a) = self.content.actions.get(self.selected) { if a.enabled { self.pending.push_back(a.id.clone()); } },
                _ => {}
            }
        }
        true
    }
}
fn menus() -> &'static Mutex<HashMap<usize, Menu>> { static MENUS: OnceLock<Mutex<HashMap<usize, Menu>>> = OnceLock::new(); MENUS.get_or_init(|| Mutex::new(HashMap::new())) }
/// `viewport` is an opaque live viewport identity; providers must not dereference it.
pub fn set_content(viewport: usize, content: Content) {
    let mut menus = menus().lock().unwrap_or_else(|e| e.into_inner());
    let menu = menus.entry(viewport).or_default();
    if menu.content.title != content.title { menu.selected = 0; }
    menu.selected = menu.selected.min(content.actions.len().saturating_sub(1));
    menu.content = content;
}
/// Drain one activation. Providers should drain on the engine thread.
pub fn take_action(viewport: usize) -> Option<String> { menus().lock().unwrap_or_else(|e| e.into_inner()).get_mut(&viewport)?.pending.pop_front() }
pub(crate) fn event(viewport: usize, key: i32, state: i32) -> bool { menus().lock().unwrap_or_else(|e| e.into_inner()).entry(viewport).or_default().event(key, state) }
pub(crate) fn is_open(viewport: usize) -> bool { menus().lock().unwrap_or_else(|e| e.into_inner()).get(&viewport).is_some_and(|m| m.open) }
pub(crate) fn focus_lost(viewport: usize) { if let Some(m) = menus().lock().unwrap_or_else(|e| e.into_inner()).get_mut(&viewport) { m.open = false; m.toggle_down = false; m.orphaned.extend(m.owned.iter().copied()); } }
/// Forget all state before the viewport address can be reused.
pub(crate) fn destroy_viewport(viewport: usize) { menus().lock().unwrap_or_else(|e| e.into_inner()).remove(&viewport); }
pub(crate) fn snapshot(viewport: usize) -> Option<(Content, usize)> { let menus = menus().lock().unwrap_or_else(|e| e.into_inner()); let m = menus.get(&viewport)?; m.open.then(|| (m.content.clone(), m.selected)) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn toggle_repeat_release_and_focus() {
        let mut m = Menu::default(); assert!(m.event(121, 1)); assert!(m.open);
        m.event(121, 1); assert!(m.open); assert!(m.event(87, 3));
        assert!(m.event(87, 1)); m.event(121, 3); m.event(27, 1); assert!(!m.open);
    }
    #[test] fn focus_loss_closes_and_resets_toggle() {
        let vp = usize::MAX - 12;
        assert!(event(vp, 121, 1)); assert!(is_open(vp));
        focus_lost(vp); assert!(!is_open(vp));
        assert!(event(vp, 121, 3));
        assert!(event(vp, 121, 1)); assert!(is_open(vp));
        menus().lock().unwrap().remove(&vp);
    }
    #[test] fn destroyed_viewport_address_reuse_starts_fresh() {
        let vp = usize::MAX - 13;
        set_content(vp, Content { title: "Old".into(), status: vec![], actions: vec![Action { id: "old".into(), label: "Old action".into(), enabled: true }] });
        assert!(event(vp, 121, 1));
        assert!(event(vp, 13, 1));
        destroy_viewport(vp);
        assert!(!is_open(vp));
        assert_eq!(take_action(vp), None);
        assert!(event(vp, 121, 1)); // A new viewport at the same address.
        let (content, selected) = snapshot(vp).unwrap();
        assert_eq!(content, Content::default());
        assert_eq!(selected, 0);
        assert_eq!(take_action(vp), None);
        destroy_viewport(vp);
    }
    #[test] fn releases_follow_press_provenance_across_close() {
        let mut m = Menu::default();
        assert!(!m.event(87, 1)); // Held before opening reached gameplay.
        assert!(m.event(121, 1)); assert!(m.event(121, 3));
        assert!(m.event(87, 1)); // Repeat does not change ownership.
        assert!(!m.event(87, 3));
        assert!(m.event(65, 1)); // Press belongs to overlay.
        assert!(m.event(27, 1)); assert!(!m.open);
        assert!(m.event(65, 3)); assert!(m.event(27, 3));
        assert!(!m.event(65, 1)); assert!(!m.event(65, 3));
        assert!(m.event(65, 3)); // Unmatched releases are suppressed.
    }
    #[test] fn focus_loss_preserves_releases_and_resets_orphaned_presses() {
        let vp = usize::MAX - 14;
        assert!(!event(vp, 87, 1)); assert!(event(vp, 121, 1)); assert!(event(vp, 65, 1));
        focus_lost(vp);
        assert!(!event(vp, 87, 3)); assert!(event(vp, 65, 3)); assert!(event(vp, 121, 3));
        assert!(event(vp, 121, 1)); assert!(event(vp, 66, 1)); focus_lost(vp);
        // A lost release must not make the next physical press disappear.
        assert!(!event(vp, 66, 1)); assert!(!event(vp, 66, 3));
        destroy_viewport(vp);
    }
    #[test] fn actions_use_ids_and_disabled_actions_do_not_fire() {
        let mut m = Menu::default(); m.content.actions = vec![Action { id: "audio".into(), label: "Audio".into(), enabled: true }, Action { id: "video".into(), label: "Video".into(), enabled: false }];
        m.event(121, 1); m.event(13, 1); m.event(40, 1); m.event(13, 1);
        assert_eq!(m.pending.pop_front().as_deref(), Some("audio")); assert!(m.pending.is_empty());
    }
}

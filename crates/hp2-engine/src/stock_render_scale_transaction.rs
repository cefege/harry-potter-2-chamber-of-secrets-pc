//! Transaction logic for the stock render-scale control.
//!
//! The adapter must initialize `committed` from the active renderer, not a
//! stale config value. It must restore the control to `visible` on failure.
//! Renderer preflight precedes config publication, and a failed publication
//! triggers an explicit renderer rollback.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scale(u8);

// Order matches the launcher choices and the stock slider's left-to-right positions.
// Above 100% the world is supersampled.
const SLIDER_SCALES: [u8; 16] = [50, 55, 60, 65, 67, 70, 75, 80, 85, 90, 95, 100, 125, 150, 175, 200];

impl Scale {
    pub(crate) fn new(percent: u8) -> Option<Self> {
        ((percent >= 50 && percent <= 100 && percent % 5 == 0) || percent == 67 || matches!(percent, 125 | 150 | 175 | 200)).then_some(Self(percent))
    }
    pub(crate) fn percent(self) -> u8 { self.0 }
    pub(crate) fn config_value(self) -> f64 { f64::from(self.0) / 100.0 }
    /// Reject malformed script values instead of silently persisting one.
    /// The stock slider's `Value` is the zero-based position in `SLIDER_SCALES`.
    pub(crate) fn from_slider(value: f32) -> Option<Self> {
        if !value.is_finite() || value.fract() != 0.0 || !(0.0..=15.0).contains(&value) {
            return None;
        }
        Some(Self(SLIDER_SCALES[value as usize]))
    }

    /// Slider position for this scale; the inverse of [`Scale::from_slider`].
    pub(crate) fn slider_value(self) -> f32 {
        SLIDER_SCALES.iter().position(|&percent| percent == self.0)
            .expect("every valid render scale has a slider position") as f32
    }
}

pub(crate) trait Backend {
    type Error;
    fn set_live(&mut self, scale: Scale) -> Result<(), Self::Error>;
    fn persist(&mut self, scale: Scale) -> Result<(), Self::Error>;
    /// Publish the successfully persisted preference to the renderer's cache.
    fn commit(&mut self) -> Result<(), Self::Error> { Ok(()) }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ChangeError<E> {
    /// GPU preflight failed. No configuration was written.
    Live(E),
    /// Configuration failed, and the prior live value was restored.
    Persist(E),
    /// Configuration failed and rollback also failed. The live value remains
    /// the requested value; the caller must report the divergence explicitly.
    Rollback { persist: E, rollback: E },
    /// The setting was saved, but the renderer preference cache was not updated.
    Commit(E),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Session {
    committed: Scale,
    visible: Scale,
    pending_commit: bool,
}
impl Session {
    pub(crate) fn new(initial: Scale) -> Self { Self::with_fallback(initial, initial) }
    pub(crate) fn with_fallback(committed: Scale, visible: Scale) -> Self { Self { committed, visible, pending_commit: false } }
    pub(crate) fn visible(&self) -> Scale { self.visible }
    pub(crate) fn committed(&self) -> Scale { self.committed }
    pub(crate) fn pending_commit(&self) -> bool { self.pending_commit }
    pub(crate) fn change<B: Backend>(&mut self, backend: &mut B, next: Scale) -> Result<(), ChangeError<B::Error>> {
        if next == self.committed && next == self.visible {
            if self.pending_commit { backend.commit().map_err(ChangeError::Commit)?; self.pending_commit = false; }
            return Ok(());
        }
        let previous = self.visible;
        backend.set_live(next).map_err(ChangeError::Live)?;
        self.visible = next;
        if let Err(persist) = backend.persist(next) {
            return match backend.set_live(previous) {
                Ok(()) => { self.visible = previous; Err(ChangeError::Persist(persist)) }
                Err(rollback) => Err(ChangeError::Rollback { persist, rollback }),
            };
        }
        self.committed = next;
        self.pending_commit = true;
        backend.commit().map_err(ChangeError::Commit)?;
        self.pending_commit = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake { calls: Vec<(char, u8)>, fail_live: Option<u8>, fail_save: bool }
    impl Backend for Fake {
        type Error = &'static str;
        fn set_live(&mut self, s: Scale) -> Result<(), Self::Error> {
            self.calls.push(('L', s.percent()));
            if self.fail_live == Some(s.percent()) { Err("gpu") } else { Ok(()) }
        }
        fn persist(&mut self, s: Scale) -> Result<(), Self::Error> {
            self.calls.push(('P', s.percent()));
            if self.fail_save { Err("disk") } else { Ok(()) }
        }
    }
    fn s(n: u8) -> Scale { Scale::new(n).unwrap() }
    #[test]
    fn bounds_and_slider_validation() {
        for n in 0..=u8::MAX { assert_eq!(Scale::new(n).is_some(), ((50..=100).contains(&n) && n % 5 == 0) || n == 67 || matches!(n, 125 | 150 | 175 | 200)); }
        for (index, percent) in [50, 55, 60, 65, 67, 70, 75, 80, 85, 90, 95, 100, 125, 150, 175, 200].into_iter().enumerate() {
            assert_eq!(Scale::from_slider(index as f32), Some(s(percent)), "index {index}");
            assert_eq!(s(percent).slider_value(), index as f32, "percent {percent}");
        }
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 16.0, 4.5, 67.0, 75.5] {
            assert_eq!(Scale::from_slider(value), None, "value {value}");
        }
        for percent in (0..=u8::MAX).filter_map(Scale::new) {
            assert_eq!(Scale::from_slider(percent.slider_value()), Some(percent));
        }
    }
    #[test]
    fn retail_script_simulation_immediate_drag_reopen_restart() {
        let mut b = Fake::default(); let mut session = Session::new(s(100));
        for n in [95, 90, 85, 80, 75, 70, 65, 60, 55, 50, 60, 70, 80, 90, 100, 75] {
            session.change(&mut b, s(n)).unwrap();
            assert_eq!(session.visible(), s(n)); assert_eq!(session.committed(), s(n));
            assert_eq!(&b.calls[b.calls.len()-2..], [('L', n), ('P', n)]);
        }
        let reopened = Session::new(session.committed());
        let restarted = Session::new(reopened.committed());
        assert_eq!(restarted.visible(), s(75));
    }
    #[test]
    fn unsupported_start_keeps_requested_and_persists_new_effective() {
        let mut b = Fake::default();
        let mut session = Session::with_fallback(s(50), s(100));
        assert_eq!(session.committed(), s(50));
        assert_eq!(session.visible(), s(100));
        session.change(&mut b, s(95)).unwrap();
        assert_eq!(b.calls, [('L', 95), ('P', 95)]);
        assert_eq!((session.committed(), session.visible()), (s(95), s(95)));
    }
    #[test]
    fn commit_occurs_only_after_persistence_and_can_be_retried() {
        #[derive(Default)] struct B { calls: Vec<char>, fail_save: bool, fail_commit: bool }
        impl Backend for B {
            type Error = &'static str;
            fn set_live(&mut self, _: Scale) -> Result<(), Self::Error> { self.calls.push('L'); Ok(()) }
            fn persist(&mut self, _: Scale) -> Result<(), Self::Error> { self.calls.push('P'); if self.fail_save { Err("save") } else { Ok(()) } }
            fn commit(&mut self) -> Result<(), Self::Error> { self.calls.push('C'); if self.fail_commit { Err("commit") } else { Ok(()) } }
        }
        let mut s0 = Session::new(s(100));
        let mut b = B { fail_save: true, ..Default::default() };
        assert_eq!(s0.change(&mut b, s(75)), Err(ChangeError::Persist("save")));
        assert_eq!(b.calls, ['L', 'P', 'L']);
        b.calls.clear(); b.fail_save = false; b.fail_commit = true;
        assert_eq!(s0.change(&mut b, s(75)), Err(ChangeError::Commit("commit")));
        assert_eq!(b.calls, ['L', 'P', 'C']);
        assert_eq!((s0.committed(), s0.visible()), (s(75), s(75)));
        assert!(s0.pending_commit());
        b.calls.clear(); b.fail_commit = false;
        s0.change(&mut b, s(75)).unwrap();
        assert_eq!(b.calls, ['C']); assert!(!s0.pending_commit());
    }
    #[test]
    fn failed_gpu_never_publishes() {
        let mut b = Fake { fail_live: Some(75), ..Default::default() }; let mut session = Session::new(s(100));
        assert_eq!(session.change(&mut b, s(75)), Err(ChangeError::Live("gpu")));
        assert_eq!(b.calls, [('L', 75)]); assert_eq!(session.visible(), s(100));
    }
    #[test]
    fn failed_save_rolls_back_and_failed_rollback_is_reported() {
        let mut b = Fake { fail_save: true, ..Default::default() }; let mut session = Session::new(s(100));
        assert_eq!(session.change(&mut b, s(75)), Err(ChangeError::Persist("disk")));
        assert_eq!(session.visible(), s(100)); assert_eq!(session.committed(), s(100));
        b.fail_live = Some(100);
        assert_eq!(session.change(&mut b, s(75)), Err(ChangeError::Rollback { persist: "disk", rollback: "gpu" }));
        assert_eq!(session.visible(), s(75)); assert_eq!(session.committed(), s(100));
    }
}

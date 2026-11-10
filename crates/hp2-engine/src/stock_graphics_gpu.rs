//! Transaction policy for live VSync and anisotropy settings.
//!
//! The renderer backend must reject unsupported values and report the observed
//! effective value, including after an apply failure.
//! Calls run on the engine thread outside a live render frame. Config writes
//! are serialized with other settings writers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VSync { Off, On, Adaptive }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Value { VSync(VSync), Anisotropy(u8), Msaa(u8) }
impl Value {
    pub(crate) fn selectable(self) -> bool {
        match self { Self::VSync(VSync::Off | VSync::On) => true, Self::VSync(VSync::Adaptive) => false,
            Self::Anisotropy(v) => [0,4,8,16].contains(&v), Self::Msaa(v) => [0,2,4].contains(&v) }
    }
    fn same_kind(self, other: Self) -> bool { matches!((self,other), (Self::VSync(_),Self::VSync(_)) | (Self::Anisotropy(_),Self::Anisotropy(_)) | (Self::Msaa(_),Self::Msaa(_))) }
}
/// GPU calls must verify the applied state, not just change reflected config.
pub(crate) trait Backend {
    type Error;
    fn current(&mut self, kind: Value) -> Result<Value, Self::Error>;
    fn supports(&mut self, requested: Value) -> Result<bool, Self::Error>;
    /// Accepts Adaptive only for rollback. Errors may leave state changed.
    fn apply(&mut self, value: Value) -> Result<(), Self::Error>;
    /// Restore the original GPU and reflected state captured by `current`.
    fn restore(&mut self, value: Value) -> Result<(), Self::Error> { self.apply(value) }
    fn persist(&mut self, value: Value) -> Result<(), Self::Error>;
    /// Publish the saved preference only after persistence succeeded.
    fn committed(&mut self, _value: Value) -> Result<(), Self::Error> { Ok(()) }
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Error<E> {
    Invalid, Unsupported, Read(E), Apply(E), WrongKind,
    /// The applied value did not match the requested value, but rollback succeeded.
    NotApplied { observed: Value },
    /// Persistence failed and the original GPU state was restored and checked.
    Save(E),
    /// The value was saved and remains active, but metadata could not be updated.
    Commit(E),
    /// A rollback or its verification failed. The caller must report divergence.
    Rollback { cause: &'static str, error: Option<E> },
}
fn restore<B: Backend>(backend: &mut B, old: Value, cause: &'static str) -> Result<(), Error<B::Error>> {
    backend.restore(old).map_err(|error|Error::Rollback {cause,error:Some(error)})?;
    match backend.current(old) {
        Ok(actual) if actual == old => Ok(()),
        Ok(_) => Err(Error::Rollback {cause,error:None}),
        Err(error) => Err(Error::Rollback {cause,error:Some(error)}),
    }
}
/// Apply and verify the GPU state before saving. On failure the caller must
/// re-read the renderer and show an error rather than assuming a value.
pub(crate) fn change<B: Backend>(backend: &mut B, next: Value) -> Result<(), Error<B::Error>> {
    if !next.selectable() { return Err(Error::Invalid); }
    let old=backend.current(next).map_err(Error::Read)?;
    if !old.same_kind(next) {return Err(Error::WrongKind);}
    // Do not replace a custom live value unless the backend can restore it.
    if !old.selectable() && old != Value::VSync(VSync::Adaptive) {return Err(Error::Invalid);}
    if !backend.supports(next).map_err(Error::Read)? {return Err(Error::Unsupported);}
    // Even if the effective GPU value already matches, the reflected value
    // may differ (for example, an Adaptive request that fell back to FIFO).
    // Apply it so that later frames do not reinstate that prior request.
    if let Err(error) = backend.apply(next) {
        // An unsuccessful surface reconfiguration can itself fail to roll
        // back. Never assume that an error preserved the old present mode.
        match backend.current(old) {
            Ok(actual) if actual == old => {},
            _ => restore(backend,old,"apply failed")?,
        }
        return Err(Error::Apply(error));
    }
    let actual=match backend.current(next) {
        Ok(actual) => actual,
        Err(error) => {restore(backend,old,"readback failed")?; return Err(Error::Read(error));}
    };
    if actual != next {restore(backend,old,"readback mismatch")?; return Err(Error::NotApplied {observed:actual});}
    if let Err(error)=backend.persist(next) {restore(backend,old,"save failed")?; return Err(Error::Save(error));}
    backend.committed(next).map_err(Error::Commit)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fake { actual:Value, supported:bool, fail_apply:Option<Value>, mutate_on_failure:bool, fail_save:bool, lie:bool, calls:Vec<&'static str> }
    impl Backend for Fake {type Error=&'static str;
        fn current(&mut self,_:Value)->Result<Value,Self::Error>{self.calls.push("read"); Ok(self.actual)}
        fn supports(&mut self,_:Value)->Result<bool,Self::Error>{self.calls.push("supports"); Ok(self.supported)}
        fn apply(&mut self,v:Value)->Result<(),Self::Error>{self.calls.push("apply"); if self.fail_apply==Some(v){if self.mutate_on_failure {self.actual=v;} return Err("GPU");} if !self.lie {self.actual=v;} Ok(())}
        fn persist(&mut self,_:Value)->Result<(),Self::Error>{self.calls.push("persist");if self.fail_save{Err("disk")}else{Ok(())}}
    }
    fn fake(value:Value)->Fake {Fake {actual:value,supported:true,fail_apply:None,mutate_on_failure:false,fail_save:false,lie:false,calls:vec![]}}
    #[test] fn successful_changes_verify_before_save() {for (old,next) in [(Value::VSync(VSync::Adaptive),Value::VSync(VSync::On)),(Value::Anisotropy(0),Value::Anisotropy(16)),(Value::Msaa(0),Value::Msaa(4))] {let mut f=fake(old);change(&mut f,next).unwrap();assert_eq!(f.actual,next);assert_eq!(f.calls,["read","supports","apply","read","persist"]);}}
    #[test] fn unsupported_and_failed_apply_never_save() {let old=Value::VSync(VSync::On);let next=Value::VSync(VSync::Off);let mut f=fake(old);f.supported=false;assert_eq!(change(&mut f,next),Err(Error::Unsupported));assert_eq!(f.actual,old);assert!(!f.calls.contains(&"persist"));let mut f=fake(old);f.fail_apply=Some(next);assert_eq!(change(&mut f,next),Err(Error::Apply("GPU")));assert_eq!(f.actual,old);assert!(!f.calls.contains(&"persist"));}
    #[test] fn failed_save_restores_adaptive_and_reports_failed_rollback() {let old=Value::VSync(VSync::Adaptive);let next=Value::VSync(VSync::On);let mut f=fake(old);f.fail_save=true;assert_eq!(change(&mut f,next),Err(Error::Save("disk")));assert_eq!(f.actual,old);let mut f=fake(old);f.fail_save=true;f.fail_apply=Some(old);assert_eq!(change(&mut f,next),Err(Error::Rollback{cause:"save failed",error:Some("GPU")}));assert_eq!(f.actual,next);}
    #[test] fn wrong_readback_is_not_saved() {let old=Value::Anisotropy(0);let mut f=fake(old);f.lie=true;assert_eq!(change(&mut f,Value::Anisotropy(8)),Err(Error::NotApplied{observed:old}));assert!(!f.calls.contains(&"persist"));}
    #[test] fn apply_failure_with_changed_state_is_rolled_back() {let old=Value::VSync(VSync::Adaptive);let next=Value::VSync(VSync::On);let mut f=fake(old);f.fail_apply=Some(next);f.mutate_on_failure=true;assert_eq!(change(&mut f,next),Err(Error::Apply("GPU")));assert_eq!(f.actual,old);assert!(!f.calls.contains(&"persist"));}
    #[test] fn same_effective_mode_still_synchronizes_and_rolls_back_reflected_request() {
        struct Diverged { gpu:Value, reflected:Value, original:Value, fail:bool }
        impl Backend for Diverged {type Error=&'static str;
            fn current(&mut self,_:Value)->Result<Value,Self::Error>{Ok(self.gpu)}
            fn supports(&mut self,_:Value)->Result<bool,Self::Error>{Ok(true)}
            fn apply(&mut self,v:Value)->Result<(),Self::Error>{self.gpu=v;self.reflected=v;Ok(())}
            fn restore(&mut self,v:Value)->Result<(),Self::Error>{self.gpu=v;self.reflected=self.original;Ok(())}
            fn persist(&mut self,_:Value)->Result<(),Self::Error>{if self.fail {Err("disk")} else {Ok(())}}
        }
        let actual=Value::VSync(VSync::On); let requested=Value::VSync(VSync::Adaptive);
        let mut b=Diverged {gpu:actual,reflected:requested,original:requested,fail:false};
        change(&mut b,actual).unwrap(); assert_eq!(b.reflected,actual);
        let mut b=Diverged {gpu:actual,reflected:requested,original:requested,fail:true};
        assert_eq!(change(&mut b,actual),Err(Error::Save("disk"))); assert_eq!(b.gpu,actual); assert_eq!(b.reflected,requested);
    }
    #[test] fn startup_fallback_warning_clears_only_after_successful_save() {
        struct Fallback { effective:Value, requested:Value, warning:bool, fail:bool, commits:usize }
        impl Backend for Fallback {type Error=&'static str;
            fn current(&mut self,_:Value)->Result<Value,Self::Error>{Ok(self.effective)}
            fn supports(&mut self,_:Value)->Result<bool,Self::Error>{Ok(true)}
            fn apply(&mut self,v:Value)->Result<(),Self::Error>{self.effective=v;Ok(())}
            fn persist(&mut self,_:Value)->Result<(),Self::Error>{if self.fail {Err("disk")} else {Ok(())}}
            fn committed(&mut self,v:Value)->Result<(),Self::Error>{self.requested=v;self.warning=false;self.commits+=1;Ok(())}
        }
        for (requested,effective) in [(Value::Msaa(4),Value::Msaa(0)),(Value::Anisotropy(16),Value::Anisotropy(0)),(Value::VSync(VSync::Off),Value::VSync(VSync::On))] {
            let mut failed=Fallback {effective,requested,warning:true,fail:true,commits:0};
            assert_eq!(change(&mut failed,effective),Err(Error::Save("disk"))); assert_eq!((failed.effective,failed.requested,failed.warning,failed.commits),(effective,requested,true,0));
            let mut ok=Fallback {effective,requested,warning:true,fail:false,commits:0};
            change(&mut ok,effective).unwrap(); assert_eq!((ok.effective,ok.requested,ok.warning,ok.commits),(effective,effective,false,1));
        }
    }
    #[test] fn commit_failure_does_not_roll_back_the_saved_value() {
        struct F { value:Value, saved:bool }
        impl Backend for F {type Error=&'static str;
            fn current(&mut self,_:Value)->Result<Value,Self::Error>{Ok(self.value)}
            fn supports(&mut self,_:Value)->Result<bool,Self::Error>{Ok(true)}
            fn apply(&mut self,v:Value)->Result<(),Self::Error>{self.value=v;Ok(())}
            fn persist(&mut self,_:Value)->Result<(),Self::Error>{self.saved=true;Ok(())}
            fn committed(&mut self,_:Value)->Result<(),Self::Error>{Err("metadata")}
        }
        let mut f=F {value:Value::Msaa(0),saved:false}; assert_eq!(change(&mut f,Value::Msaa(2)),Err(Error::Commit("metadata"))); assert!(f.saved);assert_eq!(f.value,Value::Msaa(2));
    }
    #[test] fn invalid_value_is_not_sent() {let mut f=fake(Value::Anisotropy(0));for v in [Value::Anisotropy(1),Value::Anisotropy(32),Value::Msaa(8),Value::VSync(VSync::Adaptive)] {assert_eq!(change(&mut f,v),Err(Error::Invalid));}assert!(f.calls.is_empty());}
}

//! State transition for invalidating cached textures after LOD changes.
use std::cell::Cell;

pub(crate) fn observe<E>(cache: &Cell<Option<[i32; 8]>>, current: Option<[i32; 8]>, mut flush: impl FnMut() -> Result<(), E>) -> Result<(), E> {
    match (cache.get(), current) {
        (_, None) => cache.set(None),
        (None, Some(value)) => cache.set(Some(value)),
        (Some(old), Some(value)) if old != value => {
            flush()?;
            cache.set(Some(value));
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lod_change_flushes_once_and_failure_does_not_commit() {
        let cache = Cell::new(None);
        let mut calls = 0;
        let mut lod = [0; 8];
        observe::<()>(&cache, Some(lod), || { calls += 1; Ok(()) }).unwrap();
        assert_eq!(calls, 0, "initialization must not flush");
        lod[1] = 2;
        assert_eq!(observe(&cache, Some(lod), || { calls += 1; Err("failure") }), Err("failure"));
        assert_eq!(cache.get(), Some([0; 8]), "failed refresh retains prior state");
        observe::<()>(&cache, Some(lod), || { calls += 1; Ok(()) }).unwrap();
        observe::<()>(&cache, Some(lod), || { calls += 1; Ok(()) }).unwrap();
        assert_eq!(calls, 2, "failure and one successful flush only");
        observe::<()>(&cache, None, || { calls += 1; Ok(()) }).unwrap();
        assert_eq!(cache.get(), None);
    }
}

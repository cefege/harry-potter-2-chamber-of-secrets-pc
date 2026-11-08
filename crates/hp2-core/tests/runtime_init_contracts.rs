use hp2_core::runtime_init::cache_file_selected;

#[test]
fn persistent_cache_skips_only_zero_length_even_when_file_disappears() {
    for size in [i32::MIN as i64, -1, 1, i32::MAX as i64] {
        assert!(cache_file_selected(size));
    }
    assert!(!cache_file_selected(0));
}

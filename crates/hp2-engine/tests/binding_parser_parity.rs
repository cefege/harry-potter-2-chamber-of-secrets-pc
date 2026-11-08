use hp2_engine::binding_foundation as binding;

#[test]
fn quoted_pipe_matches_the_canonical_input_parser() {
    // input_owner::process uses this exact parser with exact=false and 256 units.
    let original = "Button Jump|Say \"A|B\"|Button Duck";
    let edited = binding::remove(original, "Button Jump").unwrap();
    let wide: Vec<i32> = edited.chars().map(|c| c as i32).chain([0]).collect();
    let mut cursor = wide.as_ptr();
    let mut lines = Vec::new();
    loop {
        let mut out = [0i32; 256];
        if !unsafe { hp2_core::tokenize::parse_line(&mut cursor, out.as_mut_ptr(), 256, false) } { break; }
        let len = out.iter().position(|&c| c == 0).unwrap();
        lines.push(out[..len].iter().map(|&c| char::from_u32(c as u32).unwrap()).collect::<String>());
    }
    assert_eq!(lines, ["Say \"A|B\"", "Button Duck"]);
}

use hp2_abi::FColor;
use hp2_engine::actor_color;

#[test]
fn color_float_operators_scale_rgb_and_preserve_alpha() {
    let input = FColor {
        r: 101,
        g: 80,
        b: 33,
        a: 211,
    };
    let expected = FColor {
        r: 50,
        g: 40,
        b: 16,
        a: 211,
    };
    assert_eq!(actor_color::multiply_color_float(input, 0.5), expected);
    assert_eq!(actor_color::multiply_float_color(0.5, input), expected);
}

#[test]
fn color_add_subtract_wrap_rgb_and_preserve_left_alpha() {
    let a = FColor {
        r: 250,
        g: 2,
        b: 50,
        a: 171,
    };
    let b = FColor {
        r: 10,
        g: 9,
        b: 80,
        a: 39,
    };
    assert_eq!(
        actor_color::add_color_color(a, b),
        FColor {
            r: 4,
            g: 11,
            b: 130,
            a: 171,
        }
    );
    assert_eq!(
        actor_color::subtract_color_color(a, b),
        FColor {
            r: 240,
            g: 249,
            b: 226,
            a: 171,
        }
    );
}

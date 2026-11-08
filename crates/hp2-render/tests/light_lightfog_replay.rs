#![cfg(target_arch = "aarch64")]

use std::fmt::Debug;
use std::str::FromStr;
use std::sync::LazyLock;

use hp2_abi::FVector;
use hp2_render::light::{
    FogLight, FogParams, FogVertex, FogVtric, GMath, PF_RENDER_FOG, PF_UNLIT, light_and_fog,
};
use hp2_render::services::Hp2RenderServices;

const FIXTURE: &str = include_str!("fixtures/light_lightfog.txt");
const TABLES_FIXTURE: &str = include_str!("fixtures/light_kernels.txt");

fn records() -> impl Iterator<Item = &'static str> {
    FIXTURE
        .lines()
        .filter(|line| line.split_whitespace().next() == Some("lightfog"))
}

fn hex_u32(word: &str) -> u32 {
    u32::from_str_radix(word, 16).unwrap_or_else(|e| panic!("hex {word:?}: {e}"))
}

fn hex_f32(word: &str) -> f32 {
    f32::from_bits(hex_u32(word))
}

fn tagged_floats(tag: &str) -> Vec<f32> {
    let line = TABLES_FIXTURE
        .lines()
        .find(|line| line.split_whitespace().next() == Some(tag))
        .unwrap_or_else(|| panic!("missing table {tag}"));
    line.split_whitespace().skip(1).map(hex_f32).collect()
}

struct Tokens(std::str::SplitWhitespace<'static>);
impl Tokens {
    fn new(line: &'static str) -> Self {
        Self(line.split_whitespace())
    }
    fn word(&mut self) -> &'static str {
        self.0.next().expect("fixture token")
    }
    fn literal(&mut self, expected: &str) {
        assert_eq!(self.word(), expected);
    }
    fn dec<T: FromStr>(&mut self) -> T
    where
        T::Err: Debug,
    {
        let word = self.word();
        word.parse()
            .unwrap_or_else(|e| panic!("decimal {word:?}: {e:?}"))
    }
    fn float(&mut self) -> f32 {
        hex_f32(self.word())
    }
    fn vec3(&mut self) -> FVector {
        FVector {
            x: self.float(),
            y: self.float(),
            z: self.float(),
        }
    }
    fn vec4(&mut self) -> [f32; 4] {
        [self.float(), self.float(), self.float(), self.float()]
    }
}

struct Tables {
    sqrt_man: Vec<f32>,
    div_sqrt_man: Vec<f32>,
    div_sqrt_exp: Vec<f32>,
}
static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let t = Tables {
        sqrt_man: tagged_floats("GMATH_SQRTMAN"),
        div_sqrt_man: tagged_floats("GMATH_DIVSQRTMAN"),
        div_sqrt_exp: tagged_floats("GMATH_DIVSQRTEXP"),
    };
    assert_eq!(t.sqrt_man.len(), 2048);
    assert_eq!(t.div_sqrt_man.len(), 1024);
    assert_eq!(t.div_sqrt_exp.len(), 512);
    t
});

unsafe extern "C" fn unused_frand() -> f32 {
    f32::NAN
}
unsafe extern "C" fn unused_unary(_f: f32) -> f32 {
    f32::NAN
}
unsafe extern "C" fn unused_log_warning(_text: *const u8, _len: usize) {}
unsafe extern "C" fn sin_tab(_angle: i32) -> f32 {
    f32::NAN
}
unsafe extern "C" fn cos_tab(_angle: i32) -> f32 {
    f32::NAN
}
unsafe extern "C" fn gmath_sqrt_approx(f: f32) -> f32 {
    let bits = f.to_bits();
    let index = ((bits >> 13) & 2047) as usize;
    f32::from_bits(
        TABLES.sqrt_man[index]
            .to_bits()
            .wrapping_add((bits & 0x7f00_0000) >> 1),
    )
}
unsafe extern "C" fn gmath_div_sqrt_approx(f: f32) -> f32 {
    let bits = f.to_bits();
    TABLES.div_sqrt_man[((bits >> 13) & 1023) as usize] * TABLES.div_sqrt_exp[(bits >> 23) as usize]
}

fn services() -> Hp2RenderServices {
    Hp2RenderServices {
        frand: unused_frand,
        log_warning: unused_log_warning,
        sin_tab,
        cos_tab,
        sqrt_approx: unused_unary,
        div_sqrt_approx: unused_unary,
        div_approx: unused_unary,
        gmath_div_sqrt_approx,
        gmath_sqrt_approx,
    }
}

#[test]
fn light_and_fog_matches_oracle_fixture() {
    let services = services();
    let g = GMath::new(&services);
    let mut count = 0;
    for line in records() {
        let mut t = Tokens::new(line);
        t.literal("lightfog");
        let case: usize = t.dec();
        let poly_flags: u32 = t.dec();
        let nl: usize = t.dec();
        let nv: usize = t.dec();
        let point = t.vec3();
        let normal = t.vec3();
        let _normal_w = t.float();
        let ambient = t.vec4();
        let params = FogParams {
            ambient,
            diffuse: t.float(),
            specular: t.float(),
            spec_min_cos: t.float(),
        };
        let lights: Vec<FogLight> = (0..nl)
            .map(|_| FogLight {
                location: t.vec3(),
                light_dir: t.vec3(),
                float_color: t.vec4(),
                radius: t.float(),
                rradius_mult: t.float(),
                opt: t.dec(),
                light_source: t.dec(),
                dark_light: t.dec(),
            })
            .collect();
        let vtrics: Vec<FogVtric> = (0..nv)
            .map(|_| FogVtric {
                location: t.vec3(),
                vol_radius_squared: t.float(),
                rvol_radius: t.float(),
                rvol_radius_squared: t.float(),
                vol_brightness: t.float(),
                location_size_squared: t.float(),
                vol_inside: t.dec(),
                ambient: t.dec(),
                color: t.vec4(),
            })
            .collect();
        let expected_light = t.vec4();
        let expected_fog = t.vec4();
        assert!(t.0.next().is_none(), "case {case}: trailing fixture tokens");
        let mut vertex = FogVertex {
            point,
            normal,
            light: [0.0; 4],
            fog: [0.0; 4],
        };
        let mut fog_rejection = 0;
        light_and_fog(
            &g,
            &params,
            &mut vertex,
            poly_flags,
            lights.iter().copied(),
            vtrics.iter().copied(),
            &mut fog_rejection,
        );
        for i in 0..4 {
            assert_eq!(
                vertex.light[i].to_bits(),
                expected_light[i].to_bits(),
                "case {case} light[{i}]"
            );
            assert_eq!(
                vertex.fog[i].to_bits(),
                expected_fog[i].to_bits(),
                "case {case} fog[{i}]"
            );
        }
        count += 1;
    }
    assert_eq!(count, 48, "fixture record count");
}

#[test]
fn unused_light_ranges_are_not_read() {
    let services = services();
    let g = GMath::new(&services);
    let params = FogParams {
        ambient: [0.2, 1.2, 0.4, 0.8],
        diffuse: 1.0,
        specular: 0.0,
        spec_min_cos: 0.0,
    };
    let mut vertex = FogVertex {
        point: FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        normal: FVector {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        light: [9.0; 4],
        fog: [9.0; 4],
    };
    let mut rejection = 0;
    let unread_lights =
        std::iter::from_fn(|| -> Option<FogLight> { panic!("unlit polygon read its light range") });
    let unread_volumes = std::iter::from_fn(|| -> Option<FogVtric> {
        panic!("unlit polygon read its volumetric range")
    });
    assert!(!light_and_fog(
        &g,
        &params,
        &mut vertex,
        PF_UNLIT | PF_RENDER_FOG,
        unread_lights,
        unread_volumes,
        &mut rejection,
    ));
    assert_eq!(vertex.light, params.ambient);
    assert_eq!(vertex.fog, [0.0; 4]);

    let unread_volumes = std::iter::from_fn(|| -> Option<FogVtric> {
        panic!("non-fog polygon read its volumetric range")
    });
    assert!(light_and_fog(
        &g,
        &params,
        &mut vertex,
        0,
        std::iter::empty(),
        unread_volumes,
        &mut rejection,
    ));
    assert_eq!(vertex.light, [0.2, 1.0, 0.4, 0.8]);
    assert_eq!(vertex.fog, [0.0; 4]);
}

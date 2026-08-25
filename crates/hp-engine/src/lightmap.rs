//! Baked lightmap reconstruction (UE1 static lighting).
//!
//! Grammar and math ported from the C++-verified openhp1 study
//! (`lighting.rs`, `lighting/lightmap.rs`): each level-model `FLightMap`
//! record describes a per-surface light grid (`clamp` dimensions, `scale`
//! world-units-per-texel, `pan` offset, `data_offset` into the shared
//! 1bpp `LightBits` shadow-mask blob, `light_actors` head into the
//! model-wide None-terminated light list). A lightmap image starts filled
//! with the owning zone's ambient color (LevelInfo fallback), then each
//! listed light adds its blurred-shadow-masked contribution; the final
//! RGB is clamped to 0..1.

use crate::scene::{LightMapData, ParsedModel};
use hp_uobject::props::PropStore;
use hp_uobject::value::PropValue;

/// One decoded lightmap: RGBA8, `width × height`, plus the projection
/// parameters needed to map world positions to lm texels.
#[derive(Debug, Clone)]
pub struct LightmapImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub pan: [f32; 3],
    pub scale: [f32; 2],
}

/// Ambient color triple (hue/saturation/brightness bytes as authored).
#[derive(Debug, Clone, Copy, Default)]
pub struct Ambient {
    pub hue: u8,
    pub saturation: u8,
    pub brightness: u8,
}

impl Ambient {
    pub fn rgb(self) -> [f32; 3] {
        hsb_to_rgb(self.hue, self.saturation, self.brightness)
    }
}



/// One decoded Light actor.
#[derive(Debug, Clone, Copy)]
pub struct LightActor {
    pub location: [f32; 3],
    pub rotation: [i32; 3],
    pub effect: u8,
    pub brightness: u8,
    pub hue: u8,
    pub saturation: u8,
    pub radius: u8,
    pub cone: u8,
    pub dark: bool,
}

/// UE1's HSB -> RGB (openhp1 lighting.rs:194 — hue wraps 360° over 256).
pub fn hsb_to_rgb(hue: u8, saturation: u8, brightness: u8) -> [f32; 3] {
    let value = 6.512_735 * f32::from(brightness).sqrt();
    if saturation >= 250 {
        return [value / 255.0; 3];
    }
    if brightness == 0 {
        return [0.0; 3];
    }
    let mut saturation = f32::from(saturation) / 2.5;
    if saturation > 32.0 {
        saturation += 2.0;
    }
    let sector = f32::from(hue) / 85.0;
    let fraction = sector.fract();
    let low = saturation * value / 104.0;
    let falling = (1.0 - fraction) * value + low * fraction;
    let rising = fraction * value + low * (1.0 - fraction);
    let rgb = if hue < 85 {
        [falling, rising, low]
    } else if hue < 170 {
        [low, falling, rising]
    } else {
        [rising, low, falling]
    };
    [rgb[0] / 255.0, rgb[1] / 255.0, rgb[2] / 255.0]
}

fn prop_byte(store: &PropStore, names: &hp_uobject::name::NamePool, prop: &str) -> Option<u8> {
    match store.get(names.find_index(prop)?)? {
        PropValue::Byte(b) => Some(*b),
        PropValue::Int(i) => Some(*i as u8),
        _ => None,
    }
}

fn prop_bool(store: &PropStore, names: &hp_uobject::name::NamePool, prop: &str) -> Option<bool> {
    match store.get(names.find_index(prop)?)? {
        PropValue::Bool(b) => Some(*b),
        _ => None,
    }
}

fn prop_vec3(
    store: &PropStore,
    names: &hp_uobject::name::NamePool,
    prop: &str,
) -> Option<[f32; 3]> {
    match store.get(names.find_index(prop)?)? {
        PropValue::Struct { fields, .. } => {
            let mut o = [0.0f32; 3];
            for (fi, fv) in fields {
                let slot = match names.text(*fi).unwrap_or("").to_ascii_uppercase().as_str() {
                    "X" => 0,
                    "Y" => 1,
                    "Z" => 2,
                    _ => continue,
                };
                if let PropValue::Float(f) = fv {
                    o[slot] = *f;
                }
            }
            Some(o)
        }
        _ => None,
    }
}

fn prop_rotator(
    store: &PropStore,
    names: &hp_uobject::name::NamePool,
    prop: &str,
) -> Option<[i32; 3]> {
    match store.get(names.find_index(prop)?)? {
        PropValue::Struct { fields, .. } => {
            let mut o = [0i32; 3];
            for (fi, fv) in fields {
                let slot = match names.text(*fi).unwrap_or("").to_ascii_uppercase().as_str() {
                    "PITCH" => 0,
                    "YAW" => 1,
                    "ROLL" => 2,
                    _ => continue,
                };
                if let PropValue::Int(i) = fv {
                    o[slot] = *i;
                }
            }
            Some(o)
        }
        _ => None,
    }
}

pub fn decode_ambient(
    store: &PropStore,
    names: &hp_uobject::name::NamePool,
) -> Ambient {
    // ZoneInfo/LevelInfo default ambient is black (openhp1 AmbientLight
    // default: brightness 0, hue 0, saturation 255).
    Ambient {
        hue: prop_byte(store, names, "AmbientHue").unwrap_or(0),
        saturation: prop_byte(store, names, "AmbientSaturation").unwrap_or(255),
        brightness: prop_byte(store, names, "AmbientBrightness").unwrap_or(0),
    }
}

pub fn decode_light(
    store: &PropStore,
    names: &hp_uobject::name::NamePool,
) -> Option<LightActor> {
    // UE1 Light class defaults (openhp1 lighting.rs:33): absent props take
    // the class default, not zero.
    let mut light = LightActor {
        location: [0.0; 3],
        rotation: [0; 3],
        effect: 0,
        brightness: 64,
        hue: 0,
        saturation: 255,
        radius: 64,
        cone: 128,
        dark: false,
    };
    light.location = prop_vec3(store, names, "Location")?;
    if let Some(v) = prop_rotator(store, names, "Rotation") {
        light.rotation = v;
    }
    if let Some(v) = prop_byte(store, names, "LightEffect") {
        light.effect = v;
    }
    if let Some(v) = prop_byte(store, names, "LightBrightness") {
        light.brightness = v;
    }
    if let Some(v) = prop_byte(store, names, "LightHue") {
        light.hue = v;
    }
    if let Some(v) = prop_byte(store, names, "LightSaturation") {
        light.saturation = v;
    }
    if let Some(v) = prop_byte(store, names, "LightRadius") {
        light.radius = v;
    }
    if let Some(v) = prop_byte(store, names, "LightCone") {
        light.cone = v;
    }
    light.dark = prop_bool(store, names, "bDarkLight").unwrap_or(false);
    Some(light)
}

/// World position per lightmap texel, barycentrically interpolated across
/// the (base, base+TexU, base+TexV) surface triangle (openhp1
/// lightmap.rs:445).
#[allow(clippy::too_many_arguments)]
fn lightmap_locations(
    lightmap: &LightMapData,
    base: [f32; 3],
    texture_u: [f32; 3],
    texture_v: [f32; 3],
    width: usize,
    height: usize,
) -> Option<Vec<[f32; 3]>> {
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let pan_u = dot(texture_u, base) + lightmap.pan[0] - 0.5 * lightmap.scale[0];
    let pan_v = dot(texture_v, base) + lightmap.pan[1] - 0.5 * lightmap.scale[1];
    let points = [
        base,
        [
            base[0] + texture_u[0],
            base[1] + texture_u[1],
            base[2] + texture_u[2],
        ],
        [
            base[0] + texture_v[0],
            base[1] + texture_v[1],
            base[2] + texture_v[2],
        ],
    ];
    let coords: Vec<[f32; 2]> = points
        .iter()
        .map(|p| {
            [
                (dot(texture_u, *p) - pan_u) / lightmap.scale[0],
                (dot(texture_v, *p) - pan_v) / lightmap.scale[1],
            ]
        })
        .collect();
    if (coords[2][1] - coords[0][1]).abs() < 1e-6 || (coords[2][1] - coords[1][1]).abs() < 1e-6 {
        return None;
    }
    let left_step = (coords[2][0] - coords[0][0]) / (coords[2][1] - coords[0][1]);
    let right_step = (coords[2][0] - coords[1][0]) / (coords[2][1] - coords[1][1]);
    let lerp3 = |a: [f32; 3], b: [f32; 3], t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    let mut locations = Vec::with_capacity(width * height);
    for y in 0..height {
        let sample_y = y as f32 + 0.5;
        let mut x0 = coords[0][0] + left_step * (sample_y - coords[0][1]) + 0.5;
        let mut x1 = coords[1][0] + right_step * (sample_y - coords[1][1]) + 0.5;
        let t0 = (sample_y - coords[0][1]) / (coords[2][1] - coords[0][1]);
        let t1 = (sample_y - coords[1][1]) / (coords[2][1] - coords[1][1]);
        let mut point0 = lerp3(points[0], points[2], t0);
        let mut point1 = lerp3(points[1], points[2], t1);
        if x1 < x0 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut point0, &mut point1);
        }
        if (x1 - x0).abs() < 1e-6 {
            return None;
        }
        for x in 0..width {
            let t = (x as f32 + 0.5 - x0) / (x1 - x0);
            locations.push(lerp3(point0, point1, t));
        }
    }
    locations.iter().all(|p| p.iter().all(|c| c.is_finite())).then_some(locations)
}

/// 9-tap box-ish blur over the unpacked 1bpp shadow mask (openhp1
/// lightmap.rs:518).
fn blur_shadow_bits(bits: &[u8], width: usize, height: usize) -> Vec<f32> {
    let pitch = width.div_ceil(8);
    // HP2 LightBits polarity: 1 = LIT (UE1 convention). HP2_SHADOW_INVERT=1
    // flips to 1 = shadowed for empirical verification.
    let invert = std::env::var("HP2_SHADOW_INVERT").is_ok();
    let mut source = vec![0.0f32; width * height];
    for y in 0..height {
        for x in 0..width {
            let lit = bits[y * pitch + x / 8] & (1 << (x & 7)) != 0;
            source[y * width + x] = f32::from(lit != invert);
        }
    }
    const WEIGHTS: [f32; 9] = [
        0.125, 0.25, 0.125, 0.25, 0.5, 0.25, 0.125, 0.25, 0.125,
    ];
    let mut result = vec![0.0f32; width * height];
    for y in 0..height {
        for x in 0..width {
            let mut value = 0.0;
            for offset_y in -1i32..=1 {
                let sample_y = (y as i32 + offset_y).clamp(0, height as i32 - 1) as usize;
                for offset_x in -1i32..=1 {
                    let sample_x = (x as i32 + offset_x).clamp(0, width as i32 - 1) as usize;
                    let weight =
                        WEIGHTS[((offset_y + 1) * 3 + offset_x + 1) as usize];
                    value += source[sample_y * width + sample_x] * weight;
                }
            }
            result[y * width + x] = value;
        }
    }
    result
}

/// UE1 distance falloff ((1 + 2d³ − 3d²)/d, clamped to 1).
fn distance_falloff(distance_squared: f32) -> f32 {
    let value = (distance_squared + 0.0001).sqrt();
    ((1.0 + 2.0 * value.powi(3) - 3.0 * value.powi(2)) / value).min(1.0)
}

/// One light's contribution added into `pixels` with shadow masking and
/// the 13-id effect table (openhp1 lightmap.rs:520).
#[allow(clippy::too_many_arguments)]
fn add_light(
    pixels: &mut [[f32; 3]],
    locations: &[[f32; 3]],
    normal: [f32; 3],
    shadow: &[f32],
    light: &LightActor,
) {
    let radius = (f32::from(light.radius) + 1.0) * 25.0;
    let radius_squared = radius * radius;
    let color = hsb_to_rgb(light.hue, light.saturation, light.brightness);
    let rad = |u: i32| u as f32 * std::f32::consts::TAU / 65536.0;
    // UE1 spotlight direction: pitch about Y (negative), yaw about Z.
    let (sp, cp) = rad(light.rotation[0]).sin_cos();
    let (sy, cy) = rad(light.rotation[1]).sin_cos();
    let spot_direction = [-cp * cy, cp * sy, sp];
    for ((pixel, point), &shadow) in pixels.iter_mut().zip(locations).zip(shadow) {
        let direction = [
            light.location[0] - point[0],
            light.location[1] - point[1],
            light.location[2] - point[2],
        ];
        let distance_squared =
            direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2];
        let distance = distance_squared.sqrt();
        let unit = if distance > 0.0 {
            [direction[0] / distance, direction[1] / distance, direction[2] / distance]
        } else {
            [0.0; 3]
        };
        let n_dot_l =
            (unit[0] * normal[0] + unit[1] * normal[1] + unit[2] * normal[2]).abs();
        let illumination = match light.effect {
            13 => shadow * (1.0 - distance / radius).max(0.0),
            14 => {
                let normalized = distance / radius;
                shadow
                    * if (0.8..1.0).contains(&normalized) {
                        1.0 - 10.0 * (normalized - 0.9).abs()
                    } else {
                        0.0
                    }
            }
            17 => {
                let planar = direction[0] * direction[0] + direction[1] * direction[1];
                shadow * (1.0 - planar / radius_squared).max(0.0)
            }
            8 | 12 => {
                let normalized_distance = distance_squared / radius_squared;
                if normalized_distance >= 1.0 || light.cone == 0 || distance == 0.0 {
                    0.0
                } else {
                    let outer = 1.0 - f32::from(light.cone) / 255.0;
                    let cosine = unit[0] * spot_direction[0]
                        + unit[1] * spot_direction[1]
                        + unit[2] * spot_direction[2];
                    let spot =
                        (1.0 - ((1.0 - cosine) / (1.0 - outer)).min(1.0)).max(0.0);
                    shadow
                        * distance_falloff(normalized_distance)
                        * n_dot_l
                        * spot
                        * spot
                }
            }
            4 => 0.0,
            _ if distance_squared < radius_squared && distance != 0.0 => {
                shadow * distance_falloff(distance_squared / radius_squared) * n_dot_l
            }
            _ => 0.0,
        };
        let contribution = [
            (color[0] * illumination).min(1.0),
            (color[1] * illumination).min(1.0),
            (color[2] * illumination).min(1.0),
        ];
        if light.dark {
            *pixel = [
                (pixel[0] - contribution[0]).max(0.0),
                (pixel[1] - contribution[1]).max(0.0),
                (pixel[2] - contribution[2]).max(0.0),
            ];
        } else {
            *pixel = [
                pixel[0] + contribution[0],
                pixel[1] + contribution[1],
                pixel[2] + contribution[2],
            ];
        }
    }
}

/// Assemble one lightmap image: ambient fill + per-light contributions.
#[allow(clippy::too_many_arguments)]
fn build_lightmap(
    lightmap: &LightMapData,
    base: [f32; 3],
    texture_u: [f32; 3],
    texture_v: [f32; 3],
    normal: [f32; 3],
    ambient: Ambient,
    lights: &[(usize, LightActor)],
    light_bits: &[u8],
) -> Option<LightmapImage> {
    let width = usize::try_from(lightmap.clamp[0]).ok()?;
    let height = usize::try_from(lightmap.clamp[1]).ok()?;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return None;
    }
    if !(lightmap.scale[0].is_finite() && lightmap.scale[1].is_finite())
        || lightmap.scale[0] <= 0.0
        || lightmap.scale[1] <= 0.0
    {
        return None;
    }
    // HP2_LM_WHITE=1: bright-white fill (sampling diagnostic).
    let fill = if std::env::var("HP2_LM_WHITE").is_ok() {
        [1.0, 1.0, 1.0]
    } else {
        ambient.rgb()
    };
    let mut pixels = vec![fill; width * height];
    let Some(locations) = lightmap_locations(lightmap, base, texture_u, texture_v, width, height)
    else {
        return Some(LightmapImage {
            width: width as u32,
            height: height as u32,
            rgba: to_rgba(&pixels),
            pan: lightmap.pan,
            scale: lightmap.scale,
        });
    };
    let pitch = width.div_ceil(8);
    let mask_size = pitch * height;
    let mut applied = 0usize;
    let mut skipped_range = 0usize;
    let mut zero_brightness = 0usize;
    for (shadow_index, light) in lights.iter() {
        if light.brightness == 0 {
            zero_brightness += 1;
            continue;
        }
        let start = lightmap.data_offset as usize + shadow_index * mask_size;
        let Some(bits) = light_bits.get(start..start + mask_size) else {
            skipped_range += 1;
            continue;
        };
        applied += 1;
        let shadow = blur_shadow_bits(bits, width, height);
        if std::env::var("HP2_LM_STATS").is_ok() && applied == 1 {
            let min = locations.iter().fold([f32::MAX; 3], |a, p| {
                [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]
            });
            let max = locations.iter().fold([f32::MIN; 3], |a, p| {
                [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]
            });
            let dist: Vec<f32> = locations
                .iter()
                .map(|p| {
                    let d = [
                        light.location[0] - p[0],
                        light.location[1] - p[1],
                        light.location[2] - p[2],
                    ];
                    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
                })
                .collect();
            let dmin = dist.iter().copied().fold(f32::MAX, f32::min);
            let dmax = dist.iter().copied().fold(0.0f32, f32::max);
            eprintln!(
                "[lmstats] lattice bbox min={min:?} max={max:?}; light at {:?} radius={} dist {dmin:.1}..{dmax:.1}",
                light.location,
                (f32::from(light.radius) + 1.0) * 25.0
            );
        }
        add_light(&mut pixels, &locations, normal, &shadow, light);
    }
    if std::env::var("HP2_LM_STATS").is_ok() {
        eprintln!(
            "[lmstats] {}x{} off={} lights={} applied={} skipped_range={} zero_bri={}",
            width, height, lightmap.data_offset, lights.len(), applied, skipped_range, zero_brightness
        );
    }
    Some(LightmapImage {
        width: width as u32,
        height: height as u32,
        rgba: to_rgba(&pixels),
        pan: lightmap.pan,
        scale: lightmap.scale,
    })
}

fn to_rgba(pixels: &[[f32; 3]]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for color in pixels {
        for channel in color {
            rgba.push((channel.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
        }
        rgba.push(255);
    }
    rgba
}

/// Reconstruct every level-model lightmap image.
///
/// `ambient_of(zone_index)` decodes a zone-table actor's ambient color
/// (`None` = fall back to the LevelInfo ambient the caller supplies);
/// `light_of(export)` decodes a light-list export into a Light actor.
type ZoneAmbientFn<'a> = dyn Fn(usize) -> Option<Ambient> + 'a;
type LightResolveFn<'a> = dyn Fn(i32) -> Option<LightActor> + 'a;

pub(crate) fn build_lightmap_images(
    model: &ParsedModel,
    level_ambient: Ambient,
    ambient_of: &ZoneAmbientFn,
    light_of: &LightResolveFn,
) -> Vec<LightmapImage> {
    // lightmap index -> (surface index, zone index): first node wins.
    let mut lm_surface = vec![None::<usize>; model.light_maps.len()];
    let mut lm_zone = vec![None::<usize>; model.light_maps.len()];
    for node in &model.nodes {
        let Some(surface) = model.surfs.get(node.surface.max(0) as usize) else {
            continue;
        };
        let Ok(lm_index) = usize::try_from(surface.light_map) else {
            continue;
        };
        if lm_index >= model.light_maps.len() {
            continue;
        }
        if lm_surface[lm_index].is_none() {
            lm_surface[lm_index] = Some(node.surface.max(0) as usize);
            let zone_byte = node.izones[1] as usize;
            if zone_byte < model.zone_actors.len() {
                lm_zone[lm_index] = Some(zone_byte);
            }
        }
    }

    let mut ambient_cache: std::collections::HashMap<usize, Option<Ambient>> =
        Default::default();
    let mut light_cache: std::collections::HashMap<i32, Option<LightActor>> =
        Default::default();

    let mut images = Vec::with_capacity(model.light_maps.len());
    for (lm_index, lm) in model.light_maps.iter().enumerate() {
        let ambient = lm_zone[lm_index]
            .and_then(ambient_of)
            .unwrap_or(level_ambient);
        let ambient = *ambient_cache
            .entry(lm_zone[lm_index].unwrap_or(usize::MAX))
            .or_insert(Some(ambient))
            .as_ref()
            .unwrap_or(&level_ambient);

        // Surface basis for the texel-location lattice.
        let (base, tex_u, tex_v, normal) = match lm_surface[lm_index]
            .and_then(|si| model.surfs.get(si))
        {
            Some(surf) => {
                let base = model
                    .points
                    .get(surf.base_point)
                    .copied()
                    .unwrap_or([0.0; 3]);
                let tex_u = model.vectors.get(surf.tex_u).copied().unwrap_or([0.0; 3]);
                let tex_v = model.vectors.get(surf.tex_v).copied().unwrap_or([0.0; 3]);
                let normal = model
                    .vectors
                    .get(surf.normal)
                    .copied()
                    .map(|v| {
                        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                        if len > 1e-8 {
                            [v[0] / len, v[1] / len, v[2] / len]
                        } else {
                            v
                        }
                    })
                    .unwrap_or([0.0, 0.0, 1.0]);
                (base, tex_u, tex_v, normal)
            }
            None => ([0.0; 3], [0.0; 3], [0.0; 3], [0.0, 0.0, 1.0]),
        };

        // Resolve the light list (None-terminated run in `lights`).
        // HP2_LIGHTS_OFF=1 disables light contributions (ambient-only
        // diagnostic). shadow_index counts EVERY list slot (the shadow-mask
        // blocks are indexed by list position, not by matched lights).
        let lights_off = std::env::var("HP2_LIGHTS_OFF").is_ok();
        let mut lights: Vec<(usize, LightActor)> = Vec::new();
        if lm.light_actors >= 0 && !lights_off {
            let mut list_index = lm.light_actors as usize;
            let mut shadow_index = 0usize;
            while let Some(reference) = model.lights.get(list_index) {
                list_index += 1;
                if *reference == 0 {
                    break;
                }
                if *reference > 0
                    && let Some(light) = light_cache
                        .entry(*reference)
                        .or_insert_with(|| light_of(*reference))
                {
                    lights.push((shadow_index, *light));
                }
                shadow_index += 1;
            }
        }

        images.push(
            build_lightmap(
                lm, base, tex_u, tex_v, normal, ambient, &lights, &model.light_bits,
            )
            .unwrap_or(LightmapImage {
                width: 1,
                height: 1,
                rgba: vec![128, 128, 128, 255],
                pan: [0.0; 3],
                scale: [1.0, 1.0],
            }),
        );
    }
    images
}

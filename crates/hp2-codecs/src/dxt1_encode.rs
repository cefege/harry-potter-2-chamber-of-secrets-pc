//! DXT1/BC1 block encode, a line-by-line Rust port of the pinned libSquish
//! `CompressMasked` cluster-fit path (`kDxt1 | kColourClusterFit`), matching
//! `HarryPotter2/Unreal/Engine/Src/S3tcCompat.cpp`'s `hp2_s3tc_compress_block`.
//!
//! Byte-identical to native libSquish for identical inputs: same `ColourSet`
//! construction, weighted-covariance principal-axis power iteration, insertion
//! ordering, cluster-fit grid search and endpoint selection, and the same four
//! checked-in single-colour lookup tables. The scalar `Vec4` path (`simd_float.h`)
//! is reproduced; the SSE path produces identical IEEE-754 per-component
//! results, so there is no separate fast path.
//!
//! Acceptance: `Tests/Fixtures/s3tc/encode-v1.bin` (4096 deterministic blocks
//! captured from the native encoder). `encode_matches_native_fixture` requires
//! every 8-byte block to match.

const FLT_MAX: f32 = f32::MAX;
const FLT_EPSILON: f32 = f32::EPSILON;

type SourceBlock = [u8; 3]; // [start, end, error]

include!("dxt1_encode_lookup.rs");

#[derive(Clone, Copy, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    #[inline(always)]
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    #[inline(always)]
    fn splat(s: f32) -> Self {
        Self { x: s, y: s, z: s }
    }
    #[inline(always)]
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    #[inline(always)]
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    #[inline(always)]
    fn mul(self, o: Self) -> Self {
        Self::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    #[inline(always)]
    fn mul_scalar(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
    #[inline(always)]
    fn min(self, o: Self) -> Self {
        Self::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }
    #[inline(always)]
    fn max(self, o: Self) -> Self {
        Self::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
    #[inline(always)]
    fn truncate(self) -> Self {
        fn t(v: f32) -> f32 {
            if v > 0.0 { v.floor() } else { v.ceil() }
        }
        Self::new(t(self.x), t(self.y), t(self.z))
    }
    #[inline(always)]
    fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    #[inline(always)]
    fn length_squared(self) -> f32 {
        self.dot(self)
    }
}

#[derive(Clone, Copy, Default)]
struct Vec4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl Vec4 {
    #[inline(always)]
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
    #[inline(always)]
    fn splat(s: f32) -> Self {
        Self { x: s, y: s, z: s, w: s }
    }
    #[inline(always)]
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z, self.w + o.w)
    }
    #[inline(always)]
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z, self.w - o.w)
    }
    #[inline(always)]
    fn mul(self, o: Self) -> Self {
        Self::new(self.x * o.x, self.y * o.y, self.z * o.z, self.w * o.w)
    }
    #[inline(always)]
    fn splat_x(self) -> Self {
        Self::splat(self.x)
    }
    #[inline(always)]
    fn splat_y(self) -> Self {
        Self::splat(self.y)
    }
    #[inline(always)]
    fn splat_z(self) -> Self {
        Self::splat(self.z)
    }
    #[inline(always)]
    fn splat_w(self) -> Self {
        Self::splat(self.w)
    }
    #[inline(always)]
    fn get_vec3(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
    #[inline(always)]
    fn reciprocal(self) -> Self {
        Self::new(1.0 / self.x, 1.0 / self.y, 1.0 / self.z, 1.0 / self.w)
    }
    #[inline(always)]
    fn min(self, o: Self) -> Self {
        Self::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z), self.w.min(o.w))
    }
    #[inline(always)]
    fn max(self, o: Self) -> Self {
        Self::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z), self.w.max(o.w))
    }
    #[inline(always)]
    fn truncate(self) -> Self {
        fn t(v: f32) -> f32 {
            if v > 0.0 { v.floor() } else { v.ceil() }
        }
        Self::new(t(self.x), t(self.y), t(self.z), t(self.w))
    }
    #[inline(always)]
    fn compare_any_less_than(self, o: Self) -> bool {
        self.x < o.x || self.y < o.y || self.z < o.z || self.w < o.w
    }
}

#[inline(always)]
fn multiply_add(a: Vec4, b: Vec4, c: Vec4) -> Vec4 {
    a.mul(b).add(c) // a*b + c
}

#[inline(always)]
fn negative_multiply_subtract(a: Vec4, b: Vec4, c: Vec4) -> Vec4 {
    c.sub(a.mul(b)) // c - a*b
}

type Sym3x3 = [f32; 6];

fn compute_weighted_covariance(n: usize, points: &[Vec3], weights: &[f32]) -> Sym3x3 {
    let mut total = 0.0f32;
    let mut centroid = Vec3::splat(0.0);
    for i in 0..n {
        total += weights[i];
        centroid = centroid.add(points[i].mul_scalar(weights[i]));
    }
    if total > FLT_EPSILON {
        centroid = centroid.mul_scalar(1.0 / total);
    }

    let mut covariance = [0.0f32; 6];
    for i in 0..n {
        let a = points[i].sub(centroid);
        let b = a.mul_scalar(weights[i]);
        covariance[0] += a.x * b.x;
        covariance[1] += a.x * b.y;
        covariance[2] += a.x * b.z;
        covariance[3] += a.y * b.y;
        covariance[4] += a.y * b.z;
        covariance[5] += a.z * b.z;
    }
    covariance
}

const POWER_ITERATION_COUNT: usize = 8;

fn compute_principle_component(matrix: &Sym3x3) -> Vec3 {
    let row0 = Vec4::new(matrix[0], matrix[1], matrix[2], 0.0);
    let row1 = Vec4::new(matrix[1], matrix[3], matrix[4], 0.0);
    let row2 = Vec4::new(matrix[2], matrix[4], matrix[5], 0.0);
    let mut v = Vec4::splat(1.0);
    for _ in 0..POWER_ITERATION_COUNT {
        let mut w = row0.mul(v.splat_x());
        w = multiply_add(row1, v.splat_y(), w);
        w = multiply_add(row2, v.splat_z(), w);

        let a = w.splat_x().max(w.splat_y().max(w.splat_z()));
        let r = a.reciprocal();
        v = Vec4::new(w.x * r.x, w.y * r.y, w.z * r.z, w.w * r.w);
    }
    v.get_vec3()
}

#[derive(Clone, Copy)]
struct ColourSet {
    count: usize,
    points: [Vec3; 16],
    weights: [f32; 16],
    remap: [i32; 16],
    transparent: bool,
}

impl ColourSet {
    fn new(rgba: &[u8; 64], mask: i32, weight_by_alpha: bool) -> Self {
        const IS_DXT1: bool = true;
        let mut colours = ColourSet {
            count: 0,
            points: [Vec3::default(); 16],
            weights: [0.0; 16],
            remap: [-1; 16],
            transparent: false,
        };

        for i in 0..16 {
            if mask & (1 << i) == 0 {
                colours.remap[i] = -1;
                continue;
            }

            if IS_DXT1 && rgba[4 * i + 3] < 128 {
                colours.remap[i] = -1;
                colours.transparent = true;
                continue;
            }

            let mut j = 0;
            loop {
                if j == i {
                    let x = rgba[4 * i] as f32 / 255.0;
                    let y = rgba[4 * i + 1] as f32 / 255.0;
                    let z = rgba[4 * i + 2] as f32 / 255.0;
                    let w = (rgba[4 * i + 3] as f32 + 1.0) / 256.0;

                    colours.points[colours.count] = Vec3::new(x, y, z);
                    colours.weights[colours.count] = if weight_by_alpha { w } else { 1.0 };
                    colours.remap[i] = colours.count as i32;
                    colours.count += 1;
                    break;
                }

                let oldbit = 1 << j;
                let matches = (mask & oldbit) != 0
                    && rgba[4 * i] == rgba[4 * j]
                    && rgba[4 * i + 1] == rgba[4 * j + 1]
                    && rgba[4 * i + 2] == rgba[4 * j + 2]
                    && (rgba[4 * j + 3] >= 128 || !IS_DXT1);
                if matches {
                    let index = colours.remap[j];
                    let w = (rgba[4 * i + 3] as f32 + 1.0) / 256.0;
                    colours.weights[index as usize] += if weight_by_alpha { w } else { 1.0 };
                    colours.remap[i] = index;
                    break;
                }
                j += 1;
            }
        }

        for i in 0..colours.count {
            colours.weights[i] = colours.weights[i].sqrt();
        }
        colours
    }

    #[inline(always)]
    fn points_slice(&self) -> &[Vec3] {
        &self.points[..self.count]
    }
    #[inline(always)]
    fn weights_slice(&self) -> &[f32] {
        &self.weights[..self.count]
    }

    fn remap_indices(&self, source: &[u8; 16], target: &mut [u8; 16]) {
        for (i, output) in target.iter_mut().enumerate() {
            let j = self.remap[i];
            *output = if j == -1 { 3 } else { source[j as usize] };
        }
    }
}

fn float_to_int(a: f32, limit: i32) -> i32 {
    let mut i = (a + 0.5) as i32;
    if i < 0 {
        i = 0;
    } else if i > limit {
        i = limit;
    }
    i
}

fn float_to_565(colour: Vec3) -> i32 {
    let r = float_to_int(31.0 * colour.x, 31);
    let g = float_to_int(63.0 * colour.y, 63);
    let b = float_to_int(31.0 * colour.z, 31);
    (r << 11) | (g << 5) | b
}

fn write_colour_block(a: i32, b: i32, indices: &[u8; 16], block: &mut [u8; 8]) {
    block[0] = (a & 0xff) as u8;
    block[1] = (a >> 8) as u8;
    block[2] = (b & 0xff) as u8;
    block[3] = (b >> 8) as u8;
    for i in 0..4 {
        let ind = &indices[4 * i..4 * i + 4];
        block[4 + i] = ind[0] | (ind[1] << 2) | (ind[2] << 4) | (ind[3] << 6);
    }
}

fn write_colour_block3(start: Vec3, end: Vec3, indices: &[u8; 16], block: &mut [u8; 8]) {
    let mut a = float_to_565(start);
    let mut b = float_to_565(end);
    let mut remapped = [0u8; 16];
    if a <= b {
        remapped.copy_from_slice(indices);
    } else {
        core::mem::swap(&mut a, &mut b);
        for (i, &idx) in indices.iter().enumerate() {
            remapped[i] = if idx == 0 {
                1
            } else if idx == 1 {
                0
            } else {
                idx
            };
        }
    }
    write_colour_block(a, b, &remapped, block);
}

fn write_colour_block4(start: Vec3, end: Vec3, indices: &[u8; 16], block: &mut [u8; 8]) {
    let mut a = float_to_565(start);
    let mut b = float_to_565(end);
    let mut remapped = [0u8; 16];
    if a < b {
        core::mem::swap(&mut a, &mut b);
        for (i, &idx) in indices.iter().enumerate() {
            remapped[i] = (idx ^ 0x1) & 0x3;
        }
    } else if a == b {
        remapped = [0u8; 16];
    } else {
        remapped.copy_from_slice(indices);
    }
    write_colour_block(a, b, &remapped, block);
}

struct SingleColourFit {
    colour: [u8; 3],
    start: Vec3,
    end: Vec3,
    index: u8,
    error: i32,
    besterror: i32,
}

impl SingleColourFit {
    fn new(colours: &ColourSet) -> Self {
        let value = colours.points[0];
        let mut colour = [0u8; 3];
        colour[0] = float_to_int(255.0 * value.x, 255) as u8;
        colour[1] = float_to_int(255.0 * value.y, 255) as u8;
        colour[2] = float_to_int(255.0 * value.z, 255) as u8;
        SingleColourFit {
            colour,
            start: Vec3::default(),
            end: Vec3::default(),
            index: 0,
            error: 0,
            besterror: i32::MAX,
        }
    }

    fn compute_end_points(&mut self, lookups: [&'static [[SourceBlock; 2]; 256]; 3]) {
        self.error = i32::MAX;
        for index in [0usize, 1] {
            let mut sources = [[0u8; 3]; 3];
            let mut error = 0i32;
            for channel in 0..3 {
                let target = self.colour[channel] as usize;
                sources[channel] = lookups[channel][target][index];
                let diff = sources[channel][2] as i32;
                error += diff * diff;
            }
            if error < self.error {
                self.start = Vec3::new(
                    sources[0][0] as f32 / 31.0,
                    sources[1][0] as f32 / 63.0,
                    sources[2][0] as f32 / 31.0,
                );
                self.end = Vec3::new(
                    sources[0][1] as f32 / 31.0,
                    sources[1][1] as f32 / 63.0,
                    sources[2][1] as f32 / 31.0,
                );
                self.index = (2 * index) as u8;
                self.error = error;
            }
        }
    }

    fn compress3(&mut self, colours: &ColourSet, block: &mut [u8; 8]) {
        self.compute_end_points([&LOOKUP_5_3, &LOOKUP_6_3, &LOOKUP_5_3]);
        if self.error < self.besterror {
            let source = [self.index; 16];
            let mut indices = [0u8; 16];
            colours.remap_indices(&source, &mut indices);
            write_colour_block3(self.start, self.end, &indices, block);
            self.besterror = self.error;
        }
    }

    fn compress4(&mut self, colours: &ColourSet, block: &mut [u8; 8]) {
        self.compute_end_points([&LOOKUP_5_4, &LOOKUP_6_4, &LOOKUP_5_4]);
        if self.error < self.besterror {
            let source = [self.index; 16];
            let mut indices = [0u8; 16];
            colours.remap_indices(&source, &mut indices);
            write_colour_block4(self.start, self.end, &indices, block);
            self.besterror = self.error;
        }
    }
}

struct RangeFit {
    start: Vec3,
    end: Vec3,
    metric: Vec3,
    besterror: f32,
}

impl RangeFit {
    fn new(colours: &ColourSet, metric: Option<[f32; 3]>) -> Self {
        let metric = match metric {
            Some(m) => Vec3::new(m[0], m[1], m[2]),
            None => Vec3::splat(1.0),
        };
        let count = colours.count;
        let values = &colours.points;
        let weights = &colours.weights;
        let covariance = compute_weighted_covariance(count, values, weights);
        let principle = compute_principle_component(&covariance);

        let mut start = Vec3::splat(0.0);
        let mut end = Vec3::splat(0.0);
        if count > 0 {
            start = values[0];
            end = values[0];
            let mut min = values[0].dot(principle);
            let mut max = min;
            for &value in &values[1..count] {
                let val = value.dot(principle);
                if val < min {
                    start = value;
                    min = val;
                } else if val > max {
                    end = value;
                    max = val;
                }
            }
        }

        let one = Vec3::splat(1.0);
        let zero = Vec3::splat(0.0);
        start = one.min(zero.max(start));
        end = one.min(zero.max(end));

        let grid = Vec3::new(31.0, 63.0, 31.0);
        let gridrcp = Vec3::new(1.0 / 31.0, 1.0 / 63.0, 1.0 / 31.0);
        let half = Vec3::splat(0.5);
        let start_fit = grid.mul(start).add(half).truncate().mul(gridrcp);
        let end_fit = grid.mul(end).add(half).truncate().mul(gridrcp);

        RangeFit {
            start: start_fit,
            end: end_fit,
            metric,
            besterror: FLT_MAX,
        }
    }

    fn compress3(&mut self, colours: &ColourSet, block: &mut [u8; 8]) {
        let count = colours.count;
        let values = &colours.points;
        let codes = [self.start, self.end, self.start.add(self.end).mul_scalar(0.5)];
        let mut closest = [0u8; 16];
        let mut error = 0.0f32;
        for i in 0..count {
            let mut dist = FLT_MAX;
            let mut idx = 0;
            for (j, code) in codes.iter().enumerate() {
                let d = self.metric.mul(values[i].sub(*code)).length_squared();
                if d < dist {
                    dist = d;
                    idx = j;
                }
            }
            closest[i] = idx as u8;
            error += dist;
        }
        if error < self.besterror {
            let mut indices = [0u8; 16];
            colours.remap_indices(&closest, &mut indices);
            write_colour_block3(self.start, self.end, &indices, block);
            self.besterror = error;
        }
    }
}

struct ClusterFit {
    snapshot: ColourSet,
    order: [u8; 16 * 8],
    points_weights: [Vec4; 16],
    xsum_wsum: Vec4,
    principle: Vec3,
    metric: Vec4,
    besterror: Vec4,
}

impl ClusterFit {
    fn new(colours: &ColourSet, metric: Option<[f32; 3]>) -> Self {
        let metric = match metric {
            Some(m) => Vec4::new(m[0], m[1], m[2], 1.0),
            None => Vec4::splat(1.0),
        };
        let covariance = compute_weighted_covariance(
            colours.count,
            colours.points_slice(),
            colours.weights_slice(),
        );
        let principle = compute_principle_component(&covariance);
        ClusterFit {
            snapshot: *colours,
            order: [0u8; 16 * 8],
            points_weights: [Vec4::default(); 16],
            xsum_wsum: Vec4::default(),
            principle,
            metric,
            besterror: Vec4::splat(FLT_MAX),
        }
    }

    fn construct_ordering(&mut self, axis: Vec3, iteration: usize) -> bool {
        let count = self.snapshot.count;
        let mut dps = [0.0f32; 16];
        let mut order_work = [0u8; 16];
        for i in 0..count {
            dps[i] = self.snapshot.points[i].dot(axis);
            order_work[i] = i as u8;
        }
        for i in 0..count {
            let mut j = i;
            while j > 0 && dps[j] < dps[j - 1] {
                dps.swap(j, j - 1);
                order_work.swap(j, j - 1);
                j -= 1;
            }
        }

        for it in 0..iteration {
            let prev = &self.order[16 * it..16 * it + 16];
            for i in 0..count {
                if order_work[i] != prev[i] {
                    return false;
                }
            }
        }

        self.order[16 * iteration..16 * iteration + 16].copy_from_slice(&order_work);
        self.xsum_wsum = Vec4::splat(0.0);
        for (i, &ordered) in order_work[..count].iter().enumerate() {
            let j = ordered as usize;
            let p = Vec4::new(
                self.snapshot.points[j].x,
                self.snapshot.points[j].y,
                self.snapshot.points[j].z,
                1.0,
            );
            let w = Vec4::splat(self.snapshot.weights[j]);
            let x = p.mul(w);
            self.points_weights[i] = x;
            self.xsum_wsum = self.xsum_wsum.add(x);
        }
        true
    }

    fn compress3(&mut self, block: &mut [u8; 8]) {
        let count = self.snapshot.count;
        let two = Vec4::splat(2.0);
        let one = Vec4::splat(1.0);
        let half_half2 = Vec4::new(0.5, 0.5, 0.5, 0.25);
        let zero = Vec4::splat(0.0);
        let half = Vec4::splat(0.5);
        let grid = Vec4::new(31.0, 63.0, 31.0, 0.0);
        let gridrcp = Vec4::new(1.0 / 31.0, 1.0 / 63.0, 1.0 / 31.0, 0.0);

        self.construct_ordering(self.principle, 0);

        let mut beststart = Vec4::splat(0.0);
        let mut bestend = Vec4::splat(0.0);
        let mut besterror = self.besterror;
        let mut bestindices = [0u8; 16];
        let mut bestiteration = 0usize;
        let mut besti = 0usize;
        let mut bestj = 0usize;

        let mut iteration_index = 0usize;
        loop {
            let mut part0 = Vec4::splat(0.0);
            for i in 0..count {
                let mut part1 = if i == 0 { self.points_weights[0] } else { Vec4::splat(0.0) };
                let jmin = if i == 0 { 1 } else { i };
                let mut j = jmin;
                loop {
                    let part2 = self.xsum_wsum.sub(part1).sub(part0);

                    let alphax_sum = multiply_add(part1, half_half2, part0);
                    let alpha2_sum = alphax_sum.splat_w();

                    let betax_sum = multiply_add(part1, half_half2, part2);
                    let beta2_sum = betax_sum.splat_w();

                    let alphabeta_sum = part1.mul(half_half2).splat_w();

                    let factor = negative_multiply_subtract(
                        alphabeta_sum,
                        alphabeta_sum,
                        alpha2_sum.mul(beta2_sum),
                    )
                    .reciprocal();
                    let mut a = negative_multiply_subtract(
                        betax_sum,
                        alphabeta_sum,
                        alphax_sum.mul(beta2_sum),
                    )
                    .mul(factor);
                    let mut b = negative_multiply_subtract(
                        alphax_sum,
                        alphabeta_sum,
                        betax_sum.mul(alpha2_sum),
                    )
                    .mul(factor);

                    a = one.min(zero.max(a));
                    b = one.min(zero.max(b));
                    a = multiply_add(grid, a, half).truncate().mul(gridrcp);
                    b = multiply_add(grid, b, half).truncate().mul(gridrcp);

                    let e1 = multiply_add(a.mul(a), alpha2_sum, b.mul(b).mul(beta2_sum));
                    let e2 = negative_multiply_subtract(a, alphax_sum, a.mul(b).mul(alphabeta_sum));
                    let e3 = negative_multiply_subtract(b, betax_sum, e2);
                    let e4 = multiply_add(two, e3, e1);

                    let e5 = e4.mul(self.metric);
                    let error = e5.splat_x().add(e5.splat_y()).add(e5.splat_z());

                    if error.compare_any_less_than(besterror) {
                        beststart = a;
                        bestend = b;
                        besti = i;
                        bestj = j;
                        besterror = error;
                        bestiteration = iteration_index;
                    }

                    if j == count {
                        break;
                    }
                    part1 = part1.add(self.points_weights[j]);
                    j += 1;
                }
                part0 = part0.add(self.points_weights[i]);
            }

            if bestiteration != iteration_index {
                break;
            }
            iteration_index += 1;
            if iteration_index == 1 {
                break;
            }
            let axis = bestend.sub(beststart).get_vec3();
            if !self.construct_ordering(axis, iteration_index) {
                break;
            }
        }

        if besterror.compare_any_less_than(self.besterror) {
            let order = self.order[16 * bestiteration..16 * bestiteration + 16].to_vec();
            let mut unordered = [0u8; 16];
            for m in 0..besti {
                unordered[order[m] as usize] = 0;
            }
            for m in besti..bestj {
                unordered[order[m] as usize] = 2;
            }
            for m in bestj..count {
                unordered[order[m] as usize] = 1;
            }
            self.snapshot.remap_indices(&unordered, &mut bestindices);
            write_colour_block3(beststart.get_vec3(), bestend.get_vec3(), &bestindices, block);
            self.besterror = besterror;
        }
    }

    fn compress4(&mut self, block: &mut [u8; 8]) {
        let count = self.snapshot.count;
        let two = Vec4::splat(2.0);
        let one = Vec4::splat(1.0);
        let onethird_onethird2 = Vec4::new(1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 1.0 / 9.0);
        let twothirds_twothirds2 = Vec4::new(2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 4.0 / 9.0);
        let twonineths = Vec4::splat(2.0 / 9.0);
        let zero = Vec4::splat(0.0);
        let half = Vec4::splat(0.5);
        let grid = Vec4::new(31.0, 63.0, 31.0, 0.0);
        let gridrcp = Vec4::new(1.0 / 31.0, 1.0 / 63.0, 1.0 / 31.0, 0.0);

        self.construct_ordering(self.principle, 0);

        let mut beststart = Vec4::splat(0.0);
        let mut bestend = Vec4::splat(0.0);
        let mut besterror = self.besterror;
        let mut bestindices = [0u8; 16];
        let mut bestiteration = 0usize;
        let mut besti = 0usize;
        let mut bestj = 0usize;
        let mut bestk = 0usize;

        let mut iteration_index = 0usize;
        loop {
            let mut part0 = Vec4::splat(0.0);
            for i in 0..count {
                let mut part1 = Vec4::splat(0.0);
                let mut j = i;
                loop {
                    let mut part2 = if j == 0 { self.points_weights[0] } else { Vec4::splat(0.0) };
                    let kmin = if j == 0 { 1 } else { j };
                    let mut k = kmin;
                    loop {
                        let part3 = self.xsum_wsum.sub(part2).sub(part1).sub(part0);

                        let alphax_sum = multiply_add(
                            part2,
                            onethird_onethird2,
                            multiply_add(part1, twothirds_twothirds2, part0),
                        );
                        let alpha2_sum = alphax_sum.splat_w();

                        let betax_sum = multiply_add(
                            part1,
                            onethird_onethird2,
                            multiply_add(part2, twothirds_twothirds2, part3),
                        );
                        let beta2_sum = betax_sum.splat_w();

                        let alphabeta_sum = twonineths.mul(part1.add(part2).splat_w());

                        let factor = negative_multiply_subtract(
                            alphabeta_sum,
                            alphabeta_sum,
                            alpha2_sum.mul(beta2_sum),
                        )
                        .reciprocal();
                        let mut a = negative_multiply_subtract(
                            betax_sum,
                            alphabeta_sum,
                            alphax_sum.mul(beta2_sum),
                        )
                        .mul(factor);
                        let mut b = negative_multiply_subtract(
                            alphax_sum,
                            alphabeta_sum,
                            betax_sum.mul(alpha2_sum),
                        )
                        .mul(factor);

                        a = one.min(zero.max(a));
                        b = one.min(zero.max(b));
                        a = multiply_add(grid, a, half).truncate().mul(gridrcp);
                        b = multiply_add(grid, b, half).truncate().mul(gridrcp);

                        let e1 = multiply_add(a.mul(a), alpha2_sum, b.mul(b).mul(beta2_sum));
                        let e2 = negative_multiply_subtract(a, alphax_sum, a.mul(b).mul(alphabeta_sum));
                        let e3 = negative_multiply_subtract(b, betax_sum, e2);
                        let e4 = multiply_add(two, e3, e1);

                        let e5 = e4.mul(self.metric);
                        let error = e5.splat_x().add(e5.splat_y()).add(e5.splat_z());

                        if error.compare_any_less_than(besterror) {
                            beststart = a;
                            bestend = b;
                            besterror = error;
                            besti = i;
                            bestj = j;
                            bestk = k;
                            bestiteration = iteration_index;
                        }

                        if k == count {
                            break;
                        }
                        part2 = part2.add(self.points_weights[k]);
                        k += 1;
                    }
                    if j == count {
                        break;
                    }
                    part1 = part1.add(self.points_weights[j]);
                    j += 1;
                }
                part0 = part0.add(self.points_weights[i]);
            }

            if bestiteration != iteration_index {
                break;
            }
            iteration_index += 1;
            if iteration_index == 1 {
                break;
            }
            let axis = bestend.sub(beststart).get_vec3();
            if !self.construct_ordering(axis, iteration_index) {
                break;
            }
        }

        if besterror.compare_any_less_than(self.besterror) {
            let order = self.order[16 * bestiteration..16 * bestiteration + 16].to_vec();
            let mut unordered = [0u8; 16];
            for m in 0..besti {
                unordered[order[m] as usize] = 0;
            }
            for m in besti..bestj {
                unordered[order[m] as usize] = 2;
            }
            for m in bestj..bestk {
                unordered[order[m] as usize] = 3;
            }
            for m in bestk..count {
                unordered[order[m] as usize] = 1;
            }
            self.snapshot.remap_indices(&unordered, &mut bestindices);
            write_colour_block4(beststart.get_vec3(), bestend.get_vec3(), &bestindices, block);
            self.besterror = besterror;
        }
    }

    fn compress(&mut self, block: &mut [u8; 8]) {
        self.compress3(block);
        if !self.snapshot.transparent {
            self.compress4(block);
        }
    }
}

/// Compresses one 4x4 RGBA block to an 8-byte DXT1 block using the
/// `kDxt1 | kColourClusterFit` path, byte-identical to native libSquish.
///
/// `rgba` is 16 pixels × 4 bytes (R, G, B, A), row-major, matching libSquish's
/// source layout. `mask` enables pixels; bit `i` (LSB) enables pixel `i`.
/// `metric` is the optional perceptual metric (`metric[0..3]` in libSquish).
pub fn compress_block(rgba: &[u8; 64], mask: u16, metric: Option<[f32; 3]>, out: &mut [u8; 8]) {
    let colours = ColourSet::new(rgba, mask as i32, false);
    if colours.count == 1 {
        let mut fit = SingleColourFit::new(&colours);
        fit.compress3(&colours, out);
        if !colours.transparent {
            fit.compress4(&colours, out);
        }
    } else if colours.count == 0 {
        let mut fit = RangeFit::new(&colours, metric);
        fit.compress3(&colours, out);
    } else {
        let mut fit = ClusterFit::new(&colours, metric);
        fit.compress(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../../Tests/Fixtures/s3tc/encode-v1.bin");

    #[test]
    fn encode_matches_native_fixture() {
        assert_eq!(FIXTURE.len(), 4096 * 74, "fixture record size changed");
        let mut mismatches = 0usize;
        let mut first = None;
        for i in 0..4096 {
            let rec = &FIXTURE[i * 74..(i + 1) * 74];
            let rgba: [u8; 64] = rec[0..64].try_into().unwrap();
            let mask = u16::from_le_bytes([rec[64], rec[65]]);
            let expected = &rec[66..74];
            let mut out = [0u8; 8];
            compress_block(&rgba, mask, None, &mut out);
            if &out[..] != expected {
                mismatches += 1;
                if first.is_none() {
                    first = Some((i, mask, hex(&out), hex_slice(expected)));
                }
            }
        }
        assert_eq!(mismatches, 0, "{mismatches}/4096 blocks differ; first={first:?}");
    }

    fn hex(b: &[u8; 8]) -> String {
        b.iter().map(|v| format!("{v:02x}")).collect()
    }
    fn hex_slice(b: &[u8]) -> String {
        b.iter().map(|v| format!("{v:02x}")).collect()
    }
}
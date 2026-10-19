// Bit-exact reference for AActor::execTraceTexture's PointInsideQuad (UnActor.cpp, static).
// Expression trees copied from UnMath.h (operator-, operator^, operator|, PlaneDot) and
// the FPlane(V0, ClipNorm) constructor. Build with the engine's contraction setting:
//   clang++ -O3 -ffp-contract=on -std=c++17 pq_bits.cpp -o pq && ./pq > point_inside_quad.txt
// Each case line: "case <idx> in <18 hex> out <planes> <5 hex per plane> <result>".
// Inputs: Verts[4], Normal, Point. Per plane: ClipNorm.X/Y/Z, W, Dot (f32 bit patterns).
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <random>

struct FV { float X, Y, Z; };

static FV sub(FV a, FV b) { return FV{a.X - b.X, a.Y - b.Y, a.Z - b.Z}; }

// operator^ (UnMath.h:681): this ^ V
static FV cross(FV a, FV v) {
	return FV{a.Y * v.Z - a.Z * v.Y, a.Z * v.X - a.X * v.Z, a.X * v.Y - a.Y * v.X};
}

// operator| (UnMath.h:688)
static float dot3(FV a, FV v) { return a.X * v.X + a.Y * v.Y + a.Z * v.Z; }

static uint32_t bits(float f) { uint32_t u; std::memcpy(&u, &f, 4); return u; }

struct Plane { float cn[3]; float w; float dot; };

// PointInsideQuad (UnActor.cpp:1664). Records each computed plane; returns the UBOOL result.
static int point_inside_quad(FV point, const FV verts[4], FV normal, Plane out[4], int* planes) {
	float prev = 0.0f;
	*planes = 0;
	for (int i = 0, j = 3; i < 4; j = i, ++i) {
		FV v0 = verts[j];
		FV v1 = verts[i];
		FV cn = cross(normal, sub(v1, v0));
		float w = dot3(v0, cn);
		// FPlane::PlaneDot (UnMath.h:1021): X*P.X + Y*P.Y + Z*P.Z - W
		float d = cn.X * point.X + cn.Y * point.Y + cn.Z * point.Z - w;
		Plane& p = out[*planes];
		p.cn[0] = cn.X; p.cn[1] = cn.Y; p.cn[2] = cn.Z;
		p.w = w; p.dot = d;
		++*planes;
		if (d * prev < 0.0f) return 0;
		prev = d;
	}
	return 1;
}

int main() {
	std::mt19937 rng(0x5eed1234u);
	std::uniform_real_distribution<float> coord(-1000.0f, 1000.0f);
	std::uniform_real_distribution<float> unit(0.0f, 1.0f);
	const int cases = 200;
	for (int idx = 0; idx < cases; ++idx) {
		FV verts[4];
		for (int k = 0; k < 4; ++k) verts[k] = FV{coord(rng), coord(rng), coord(rng)};
		FV normal{coord(rng), coord(rng), coord(rng)};
		FV point;
		if (idx % 2 == 0) {
			point = FV{coord(rng), coord(rng), coord(rng)};
		} else {
			// Near the edge verts[0] -> verts[1] with a non-dyadic offset.
			float t = unit(rng);
			point = FV{verts[0].X + t * (verts[1].X - verts[0].X) + 1e-3f,
			           verts[0].Y + t * (verts[1].Y - verts[0].Y) - 3e-4f,
			           verts[0].Z + t * (verts[1].Z - verts[0].Z) + 0.1f};
		}
		Plane pl[4];
		int planes = 0;
		int r = point_inside_quad(point, verts, normal, pl, &planes);
		const float in[18] = {verts[0].X, verts[0].Y, verts[0].Z, verts[1].X, verts[1].Y, verts[1].Z,
		                      verts[2].X, verts[2].Y, verts[2].Z, verts[3].X, verts[3].Y, verts[3].Z,
		                      normal.X, normal.Y, normal.Z, point.X, point.Y, point.Z};
		std::printf("case %d in", idx);
		for (float f : in) std::printf(" %08x", bits(f));
		std::printf(" out %d", planes);
		for (int p = 0; p < planes; ++p) {
			for (int c = 0; c < 3; ++c) std::printf(" %08x", bits(pl[p].cn[c]));
			std::printf(" %08x %08x", bits(pl[p].w), bits(pl[p].dot));
		}
		std::printf(" %d\n", r);
	}
	return 0;
}

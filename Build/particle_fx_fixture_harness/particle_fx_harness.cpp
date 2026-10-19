// Fixture generator for the AParticleFX replay (UnParticleFX.cpp, oracle ref 5782e7f).
//
// This TU does NOT include UnParticleFX.cpp, UnParticle.cpp, UnParticleList.cpp or UnAnsi.cpp. It
// links the ORIGINAL objects built by gen_particle_fx.sh:
//   UnParticleFX.o     from 5782e7f
//   UnParticle.o, UnParticleList.o, UnMath.o, UnAnsi.o   from 452b122
// Every other undefined symbol is provided by a generated stub that prints "STUB <name>" and
// aborts, so a stub firing is a failure. The fakes below are limited to engine hooks the oracle
// calls; each is documented next to its definition.
//
// Random numbers: the oracle's appFrand/appRandRange come from the real UnAnsi.o, which draws from
// libc rand(). This harness calls srand(seed) at the start of every case, so the Rust replay calls
// srand/rand from the same libc and reproduces the stream without a port.
//
// Build flags: product flags from compile_commands plus -O3 -g -ftrivial-auto-var-init=zero.
// -ftrivial-auto-var-init=zero is kept from the original oracle build: it zero-fills automatic
// variables the oracle reads before writing (Tail, ChaosDelay, Spin in AddParticle).
//
// Usage: particle_fx_harness <emit> <bbox> <update> <params> <trig>
//
// Output format (all files):
//   defaults <fx-state>                      parent class defaults (ctor state), not a record
//   case <family> <name> <knobs> <fx-state> <extras>   state before populate, not a record
//   <log lines>                              "stat cycle <k>", "stat emitted <n>", "destroy"
//   <record lines>                           counted by the header
//   end <family> <name> destroys=<n> editor=<e> fx_flags=<hex> owner_flags=<hex>
//   header <family> cases=<N> records=<M>    written first; M counts record lines only
//
// <knobs>    = seed(hex16) editor(dec) level_time(f) zone_gravity(v) hit_kind(dec) base_kind(dec)
// <fx-state> = fx_flags(hex8) owner_present(dec) [owner_flags(hex8) owner_velocity(v)]
//              lod_bias(f) draw_scale(f) draw_type(dec) distribution(dec)
//              then FFloatParams Base,Rand (f) in this order: ParticlesPerSec, Period,
//              SourceWidth, SourceHeight, SourceDepth, Decay, AngularSpreadWidth,
//              AngularSpreadHeight, Speed, Lifetime, AlphaStart, AlphaEnd, SizeWidth,
//              SizeLength, SizeEndScale, SpinRate, DripTime
//              then ColorStart.Base, ColorStart.Rand, ColorEnd.Base, ColorEnd.Rand (4 dec bytes)
//              gravity(v) damping wind_modifier gravity_modifier parent_blend chaos chaos_delay
//              elasticity alpha_delay color_delay size_delay alpha_grow_period size_grow_period
//              emit_delay emission_residue (f) particles_max(dec) color_palette(0 none,1 without,
//              2 with) region_zone(0/1) pattern(0/1) b_prime b_system_relative b_velocity_relative
//              b_wind_per_particle b_emit b_shell_only b_steady_state b_update particle_list(0/1)
//              render_primitive(dec) lod(f) elapsed_time(f) age(f) particles_emitted(dec)
//              particles_alive(dec) current_priority_tag(dec) last_update_location(v)
//              last_emit_location(v) location(v)
// <extras>   per family, see each driver.

#include "EnginePrivate.h"
#include "UnParticle.h"
#include "UnParticleList.h"
#include "UnMesh.h"
#include "UnRender.h"
#include "UnStat.h"

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <new>
#include <string>
#include <vector>

// ---------------------------------------------------------------------------
// Globals the oracle objects reference. Not defined in the linked set.
// ---------------------------------------------------------------------------

FGlobalMath GMath;
FStats GStat;
UBOOL GIsEditor = 0;

// Per-case world state, set by begin_case().
static int g_destroys = 0;
static int g_hit_kind = 0;
static bool g_trace = false;
static std::vector<std::string> g_log;
static DWORD g_cycles = 0;

// Output buffer for the current family.
static std::vector<std::string> g_lines;
static int g_records = 0;
static int g_cases = 0;

// KGen: the C++-only draw source for fixture knobs. Rust never draws from it.
static uint64_t kgen_next(uint64_t& s) {
	s = s * 6364136223846793005ULL + 1442695040888963407ULL;
	return (uint32_t)(s >> 40);
}
struct KGen {
	uint64_t s;
	explicit KGen(uint64_t seed) : s(seed) {}
	uint32_t D(uint32_t n) { return kgen_next(s) % n; }
};

// Floats are integer multiples of a non-dyadic step (step + 0.0137f), so products and sums of
// generated inputs round in f32 and fused-vs-unfused operations are distinguishable.
static float KF(uint32_t n, float step) { return (float)n * (step + 0.0137f); }
static float kf(KGen& kg, uint32_t add, uint32_t mod, float step) { return KF(add + kg.D(mod), step); }

// Fake allocator: malloc-backed, no bookkeeping. Used by the oracle's TArray growth.
class HarnessMalloc : public FMalloc {
public:
	void* Malloc(DWORD Count, const TCHAR*) { return calloc(1, Count ? Count : 1); }
	void* Realloc(void* Ptr, DWORD Count, const TCHAR*) { return realloc(Ptr, Count ? Count : 1); }
	void Free(void* Ptr) { free(Ptr); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc g_malloc;
FMalloc* GMalloc = &g_malloc;

// appFrand, appRandRange (both overloads), appExp, appSin, appCos, appSqrt, appPow, appMemcpy and
// appMemmove are defined by the real UnAnsi.o and are NOT defined here.

// Stat timer thunk. clock() subtracts and unclock() adds appCycles()-34; the value is never
// stored in a fixture, only the call order is traced ("stat cycle <k>").
DWORD appCycles() {
	++g_cycles;
	if (g_trace) {
		char b[32];
		snprintf(b, sizeof b, "stat cycle %u", (unsigned)g_cycles);
		g_log.push_back(b);
	}
	return g_cycles;
}

// Super bodies: AActor methods the oracle calls through Super:: are outside the slice.
// Documented gap; AActor::Tick returns 1 (UE convention: continue ticking).
void AActor::InitExecution() {}
UBOOL AActor::Tick(FLOAT, ELevelTick) { return 1; }
UPrimitive* AActor::GetPrimitive() const { return NULL; }
void AActor::Destroy() {}

// Object construction: UObject base ctor/dtor are no-ops (no package/linker machinery).
UObject::UObject() {}
UObject::~UObject() {}
// UMesh ctor: the harness mesh supplies its own frame/bbox/tri queries; UMesh's member defaults are unused.
UMesh::UMesh() {}

// UObject::IsA is inline in UnType.h and walks SuperField; no harness definition.

// Particle list allocation: StaticAllocateObject returns zeroed storage large enough for
// UParticleListPriv (the private class is not visible here). The real placement ctor runs.
UObject* UObject::StaticAllocateObject(UClass*, UObject*, FName, DWORD, UObject*, FOutputDevice*, UObject*) { return (UObject*)calloc(1, 4096); }

// Wind: static query used by the oracle's wind term.
FVector AWind::GetTotalWind(ULevel*, const FVector& Loc) {
	return FVector(Loc.Y * 0.5f, -Loc.X * 0.25f, 3.0f);
}

// Level geometry fast check: UE convention, nonzero means clear.
BYTE UModel::FastLineCheck(FVector End, FVector Start) {
	return (((int)End.X ^ (int)Start.Y) & 3) != 0;
}

// Hit actor for the trace: kind 0 is the level info (IsA ALevelInfo), kind 1 is a non-level actor
// whose class chain ends at NULL.
static AActor* g_other_actor = nullptr;
static ALevelInfo* g_levelinfo = nullptr;
static AActor* hit_actor() { return g_hit_kind ? g_other_actor : (AActor*)g_levelinfo; }

// ---------------------------------------------------------------------------
// Engine objects. Level and mesh have vtables (overridden virtuals) and are built with the
// real ctors; the rest are zeroed storage (no virtuals reached).
// ---------------------------------------------------------------------------

class HarnessLevel : public ULevel {
public:
	// Level destroy: the oracle's DestroyActor branch is a covered path. Counted per case and logged.
	UBOOL DestroyActor(AActor*, UBOOL = 0) {
		++g_destroys;
		if (g_trace)
			g_log.push_back("destroy");
		return 1;
	}
	// Trace hook. Hit when sel in {1,3}; sets Hit.Actor to hit_actor(). UE convention: 0 = hit,
	// 1 = clear.
	UBOOL SingleLineCheck(FCheckResult& Hit, AActor*, const FVector& End, const FVector& Start, DWORD, FVector, BYTE) {
		int sel = ((int)End.Y ^ (int)Start.Z) & 3;
		if (sel != 1 && sel != 3)
			return 1;
		FVector N;
		switch (((int)End.X) & 3) {
		case 0: N = FVector(0.0f, 0.0f, 1.0f); break;
		case 1: N = FVector(1.0f, 0.0f, 0.0f); break;
		case 2: N = FVector(0.0f, -1.0f, 0.0f); break;
		default: N = FVector(0.0f, 0.0f, -1.0f); break;
		}
		Hit.Normal = N;
		Hit.Location = FVector(End.X * 0.5f, End.Y * 0.5f, End.Z * 0.5f);
		Hit.Actor = hit_actor();
		return 0;
	}
};

class HarnessMesh : public UMesh {
public:
	FCoords GetRenderBoundingBox(const AActor*, UBOOL) {
		FCoords C;
		C.Origin = FVector(0.0f, 0.0f, 0.0f);
		C.XAxis = FVector(30.0f, 0.0f, 0.0f);
		C.YAxis = FVector(0.0f, 20.0f, 0.0f);
		C.ZAxis = FVector(0.0f, 0.0f, 10.0f);
		return C;
	}
	void GetFrame(FVector* Verts, INT Size, FCoords, AActor*) {
		for (INT k = 0; k < 8; k++) {
			FVector* V = (FVector*)((BYTE*)Verts + Size * k);
			*V = FVector(1.5f * (float)k, -0.5f * (float)k, 2.0f * (float)k + 1.0f);
		}
	}
	INT GetNumTris() const { return 6; }
	void GetTriVerts(INT T, INT Verts[3]) const {
		Verts[0] = T % 8;
		Verts[1] = (T + 1) % 8;
		Verts[2] = (T + 2) % 8;
	}
};

// Shared fixture world, built once in init_world().
static HarnessLevel* g_level = nullptr;
static AZoneInfo* g_zone = nullptr;
static HarnessMesh* g_mesh = nullptr;
static UPalette* g_palette = nullptr;
static UTexture* g_tex_with = nullptr;
static UTexture* g_tex_without = nullptr;
static UGesture* g_gesture = nullptr;

// Class objects reached through GetClass()->GetSuperClass(). PropertiesSize and Defaults match.
alignas(16) static unsigned char g_parent_mem[sizeof(AParticleFX)];
alignas(16) static unsigned char g_fx_class_mem[sizeof(UClass)];
alignas(16) static unsigned char g_parent_class_mem[sizeof(UClass)];
alignas(16) static unsigned char g_other_class_mem[sizeof(UClass)];
alignas(16) static unsigned char g_levelinfo_class_mem[sizeof(UClass)];
alignas(16) static unsigned char g_plist_class_mem[sizeof(UClass)];
alignas(16) static unsigned char g_plistpriv_class_mem[sizeof(UClass)];
// UParticleListPriv is private to UnParticleList.cpp and not nameable here. Its PrivateStaticClass is
// bound by the mangled (Mach-O) symbol; the placement-new in UParticleList::Create reads it via StaticClass().
extern UClass* g_plistpriv_static_class __asm__("__ZN17UParticleListPriv18PrivateStaticClassE");
// Class registration: StaticClass() returns PrivateStaticClass when set, so the generated
// GetPrivateStaticClass/InitializePrivateStaticClass (in unlinked TUs) never run. The zeroed class has
// no SuperField, so IsA(ALevelInfo::StaticClass()) holds for the level-info hit only.
UClass* ALevelInfo::PrivateStaticClass = (UClass*)g_levelinfo_class_mem;

static void draw_color(FColor& c, KGen& kg);

static void init_world() {
	AParticleFX::PrivateStaticClass = (UClass*)g_fx_class_mem;
	UParticleList::PrivateStaticClass = (UClass*)g_plist_class_mem;
	g_plistpriv_static_class = (UClass*)g_plistpriv_class_mem;
	g_level = ::new (calloc(1, sizeof(HarnessLevel))) HarnessLevel();
	g_mesh = ::new (calloc(1, sizeof(HarnessMesh))) HarnessMesh();
	g_mesh->FrameVerts = 8;

	g_levelinfo = (ALevelInfo*)calloc(1, sizeof(ALevelInfo));
	g_levelinfo->Class = ALevelInfo::StaticClass();
	g_levelinfo->TimeSeconds = 1.0f;
	g_levelinfo->ZoneGravity = FVector(0.0f, 0.0f, -300.0f);

	g_other_actor = (AActor*)calloc(1, sizeof(AActor));
	g_other_actor->Class = (UClass*)g_other_class_mem;

	g_zone = (AZoneInfo*)calloc(1, sizeof(AZoneInfo));
	g_zone->ZoneGravity = FVector(0.0f, 0.0f, -50.0f);

	// Palette: every entry is non-zero, including Colors(0), so the palette branch has a visible
	// colour at index 0.
	g_palette = (UPalette*)calloc(1, sizeof(UPalette));
	g_palette->Colors.Add(256);
	for (int k = 0; k < 256; k++) {
		BYTE* b = (BYTE*)&g_palette->Colors(k);
		b[0] = (BYTE)(7 * k + 1);
		b[1] = (BYTE)(3 * k + 2);
		b[2] = (BYTE)(5 * k + 3);
		b[3] = (BYTE)(11 * k + 4);
	}
	g_tex_with = (UTexture*)calloc(1, sizeof(UTexture));
	g_tex_with->Palette = g_palette;
	g_tex_without = (UTexture*)calloc(1, sizeof(UTexture));
	g_tex_without->Palette = NULL;

	g_gesture = (UGesture*)calloc(1, sizeof(UGesture));
	g_gesture->Points.Add(4);
	g_gesture->Points(0) = FVector(0.0f, 0.0f, 0.0f);
	g_gesture->Points(1) = FVector(0.5f, 0.0f, 0.0f);
	g_gesture->Points(2) = FVector(0.5f, 1.0f, 0.0f);
	g_gesture->Points(3) = FVector(0.0f, 1.0f, 0.0f);

	UClass* FxClass = (UClass*)g_fx_class_mem;
	UClass* ParentClass = (UClass*)g_parent_class_mem;
	FxClass->SuperField = (UField*)ParentClass;
	FxClass->PropertiesSize = sizeof(AParticleFX);
	ParentClass->PropertiesSize = sizeof(AParticleFX);
	ParentClass->Defaults.Data = g_parent_mem;
	ParentClass->Defaults.ArrayNum = sizeof(AParticleFX);
	ParentClass->Defaults.ArrayMax = sizeof(AParticleFX);

	// Parent defaults: the ctor's state, with the class's DrawType set the way the class defaults
	// are copied in StaticConstructObject.
	AParticleFX* d = (AParticleFX*)g_parent_mem;
	d->DrawType = DT_Particles;
	::new (d) AParticleFX();
	// Class defaultproperties for the GetSysParams-blended params. The ctor alone leaves them zero,
	// which makes every parent-blend product exact and hides fused-vs-unfused differences. Drawn
	// from a fixed separate stream so the per-case draws are unchanged.
	KGen pk(0x9a7a0005ULL);
	FFloatParams* blended[] = {
		&d->AngularSpreadWidth, &d->AngularSpreadHeight, &d->SourceWidth, &d->SourceDepth,
		&d->SourceHeight, &d->Speed, &d->Lifetime, &d->AlphaStart, &d->AlphaEnd, &d->SpinRate,
		&d->DripTime, &d->SizeWidth, &d->SizeLength, &d->SizeEndScale,
	};
	for (FFloatParams* p : blended) {
		p->Base = kf(pk, 0, 64, 0.125f);
		p->Rand = kf(pk, 0, 32, 0.125f);
	}
	draw_color(d->ColorStart.Base, pk);
	draw_color(d->ColorStart.Rand, pk);
	draw_color(d->ColorEnd.Base, pk);
	draw_color(d->ColorEnd.Rand, pk);
}

// ---------------------------------------------------------------------------
// Case construction. Draw order is authoritative for the C++ generator only.
// ---------------------------------------------------------------------------

struct CaseKnobs {
	uint64_t seed;
	int editor;
	float level_time;
	FVector zone_gravity;
	int hit_kind;
	int base_kind; // 0 NULL, 1 Base==Owner (sync-owner branch), 2 other actor
	uint32_t owner_flags; // RF_Transient (0x4000) enables the sync-owner transient branch
};

static CaseKnobs draw_knobs(KGen& kg, uint64_t seed) {
	CaseKnobs k;
	k.seed = seed;
	k.editor = (int)kg.D(2);
	k.level_time = kf(kg, 1, 16, 0.0625f);
	k.zone_gravity.X = (float)((int)kg.D(5) - 2) * 0.25f;
	k.zone_gravity.Y = (float)((int)kg.D(5) - 2) * 0.25f;
	k.zone_gravity.Z = -(KF(kg.D(16), 0.75f) + 0.5f);
	k.hit_kind = (int)kg.D(2);
	k.base_kind = (int)kg.D(3);
	k.owner_flags = kg.D(2) ? 0x4000u : 0u;
	return k;
}

// srand(seed) happens here, before the ctor, matching the Rust replay's srand at the same point.
static AParticleFX* make_fx(const CaseKnobs& k) {
	srand((unsigned)k.seed);
	AParticleFX* fx = (AParticleFX*)calloc(1, sizeof(AParticleFX));
	// StaticConstructObject copies class defaults before the ctor runs. CreateParticleList checks
	// DrawType == DT_Particles, so DrawType is set here, not in the ctor.
	fx->DrawType = DT_Particles;
	fx->Class = (UClass*)g_fx_class_mem;
	fx->XLevel = g_level;
	fx->Level = g_levelinfo;
	fx->LODBias = 1.0f;
	::new (fx) AParticleFX(); // ctor: CreateParticleList, Age, ElapsedTime, bUpdate, LOD=1

	AActor* owner = (AActor*)calloc(1, sizeof(AActor));
	owner->Mesh = g_mesh;
	owner->ObjectFlags = k.owner_flags;
	fx->Owner = owner;
	if (k.base_kind == 1)
		fx->Base = owner;
	else if (k.base_kind == 2)
		fx->Base = g_other_actor;
	else
		fx->Base = NULL;
	return fx;
}

static void set_color_bytes(FColor& c, BYTE b0, BYTE b1, BYTE b2, BYTE b3) {
	BYTE* p = (BYTE*)&c;
	p[0] = b0;
	p[1] = b1;
	p[2] = b2;
	p[3] = b3;
}

// Each color is four sequential single-byte draws in memory order.
static void draw_color(FColor& c, KGen& kg) {
	BYTE b0 = (BYTE)kg.D(256);
	BYTE b1 = (BYTE)kg.D(256);
	BYTE b2 = (BYTE)kg.D(256);
	BYTE b3 = (BYTE)kg.D(256);
	set_color_bytes(c, b0, b1, b2, b3);
}

static void build_case(AParticleFX* fx, KGen& kg) {
	fx->Distribution = (BYTE)kg.D(3);

	uint32_t pause = kg.D(8);
	uint32_t pb = kg.D(24);
	uint32_t pr = kg.D(4);
	fx->ParticlesPerSec.Base = pause == 0 ? 0.0f : KF(pb, 0.5f);
	fx->ParticlesPerSec.Rand = KF(pr, 0.5f);

	uint32_t K = 1 + kg.D(4);
	fx->Period.Base = KF(K, 0.25f);
	fx->Period.Rand = kf(kg, 0, 4, 0.125f);

	fx->SourceWidth.Base = kf(kg, 0, 8, 0.5f);
	fx->SourceWidth.Rand = kf(kg, 0, 4, 0.25f);
	fx->SourceHeight.Base = kf(kg, 0, 8, 0.5f);
	fx->SourceHeight.Rand = kf(kg, 0, 4, 0.25f);
	fx->SourceDepth.Base = kf(kg, 0, 8, 0.5f);
	fx->SourceDepth.Rand = kf(kg, 0, 4, 0.25f);

	fx->AngularSpreadWidth.Base = kf(kg, 0, 90, 1.0f);
	fx->AngularSpreadWidth.Rand = kf(kg, 0, 20, 1.0f);
	fx->AngularSpreadHeight.Base = kf(kg, 0, 90, 1.0f);
	fx->AngularSpreadHeight.Rand = kf(kg, 0, 20, 1.0f);

	fx->Speed.Base = kf(kg, 0, 160, 0.5f);
	fx->Speed.Rand = kf(kg, 0, 40, 0.5f);

	fx->Lifetime.Base = kf(kg, 1, 20, 0.25f);
	fx->Lifetime.Rand = kf(kg, 0, 8, 0.25f);

	fx->AlphaStart.Base = kf(kg, 0, 5, 0.25f);
	fx->AlphaStart.Rand = kf(kg, 0, 3, 0.25f);
	fx->AlphaEnd.Base = kf(kg, 0, 5, 0.25f);
	fx->AlphaEnd.Rand = kf(kg, 0, 3, 0.25f);

	fx->SizeWidth.Base = kf(kg, 1, 16, 0.5f);
	fx->SizeWidth.Rand = kf(kg, 0, 8, 0.25f);
	fx->SizeLength.Base = kf(kg, 1, 16, 0.5f);
	fx->SizeLength.Rand = kf(kg, 0, 8, 0.25f);
	fx->SizeEndScale.Base = kf(kg, 0, 8, 0.25f);
	fx->SizeEndScale.Rand = kf(kg, 0, 4, 0.125f);

	fx->SpinRate.Base = (float)((int)kg.D(8) - 4);
	fx->SpinRate.Rand = kf(kg, 0, 4, 0.5f);

	fx->DripTime.Base = kf(kg, 0, 8, 0.25f);
	fx->DripTime.Rand = kf(kg, 0, 4, 0.25f);

	fx->Decay.Base = kf(kg, 0, 8, 0.25f);
	fx->Decay.Rand = kf(kg, 0, 4, 0.25f);

	draw_color(fx->ColorStart.Base, kg);
	draw_color(fx->ColorStart.Rand, kg);
	draw_color(fx->ColorEnd.Base, kg);
	draw_color(fx->ColorEnd.Rand, kg);

	fx->Gravity.X = (float)((int)kg.D(5) - 2);
	fx->Gravity.Y = (float)((int)kg.D(5) - 2);
	fx->Gravity.Z = -(float)(kg.D(4) * 10);

	fx->Damping = kf(kg, 0, 8, 0.05f);
	fx->WindModifier = kf(kg, 0, 4, 0.5f);
	fx->GravityModifier = kf(kg, 0, 4, 0.5f);
	fx->ParentBlend = kf(kg, 0, 5, 0.25f);
	fx->Chaos = kf(kg, 0, 4, 0.5f);
	fx->ChaosDelay = kf(kg, 0, 4, 0.25f);
	fx->Elasticity = kf(kg, 0, 4, 0.25f);
	fx->AlphaDelay = kf(kg, 0, 4, 0.25f);
	fx->ColorDelay = kf(kg, 0, 4, 0.25f);
	fx->SizeDelay = kf(kg, 0, 4, 0.25f);
	fx->AlphaGrowPeriod = kf(kg, 0, 4, 0.25f);
	fx->SizeGrowPeriod = kf(kg, 0, 4, 0.25f);
	fx->EmitDelay = kf(kg, 0, 4, 0.125f);
	fx->EmissionResidue = kf(kg, 0, 4, 0.125f);

	// Emission cap: small so the per-case record volume stays reviewable. The knob still consumes its
	// draw. 1+D(5) gives 1..32 (the uncapped-looking branch); the other branch is a fixed 32.
	uint32_t pm = kg.D(4);
	if (pm == 0)
		fx->ParticlesMax = (INT)(1 + kg.D(5));
	else
		fx->ParticlesMax = 32;

	uint32_t pal = kg.D(3);
	if (pal == 0)
		fx->ColorPalette = NULL;
	else if (pal == 1)
		fx->ColorPalette = g_tex_without;
	else
		fx->ColorPalette = g_tex_with;

	fx->Region.Zone = kg.D(3) == 0 ? g_zone : NULL;
	fx->Pattern = kg.D(3) == 0 ? g_gesture : NULL;

	fx->bPrime = kg.D(3) == 0;
	fx->bSystemRelative = kg.D(2);
	fx->bVelocityRelative = kg.D(2);
	fx->bWindPerParticle = kg.D(2);
	fx->bEmit = kg.D(8) != 0;
	fx->bShellOnly = kg.D(4) == 0;
	fx->bSteadyState = kg.D(2);

	fx->RenderPrimitive = (BYTE)kg.D(4);
	fx->DrawScale = kf(kg, 1, 8, 0.25f);
	fx->LOD = kf(kg, 0, 5, 0.25f);

	fx->Owner->Velocity.X = (float)((int)kg.D(5) - 2) * 10.0f;
	fx->Owner->Velocity.Y = (float)((int)kg.D(5) - 2) * 10.0f;
	fx->Owner->Velocity.Z = (float)((int)kg.D(5) - 2) * 10.0f;
	fx->LODBias = kf(kg, 1, 8, 0.25f);
}

// Sets the per-case world (editor flag, level time, zone gravity, hit kind, destroy count) and
// constructs the case's fx. Callers may override fields before calling emit_case().
static AParticleFX* begin_case(const CaseKnobs& k, bool build, KGen& kg) {
	GIsEditor = (UBOOL)k.editor;
	g_levelinfo->TimeSeconds = k.level_time;
	g_levelinfo->ZoneGravity = k.zone_gravity;
	g_hit_kind = k.hit_kind;
	g_destroys = 0;
	g_cycles = 0;
	g_log.clear();
	g_trace = false;
	AParticleFX* fx = make_fx(k);
	if (build)
		build_case(fx, kg);
	return fx;
}

// ---------------------------------------------------------------------------
// Output helpers. Field order matches the put_* order in particle_fx_replay.rs.
// ---------------------------------------------------------------------------

static uint32_t bits(float v) {
	uint32_t u;
	memcpy(&u, &v, 4);
	return u;
}
static void pf(std::string& s, float v) {
	char b[16];
	snprintf(b, sizeof b, " %08x", bits(v));
	s += b;
}
static void pv(std::string& s, const FVector& v) {
	pf(s, v.X);
	pf(s, v.Y);
	pf(s, v.Z);
}
static void pp(std::string& s, const FPlane& p) {
	pf(s, p.X);
	pf(s, p.Y);
	pf(s, p.Z);
	pf(s, p.W);
}
static void pcol(std::string& s, const FColor& c) {
	const BYTE* b = (const BYTE*)&c;
	char t[32];
	snprintf(t, sizeof t, " %u %u %u %u", (unsigned)b[0], (unsigned)b[1], (unsigned)b[2], (unsigned)b[3]);
	s += t;
}
static void pcoords(std::string& s, const FCoords& c) {
	pv(s, c.Origin);
	pv(s, c.XAxis);
	pv(s, c.YAxis);
	pv(s, c.ZAxis);
}
static void pi(std::string& s, long long v) {
	char t[32];
	snprintf(t, sizeof t, " %lld", v);
	s += t;
}
static void pff(std::string& s, const FFloatParams& p) {
	pf(s, p.Base);
	pf(s, p.Rand);
}

static void put_particle(std::string& s, const UParticle& p) {
	pv(s, p.Position);
	pv(s, p.Velocity);
	pf(s, p.Lifetime);
	pf(s, p.Age);
	pf(s, p.Alpha);
	pf(s, p.AlphaFull);
	pf(s, p.AlphaDelta);
	pp(s, p.Color);
	pp(s, p.ColorDelta);
	pf(s, p.Width);
	pf(s, p.WidthFull);
	pf(s, p.WidthDelta);
	pf(s, p.Length);
	pf(s, p.LengthFull);
	pf(s, p.LengthDelta);
	pf(s, p.Tail);
	pf(s, p.DripTimer);
	pf(s, p.ChaosDelay);
	pf(s, p.Spin);
	pf(s, p.SpinRate);
	char t[32];
	snprintf(t, sizeof t, " %d %d", (int)p.Id, (int)p.PriorityTag);
	s += t;
}

static void put_sys_params(std::string& s, const AParticleFX& r) {
	pff(s, r.AngularSpreadWidth);
	pff(s, r.AngularSpreadHeight);
	pff(s, r.SourceWidth);
	pff(s, r.SourceDepth);
	pff(s, r.SourceHeight);
	pff(s, r.Speed);
	pff(s, r.Lifetime);
	pff(s, r.AlphaStart);
	pff(s, r.AlphaEnd);
	pff(s, r.SpinRate);
	pff(s, r.DripTime);
	pff(s, r.SizeWidth);
	pff(s, r.SizeLength);
	pff(s, r.SizeEndScale);
	s += ' ';
	pcol(s, r.ColorStart.Base);
	s += ' ';
	pcol(s, r.ColorStart.Rand);
	s += ' ';
	pcol(s, r.ColorEnd.Base);
	s += ' ';
	pcol(s, r.ColorEnd.Rand);
}

static void put_fparams(std::string& s, const FParams& p) {
	pf(s, p.ParticlesPerSec);
	pf(s, p.SourceWidth);
	pf(s, p.SourceHeight);
	pf(s, p.SourceDepth);
	pf(s, p.Period);
	pf(s, p.Decay);
	pf(s, p.AngularSpreadWidth);
	pf(s, p.AngularSpreadHeight);
	pf(s, p.Speed);
	pf(s, p.Lifetime);
	pf(s, p.AlphaStart);
	pf(s, p.AlphaEnd);
	pf(s, p.SizeWidth);
	pf(s, p.SizeLength);
	pf(s, p.SizeEndScale);
	pf(s, p.SpinRate);
	pf(s, p.DripTime);
	s += ' ';
	pcol(s, p.ColorStart);
	s += ' ';
	pcol(s, p.ColorEnd);
}

// Full fx state, in the order documented in the file header.
static void put_fx_state(std::string& s, const AParticleFX& f) {
	char t[48];
	snprintf(t, sizeof t, " %08x", (unsigned)f.ObjectFlags);
	s += t;
	if (f.Owner) {
		s += " 1";
		snprintf(t, sizeof t, " %08x", (unsigned)f.Owner->ObjectFlags);
		s += t;
		pv(s, f.Owner->Velocity);
	} else {
		s += " 0";
	}
	pf(s, f.LODBias);
	pf(s, f.DrawScale);
	pi(s, (long long)f.DrawType);
	pi(s, (long long)f.Distribution);

	pff(s, f.ParticlesPerSec);
	pff(s, f.Period);
	pff(s, f.SourceWidth);
	pff(s, f.SourceHeight);
	pff(s, f.SourceDepth);
	pff(s, f.Decay);
	pff(s, f.AngularSpreadWidth);
	pff(s, f.AngularSpreadHeight);
	pff(s, f.Speed);
	pff(s, f.Lifetime);
	pff(s, f.AlphaStart);
	pff(s, f.AlphaEnd);
	pff(s, f.SizeWidth);
	pff(s, f.SizeLength);
	pff(s, f.SizeEndScale);
	pff(s, f.SpinRate);
	pff(s, f.DripTime);
	s += ' ';
	pcol(s, f.ColorStart.Base);
	s += ' ';
	pcol(s, f.ColorStart.Rand);
	s += ' ';
	pcol(s, f.ColorEnd.Base);
	s += ' ';
	pcol(s, f.ColorEnd.Rand);

	pv(s, f.Gravity);
	pf(s, f.Damping);
	pf(s, f.WindModifier);
	pf(s, f.GravityModifier);
	pf(s, f.ParentBlend);
	pf(s, f.Chaos);
	pf(s, f.ChaosDelay);
	pf(s, f.Elasticity);
	pf(s, f.AlphaDelay);
	pf(s, f.ColorDelay);
	pf(s, f.SizeDelay);
	pf(s, f.AlphaGrowPeriod);
	pf(s, f.SizeGrowPeriod);
	pf(s, f.EmitDelay);
	pf(s, f.EmissionResidue);
	pi(s, (long long)f.ParticlesMax);

	int palette = f.ColorPalette == NULL ? 0 : (f.ColorPalette == g_tex_without ? 1 : 2);
	pi(s, palette);
	pi(s, f.Region.Zone == g_zone ? 1 : 0);
	pi(s, f.Pattern == g_gesture ? 1 : 0);

	pi(s, f.bPrime ? 1 : 0);
	pi(s, f.bSystemRelative ? 1 : 0);
	pi(s, f.bVelocityRelative ? 1 : 0);
	pi(s, f.bWindPerParticle ? 1 : 0);
	pi(s, f.bEmit ? 1 : 0);
	pi(s, f.bShellOnly ? 1 : 0);
	pi(s, f.bSteadyState ? 1 : 0);
	pi(s, f.bUpdate ? 1 : 0);
	pi(s, f.ParticleList ? 1 : 0);
	pi(s, (long long)f.RenderPrimitive);
	pf(s, f.LOD);

	pf(s, f.ElapsedTime);
	pf(s, f.Age);
	pi(s, f.ParticlesEmitted);
	pi(s, f.ParticlesAlive);
	pi(s, (long long)f.CurrentPriorityTag);
	pv(s, f.LastUpdateLocation);
	pv(s, f.LastEmitLocation);
	pv(s, f.Location);
}

static void put_knobs(std::string& s, const CaseKnobs& k) {
	char t[40];
	snprintf(t, sizeof t, " %016llx %d", (unsigned long long)k.seed, k.editor);
	s += t;
	pf(s, k.level_time);
	pv(s, k.zone_gravity);
	snprintf(t, sizeof t, " %d %d", k.hit_kind, k.base_kind);
	s += t;
}

// Record line writer: flushes stat/destroy log lines produced since the last record first.
static void rec(const std::string& s) {
	for (auto& l : g_log)
		g_lines.push_back(l);
	g_log.clear();
	g_lines.push_back(s);
	g_records++;
}

static void emit_case(const char* family, const char* name, const CaseKnobs& k, const AParticleFX* fx, const std::string& extras) {
	std::string s = "case ";
	s += family;
	s += ' ';
	s += name;
	put_knobs(s, k);
	put_fx_state(s, *fx);
	s += extras;
	g_lines.push_back(s);
	g_cases++;
}

static void end_case(const char* family, const char* name, const AParticleFX* fx, const CaseKnobs& k) {
	char t[160];
	snprintf(t, sizeof t, "end %s %s destroys=%d editor=%d fx_flags=%08x owner_flags=%08x", family, name, g_destroys,
		(int)GIsEditor, (unsigned)fx->ObjectFlags, (unsigned)(fx->Owner ? fx->Owner->ObjectFlags : 0));
	for (auto& l : g_log)
		g_lines.push_back(l);
	g_log.clear();
	g_lines.push_back(t);
	(void)k;
}

static void write_family(const char* path, const char* family) {
	FILE* f = fopen(path, "w");
	if (!f) {
		fprintf(stderr, "particle_fx_harness: cannot open %s\n", path);
		abort();
	}
	fprintf(f, "header %s cases=%d records=%d\n", family, g_cases, g_records);
	for (auto& l : g_lines)
		fprintf(f, "%s\n", l.c_str());
	fclose(f);
	g_lines.clear();
	g_records = 0;
	g_cases = 0;
}

static void emit_defaults() {
	std::string s = "defaults";
	put_fx_state(s, *(const AParticleFX*)g_parent_mem);
	g_lines.push_back(s);
}

static UParticle* last_particle(UParticleList* L) {
	UParticle* Last = NULL;
	for (UParticle* P = L->StartParticle(); P; P = L->NextParticle())
		Last = P;
	return Last;
}

static void put_found(std::string& s, const UParticle* P) {
	if (!P)
		s += " -";
	else {
		s += ' ';
		put_particle(s, *P);
	}
}

static void populate(AParticleFX* fx, int calls, float tick) {
	bool saved = g_trace;
	g_trace = false;
	for (int i = 0; i < calls; i++)
		fx->EmitParticles(tick);
	g_trace = saved;
}

static FSceneNode frame_node() {
	FSceneNode F;
	memset((void*)&F, 0, sizeof F);
	F.Proj = FVector(0.0f, 0.0f, 2.0f);
	F.FX = 256.0f;
	F.FY = 128.0f;
	return F;
}

// ---------------------------------------------------------------------------
// Drivers. Each family writes: defaults, then per case: case, records, end.
// ---------------------------------------------------------------------------

// Records the libc rand stream and the appFrand range so a mismatch shows up on the first line.
static void probe_rand() {
	srand(1);
	std::string s = "probe rand";
	for (int i = 0; i < 4; i++) {
		char t[32];
		snprintf(t, sizeof t, " %d", (int)appRand());
		s += t;
	}
	pf(s, appFrand());
	pf(s, appFrand(-1.0f, 2.0f));
	pi(s, appRandRange((INT)3, (INT)11));
	pf(s, appRandRange(-1.0f, 2.0f));
	float lo = 1.0f, hi = 0.0f;
	for (int i = 0; i < 1000; i++) {
		float r = appFrand();
		if (r < lo) lo = r;
		if (r > hi) hi = r;
	}
	pf(s, lo);
	pf(s, hi);
	rec(s);
}

static void drive_emit() {
	static const float kTick[4] = {0.0f, 0.03f, 0.1f, 0.5f};
	probe_rand();
	for (int i = 0; i < 241; i++) {
		uint64_t seed = (i < 240) ? 0x5EED0000ULL + (uint64_t)i : 0xC70E0000ULL;
		char name[24];
		if (i < 240) {
			snprintf(name, sizeof name, "e%04d", i);
		} else {
			snprintf(name, sizeof name, "ctor_state");
		}
		KGen kg(seed ^ 0xD1CE000000000000ULL);
		CaseKnobs k = draw_knobs(kg, seed);
		const bool ctor_only = (i == 240);
		AParticleFX* fx = begin_case(k, !ctor_only, kg);
		float step = 0.0f, tick = 0.0f;
		int ncalls = 0;
		FVector locs[3];
		if (!ctor_only) {
			step = kf(kg, 1, 8, 0.5f);
			tick = kTick[kg.D(4)];
			ncalls = 3;
			for (int c = 0; c < 3; c++) {
				float k2 = (float)(c + 1);
				locs[c] = FVector(step * k2, step * 0.5f * k2, step * 0.25f * k2);
			}
		}
		std::string extras = " ";
		{
			char t[16];
			snprintf(t, sizeof t, "%d", ncalls);
			extras += t;
		}
		pf(extras, tick);
		for (int c = 0; c < ncalls; c++)
			pv(extras, locs[c]);
		emit_case("emit", name, k, fx, extras);

		for (int call = 0; call < ncalls; call++) {
			fx->Location = locs[call];
			int before = fx->ParticlesEmitted;
			int gbefore = GStat.ParticlesEmitted;
			g_trace = true;
			int ret = fx->EmitParticles(tick);
			g_trace = false;
			g_log.push_back("stat emitted " + std::to_string(GStat.ParticlesEmitted - gbefore));
			int nw = fx->ParticlesEmitted - before;
			int n = fx->ParticleList->Size();
			int idx = 0;
			for (UParticle* P = fx->ParticleList->StartParticle(); P; P = fx->ParticleList->NextParticle()) {
				if (idx >= n - nw) {
					std::string s;
					char h[64];
					snprintf(h, sizeof h, "emit %s P %d %d", name, call, idx);
					s += h;
					put_particle(s, *P);
					rec(s);
				}
				idx++;
			}
			std::string s;
			char h[64];
			snprintf(h, sizeof h, "emit %s S %d %d", name, call, ret);
			s += h;
			pf(s, fx->EmitDelay);
			pf(s, fx->EmissionResidue);
			char t[64];
			snprintf(t, sizeof t, " %d", fx->ParticlesEmitted);
			s += t;
			pv(s, fx->LastEmitLocation);
			snprintf(t, sizeof t, " %d %d %d", fx->CurrentPriorityTag, fx->ParticlesAlive, n);
			s += t;
			rec(s);
		}
		end_case("emit", name, fx, k);
	}
}

static void drive_bbox() {
	for (int i = 0; i < 240; i++) {
		uint64_t seed = 0xB0B00000ULL + (uint64_t)i;
		char name[16];
		snprintf(name, sizeof name, "b%04d", i);
		KGen kg(seed ^ 0xD1CE000000000000ULL);
		CaseKnobs k = draw_knobs(kg, seed);
		AParticleFX* fx = begin_case(k, true, kg);
		std::string extras = " 2";
		pf(extras, 0.1f);
		emit_case("bbox", name, k, fx, extras);
		populate(fx, 2, 0.1f);
		fx->ParticleList->UpdateBox();
		for (int exact = 0; exact < 2; exact++) {
			g_trace = true;
			FCoords c = fx->GetRenderBoundingBox(exact);
			g_trace = false;
			std::string s;
			char h[64];
			snprintf(h, sizeof h, "bbox %s %d", name, exact);
			s += h;
			pcoords(s, c);
			pf(s, fx->ElapsedTime);
			pf(s, fx->Age);
			pv(s, fx->LastUpdateLocation);
			rec(s);
		}
		end_case("bbox", name, fx, k);
	}
}

struct UpdStep {
	int kind; // 0 UP, 1 UPD, 2 INIT, 3 TICK
	float arg;
	ELevelTick tick_type;
};

static void rec_update(const char* name, const char* op, const char* ret, float arg, AParticleFX* fx) {
	UParticleList* L = fx->ParticleList;
	std::string s;
	char h[96];
	snprintf(h, sizeof h, "update %s %s %s", name, op, ret);
	s += h;
	pf(s, arg);
	snprintf(h, sizeof h, " %d", L->Size());
	s += h;
	pf(s, fx->ElapsedTime);
	pf(s, fx->Age);
	snprintf(h, sizeof h, " %d", fx->ParticlesEmitted);
	s += h;
	for (UParticle* P = L->StartParticle(); P; P = L->NextParticle()) {
		pv(s, P->Position);
		pv(s, P->Velocity);
		pf(s, P->Age);
		pf(s, P->Alpha);
		pf(s, P->Width);
		pf(s, P->Length);
		pf(s, P->Spin);
		snprintf(h, sizeof h, " %d", (int)P->Id);
		s += h;
	}
	rec(s);
}

static void run_plan(const char* name, AParticleFX* fx, const std::vector<UpdStep>& plan) {
	for (const UpdStep& st : plan) {
		g_trace = true;
		if (st.kind == 0) {
			fx->UpdateParticles(st.arg);
			g_trace = false;
			rec_update(name, "UP", "-", st.arg, fx);
		} else if (st.kind == 1) {
			UBOOL r = fx->Update(st.arg);
			g_trace = false;
			rec_update(name, "UPD", r ? "1" : "0", st.arg, fx);
		} else if (st.kind == 2) {
			fx->InitExecution();
			g_trace = false;
			rec_update(name, "INIT", "-", st.arg, fx);
		} else {
			UBOOL r = fx->Tick(st.arg, st.tick_type);
			g_trace = false;
			rec_update(name, "TICK", r ? "1" : "0", st.arg, fx);
		}
	}
}

static std::string plan_extras(uint32_t pop, const std::vector<UpdStep>& plan) {
	std::string s = " ";
	char t[32];
	snprintf(t, sizeof t, "%u %u", (unsigned)pop, (unsigned)plan.size());
	s += t;
	for (const UpdStep& st : plan) {
		snprintf(t, sizeof t, " %d", st.kind);
		s += t;
		pf(s, st.arg);
		snprintf(t, sizeof t, " %d", (int)st.tick_type);
		s += t;
	}
	return s;
}

static void drive_update() {
	static const float kStep[5] = {0.0f, 0.03125f, 0.0625f, 0.1f, 0.5f};
	for (int i = 0; i < 204; i++) {
		uint64_t seed = (i < 200) ? 0x0DD00000ULL + (uint64_t)i : 0x0DD0FFFFULL;
		char name[40];
		uint32_t pop;
		std::vector<UpdStep> plan;
		KGen kg(seed ^ 0xD1CE000000000000ULL);
		CaseKnobs k = draw_knobs(kg, seed);
		AParticleFX* fx;
		if (i < 200) {
			snprintf(name, sizeof name, "u%04d", i);
			fx = begin_case(k, true, kg);
			pop = kg.D(3);
			uint32_t nsteps = 2 + kg.D(5);
			for (uint32_t s = 0; s < nsteps; s++) {
				UpdStep st;
				st.kind = (int)kg.D(4);
				st.arg = kStep[kg.D(5)];
				st.tick_type = LEVELTICK_All;
				if (st.kind == 3 && kg.D(4) == 0)
					st.tick_type = LEVELTICK_ViewportsOnly;
				plan.push_back(st);
			}
		} else if (i == 200) {
			snprintf(name, sizeof name, "remove_all_tick_gt_maxlife");
			fx = begin_case(k, true, kg);
			// Lifetime is fixed at 1 so a tick of 6.25 exceeds MaxLifetime and removes all.
			fx->Lifetime.Base = 1.0f;
			fx->Lifetime.Rand = 0.0f;
			pop = 2;
			UpdStep up;
			up.kind = 0;
			up.arg = 0.0375f;
			up.tick_type = LEVELTICK_All;
			UpdStep upd;
			upd.kind = 1;
			upd.arg = 6.25f;
			upd.tick_type = LEVELTICK_All;
			plan.push_back(up);
			plan.push_back(upd);
		} else {
			// Named DestroyActor cases. Knobs are set before begin_case so the case line prints them.
			static const char* kNames[3] = {"tick_destroy", "update_destroy", "editor_reset"};
			const int j = i - 201;
			snprintf(name, sizeof name, "%s", kNames[j]);
			k.editor = (j == 2) ? 1 : 0;
			fx = begin_case(k, true, kg);
			fx->ParticlesMax = (j == 0) ? 4 : 2;
			fx->ParticlesPerSec.Base = (j == 0) ? 10.0f : 0.0f;
			fx->ParticlesPerSec.Rand = 0.0f;
			fx->Lifetime.Base = 1.0f;
			fx->Lifetime.Rand = 0.0f;
			fx->ParticlesEmitted = (j == 0) ? 0 : 2;
			pop = 0;
			UpdStep st;
			st.kind = (j == 0) ? 3 : 1;
			st.arg = (j == 0) ? 2.0f : 0.03125f;
			st.tick_type = LEVELTICK_All;
			plan.push_back(st);
			if (j == 0) {
				st.arg = 0.5f;
				plan.push_back(st);
			}
		}
		emit_case("update", name, k, fx, plan_extras(pop, plan));
		populate(fx, (int)pop, 0.1f);
		run_plan(name, fx, plan);
		end_case("update", name, fx, k);
	}
}

enum ParamMode { PM_SYS, PM_GP, PM_ADD, PM_ADDNULL, PM_RD, PM_LOD, PM_DENS, PM_DENSNOLIST, PM_MAXL, PM_COUNT };

static void run_param_mode(const char* name, AParticleFX* fx, uint32_t mode, uint32_t arg, uint32_t arg2) {
	FVector pos(1.0f, 2.0f, 3.0f);
	FSceneNode frame = frame_node();
	std::string s;
	char h[96];
	g_trace = true;
	switch (mode) {
	case PM_SYS: {
		AParticleFX* buf = (AParticleFX*)calloc(1, sizeof(AParticleFX));
		const AParticleFX* r = fx->GetSysParams((char*)buf);
		g_trace = false;
		int which = r == fx ? 0 : (r == (const AParticleFX*)buf ? 2 : (r == (const AParticleFX*)g_parent_mem ? 1 : 3));
		snprintf(h, sizeof h, "params %s SYS %d ", name, which);
		s += h;
		put_sys_params(s, *r);
		rec(s);
		break;
	}
	case PM_GP: {
		FParams p;
		memset(&p, 0, sizeof p);
		fx->GetParams(p);
		g_trace = false;
		snprintf(h, sizeof h, "params %s GP ", name);
		s += h;
		put_fparams(s, p);
		rec(s);
		break;
	}
	case PM_ADD: {
		FParams p;
		memset(&p, 0, sizeof p);
		fx->GetParams(p);
		FVector P = pos;
		UBOOL added = fx->AddParticle((INT)arg, P, &p);
		int size = fx->ParticleList->Size();
		snprintf(h, sizeof h, "params %s ADD %d %d", name, added ? 1 : 0, size);
		s += h;
		put_found(s, last_particle(fx->ParticleList));
		g_trace = false;
		rec(s);
		g_trace = true;
		int rr = fx->RecomputeDeltas((INT)arg);
		g_trace = false;
		std::string t;
		snprintf(h, sizeof h, "params %s RD %d %d", name, rr, (int)arg);
		t += h;
		put_found(t, fx->ParticleList->GetParticle((INT)arg));
		rec(t);
		break;
	}
	case PM_ADDNULL: {
		FVector P = pos;
		UBOOL added = fx->AddParticle((INT)arg, P, NULL);
		int size = fx->ParticleList->Size();
		g_trace = false;
		snprintf(h, sizeof h, "params %s ADD %d %d -", name, added ? 1 : 0, size);
		s += h;
		rec(s);
		break;
	}
	case PM_RD: {
		FParams p;
		memset(&p, 0, sizeof p);
		fx->GetParams(p);
		FVector P = pos;
		fx->AddParticle(1, P, &p);
		if (arg2 != 0) {
			UParticle* p1 = fx->ParticleList->GetParticle(1);
			if (!p1) {
				fprintf(stderr, "particle_fx_harness: %s: particle 1 missing for RD\n", name);
				abort();
			}
			p1->Age = p1->Lifetime + 1.0f;
		}
		int rr = fx->RecomputeDeltas((INT)arg);
		g_trace = false;
		snprintf(h, sizeof h, "params %s RD %d %d", name, rr, (int)arg);
		s += h;
		put_found(s, fx->ParticleList->GetParticle((INT)arg));
		rec(s);
		break;
	}
	case PM_LOD: {
		static const float frs[3] = {0.0f, 0.3f, 0.9f};
		for (float fr : frs) {
			float r = fx->Lod(fr);
			g_trace = false;
			std::string t;
			snprintf(h, sizeof h, "params %s LOD", name);
			t += h;
			pf(t, fr);
			pf(t, r);
			rec(t);
			g_trace = true;
		}
		float lp = fx->LodParticles(0.3f);
		g_trace = false;
		std::string t;
		snprintf(h, sizeof h, "params %s LODP", name);
		t += h;
		pf(t, 0.3f);
		pf(t, lp);
		rec(t);
		break;
	}
	case PM_DENS: {
		static const float frs[2] = {0.0f, 0.5f};
		for (float fr : frs) {
			float r = fx->LodParticleDensity(&frame, fr);
			g_trace = false;
			std::string t;
			snprintf(h, sizeof h, "params %s DENS", name);
			t += h;
			pf(t, fr);
			pf(t, r);
			rec(t);
			g_trace = true;
		}
		float lp = fx->LodParticles(0.5f);
		g_trace = false;
		std::string t;
		snprintf(h, sizeof h, "params %s LODP", name);
		t += h;
		pf(t, 0.5f);
		pf(t, lp);
		rec(t);
		break;
	}
	case PM_DENSNOLIST: {
		fx->ParticleList = NULL;
		float r = fx->LodParticleDensity(&frame, 0.5f);
		g_trace = false;
		std::string t;
		snprintf(h, sizeof h, "params %s DENS", name);
		t += h;
		pf(t, 0.5f);
		pf(t, r);
		rec(t);
		g_trace = true;
		float lp = fx->LodParticles(0.5f);
		g_trace = false;
		std::string u;
		snprintf(h, sizeof h, "params %s LODP", name);
		u += h;
		pf(u, 0.5f);
		pf(u, lp);
		rec(u);
		break;
	}
	case PM_MAXL: {
		float r = fx->MaxLifetime();
		g_trace = false;
		snprintf(h, sizeof h, "params %s MAXL", name);
		s += h;
		pf(s, r);
		rec(s);
		break;
	}
	}
	g_trace = false;
}

static void drive_params() {
	for (int i = 0; i < 301; i++) {
		uint64_t seed = (i < 300) ? 0x9A7A0000ULL + (uint64_t)i : 0x9A7AFFFFULL;
		char name[24];
		KGen kg(seed ^ 0xD1CE000000000000ULL);
		CaseKnobs k = draw_knobs(kg, seed);
		AParticleFX* fx;
		uint32_t mode, arg, arg2, pop;
		if (i < 300) {
			snprintf(name, sizeof name, "p%04d", i);
			fx = begin_case(k, true, kg);
			mode = kg.D(PM_COUNT);
			arg = kg.D(4);
			arg2 = kg.D(2);
			pop = kg.D(2);
		} else {
			// Named case: AddParticle with a NULL palette-free fx, then the palette branch.
			snprintf(name, sizeof name, "add_null");
			fx = begin_case(k, true, kg);
			mode = PM_ADDNULL;
			arg = 3;
			arg2 = 0;
			pop = 1;
		}
		std::string extras = " ";
		char t[64];
		snprintf(t, sizeof t, "%u %u %u %u", mode, arg, arg2, pop);
		extras += t;
		emit_case("params", name, k, fx, extras);
		populate(fx, (int)pop, 0.1f);
		run_param_mode(name, fx, mode, arg, arg2);
		end_case("params", name, fx, k);
	}
	// add_palette: a named case that routes AddParticle through the palette (Colors(0) non-zero).
	{
		uint64_t seed = 0x9A7AEEEEULL;
		KGen kg(seed ^ 0xD1CE000000000000ULL);
		CaseKnobs k = draw_knobs(kg, seed);
		AParticleFX* fx = begin_case(k, true, kg);
		fx->ColorPalette = g_tex_with;
		uint32_t mode = PM_ADD, arg = 0, arg2 = 0, pop = 0;
		std::string extras = " ";
		char t[64];
		snprintf(t, sizeof t, "%u %u %u %u", mode, arg, arg2, pop);
		extras += t;
		emit_case("params", "add_palette", k, fx, extras);
		run_param_mode("add_palette", fx, mode, arg, arg2);
		end_case("params", "add_palette", fx, k);
	}
}

static void write_trig(const char* path) {
	FILE* f = fopen(path, "w");
	for (int i = 0; i < 16384; i++)
		fprintf(f, "%08x\n", bits(GMath.TrigFLOAT[i]));
	fclose(f);
}

// SqrtManTbl lives in its own fixture so particle_trig.txt stays the 16384-entry sin/cos table.
static void write_sqrt(const char* path) {
	FILE* f = fopen(path, "w");
	const int n = (int)(sizeof(GMath.SqrtManTbl) / sizeof(GMath.SqrtManTbl[0]));
	for (int i = 0; i < n; i++)
		fprintf(f, "%08x\n", bits(GMath.SqrtManTbl[i]));
	fclose(f);
}

int main(int argc, char** argv) {
	if (argc != 7) {
		fprintf(stderr, "usage: particle_fx_harness <emit> <bbox> <update> <params> <trig> <sqrt>\n");
		return 2;
	}
	init_world();

	emit_defaults();
	drive_emit();
	write_family(argv[1], "emit");

	emit_defaults();
	drive_bbox();
	write_family(argv[2], "bbox");

	emit_defaults();
	drive_update();
	write_family(argv[3], "update");

	emit_defaults();
	drive_params();
	write_family(argv[4], "params");

	write_trig(argv[5]);
	write_sqrt(argv[6]);
	return 0;
}

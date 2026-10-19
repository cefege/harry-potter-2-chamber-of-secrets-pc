// Fixture generator for the particle port.
//
// Built by gen_particle.sh against the ORIGINAL UnParticle.cpp and UnMath.cpp extracted from the
// pinned ref, with product flags and -fno-access-control. UnParticle.cpp and UnMath.cpp are separate
// objects. UnParticleList.cpp is included verbatim below (it is private to that file and includes
// EnginePrivate.h, which is unguarded, so it must not be included twice). Engine services that the
// bodies call (appFrand, UModel::FastLineCheck, AWind::GetTotalWind, ULevel::SingleLineCheck via a
// fake vtable, UObject::IsA, the palette) are defined here. Each hook mirrors the contract in
// crates/hp2-engine/tests/particle_update.rs exactly.
//
// Output lines (gen script splits them into fixtures):
//   upd <40 fields>   one UParticle::Update case; see emit_update_case for field order
//   list <op> ...     one UParticleListPriv operation; see run_list_scenario
//   trig <hex>        GMath.TrigFLOAT[i] bits, i = 0..16383

// UnParticleList.cpp is included verbatim (original std::list bodies from the pinned ref). It
// includes EnginePrivate.h, which is unguarded, so the harness must not include it again. The class
// UParticleListPriv is private to that file, which is why it is included here rather than linked.
#include "UnParticleList.cpp"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>

static_assert(sizeof(UParticle) == 140, "UParticle layout drifted from the Rust mirror");

FGlobalMath GMath;

// ---------------------------------------------------------------------------------------------
// Engine hooks
// ---------------------------------------------------------------------------------------------

// 24-bit LCG draw in [0,1). Rust mock: state = seed as u64; state = state*A + C; (state>>40) as f32 * 2^-24.
static uint64_t g_frand_state = 0;
FLOAT appFrand()
{
	g_frand_state = g_frand_state * 6364136223846793005ULL + 1442695040888963407ULL;
	return (FLOAT)(uint32_t)(g_frand_state >> 40) * (1.0f / 16777216.0f);
}

// libm wrappers with the bodies of the pinned Core/Src/UnAnsi.cpp. They are defined here (not linked
// from Core) so the harness does not pull in the whole Core object set; the original TU calls them.
#include <math.h>
DOUBLE appSin( DOUBLE Value )            { return sin(Value); }
DOUBLE appCos( DOUBLE Value )            { return cos(Value); }
DOUBLE appAcos( DOUBLE Value )           { return acos(Value); }
DOUBLE appAtan2( DOUBLE Y, DOUBLE X )    { return atan2(Y,X); }
DOUBLE appSqrt( DOUBLE Value )           { return sqrt(Value); }
DOUBLE appPow( DOUBLE A, DOUBLE B )      { return pow(A,B); }
UBOOL appIsNan( DOUBLE A )               { return isnan(A)==1; }

// Rust mock: ((end.x as i32) ^ (start.y as i32)) & 3 != 0.
BYTE UModel::FastLineCheck( FVector End, FVector Start )
{
	return ((((INT)End.X) ^ ((INT)Start.Y)) & 3) != 0;
}

// Rust mock: (loc.y * 0.5, -loc.x * 0.25, 3.0).
FVector AWind::GetTotalWind( ULevel* Level, const FVector& Loc )
{
	return FVector( Loc.Y*0.5f, -Loc.X*0.25f, 3.0f );
}

// UObject's out-of-line constructor and destructor live in UnObj.cpp, which this harness does not
// link. The list objects are calloc'd and this TU never reads UObject base fields, so empty bodies
// are sufficient.
UObject::UObject() {}
UObject::~UObject() {}

// The original std::list nodes (and std::vector storage) allocate through the engine's inline global
// operator new (UnFile.h), which calls GMalloc->Malloc. Core.cpp is not linked, so provide a
// malloc/free allocator here.
class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { (void)Tag; return malloc(Count); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag ) { (void)Tag; return realloc(Original, Count); }
	void Free( void* Original ) { free(Original); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc g_harness_malloc;
FMalloc* GMalloc = &g_harness_malloc;

// UObject::IsA is inline (UnType.h) and walks Class->SuperField. ALevelInfo is declared with
// __STATIC_LINK=1, so StaticClass() returns PrivateStaticClass once it is non-null. The level
// actor's Class is that pointer; the other actor has no class, so IsA is false for it.
alignas(16) static unsigned char g_level_info_class_mem[sizeof(UClass)];
UClass* ALevelInfo::PrivateStaticClass = reinterpret_cast<UClass*>(g_level_info_class_mem);
static AActor* g_level_actor = NULL;   // Class == ALevelInfo::PrivateStaticClass
static AActor* g_other_actor = NULL;   // Class == NULL

// Rust mock: sel = ((end.y as i32) ^ (start.z as i32)) & 3; sel 0 -> no hit; sel 2 -> hit on a
// non-level actor (no hit); sel 1 or 3 -> hit on the level actor, which sets Normal and Location.
// Normal by (end.x as i32) & 3: 0 (0,0,1), 1 (1,0,0), 2 (0,-1,0), 3 (0,0,-1). Location = end*0.5.
static UBOOL FakeSingleLineCheck( ULevel* Self, FCheckResult& Hit, AActor* SourceActor, const FVector& End, const FVector& Start, DWORD TraceFlags, FVector Extent, BYTE NodeFlags )
{
	INT Sel = (((INT)End.Y) ^ ((INT)Start.Z)) & 3;
	Hit.Actor = (Sel == 0) ? NULL : ((Sel == 2) ? g_other_actor : g_level_actor);
	INT Dir = ((INT)End.X) & 3;
	if (Dir == 0)      Hit.Normal = FVector( 0.0f, 0.0f,  1.0f);
	else if (Dir == 1) Hit.Normal = FVector( 1.0f, 0.0f,  0.0f);
	else if (Dir == 2) Hit.Normal = FVector( 0.0f,-1.0f,  0.0f);
	else               Hit.Normal = FVector( 0.0f, 0.0f, -1.0f);
	Hit.Location = FVector( End.X*0.5f, End.Y*0.5f, End.Z*0.5f );
	return 1;
}

static void* g_level_vtable[1024];
static void FakeUnreachedVirtual()
{
	fprintf(stderr, "particle harness: unexpected virtual call\n");
	abort();
}

static int SingleLineCheckSlot()
{
	decltype(&ULevel::SingleLineCheck) Mp = &ULevel::SingleLineCheck;
	struct { intptr_t Ptr; intptr_t Adj; } Rep;
	static_assert(sizeof(Mp) == sizeof(Rep), "member pointer is not Itanium-shaped");
	memcpy(&Rep, &Mp, sizeof(Rep));
#if defined(__aarch64__)
	// AArch64 Itanium ABI: the virtual flag is bit 0 of Adj; Ptr is the byte offset into the vtable.
	if ((Rep.Adj & 1) == 0) { fprintf(stderr, "particle harness: SingleLineCheck is not virtual\n"); abort(); }
	return (int)(Rep.Ptr / (intptr_t)sizeof(void*));
#else
	// Generic Itanium ABI: a virtual member pointer stores 1 + byte offset in Ptr.
	if ((Rep.Ptr & 1) == 0) { fprintf(stderr, "particle harness: SingleLineCheck is not virtual\n"); abort(); }
	return (int)((Rep.Ptr - 1) / (intptr_t)sizeof(void*));
#endif
}

static UModel* g_model = NULL;
static ULevel* g_level = NULL;
static UTexture* g_tex_with_palette = NULL;
static UTexture* g_tex_without_palette = NULL;
static FColor g_palette_colors[256];

static void setup_engine_fakes()
{
	int Slot = SingleLineCheckSlot();
	for (int i = 0; i < 1024; i++)
		g_level_vtable[i] = (void*)&FakeUnreachedVirtual;
	g_level_vtable[Slot] = (void*)&FakeSingleLineCheck;

	g_model = (UModel*)calloc(1, sizeof(UModel));
	g_level = (ULevel*)calloc(1, sizeof(ULevel));
	*(void**)g_level = (void*)g_level_vtable;
	g_level->Model = g_model;

	g_level_actor = (AActor*)calloc(1, sizeof(AActor));
	g_other_actor = (AActor*)calloc(1, sizeof(AActor));
	g_level_actor->Class = ALevelInfo::PrivateStaticClass;

	// Palette: colour i = (i*7, i*3, i*5, i*11) as bytes. Rust mock mirrors this formula.
	for (int i = 0; i < 256; i++)
		g_palette_colors[i] = FColor((BYTE)(i*7), (BYTE)(i*3), (BYTE)(i*5), (BYTE)(i*11));
	UPalette* Pal = (UPalette*)calloc(1, sizeof(UPalette));
	Pal->Colors.Data = g_palette_colors;
	Pal->Colors.ArrayNum = 256;
	Pal->Colors.ArrayMax = 256;
	g_tex_with_palette = (UTexture*)calloc(1, sizeof(UTexture));
	g_tex_with_palette->Palette = Pal;
	g_tex_without_palette = (UTexture*)calloc(1, sizeof(UTexture));
	g_tex_without_palette->Palette = NULL;
}

// ---------------------------------------------------------------------------------------------
// Record generation (values are recorded verbatim; the generator arithmetic is not mirrored)
// ---------------------------------------------------------------------------------------------

static uint64_t g_gen = 0x243F6A8885A308D3ULL;
static uint32_t gen32()
{
	g_gen = g_gen * 6364136223846793005ULL + 1442695040888963407ULL;
	return (uint32_t)(g_gen >> 32);
}
static float gen_unit()
{
	return (float)(gen32() >> 8) * (1.0f / 16777216.0f);
}
static float gen_range(float Lo, float Hi)
{
	return Lo + (Hi - Lo) * gen_unit();
}
static FVector gen_vec(float Lo, float Hi)
{
	float X = gen_range(Lo, Hi);
	float Y = gen_range(Lo, Hi);
	float Z = gen_range(Lo, Hi);
	return FVector(X, Y, Z);
}
static uint32_t fbits(float F)
{
	uint32_t U;
	memcpy(&U, &F, 4);
	return U;
}
static void put_f(float F)
{
	printf(" %08x", fbits(F));
}
static void put_i(int32_t I)
{
	printf(" %08x", (uint32_t)I);
}
static void put_v(const FVector& V)
{
	put_f(V.X); put_f(V.Y); put_f(V.Z);
}
static void put_words(const uint32_t* W, int N)
{
	for (int i = 0; i < N; i++)
		printf(" %08x", W[i]);
}

// Record layout (all hex u32 words except the leading tag):
//   seed, timestep, gravity[3], system_damping, system_wind[3],
//   sys.damping, sys.wind_modifier, sys.b_wind_per_particle, sys.location[3], sys.rotation[3] (i32),
//   sys.last_update_location[3], sys.last_update_rotation[3] (i32), sys.attraction[3],
//   sys.chaos, sys.chaos_delay, sys.alpha_grow_period, sys.alpha_delay, sys.color_delay,
//   sys.size_grow_period, sys.size_delay, sys.elasticity, sys.b_update, sys.b_system_relative,
//   sys.render_primitive, palette_mode (0 none, 1 palette, 2 texture without palette),
//   particle_in[35] (UParticle words), ret, particle_out[35].
static void emit_update_case()
{
	uint32_t Seed = gen32();
	FVector Pos = gen_vec(-300.0f, 300.0f);
	FVector Last = Pos + gen_vec(-5.0f, 5.0f);
	if (gen_unit() < 0.25f) Last = Pos;
	FVector Vel = gen_vec(-50.0f, 50.0f);

	UParticle P(FVector(0,0,0), FVector(0,0,0), NULL);
	P.Position = Pos;
	P.LastPosition = Last;
	P.Velocity = Vel;
	P.Lifetime = (gen_unit() < 0.15f) ? 0.0f : gen_range(0.5f, 10.0f);
	P.Age = (P.Lifetime > 0.0f) ? P.Lifetime * gen_range(0.0f, 0.999f) : gen_range(0.0f, 20.0f);
	P.Alpha = gen_unit();
	P.AlphaFull = gen_unit();
	P.AlphaDelta = gen_range(-0.5f, 0.5f);
	P.Color = FPlane(gen_unit(), gen_unit(), gen_unit(), gen_unit());
	if (gen_unit() < 0.5f)
		P.ColorDelta = FPlane(0.0f, 0.0f, 0.0f, 0.0f);
	else
		P.ColorDelta = FPlane(gen_range(-0.5f, 0.5f), gen_range(-0.5f, 0.5f), gen_range(-0.5f, 0.5f), gen_range(-0.5f, 0.5f));
	P.Width = gen_range(0.0f, 20.0f);
	P.WidthFull = gen_range(0.0f, 20.0f);
	P.WidthDelta = gen_range(-1.0f, 1.0f);
	P.Length = gen_range(0.0f, 20.0f);
	P.LengthFull = gen_range(0.0f, 20.0f);
	P.LengthDelta = gen_range(-1.0f, 1.0f);
	P.Tail = gen_unit();
	P.DripTimer = (gen_unit() < 0.33f) ? gen_range(0.0f, 0.3f) : 0.0f;
	P.ChaosDelay = (gen_unit() < 0.5f) ? gen_range(0.0f, 0.5f) : 0.0f;
	P.Spin = gen_range(-3.0f, 3.0f);
	P.SpinRate = gen_range(-5.0f, 5.0f);
	P.Id = (INT)(gen32() & 0xffff);
	P.PriorityTag = (INT)(gen32() & 7);

	float Timestep = gen_range(0.001f, 0.2f);
	FVector Gravity = gen_vec(-200.0f, 200.0f);
	float SystemDamping = gen_range(0.5f, 1.5f);
	FVector SystemWind = gen_vec(-20.0f, 20.0f);

	AParticleFX* Sys = (AParticleFX*)calloc(1, sizeof(AParticleFX));
	Sys->Damping = gen_range(0.5f, 4.0f);
	Sys->WindModifier = gen_range(0.0f, 2.0f);
	Sys->bWindPerParticle = gen_unit() < 0.5f;
	Sys->Location = gen_vec(-300.0f, 300.0f);
	Sys->Rotation = FRotator((INT)(gen32() % 131072) - 65536, (INT)(gen32() % 131072) - 65536, (INT)(gen32() % 131072) - 65536);
	Sys->LastUpdateLocation = gen_vec(-300.0f, 300.0f);
	Sys->LastUpdateRotation = FRotator((INT)(gen32() % 131072) - 65536, (INT)(gen32() % 131072) - 65536, (INT)(gen32() % 131072) - 65536);
	Sys->Attraction = (gen_unit() < 0.33f) ? FVector(0,0,0) : gen_vec(-30.0f, 30.0f);
	Sys->Chaos = (gen_unit() < 0.5f) ? 0.0f : gen_range(0.1f, 5.0f);
	Sys->ChaosDelay = gen_range(0.0f, 1.0f);
	Sys->AlphaGrowPeriod = gen_range(0.05f, 1.0f);
	Sys->AlphaDelay = gen_range(0.0f, 5.0f);
	Sys->ColorDelay = gen_range(0.0f, 5.0f);
	Sys->SizeGrowPeriod = gen_range(0.05f, 1.0f);
	Sys->SizeDelay = gen_range(0.0f, 5.0f);
	Sys->Elasticity = (gen_unit() < 0.5f) ? 0.0f : gen_range(0.1f, 1.0f);
	Sys->bUpdate = gen_unit() < 0.8f;
	Sys->bSystemRelative = gen_unit() < 0.4f;
	Sys->RenderPrimitive = (gen_unit() < 0.3f) ? PPRIM_Liquid : 0;
	uint32_t PaletteMode = gen32() % 3;
	Sys->ColorPalette = (PaletteMode == 0) ? NULL : ((PaletteMode == 1) ? g_tex_with_palette : g_tex_without_palette);
	Sys->XLevel = g_level;

	uint32_t In[35];
	memcpy(In, &P, sizeof(UParticle));

	g_frand_state = Seed;
	bool Ret = P.Update(Gravity, SystemDamping, SystemWind, g_level, Timestep, Sys);
	uint32_t Out[35];
	memcpy(Out, &P, sizeof(UParticle));

	printf("upd %08x", Seed);
	put_f(Timestep);
	put_v(Gravity);
	put_f(SystemDamping);
	put_v(SystemWind);
	put_f(Sys->Damping);
	put_f(Sys->WindModifier);
	put_i(Sys->bWindPerParticle ? 1 : 0);
	put_v(Sys->Location);
	put_i(Sys->Rotation.Pitch); put_i(Sys->Rotation.Yaw); put_i(Sys->Rotation.Roll);
	put_v(Sys->LastUpdateLocation);
	put_i(Sys->LastUpdateRotation.Pitch); put_i(Sys->LastUpdateRotation.Yaw); put_i(Sys->LastUpdateRotation.Roll);
	put_v(Sys->Attraction);
	put_f(Sys->Chaos);
	put_f(Sys->ChaosDelay);
	put_f(Sys->AlphaGrowPeriod);
	put_f(Sys->AlphaDelay);
	put_f(Sys->ColorDelay);
	put_f(Sys->SizeGrowPeriod);
	put_f(Sys->SizeDelay);
	put_f(Sys->Elasticity);
	put_i(Sys->bUpdate ? 1 : 0);
	put_i(Sys->bSystemRelative ? 1 : 0);
	put_i((int32_t)Sys->RenderPrimitive);
	put_i((int32_t)PaletteMode);
	put_words(In, 35);
	put_i(Ret ? 1 : 0);
	put_words(Out, 35);
	printf("\n");
	free(Sys);
}

// ---------------------------------------------------------------------------------------------
// List scenarios. A shadow model mirrors the original cursor (itParticles / bAdvance) only to
// choose operations that are defined: NextParticle with the cursor at end and bAdvance set,
// Remove/RemoveCurrent with the cursor at end, and any cursor use after RemoveAll (iterator
// invalidated by clear()) or RemoveFromFront on the cursor's element are all skipped. Every
// emitted result is checked against the shadow, so a model mismatch aborts generation.
// ---------------------------------------------------------------------------------------------

struct ListShadow
{
	std::vector<INT> Ids;
	INT  Cur;        // index of itParticles, -1 = end()
	bool bAdvance;
	bool Dangling;   // iterator invalidated by RemoveAll
};

static void shadow_fail(const char* What)
{
	fprintf(stderr, "particle harness: shadow mismatch in %s\n", What);
	exit(3);
}

static void run_list_scenario(int Scenario, int Ops)
{
	UParticleListPriv* L = (UParticleListPriv*)calloc(1, sizeof(UParticleListPriv));
	::new (L) UParticleListPriv(NULL);

	ListShadow S;
	S.Cur = -1;
	S.bAdvance = true;
	S.Dangling = false;
	INT NextId = 1;
	UParticle Dummy(FVector(0,0,0), FVector(0,0,0), NULL);
	(void)Scenario;

	for (int Op = 0; Op < Ops; Op++)
	{
		uint32_t R = gen32() % 100;
		int Kind;
		if (R < 22)                    Kind = 0;   // add
		else if (R < 30)               Kind = 1;   // start
		else if (R < 50)               Kind = 2;   // next
		else if (R < 58)               Kind = 3;   // remove_current
		else if (R < 68)               Kind = 4;   // remove existing id
		else if (R < 72)               Kind = 5;   // remove missing id
		else if (R < 79)               Kind = 6;   // remove_from_front
		else if (R < 81)               Kind = 7;   // remove_all
		else if (R < 87)               Kind = 8;   // size
		else if (R < 95)               Kind = 9;   // get
		else                           Kind = 10;  // update_box

		INT Size = (INT)S.Ids.size();
		bool Valid = true;
		switch (Kind)
		{
		case 2: Valid = !S.Dangling && !(S.Cur == -1 && S.bAdvance); break;
		case 3: Valid = !S.Dangling && S.Cur != -1; break;
		case 4: case 5: Valid = !S.Dangling && S.Cur != -1; break;
		case 6: Valid = Size > 0 && !S.Dangling && S.Cur != 0; break;
		default: break;
		}
		if (!Valid) Kind = 0;

		if (Kind == 0)
		{
			FVector Pos = gen_vec(-200.0f, 200.0f);
			FVector Vel = gen_vec(-10.0f, 10.0f);
			float W = gen_range(0.0f, 30.0f);
			float Len = gen_range(0.0f, 30.0f);
			UParticle P(Pos, Vel, NULL);
			P.Id = NextId++;
			P.Width = W;
			P.Length = Len;
			L->Add(P);
			S.Ids.push_back(P.Id);
			printf("list add %d %08x %08x", P.Id, fbits(W), fbits(Len));
			put_v(Pos);
			printf("\n");
		}
		else if (Kind == 1)
		{
			UParticle* P = L->StartParticle();
			S.Cur = S.Ids.empty() ? -1 : 0;
			if (S.Cur == -1) S.bAdvance = false;
			S.Dangling = false;
			INT Expect = (S.Cur == -1) ? -1 : S.Ids[S.Cur];
			INT Got = P ? P->Id : -1;
			if (Got != Expect) shadow_fail("start");
			if (P) printf("list start => %d\n", Got); else printf("list start => null\n");
		}
		else if (Kind == 2)
		{
			UParticle* P = L->NextParticle();
			if (S.bAdvance)
			{
				S.Cur++;
				if (S.Cur >= Size) S.Cur = -1;
			}
			S.bAdvance = true;
			if (S.Cur == -1) S.bAdvance = false;
			INT Expect = (S.Cur == -1) ? -1 : S.Ids[S.Cur];
			INT Got = P ? P->Id : -1;
			if (Got != Expect) shadow_fail("next");
			if (P) printf("list next => %d\n", Got); else printf("list next => null\n");
		}
		else if (Kind == 3)
		{
			L->RemoveCurrent();
			S.Ids.erase(S.Ids.begin() + S.Cur);
			S.Cur = (S.Cur < (INT)S.Ids.size()) ? S.Cur : -1;
			S.bAdvance = false;
			if (L->Size() != (INT)S.Ids.size()) shadow_fail("remove_current");
			printf("list remove_current => %d\n", L->Size());
		}
		else if (Kind == 4 || Kind == 5)
		{
			INT Id;
			if (Kind == 4 && !S.Ids.empty())
				Id = S.Ids[gen32() % S.Ids.size()];
			else
				Id = 100000 + (INT)(gen32() % 1000);
			UParticle* P = L->GetParticle(Id);
			L->Remove(P ? P : &Dummy);
			INT Idx = -1;
			for (INT i = 0; i < Size; i++)
				if (S.Ids[i] == Id) { Idx = i; break; }
			if (Idx >= 0)
			{
				S.Ids.erase(S.Ids.begin() + Idx);
				if (Idx == S.Cur)
				{
					S.Cur = (Idx < (INT)S.Ids.size()) ? Idx : -1;
					S.bAdvance = false;
				}
				else if (Idx < S.Cur)
				{
					S.Cur--;
				}
			}
			if (L->Size() != (INT)S.Ids.size()) shadow_fail("remove");
			printf("list remove %d => %d\n", Id, L->Size());
		}
		else if (Kind == 6)
		{
			L->RemoveFromFront();
			S.Ids.erase(S.Ids.begin());
			if (S.Cur > 0) S.Cur--;
			if (L->Size() != (INT)S.Ids.size()) shadow_fail("remove_from_front");
			printf("list remove_from_front => %d\n", L->Size());
		}
		else if (Kind == 7)
		{
			L->RemoveAll();
			S.Ids.clear();
			S.Dangling = true;
			S.Cur = -1;
			printf("list remove_all => %d\n", L->Size());
		}
		else if (Kind == 8)
		{
			if (L->Size() != Size) shadow_fail("size");
			printf("list size => %d\n", L->Size());
		}
		else if (Kind == 9)
		{
			INT Id;
			if (!S.Ids.empty() && (gen32() & 1))
				Id = S.Ids[gen32() % S.Ids.size()];
			else
				Id = 100000 + (INT)(gen32() % 1000);
			UParticle* P = L->GetParticle(Id);
			INT Expect = -1;
			for (INT i = 0; i < Size; i++)
				if (S.Ids[i] == Id) { Expect = Id; break; }
			INT Got = P ? P->Id : -1;
			if (Got != Expect) shadow_fail("get");
			if (P) printf("list get %d => %d\n", Id, Got); else printf("list get %d => null\n", Id);
		}
		else
		{
			L->UpdateBox();
			FBox& B = L->WorldBox;
			printf("list update_box =>");
			put_f(B.Min.X); put_f(B.Min.Y); put_f(B.Min.Z);
			put_f(B.Max.X); put_f(B.Max.Y); put_f(B.Max.Z);
			put_i((INT)B.IsValid);
			printf("\n");
		}
	}
}

int main()
{
	setup_engine_fakes();
	g_gen = 0x243F6A8885A308D3ULL;
	for (int i = 0; i < 640; i++)
		emit_update_case();
	for (int s = 0; s < 96; s++)
		run_list_scenario(s, 60);
	for (int i = 0; i < 16384; i++)
		printf("trig %08x\n", fbits(GMath.TrigFLOAT[i]));
	return 0;
}

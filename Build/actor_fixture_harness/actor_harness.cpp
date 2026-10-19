// Fixture driver for the UnActor.cpp port (EngActorCore slice).
// Built by gen_actor.sh against the git-archive copy of UnActor.cpp at 90d1e8d,
// whose engine-facing call sites were rewritten to actor_hooks.h. Every case
// prints one "case <family> <idx> <knobs>" line, then "hook ..." lines from the
// hooks it reaches, then one "res ..." line. Pointers print by registered name.
#include <cstdint>
#include <cstdio>
#include <cstring>

#define private public
#define protected public
#include "EnginePrivate.h"
#include "UnNet.h"
#undef private
#undef protected

#include "actor_hooks.h"

alignas(16) static unsigned char g_net_obj[4096];
alignas(16) static unsigned char g_demo_driver[2048];
alignas(16) static unsigned char g_client[2048];
alignas(16) static unsigned char g_viewport[2048];
alignas(16) static unsigned char g_net_connection[4096];
static UViewport* g_viewport_array[1];

// ---------------------------------------------------------------- globals
UBOOL GIsEditor = 0;

FVector ActorArgTraceEnd;
FVector ActorArgTraceStart;
INT*    ActorArgFlags = NULL;
UBOOL   ActorArgDecals = 0;

// Flags the hooks read; set per case.
static UBOOL g_ar_loading, g_ar_saving, g_ar_trans;
static UBOOL g_trace_hit_kind; // 0 none, 1 plain surf, 2 surf with tex flags
static UBOOL g_smooth_noop;
static UBOOL g_level_begun;

// Storage pools. Raw bytes, zeroed per case; the engine types are only
// accessed through field pointers, never constructed or dispatched virtually.
#define POOL(name, T) alignas(16) static unsigned char name[sizeof(T)]
POOL(g_self, AActor);
POOL(g_other, AActor);
POOL(g_owner, AActor);
POOL(g_base, AActor);
POOL(g_touch, AActor);
POOL(g_pawn, APlayerPawn);
POOL(g_zone, AZoneInfo);
POOL(g_carcass, ACarcass);
POOL(g_level, ULevel);
POOL(g_levelinfo, ALevelInfo);
POOL(g_engine, UEngine);
POOL(g_class, UClass);
POOL(g_uclass_static, UClass);
POOL(g_model, UModel);
POOL(g_polys, UPolys);
alignas(16) static unsigned char g_mesh[256]; // UMesh is only forward-declared here; the hooks receive it as an opaque pointer.
POOL(g_player, UPlayer);
POOL(g_ri, UObject);
POOL(g_ar, FArchive);
POOL(g_frame, FFrame);
POOL(g_surf, FBspSurf);
alignas(16) static unsigned char g_prim[5][sizeof(UPrimitive)];
alignas(16) static unsigned char g_tex[8][sizeof(UTexture)];
alignas(16) static unsigned char g_hash[64];
alignas(16) static unsigned char g_func[64];
alignas(16) static unsigned char g_fn_parms[16];
POOL(g_decal_actor, ADecal);
static FDecal g_decals[1];
static FVector g_hit_loc(0.f, 0.f, 0.f);
static FVector g_pts[1];
static FVector g_vecs[1];
#include "decal_flip_cases.h"

// -------------------------------------------------------- name registry
struct Ent { const void* p; const char* n; const char* isa; };
static Ent g_ents[64];
static int g_nents;

static void reg(const void* p, const char* n, const char* isa = "")
{
	for (int i = 0; i < g_nents; ++i)
		if (g_ents[i].p == p)
		{
			g_ents[i].n = n;
			g_ents[i].isa = isa;
			return;
		}
	g_ents[g_nents].p = p;
	g_ents[g_nents].n = n;
	g_ents[g_nents].isa = isa;
	++g_nents;
}

static const Ent* ent(const void* p)
{
	for (int i = 0; i < g_nents; ++i)
		if (g_ents[i].p == p) return &g_ents[i];
	return NULL;
}

static const char* nm(const void* p)
{
	if (!p) return "null";
	const Ent* e = ent(p);
	return e ? e->n : "?";
}

static UBOOL isa_of(const void* p, const char* cls)
{
	const Ent* e = ent(p);
	if (!e) return 0;
	char pat[64];
	snprintf(pat, sizeof(pat), " %s ", cls);
	char buf[128];
	snprintf(buf, sizeof(buf), " %s ", e->isa);
	return strstr(buf, pat) != NULL;
}


static void bits(const char* lbl, float f)
{
	union { float f; uint32_t u; } v;
	v.f = f;
	printf(" %s=%08x", lbl, (unsigned)v.u);
}

static void vec(const char* lbl, const FVector& v)
{
	bits(lbl, v.X); bits("", v.Y); bits("", v.Z);
}

static void case_line(const char* fam, int idx, const char* knobs)
{
	printf("case %s %d %s\n", fam, idx, knobs);
}

// ------------------------------------------------------------ hooks
UBOOL ActorHookIsA(const void* obj, const char* cls)
{
	UBOOL r = isa_of(obj, cls);
	printf("hook isa %s %s %d\n", nm(obj), cls, (int)r);
	return r;
}

UBOOL ActorHookIsBrush(const void* obj)
{
	// Matches the Rust boundary: actor_is_a(ABRUSH) is only reached when Brush != NULL.
	const AActor* a = (const AActor*)obj;
	if (a->Brush == NULL) return 0;
	UBOOL r = isa_of(obj, "ABrush");
	printf("hook is_brush %s %d\n", nm(obj), (int)r);
	return r;
}

void ActorHookEvent(void* obj, const char* name, void* arg)
{
	printf("hook event %s %s %s\n", nm(obj), name, nm(arg));
}

void ActorHookSetFlags(void* obj, DWORD flags)
{
	printf("hook set_flags %s %u\n", nm(obj), (unsigned)flags);
}

void ActorHookConditionalDestroy(void* obj) { printf("hook cond_destroy %s\n", nm(obj)); }
UBOOL ActorHookIsValid(void* obj) { printf("hook is_valid %s\n", nm(obj)); return obj != NULL; }
void ActorHookRemoveFromRoot(void* obj) { printf("hook remove_from_root %s\n", nm(obj)); }
void ActorHookDeleteRI(void* obj) { printf("hook delete_ri %s\n", nm(obj)); }
void ActorHookLoadLocalized(void* obj) { printf("hook load_localized %s\n", nm(obj)); }
void ActorHookValidate(void* prim, void* actor)
{
	printf("hook validate %s %s\n", nm(prim), nm(actor));
}
void ActorHookHash(void* actor, const char* op) { printf("hook hash %s %s\n", nm(actor), op); }
void ActorHookSuperProcessEvent(void* obj, void* func, void* parms, void* result)
{
	printf("hook super_process_event %s %s %s %s\n", nm(obj), nm(func), nm(parms), nm(result));
}
void ActorHookPossessed(void* pawn) { printf("hook possessed %s\n", nm(pawn)); }
void ActorHookCacheFlush() { printf("hook cache_flush\n"); }
UBOOL ActorHookCanCast(const void* obj, const char* cls)
{
	return isa_of(obj, cls);
}
void ActorHookFarMove(void* lvl, AActor* actor, FVector dest, INT test, INT no_check)
{
	printf("hook far_move %s ", nm(lvl)); vec("dest", dest);
	printf(" test=%d no_check=%d\n", test, no_check);
	actor->Location = dest;
}

void ActorHookMoveActor(void* lvl, AActor* actor, FVector delta, FRotator rot,
                        FCheckResult& hit, INT encroacher, INT wall, INT grav,
                        INT no_check)
{
	(void)hit;
	printf("hook move_actor %s %s", nm(lvl), nm(actor));
	vec("delta", delta);
	printf(" rot=%d,%d,%d flags=%d,%d,%d,%d\n", rot.Pitch, rot.Yaw, rot.Roll,
	       encroacher, wall, grav, no_check);
	actor->Rotation = rot;
}

INT ActorHookMoveSmooth(AActor* actor, FVector delta)
{
	printf("hook move_smooth %s", nm(actor));
	vec("delta", delta);
	printf("\n");
	if (!g_smooth_noop) actor->Location += delta;
	return 1;
}

void ActorHookSetPhysics(AActor* actor, BYTE physics)
{
	printf("hook set_physics %s %u\n", nm(actor), (unsigned)physics);
	actor->Physics = physics;
}


FVector ActorHookCollisionExtent(void* prim, void* actor)
{
	printf("hook collision_extent %s %s\n", nm(prim), nm(actor));
	return FVector(16.f, 24.f, 32.f);
}

FCoords ActorHookMeshRenderBox(void* mesh, void* actor, UBOOL exact)
{
	printf("hook mesh_render_box %s %s exact=%d\n", nm(mesh), nm(actor), (int)exact);
	return FCoords(0);
}

FCoords ActorHookBrushRenderBox(void* brush, void* actor, UBOOL exact)
{
	printf("hook brush_render_box %s %s exact=%d\n", nm(brush), nm(actor), (int)exact);
	return FCoords(0);
}

void ActorHookSingleLineCheck(void* level, FCheckResult& hit, void* src,
                              const FVector& end, const FVector& start,
                              DWORD flags, FVector extent)
{
	(void)end; (void)start; (void)extent;
	printf("hook single_line_check %s %s flags=%u\n", nm(level), nm(src), (unsigned)flags);
	hit.Actor = NULL;
	hit.Location = g_hit_loc;
}

const FBspSurf* ActorHookBspSurf(const FCheckResult& hit)
{
	(void)hit;
	printf("hook bsp_surf kind=%d\n", (int)g_trace_hit_kind);
	return g_trace_hit_kind ? (const FBspSurf*)g_surf : NULL;
}

UClass* ActorHookUClassStatic()
{
	printf("hook uclass_static\n");
	return (UClass*)g_uclass_static;
}

UBOOL ActorHookIsLoading(void* ar) { printf("hook is_loading %s\n", nm(ar)); return g_ar_loading; }
UBOOL ActorHookIsSaving(void* ar) { printf("hook is_saving %s\n", nm(ar)); return g_ar_saving; }
UBOOL ActorHookIsTrans(void* ar) { printf("hook is_trans %s\n", nm(ar)); return g_ar_trans; }
void ActorHookPreload(void* ar, void* cls) { printf("hook preload %s %s\n", nm(ar), nm(cls)); }
void ActorHookCountBytes(void* ar, SIZE_T n) { printf("hook count_bytes %s %u\n", nm(ar), (unsigned)n); }
void ActorHookSerializeTagged(void* cls, void* ar, void* obj)
{
	printf("hook serialize_tagged %s %s %s\n", nm(cls), nm(ar), nm(obj));
}
void ActorHookSerializeBin(void* cls, void* ar, void* obj)
{
	printf("hook serialize_bin %s %s %s\n", nm(cls), nm(ar), nm(obj));
}
SIZE_T ActorHookPropertiesSize(void* cls)
{
	printf("hook properties_size %s\n", nm(cls));
	return 96;
}

// Engine-side bodies the harness owns (UObject virtuals reached via Super::).
void UObject::Destroy() { printf("hook uobject_destroy %s\n", nm(this)); }
void UObject::PostLoad() { printf("hook uobject_postload %s\n", nm(this)); }
void UObject::PostEditChange() { printf("hook uobject_post_edit_change %s\n", nm(this)); }

// ------------------------------------------------------------ setup
static void zero_all()
{
	memset(g_self, 0, sizeof(g_self));
	memset(g_other, 0, sizeof(g_other));
	memset(g_owner, 0, sizeof(g_owner));
	memset(g_base, 0, sizeof(g_base));
	memset(g_touch, 0, sizeof(g_touch));
	memset(g_pawn, 0, sizeof(g_pawn));
	memset(g_zone, 0, sizeof(g_zone));
	memset(g_carcass, 0, sizeof(g_carcass));
	memset(g_level, 0, sizeof(g_level));
	memset(g_engine, 0, sizeof(g_engine));
	memset(g_class, 0, sizeof(g_class));
	memset(g_uclass_static, 0, sizeof(g_uclass_static));
	memset(g_model, 0, sizeof(g_model));
	memset(g_polys, 0, sizeof(g_polys));
	memset(g_mesh, 0, sizeof(g_mesh));
	memset(g_player, 0, sizeof(g_player));
	memset(g_ri, 0, sizeof(g_ri));
	memset(g_ar, 0, sizeof(g_ar));
	memset(g_frame, 0, sizeof(g_frame));
	memset(g_surf, 0, sizeof(g_surf));
	memset(g_prim, 0, sizeof(g_prim));
	memset(g_tex, 0, sizeof(g_tex));
	memset(g_hash, 0, sizeof(g_hash));
	memset(g_func, 0, sizeof(g_func));
	memset(g_fn_parms, 0, sizeof(g_fn_parms));
	g_nents = 0;
	g_ar_loading = g_ar_saving = g_ar_trans = 0;
	g_trace_hit_kind = 0;
	g_smooth_noop = 0;
	g_level_begun = 0;
	GIsEditor = 0;
	ActorArgTraceEnd = FVector(0.f, 0.f, 0.f);
	ActorArgTraceStart = FVector(0.f, 0.f, 0.f);
	ActorArgFlags = NULL;
	ActorArgDecals = 0;

	reg(g_self, "self", " AActor ");
	reg(g_other, "other", " AActor ");
	reg(g_owner, "owner", " AActor ");
	reg(g_base, "base", " AActor ");
	reg(g_touch, "touch", " AActor ");
	reg(g_pawn, "pawn", " AActor APlayerPawn APawn ");
	reg(g_zone, "zone", " AActor AZoneInfo ");
	reg(g_carcass, "carcass", " AActor ACarcass ");
	reg(g_level, "level");
	reg(g_engine, "engine");
	reg(g_class, "class");
	reg(g_uclass_static, "uclass_static");
	reg(g_model, "brush_model");
	reg(g_polys, "polys");
	reg(g_mesh, "mesh");
	reg(g_player, "player");
	reg(g_ri, "render_iface");
	reg(g_ar, "ar");
	reg(g_frame, "frame");
	reg(g_surf, "surf");
	reg(g_hash, "hash");
	reg(g_func, "func");
	reg(g_fn_parms, "parms");
	for (int i = 0; i < 5; ++i)
	{
		static const char* pn[5] = { "prim_cyl", "prim_ocyl", "prim_oval", "prim_ooval", "prim_box" };
		reg(g_prim[i], pn[i]);
	}
	for (int i = 0; i < 8; ++i)
	{
		static const char* tn[8] = { "tex0", "tex1", "tex2", "tex3", "tex4", "tex5", "tex6", "tex7" };
		reg(g_tex[i], tn[i]);
	}
}

static ULevel* level() { return (ULevel*)g_level; }
static AActor* self() { return (AActor*)g_self; }
static AActor* other() { return (AActor*)g_other; }

// Common world: level with engine, hash optional, self bound to level and class.
static void world(bool hash)
{
	ULevel* L = level();
	UEngine* E = (UEngine*)g_engine;
	L->Engine = E;
	((ALevelInfo*)g_levelinfo)->bBegunPlay = g_level_begun;
	L->Hash = hash ? reinterpret_cast<decltype(L->Hash)>(g_hash) : NULL;
	E->Cylinder = reinterpret_cast<decltype(E->Cylinder)>(g_prim[0]);
	E->OCylinder = reinterpret_cast<decltype(E->OCylinder)>(g_prim[1]);
	E->OvalCylinder = reinterpret_cast<decltype(E->OvalCylinder)>(g_prim[2]);
	E->OOvalCylinder = reinterpret_cast<decltype(E->OOvalCylinder)>(g_prim[3]);
	E->Box = reinterpret_cast<decltype(E->Box)>(g_prim[4]);
	AActor* A = self();
	A->XLevel = L;
	A->Class = (UClass*)g_class;
	AActor* O = other();
	O->XLevel = L;
	O->Class = (UClass*)g_class;
	((AActor*)g_owner)->XLevel = L;
	((AActor*)g_base)->XLevel = L;
	((AActor*)g_touch)->XLevel = L;
	((AActor*)g_zone)->XLevel = L;
	((AActor*)g_carcass)->XLevel = L;
	((APlayerPawn*)g_pawn)->XLevel = L;
	((APlayerPawn*)g_pawn)->Class = (UClass*)g_class;
	((ACarcass*)g_carcass)->Class = (UClass*)g_class;
	((AZoneInfo*)g_zone)->Class = (UClass*)g_class;
	((AActor*)g_owner)->Class = (UClass*)g_class;
	((AActor*)g_base)->Class = (UClass*)g_class;
	((AActor*)g_touch)->Class = (UClass*)g_class;
	ALevelInfo* LI = (ALevelInfo*)g_levelinfo;
	AActor* lv_actors[] = { self(), other(), (AActor*)g_owner, (AActor*)g_base, (AActor*)g_touch, (AActor*)g_zone, (AActor*)g_carcass, (AActor*)g_pawn };
	for (AActor* x : lv_actors) { x->Level = LI; x->XLevel = L; }
}


// ------------------------------------------------------------ families
static void fam_get_skin()
{
	for (int fill = 0; fill < 2; ++fill)
		for (int idx = 0; idx < 8; ++idx)
		{
			zero_all(); world(false);
			for (int k = 0; k < 8; ++k)
				self()->MultiSkins[k] = fill ? (UTexture*)g_tex[k] : NULL;
			char kn[64];
			snprintf(kn, sizeof(kn), "fill=%d index=%d", fill, idx);
			case_line("get_skin", fill * 8 + idx, kn);
			UTexture* r = self()->AActor::GetSkin(idx);
			printf("res skin=%s\n", nm(r));
		}
}

static void fam_get_net_priority()
{
	static const float times[3] = { 0.f, 0.5f, 2.f };
	static const int nufs[3] = { 0, 5, 20 };
	int idx = 0;
	for (int ar = 0; ar < 2; ++ar)
		for (int t = 0; t < 3; ++t)
			for (int u = 0; u < 3; ++u, ++idx)
			{
				zero_all(); world(false);
				self()->bAlwaysRelevant = ar;
				self()->NetPriority = 1.5f;
				self()->NetUpdateFrequency = (FLOAT)nufs[u];
				char kn[96];
				snprintf(kn, sizeof(kn), "always_relevant=%d time=%g net_update_freq=%d", ar, times[t], nufs[u]);
				case_line("get_net_priority", idx, kn);
				FLOAT r = self()->AActor::GetNetPriority(other(), times[t], 0.f);
				bits("res_priority", r); printf("\n");
			}
}

static void fam_set_collision()
{
	static const int nc[3][3] = { {0, 0, 0}, {1, 1, 1}, {1, 0, 1} };
	int idx = 0;
	for (int old = 0; old < 2; ++old)
		for (int hash = 0; hash < 2; ++hash)
			for (int n = 0; n < 3; ++n, ++idx)
			{
				zero_all(); world(hash);
				self()->bCollideActors = old;
				self()->bBlockActors = old;
				char kn[96];
				snprintf(kn, sizeof(kn), "old_collide=%d hash=%d new=%d%d%d", old, hash, nc[n][0], nc[n][1], nc[n][2]);
				case_line("set_collision", idx, kn);
				self()->AActor::SetCollision(nc[n][0], nc[n][1], nc[n][2]);
				printf("res collide=%d block=%d players=%d\n",
					(int)self()->bCollideActors, (int)self()->bBlockActors, (int)self()->bBlockPlayers);
			}
}

static void fam_set_collision_size()
{
	static const float rh[3][2] = { {20.f, 40.f}, {0.f, 0.f}, {-1.f, 5.f} };
	int idx = 0;
	for (int old = 0; old < 2; ++old)
		for (int hash = 0; hash < 2; ++hash)
			for (int s = 0; s < 3; ++s, ++idx)
			{
				zero_all(); world(hash);
				self()->bCollideActors = old;
				char kn[96];
				snprintf(kn, sizeof(kn), "old_collide=%d hash=%d radius=%g height=%g", old, hash, rh[s][0], rh[s][1]);
				case_line("set_collision_size", idx, kn);
				self()->AActor::SetCollisionSize(rh[s][0], rh[s][1]);
				bits("res_radius", self()->CollisionRadius); bits("height", self()->CollisionHeight);
				printf(" collide=%d\n", (int)self()->bCollideActors);
			}
}

static void fam_set_owner()
{
	int idx = 0;
	for (int old = 0; old < 2; ++old)
		for (int nw = 0; nw < 2; ++nw, ++idx)
		{
			zero_all(); world(false);
			self()->Owner = old ? (AActor*)g_owner : NULL;
			char kn[64];
			snprintf(kn, sizeof(kn), "old_owner=%d new_owner=%d", old, nw);
			case_line("set_owner", idx, kn);
			self()->AActor::SetOwner(nw ? (AActor*)g_owner : NULL);
			printf("res owner=%s\n", nm(self()->Owner));
		}
}

static void fam_set_base()
{
	// old base: none/base; new base: none/base/self; notify: 0/1
	int idx = 0;
	for (int old = 0; old < 2; ++old)
		for (int nw = 0; nw < 3; ++nw)
			for (int notify = 0; notify < 2; ++notify, ++idx)
			{
				zero_all(); world(false);
				self()->Base = old ? (AActor*)g_base : NULL;
				AActor* target = nw == 0 ? NULL : (nw == 1 ? (AActor*)g_base : self());
				char kn[96];
				snprintf(kn, sizeof(kn), "old_base=%d new_base=%d notify=%d", old, nw, notify);
				case_line("set_base", idx, kn);
				self()->AActor::SetBase(target, notify);
				printf("res base=%s\n", nm(self()->Base));
			}
}

static void fam_begin_touch()
{
	// (self role, other role); 0 plain, 1 projectile, 2 trigger.
	static const int roles[6][2] = { {1, 2}, {2, 1}, {0, 0}, {0, 2}, {1, 0}, {2, 0} };
	static const char* rn[3] = { "plain", "projectile", "trigger" };
	static const char* risa[3] = { " AActor ", " AActor AProjectile ", " AActor ATrigger " };
	int idx = 0;
	for (int r = 0; r < 6; ++r)
		for (int full = 0; full < 2; ++full, ++idx)
		{
			zero_all(); world(false);
			reg(g_self, "self", risa[roles[r][0]]);
			reg(g_other, "other", risa[roles[r][1]]);
			if (full)
				for (int i = 0; i < 4; ++i) self()->Touching[i] = (AActor*)g_touch;
			char kn[128];
			snprintf(kn, sizeof(kn), "self_role=%s other_role=%s touching_full=%d",
				rn[roles[r][0]], rn[roles[r][1]], full);
				case_line("begin_touch", idx, kn);
				self()->AActor::BeginTouch(other());
				printf("res touching0=%s touching1=%s\n", nm(self()->Touching[0]), nm(self()->Touching[1]));
			}
}

static void fam_end_touch()
{
	int idx = 0;
	for (int ts = 0; ts < 2; ++ts)
		for (int to = 0; to < 2; ++to)
			for (int nn = 0; nn < 2; ++nn, ++idx)
			{
				zero_all(); world(false);
				if (ts) self()->Touching[0] = other();
				if (to) other()->Touching[0] = self();
				char kn[96];
				snprintf(kn, sizeof(kn), "self_touching=%d other_touching=%d no_notify_self=%d", ts, to, nn);
				case_line("end_touch", idx, kn);
				self()->AActor::EndTouch(other(), nn);
				printf("res self_touch0=%s other_touch0=%s\n",
					nm(self()->Touching[0]), nm(other()->Touching[0]));
			}
}

static void fam_is_overlapping()
{
	// other kind: near / far / brush (other is a brush: Brush set, class ABrush)
	const char* okind[3] = { "near", "far", "brush" };
	int idx = 0;
	for (int sb = 0; sb < 2; ++sb)
		for (int ok = 0; ok < 3; ++ok, ++idx)
		{
			zero_all(); world(false);
			if (sb)
			{
				self()->Brush = (UModel*)g_model;
				reg(g_self, "self", " AActor ABrush ");
			}
			if (ok == 2)
			{
				other()->Brush = (UModel*)g_model;
				reg(g_other, "other", " AActor ABrush ");
			}
			self()->Location = FVector(0.f, 0.f, 0.f);
			other()->Location = ok == 1 ? FVector(10000.f, 0.f, 0.f) : FVector(5.f, 0.f, 0.f);
			self()->CollisionRadius = 10.f;
			self()->CollisionHeight = 10.f;
			other()->CollisionRadius = 10.f;
			other()->CollisionHeight = 10.f;
			char kn[96];
			snprintf(kn, sizeof(kn), "self_brush=%d other=%s", sb, okind[ok]);
			case_line("is_overlapping", idx, kn);
			UBOOL r = self()->AActor::IsOverlapping(other());
			printf("res overlapping=%d\n", (int)r);
		}
}

static void fam_get_visibility_box()
{
	static const float rh[3][2] = { {0.f, 0.f}, {100.f, 50.f}, {-1.f, 2.f} };
	for (int i = 0; i < 3; ++i)
	{
		zero_all(); world(false);
		self()->Location = FVector(10.f, 20.f, 30.f);
		self()->VisibilityRadius = rh[i][0];
		self()->VisibilityHeight = rh[i][1];
		char kn[96];
		snprintf(kn, sizeof(kn), "radius=%g height=%g", rh[i][0], rh[i][1]);
		case_line("get_visibility_box", i, kn);
		FBox b = self()->AActor::GetVisibilityBox();
		vec("res_min", b.Min); vec(" max", b.Max); printf("\n");
	}
}

static void fam_carcass_update_frequency()
{
	int idx = 0;
	for (int phys = 0; phys < 2; ++phys)
		for (int anim = 0; anim < 2; ++anim)
			for (int u = 0; u < 3; ++u, ++idx)
			{
				static const int nuf[3] = { 2, 4, 9 };
				zero_all(); world(false);
				ACarcass* c = (ACarcass*)g_carcass;
				c->Physics = phys ? 1 : 0;
				c->AnimRate = anim ? 1.f : 0.f;
				if (anim) c->AnimSequence.Index = 1;
				c->NetUpdateFrequency = (FLOAT)nuf[u];
				char kn[96];
				snprintf(kn, sizeof(kn), "physics=%d anim=%d net_update_freq=%d", phys, anim, nuf[u]);
				case_line("carcass_update_frequency", idx, kn);
				FVector dir(1.f, 0.f, 0.f), pos(0.f, 0.f, 0.f);
				FLOAT r = c->ACarcass::UpdateFrequency(other(), dir, pos);
				bits("res_freq", r); printf("\n");
			}
}

static void fam_get_hit_actor()
{
	for (int h = 0; h < 2; ++h)
	{
		zero_all(); world(false);
		if (h) self()->HitActor = other();
		char kn[32];
		snprintf(kn, sizeof(kn), "hit_actor=%d", h);
		case_line("get_hit_actor", h, kn);
		AActor* r = self()->AActor::GetHitActor();
		printf("res hit=%s\n", nm(r));
	}
}

static void fam_post_edit_change()
{
	for (int e = 0; e < 2; ++e)
	{
		zero_all(); world(false);
		GIsEditor = e;
		char kn[32];
		snprintf(kn, sizeof(kn), "editor=%d", e);
		case_line("post_edit_change", e, kn);
		self()->AActor::PostEditChange();
		printf("res light_changed=%d\n", (int)self()->bLightChanged);
	}
}

static void fam_zone_info_post_edit_change()
{
	for (int e = 0; e < 2; ++e)
	{
		zero_all(); world(false);
		GIsEditor = e;
		char kn[32];
		snprintf(kn, sizeof(kn), "editor=%d", e);
		case_line("zone_info_post_edit_change", e, kn);
		((AZoneInfo*)g_zone)->AZoneInfo::PostEditChange();
		printf("res done=1\n");
	}
}

static void fam_get_primitive()
{
	int idx = 0;
	for (int ct = 0; ct < 6; ++ct)
		for (int m = 0; m < 2; ++m, ++idx)
		{
			zero_all(); world(false);
			self()->CollideType = ct;
			self()->Mesh = m ? (UMesh*)g_mesh : NULL;
			char kn[64];
			snprintf(kn, sizeof(kn), "collide_type=%d mesh=%d", ct, m);
			case_line("get_primitive", idx, kn);
			UPrimitive* p = self()->AActor::GetPrimitive();
			printf("res prim=%s\n", nm(p));
		}
}

static void fam_spawned()
{
	int idx = 0;
	for (int ct = 0; ct < 2; ++ct)
		for (int m = 0; m < 2; ++m)
			for (int b = 0; b < 2; ++b, ++idx)
			{
				static const int cts[2] = { 3, 0 };
				zero_all(); world(false);
				self()->CollideType = cts[ct];
				self()->Mesh = m ? (UMesh*)g_mesh : NULL;
				self()->Brush = b ? (UModel*)g_model : NULL;
				char kn[96];
				snprintf(kn, sizeof(kn), "collide_type=%d mesh=%d brush=%d", cts[ct], m, b);
				case_line("spawned", idx, kn);
				self()->AActor::Spawned();
				printf("res done=1\n");
			}
}

static void fam_process_event()
{
	int idx = 0;
	for (int begun = 0; begun < 2; ++begun)
		for (int fn = 0; fn < 2; ++fn, ++idx)
		{
			zero_all(); world(false);
			g_level_begun = begun;
			((ALevelInfo*)g_levelinfo)->bBegunPlay = begun;
			reg(g_func, "func");
			char kn[64];
			snprintf(kn, sizeof(kn), "begun=%d function=%d", begun, fn);
			case_line("process_event", idx, kn);
			self()->AActor::ProcessEvent((UFunction*)(fn ? (void*)g_func : NULL), g_fn_parms, NULL);
			printf("res done=1\n");
		}
}

static void fam_destroy()
{
	for (int ri = 0; ri < 2; ++ri)
	{
		zero_all(); world(false);
		self()->RenderInterface = ri ? (URenderIterator*)g_ri : NULL;
		char kn[32];
		snprintf(kn, sizeof(kn), "render_iface=%d", ri);
		case_line("destroy", ri, kn);
		self()->AActor::Destroy();
		printf("res render_iface=%s\n", nm(self()->RenderInterface));
	}
}

static void fam_post_load()
{
	int idx = 0;
	for (int loc = 0; loc < 2; ++loc)
		for (int brush = 0; brush < 2; ++brush)
			for (int polys = 0; polys < 2; ++polys)
				for (int ct = 0; ct < 2; ++ct, ++idx)
				{
					static const int cts[2] = { 3, 0 };
					zero_all(); world(false);
					((UClass*)g_class)->ClassFlags = loc ? CLASS_Localized : 0;
					self()->Brush = brush ? (UModel*)g_model : NULL;
					if (brush) ((UModel*)g_model)->Polys = polys ? (UPolys*)g_polys : NULL;
					self()->CollideType = cts[ct];
					char kn[96];
					snprintf(kn, sizeof(kn), "localized=%d brush=%d polys=%d collide_type=%d",
						loc, brush, polys, cts[ct]);
					case_line("post_load", idx, kn);
					self()->AActor::PostLoad();
					printf("res done=1\n");
				}
}

static void fam_set_player()
{
	int idx = 0;
	for (int prev = 0; prev < 2; ++prev)
		for (int pp = 0; pp < 2; ++pp, ++idx)
		{
			zero_all(); world(false);
			APlayerPawn* pawn = (APlayerPawn*)g_pawn;
			UPlayer* pl = (UPlayer*)g_player;
			if (prev) pl->Actor = (APlayerPawn*)g_other;
			if (pp) pawn->Player = (UPlayer*)g_player;
			char kn[64];
			snprintf(kn, sizeof(kn), "prev_actor=%d pawn_player=%d", prev, pp);
			case_line("set_player", idx, kn);
			pawn->APlayerPawn::SetPlayer(pl);
			printf("res pawn_player=%s\n", nm(pawn->Player));
		}
}

static void fam_get_cylinder_extent()
{
	static const int cts[6] = { 0, 1, 2, 3, 4, 5 };
	int idx = 0;
	for (int c = 0; c < 6; ++c)
		for (int w = 0; w < 2; ++w, ++idx)
		{
			zero_all(); world(false);
			self()->CollideType = cts[c];
			self()->CollisionWidth = w ? 7.f : 0.f;
			self()->CollisionRadius = 12.f;
			self()->CollisionHeight = 24.f;
			char kn[96];
			snprintf(kn, sizeof(kn), "collide_type=%d width=%g", cts[c], w ? 7.f : 0.f);
			case_line("get_cylinder_extent", idx, kn);
			FVector r = self()->AActor::GetCylinderExtent();
			vec("res", r); printf("\n");
		}
	// Shape with a mesh and shape with a brush.
	zero_all(); world(false);
	self()->CollideType = 3;
	self()->Mesh = (UMesh*)g_mesh;
	case_line("get_cylinder_extent", idx, "collide_type=3 mesh=1 brush=0");
	{ FVector r = self()->AActor::GetCylinderExtent(); vec("res", r); printf("\n"); }
	++idx;
	zero_all(); world(false);
	self()->CollideType = 3;
	self()->Brush = (UModel*)g_model;
	case_line("get_cylinder_extent", idx, "collide_type=3 mesh=0 brush=1");
	{ FVector r = self()->AActor::GetCylinderExtent(); vec("res", r); printf("\n"); }
}

static void fam_get_render_bounding_box()
{
	int idx = 0;
	for (int ex = 0; ex < 2; ++ex)
		for (int m = 0; m < 2; ++m)
			for (int b = 0; b < 2; ++b, ++idx)
			{
				zero_all(); world(false);
				self()->Mesh = m ? (UMesh*)g_mesh : NULL;
				self()->Brush = b ? (UModel*)g_model : NULL;
				char kn[96];
				snprintf(kn, sizeof(kn), "exact=%d mesh=%d brush=%d", ex, m, b);
				case_line("get_render_bounding_box", idx, kn);
				FCoords r = self()->AActor::GetRenderBoundingBox(ex);
				vec("res_origin", r.Origin); printf("\n");
			}
}

static void fam_serialize_persistence()
{
	static const int combos[6][3] = { {0,0,0}, {0,0,1}, {1,0,0}, {1,0,1}, {0,1,0}, {0,1,1} };
	for (int i = 0; i < 6; ++i)
	{
		zero_all(); world(false);
		g_ar_loading = combos[i][0];
		g_ar_saving = combos[i][1];
		g_ar_trans = combos[i][2];
		char kn[96];
		snprintf(kn, sizeof(kn), "loading=%d saving=%d trans=%d",
			combos[i][0], combos[i][1], combos[i][2]);
		case_line("serialize_persistence", i, kn);
		self()->AActor::SerializePersistence(*(FArchive*)g_ar);
		printf("res done=1\n");
	}
}

static void fam_trace_texture()
{
	int idx = 0;
	for (int kind = 0; kind < 3; ++kind)
		for (int dec = 0; dec < 2; ++dec, ++idx)
		{
			zero_all(); world(false);
			g_trace_hit_kind = kind;
			self()->XLevel->Model = (UModel*)g_model;
			if (kind >= 1)
			{
				FBspSurf* s = (FBspSurf*)g_surf;
				s->Texture = (UTexture*)g_tex[0];
				s->PolyFlags = kind == 2 ? 0x10u : 0x1u;
			}
			if (kind == 2) ((UTexture*)g_tex[0])->PolyFlags = 0x100u;
			ActorArgTraceEnd = FVector(0.f, 0.f, -100.f);
			ActorArgTraceStart = FVector(0.f, 0.f, 100.f);
			INT flags = 0;
			ActorArgFlags = &flags;
			ActorArgDecals = dec;
			char kn[96];
			snprintf(kn, sizeof(kn), "surf=%d trace_decals=%d", kind, dec);
			case_line("trace_texture", idx, kn);
			void* out = NULL;
			self()->AActor::execTraceTexture(*(FFrame*)g_frame, &out);
			printf("res texture=%s flags=%u\n", nm(out), (unsigned)flags);
		}
}

// Decal-path cases: PointInsideQuad inputs that flip between contract on and off.
// Hit.Location - SurfBase is exact because SurfBase is zero, so the replay sees the
// same point the standalone generator searched.
static void fam_trace_decal_flip()
{
	for (int c = 0; c < kFlipCount; ++c)
	{
		const FlipCase& fc = kFlipCases[c];
		float f[18];
		std::memcpy(f, fc.bits, sizeof(f));
		zero_all(); world(false);
		g_trace_hit_kind = 1;
		UModel* model = (UModel*)g_model;
		self()->XLevel->Model = model;
		FBspSurf* s = (FBspSurf*)g_surf;
		s->Texture = (UTexture*)g_tex[0];
		s->PolyFlags = 0x1u;
		s->pBase = 0;
		s->vNormal = 0;
		g_pts[0] = FVector(0.f, 0.f, 0.f);
		g_vecs[0] = FVector(f[12], f[13], f[14]);
		model->Points.Data = g_pts;
		model->Points.ArrayNum = 1;
		model->Vectors.Data = g_vecs;
		model->Vectors.ArrayNum = 1;
		for (int k = 0; k < 4; ++k)
			g_decals[0].Vertices[k] = FVector(f[3 * k], f[3 * k + 1], f[3 * k + 2]);
		g_decals[0].Actor = (ADecal*)g_decal_actor;
		((AActor*)g_decal_actor)->Texture = (UTexture*)g_tex[1];
		s->Decals.Data = g_decals;
		s->Decals.ArrayNum = 1;
		g_hit_loc = FVector(f[15], f[16], f[17]);
		ActorArgTraceEnd = FVector(0.f, 0.f, -100.f);
		ActorArgTraceStart = FVector(0.f, 0.f, 100.f);
		INT flags = 0;
		ActorArgFlags = &flags;
		ActorArgDecals = 1;
		char kn[512];
		int kp = snprintf(kn, sizeof(kn), "decal_flip bits=");
		for (int w = 0; w < 18; ++w) kp += snprintf(kn + kp, sizeof(kn) - kp, "%s%08x", w ? "," : "", fc.bits[w]);
		case_line("trace_decal_flip", c, kn);
		void* out = NULL;
		self()->AActor::execTraceTexture(*(FFrame*)g_frame, &out);
		printf("res texture=%s flags=%u\n", nm(out), (unsigned)flags);
	}
}

static void fam_net_receive()
{
	for (int c = 0; c < 11; ++c)
	{
		zero_all();
		memset(g_net_obj, 0, sizeof(g_net_obj));
		memset(g_client, 0, sizeof(g_client));
		memset(g_demo_driver, 0, sizeof(g_demo_driver));
		memset(g_viewport, 0, sizeof(g_viewport));
		memset(g_net_connection, 0, sizeof(g_net_connection));
		g_smooth_noop = c == 1 || c == 3;
		world(true);
		ULevel* L = level();
		AActor* a = reinterpret_cast<AActor*>(g_net_obj);
		const char* isa = c == 2 ? " AActor ABrush AMover "
			: (c == 4 ? " AActor " : ((c == 5 || c == 9 || c == 10) ? " AActor APlayerPawn APawn "
			: (c == 7 ? " AActor APlayerReplicationInfo " : " AActor APawn ")));
		reg(g_net_obj, "self", isa);
		a->XLevel = L;
		a->Level = (ALevelInfo*)g_levelinfo;
		a->bCollideActors = c == 4;
		if (c == 2)
		{
			AMover* m = reinterpret_cast<AMover*>(a);
			m->SimInterpolate = FVector(1.f, 2.f, 3.f);
			m->SimOldPos = FVector(7.f, 8.f, 9.f);
			m->SimOldRotPitch = 10;
			m->SimOldRotYaw = 20;
			m->SimOldRotRoll = 30;
		}
		else if (c != 5 && c != 7)
		{
			a->Role = ROLE_SimulatedProxy;
			a->Velocity = FVector(1.f, 0.f, 0.f);
			a->AnimMinRate = 0.2f;
		}
		if (c == 5 || c == 10)
		{
			UDemoRecDriver* d = reinterpret_cast<UDemoRecDriver*>(g_demo_driver);
			if (c == 5)
				d->ServerConnection = reinterpret_cast<UNetConnection*>(g_hash);
			L->DemoRecDriver = d;
		}
		if (c == 7)
		{
			UClient* client = reinterpret_cast<UClient*>(g_client);
			UViewport* viewport = reinterpret_cast<UViewport*>(g_viewport);
			UNetDriver* driver = reinterpret_cast<UNetDriver*>(g_demo_driver);
			UNetConnection* conn = reinterpret_cast<UNetConnection*>(g_net_connection);
			((ALevelInfo*)g_levelinfo)->NetMode = NM_Client;
			L->NetDriver = driver;
			driver->ServerConnection = conn;
			conn->AverageFrameTime = 0.011f;
			((UEngine*)g_engine)->Client = client;
			g_viewport_array[0] = viewport;
			client->Viewports.Data = g_viewport_array;
			client->Viewports.ArrayNum = 1;
			viewport->Actor = (APlayerPawn*)g_pawn;
			((APawn*)g_pawn)->PlayerReplicationInfo = (APlayerReplicationInfo*)g_net_obj;
			((APlayerReplicationInfo*)a)->Ping = 10;
		}
		static const char* kinds[11] = {
			"pawn_small", "pawn_smooth", "mover", "pawn_fma",
			"generic_state", "demo_view", "pawn_teleport", "pri_ping",
			"pawn_over_10000", "demo_driver_absent", "demo_server_connection_absent"
		};
		case_line("net_receive", c, kinds[c]);
		a->AActor::PreNetReceive();
		switch (c)
		{
		case 0:
			a->Location = FVector(10.f, 0.f, 0.f);
			a->SimAnim = FPlane(1000.f, 2000.f, 3000.f, -4000.f);
			break;
		case 1:
			a->Location = FVector(50.f, 0.f, 0.f);
			break;
		case 2:
			reinterpret_cast<AMover*>(a)->SimInterpolate = FVector(4.f, 5.f, 513.5f);
			break;
		case 3:
			a->Location = FVector(34.52090835571289f, 26.258970260620117f, 90.10423278808594f);
			break;
		case 4:
			a->bCollideActors = 0;
			a->Base = (AActor*)g_base;
			a->Rotation = FRotator(123, -456, 789);
			a->CollisionRadius = 3.f;
			a->CollisionHeight = 4.f;
			a->bBlockActors = 1;
			a->bBlockPlayers = 0;
			break;
		case 5:
		case 9:
		case 10:
			reinterpret_cast<APlayerPawn*>(a)->DemoViewPitch = 1234;
			reinterpret_cast<APlayerPawn*>(a)->DemoViewYaw = -5678;
			break;
		case 6:
			a->Location = FVector(100.f, 0.f, 0.f);
			a->SimAnim = FPlane(1000.f, 2000.f, 3000.f, 4000.f);
			break;
		case 8:
			a->Location = FVector(101.f, 0.f, 0.f);
			break;
		}
		a->AActor::PostNetReceive();
		printf("res loc="); vec("", a->Location);
		printf(" anim=%08x,%08x,%08x,%08x loop=%u min=%08x tele=%u",
		       *(unsigned*)&a->AnimFrame, *(unsigned*)&a->AnimRate,
		       *(unsigned*)&a->TweenRate, *(unsigned*)&a->AnimLast,
		       (unsigned)a->bAnimLoop, *(unsigned*)&a->AnimMinRate,
		       (unsigned)a->bJustTeleported);
		if (c == 2)
		{
			AMover* m = reinterpret_cast<AMover*>(a);
			printf(" key=%u,%u alpha=%08x rate=%08x interpolating=%u",
			       (unsigned)m->KeyNum, (unsigned)m->PrevKeyNum,
			       *(unsigned*)&m->PhysAlpha, *(unsigned*)&m->PhysRate,
			       (unsigned)m->bInterpolating);
		}
		if (c == 4)
			printf(" base=%s collide=%u block=%u,%u radius=%08x height=%08x rot=%d,%d,%d",
			       nm(a->Base), (unsigned)a->bCollideActors, (unsigned)a->bBlockActors,
			       (unsigned)a->bBlockPlayers, *(unsigned*)&a->CollisionRadius,
			       *(unsigned*)&a->CollisionHeight, a->Rotation.Pitch, a->Rotation.Yaw,
			       a->Rotation.Roll);
		if (c == 5 || c == 9 || c == 10)
		{
			APawn* p = reinterpret_cast<APawn*>(a);
			printf(" viewrot=%d,%d", p->ViewRotation.Pitch, p->ViewRotation.Yaw);
		}
		if (c == 7)
			printf(" ping=%d", ((APlayerReplicationInfo*)a)->Ping);
		printf("\n");
	}
}

// ------------------------------------------------------------ main
int main()
{
	fam_get_skin();
	fam_get_net_priority();
	fam_set_collision();
	fam_set_collision_size();
	fam_set_owner();
	fam_set_base();
	fam_begin_touch();
	fam_end_touch();
	fam_is_overlapping();
	fam_get_visibility_box();
	fam_carcass_update_frequency();
	fam_get_hit_actor();
	fam_post_edit_change();
	fam_zone_info_post_edit_change();
	fam_get_primitive();
	fam_spawned();
	fam_process_event();
	fam_destroy();
	fam_post_load();
	fam_set_player();
	fam_get_cylinder_extent();
	fam_get_render_bounding_box();
	fam_serialize_persistence();
	fam_trace_texture();
	fam_trace_decal_flip();
	fam_net_receive();
	return 0;
}

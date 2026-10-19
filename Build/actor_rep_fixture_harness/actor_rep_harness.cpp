// Oracle harness for the replication block of the pristine UnActor.cpp (ref 90d1e8d).
// gen_actor_rep.sh compiles that translation unit unchanged and links it against
// this file. Every case is one `case` record (its knob tokens) followed by `res`
// lines. The Rust replay (crates/hp2-engine/tests/actor_rep_replay.rs) reads the
// same layout and knob grammar and must reproduce every record bit-exactly.
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cwchar>
#include <new>
#include <string>
#include <type_traits>
#include <vector>
#include "EnginePrivate.h"
#include "UnNet.h"
#include "UnMesh.h"

UBOOL GIsEditor = 0;
// Native registration runs in static initializers of the original TU; no table is needed here.
BYTE GRegisterNative(INT, const Native&) { return 0; }
// Static initializers of the original TU allocate through GMalloc; back it with libc.
class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { return malloc( Count ); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag ) { return realloc( Original, Count ); }
	void Free( void* Original ) { free( Original ); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc g_harness_malloc;
FMalloc* GMalloc = &g_harness_malloc;

namespace {

[[noreturn]] void fail(const char* msg, const std::string& detail = std::string())
{
	fprintf(stderr, "HARNESS FAIL: %s %s\n", msg, detail.c_str());
	exit(2);
}

enum Kind { K_ACTOR, K_PAWN, K_PLAYERPAWN, K_MOVER, K_ZONE, K_LEVEL, K_PRI, K_GRI, K_INV, K_WEAPON, K_AMMO, K_PICKUP, K_COUNT };
enum Mode { M_REP, M_FREQ, M_SCRIPT, M_NOVARS };

// Owned class names (Rust mirror names) and single-parent chain for IsA. AInfo is not a kind.
const char* const kTypeName[K_COUNT] = {
	"AActor", "APawn", "APlayerPawn", "AMover", "AZoneInfo", "ALevelInfo",
	"APlayerReplicationInfo", "AGameReplicationInfo", "AInventory", "AWeapon", "AAmmo", "APickup" };
const int kParent[K_COUNT] = { -1, 0, 1, 0, 0, 4, 0, 0, 0, 8, 11, 8 };
const size_t kSize[K_COUNT] = {
	sizeof(AActor), sizeof(APawn), sizeof(APlayerPawn), sizeof(AMover), sizeof(AZoneInfo), sizeof(ALevelInfo),
	sizeof(APlayerReplicationInfo), sizeof(AGameReplicationInfo), sizeof(AInventory), sizeof(AWeapon), sizeof(AAmmo), sizeof(APickup) };

const size_t kObjBytes = 16384;
alignas(16) unsigned char g_cls_buf[K_COUNT][sizeof(UClass)];
alignas(16) unsigned char g_prop_cls_buf[sizeof(UClass)];
UClass* g_class[K_COUNT];
UClass* g_prop_class = nullptr;
alignas(16) unsigned char g_self[kObjBytes];
alignas(16) unsigned char g_recent[kObjBytes];
alignas(16) unsigned char g_pawn[kObjBytes];
alignas(16) unsigned char g_defobj[kObjBytes];
TCHAR g_strpool[64][64];
int g_strnext = 0;
const TCHAR kEmptyStr[] = L"";
UObject* g_reject_obj = nullptr;

// Type-keyed classification of field types into single-character tags.
// f float, i 32-bit int/DWORD/enum, b 8-bit/enum, w 16-bit, x bitfield bit,
// v FVector, r FRotator, p FPlane, n FName, c FColor, s FString, o object pointer.
template<class X, class E = void> struct KindOf { static constexpr char value = '?'; };
template<> struct KindOf<float> { static constexpr char value = 'f'; };
template<> struct KindOf<int> { static constexpr char value = 'i'; };
template<> struct KindOf<unsigned int> { static constexpr char value = 'i'; };
template<> struct KindOf<unsigned char> { static constexpr char value = 'b'; };
template<> struct KindOf<unsigned short> { static constexpr char value = 'w'; };
template<> struct KindOf<FVector> { static constexpr char value = 'v'; };
template<> struct KindOf<FRotator> { static constexpr char value = 'r'; };
template<> struct KindOf<FPlane> { static constexpr char value = 'p'; };
template<> struct KindOf<FName> { static constexpr char value = 'n'; };
template<> struct KindOf<FColor> { static constexpr char value = 'c'; };
template<> struct KindOf<FString> { static constexpr char value = 's'; };
template<> struct KindOf<FStringNoInit> { static constexpr char value = 's'; };
template<class X> struct KindOf<X*, void> { static constexpr char value = 'o'; };
template<class X> struct KindOf<X, typename std::enable_if<std::is_enum<X>::value>::type>
{ static constexpr char value = sizeof(X) == 1 ? 'b' : 'i'; };

template<class M> struct ElemOf { static constexpr size_t count = std::extent<M>::value ? std::extent<M>::value : 1; };

struct Entry
{
	int kind;
	std::string name;
	char tag;
	size_t off;
	size_t size;
	size_t count;
	unsigned mask;
	bool toggle;
};
std::vector<Entry> g_reg[K_COUNT];

void add_entry(int k, const char* name, char tag, size_t off, size_t size, size_t count, unsigned mask, bool toggle)
{
	if (tag == '?') fail("unclassified field type", std::string(kTypeName[k]) + "." + name);
	g_reg[k].push_back(Entry{ k, name, tag, off, size, count, mask, toggle });
}

// Bitfields have no offsetof; probe the containing byte and mask by setting the bit on zeroed storage.
template<class T> void add_bit(int k, const char* name, void (*set)(void*))
{
	std::vector<unsigned char> z(sizeof(T), 0);
	set(z.data());
	size_t off = 0;
	unsigned mask = 0;
	int nonzero = 0;
	for (size_t i = 0; i < z.size(); ++i)
		if (z[i]) { ++nonzero; off = i; mask = z[i]; }
	if (nonzero != 1 || (mask & (mask - 1)) != 0) fail("bitfield probe", std::string(kTypeName[k]) + "." + name);
	add_entry(k, name, 'x', off, 1, 1, mask, true);
}

#define RF(F) do { using M_ = decltype(((T*)nullptr)->F); \
	add_entry(k, #F, KindOf<typename std::remove_all_extents<M_>::type>::value, __builtin_offsetof(T, F), sizeof(M_), ElemOf<M_>::count, 0, true); } while (0)
#define RFN(F) do { using M_ = decltype(((T*)nullptr)->F); \
	add_entry(k, #F, KindOf<typename std::remove_all_extents<M_>::type>::value, __builtin_offsetof(T, F), sizeof(M_), ElemOf<M_>::count, 0, false); } while (0)
#define RB(F) add_bit<T>(k, #F, [](void* p) { ((T*)p)->F = 1; })

// Gate fields (LifeSpan, NetUpdateFrequency, bNet*, ...) and the DOREP lists of AActor.
template<class T> void reg_actor(int k)
{
	RFN(Class);
	RF(Acceleration); RF(AmbientGlow); RF(AmbientSound); RF(AnimMinRate); RF(AnimSequence); RF(Base); RF(Brush);
	RF(CollisionHeight); RF(CollisionRadius); RF(DesiredRotation); RF(DrawScale); RF(DrawType); RF(Fatness);
	RF(Instigator); RF(Inventory); RF(LightBrightness); RF(LightEffect); RF(LightHue); RF(LightPeriod); RF(LightPhase);
	RF(LightRadius); RF(LightRadiusInner); RF(LightSaturation); RF(LightType); RF(Location); RF(Mesh); RF(MultiSkins);
	RF(Owner); RF(Physics); RF(PrePivot); RF(RemoteRole); RF(Role); RF(Rotation); RF(RotationRate); RF(ScaleGlow);
	RF(SimAnim); RF(Skin); RF(SoundOcclusion); RF(SoundPitch); RF(SoundRadius); RF(SoundVolume); RF(Sprite); RF(Style);
	RF(Texture); RF(Velocity); RF(VolumeBrightness); RF(VolumeRadius); RF(Wideness);
	RF(LifeSpan); RF(NetUpdateFrequency);
	RB(bAnimNotify); RB(bBlockActors); RB(bBlockPlayers); RB(bBounce); RB(bCollideActors); RB(bCollideWorld);
	RB(bDarkLight); RB(bFixedRotationDir); RB(bHidden); RB(bMeshEnviroMap); RB(bNetOwner); RB(bOnlyOwnerSee);
	RB(bProjTarget); RB(bRotateToDesired); RB(bSpecialLit); RB(bUnlit);
	RB(bNetTemporary); RB(bNetOptional); RB(bNetInitial); RB(bClientAnim); RB(bCarriedItem); RB(bReplicateInstigator);
	RB(bSimFall); RB(bIsMover); RB(bSimulatedPawn); RB(bDemoRecording); RB(bAlwaysRelevant); RB(bLightChanged);
	RB(bJustTeleported); RB(bInterpolating);
}
template<class T> void reg_pawn(int k)
{
	RF(AccelRate); RF(AirControl); RF(AirSpeed); RF(EyeHeight); RF(GroundSpeed); RF(Health); RF(JumpZ);
	RF(MoveTarget); RF(PlayerReplicationInfo); RF(SelectedItem); RF(ViewRotation); RF(WaterSpeed); RF(Weapon);
	RF(carriedDecoration);
	RB(bBehindView); RB(bCanFly); RB(bIsPlayer);
}
template<class T> void reg_playerpawn(int k)
{
	RF(DemoViewPitch); RF(DemoViewYaw); RF(GameReplicationInfo); RF(HUDType); RF(Password); RF(ScoringType);
	RF(TargetEyeHeight); RF(TargetViewRotation); RF(TargetWeaponViewOffset); RF(ViewTarget);
	RB(bCheatsEnabled); RB(bFixedCamera); RB(bNeverAutoSwitch); RB(bReadyToPlay);
}
template<class T> void reg_mover(int k)
{
	RF(RealPosition); RF(RealRotation); RF(SimInterpolate); RF(SimOldPos); RF(SimOldRotPitch); RF(SimOldRotRoll);
	RF(SimOldRotYaw);
}
template<class T> void reg_zone(int k)
{
	RF(AmbientBrightness); RF(AmbientHue); RF(AmbientSaturation); RF(FogColor); RF(TexUPanSpeed); RF(TexVPanSpeed);
	RF(ZoneGravity); RF(ZoneVelocity);
	RB(bReverbZone);
}
template<class T> void reg_level(int k)
{
	RF(Pauser); RF(TimeDilation);
	RB(bAllowFOV); RB(bNoCheating);
}
template<class T> void reg_pri(int k)
{
	RF(Deaths); RF(HasFlag); RF(OldName); RF(PacketLoss); RF(Ping); RF(PlayerID); RF(PlayerLocation); RF(PlayerName);
	RF(PlayerZone); RF(Score); RF(StartTime); RF(TalkTexture); RF(Team); RF(TeamID); RF(TeamName); RF(VoiceType);
	RB(bAdmin); RB(bFeigningDeath); RB(bIsABot); RB(bIsFemale); RB(bIsSpectator); RB(bWaitingPlayer);
}
template<class T> void reg_gri(int k)
{
	RF(AdminEmail); RF(AdminName); RF(ElapsedTime); RF(GameClass); RF(GameEndedComments); RF(GameName);
	RF(MOTDLine1); RF(MOTDLine2); RF(MOTDLine3); RF(MOTDLine4); RF(NumPlayers); RF(RemainingMinute);
	RF(RemainingTime); RF(ServerName); RF(ShortName);
	RB(bStopCountDown); RB(bTeamGame);
}
template<class T> void reg_inv(int k)
{
	RF(Charge); RF(FlashCount); RF(PlayerViewMesh); RF(PlayerViewOffset); RF(PlayerViewScale); RF(ThirdPersonMesh);
	RF(ThirdPersonScale);
	RB(bActivatable); RB(bActive); RB(bIsAnArmor); RB(bSteadyFlash3rd);
}
template<class T> void reg_weapon(int k)
{
	RF(AmmoType);
	RB(bHideWeapon); RB(bLockedOn);
}
template<class T> void reg_ammo(int k)
{
	RF(AmmoAmount);
}
template<class T> void reg_pickup(int k)
{
	RF(NumCopies);
}

void build_registry()
{
	reg_actor<AActor>(K_ACTOR);
	reg_actor<APawn>(K_PAWN); reg_pawn<APawn>(K_PAWN);
	reg_actor<APlayerPawn>(K_PLAYERPAWN); reg_pawn<APlayerPawn>(K_PLAYERPAWN); reg_playerpawn<APlayerPawn>(K_PLAYERPAWN);
	reg_actor<AMover>(K_MOVER); reg_mover<AMover>(K_MOVER);
	reg_actor<AZoneInfo>(K_ZONE); reg_zone<AZoneInfo>(K_ZONE);
	reg_actor<ALevelInfo>(K_LEVEL); reg_zone<ALevelInfo>(K_LEVEL); reg_level<ALevelInfo>(K_LEVEL);
	reg_actor<APlayerReplicationInfo>(K_PRI); reg_pri<APlayerReplicationInfo>(K_PRI);
	reg_actor<AGameReplicationInfo>(K_GRI); reg_gri<AGameReplicationInfo>(K_GRI);
	reg_actor<AInventory>(K_INV); reg_inv<AInventory>(K_INV);
	reg_actor<AWeapon>(K_WEAPON); reg_inv<AWeapon>(K_WEAPON); reg_weapon<AWeapon>(K_WEAPON);
	reg_actor<AAmmo>(K_AMMO); reg_inv<AAmmo>(K_AMMO); reg_pickup<AAmmo>(K_AMMO); reg_ammo<AAmmo>(K_AMMO);
	reg_actor<APickup>(K_PICKUP); reg_inv<APickup>(K_PICKUP); reg_pickup<APickup>(K_PICKUP);
}

// ---- knob grammar -------------------------------------------------------------------------

struct Ctx
{
	int num = 0;
	unsigned native = 0;
	int viewer = 0;   // pointer index: 0 null, 1 t (this), 2 r (recent), 3 o (pawn)
	int old = 2;      // default r
	int reject = 0;
	unsigned vdir[3] = { 0, 0, 0 };
	unsigned vpos[3] = { 0, 0, 0 };
};

std::vector<std::string> split(const std::string& s, char sep)
{
	std::vector<std::string> out;
	size_t i = 0;
	for (;;) {
		size_t j = s.find(sep, i);
		out.push_back(s.substr(i, j == std::string::npos ? std::string::npos : j - i));
		if (j == std::string::npos) break;
		i = j + 1;
	}
	return out;
}

std::vector<std::string> words(const std::string& s)
{
	std::vector<std::string> out;
	size_t i = 0;
	while (i < s.size()) {
		while (i < s.size() && s[i] == ' ') ++i;
		size_t j = i;
		while (j < s.size() && s[j] != ' ') ++j;
		if (j > i) out.push_back(s.substr(i, j - i));
		i = j;
	}
	return out;
}

unsigned hexu(const std::string& s) { return (unsigned)strtoul(s.c_str(), nullptr, 16); }

std::vector<unsigned> hexlist(const std::string& s, size_t n)
{
	std::vector<unsigned> out;
	for (const std::string& p : split(s, ',')) out.push_back(hexu(p));
	if (out.size() != n) fail("hex list arity", s);
	return out;
}

std::vector<long long> declist(const std::string& s, size_t n)
{
	std::vector<long long> out;
	for (const std::string& p : split(s, ',')) out.push_back(strtoll(p.c_str(), nullptr, 10));
	if (out.size() != n) fail("dec list arity", s);
	return out;
}

unsigned fbits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
float bitsf(unsigned u) { float f; memcpy(&f, &u, 4); return f; }

int ptr_index(const std::string& s)
{
	if (s == "null") return 0;
	if (s == "t") return 1;
	if (s == "r") return 2;
	if (s == "o") return 3;
	fail("pointer keyword", s);
}

unsigned char* ptr_by_index(int i)
{
	switch (i) {
	case 0: return nullptr;
	case 1: return g_self;
	case 2: return g_recent;
	case 3: return g_pawn;
	}
	fail("pointer index");
}

const Entry& find_entry(int kind, const std::string& name)
{
	for (const Entry& e : g_reg[kind])
		if (e.name == name) return e;
	fail("unknown field", std::string(kTypeName[kind]) + "." + name);
}

void write_entry(unsigned char* base, const Entry& e, const std::string& v)
{
	if (e.tag == 'x') {
		if (v != "0" && v != "1") fail("bit value", e.name);
		if (v == "1") base[e.off] |= (unsigned char)e.mask;
		else base[e.off] &= (unsigned char)~e.mask;
		return;
	}
	unsigned char* dst = base + e.off;
	memset(dst, 0, e.size);
	switch (e.tag) {
	case 'b':
	case 'w':
	case 'i': {
		if (e.count != 1) fail("scalar array", e.name);
		long long x = strtoll(v.c_str(), nullptr, 10);
		memcpy(dst, &x, e.size);
		return;
	}
	case 'f':
	case 'n': {
		if (v.compare(0, 2, e.tag == 'f' ? "f:" : "n:") != 0) fail("float/name token", e.name);
		unsigned u = hexu(v.substr(2));
		memcpy(dst, &u, 4);
		return;
	}
	case 'v': {
		if (v.compare(0, 2, "v:") != 0 || e.size < 12) fail("vector token", e.name);
		std::vector<unsigned> w = hexlist(v.substr(2), 3);
		memcpy(dst, w.data(), 12);
		return;
	}
	case 'p': {
		if (v.compare(0, 2, "p:") != 0 || e.size < 16) fail("plane token", e.name);
		std::vector<unsigned> w = hexlist(v.substr(2), 4);
		memcpy(dst, w.data(), 16);
		return;
	}
	case 'r': {
		if (v.compare(0, 2, "R:") != 0 || e.size < 12) fail("rotator token", e.name);
		std::vector<long long> d = declist(v.substr(2), 3);
		for (int i = 0; i < 3; ++i) { int32_t x = (int32_t)d[i]; memcpy(dst + 4 * i, &x, 4); }
		return;
	}
	case 'c': {
		if (v.compare(0, 2, "c:") != 0 || e.size < 4) fail("color token", e.name);
		std::vector<long long> d = declist(v.substr(2), 4);
		for (int i = 0; i < 4; ++i) dst[i] = (unsigned char)d[i];
		return;
	}
	case 's': {
		if (v.compare(0, 2, "s:") != 0 || e.size < 16) fail("string token", e.name);
		std::string text = v.substr(2);
		void* data = (void*)kEmptyStr;
		int32_t num = 0, max = 0;
		if (!text.empty()) {
			if (g_strnext >= 64 || text.size() > 62) fail("string pool", e.name);
			TCHAR* slot = g_strpool[g_strnext++];
			for (size_t i = 0; i < text.size(); ++i) slot[i] = (TCHAR)(unsigned char)text[i];
			slot[text.size()] = 0;
			data = (void*)slot;
			num = max = (int32_t)text.size() + 1;
		}
		memcpy(dst, &data, 8);
		memcpy(dst + 8, &num, 4);
		memcpy(dst + 12, &max, 4);
		return;
	}
	case 'o': {
		std::vector<std::string> els = split(v, ',');
		if (els.size() != e.count || e.size != 8 * e.count) fail("pointer array arity", e.name);
		for (size_t i = 0; i < els.size(); ++i) {
			unsigned char* p = ptr_by_index(ptr_index(els[i]));
			memcpy(dst + 8 * i, &p, 8);
		}
		return;
	}
	}
	fail("unknown tag", e.name);
}

// Toggle value: the baseline value with one bit flipped (or null<->non-null for pointers,
// empty<->"x" for strings). Rendered in the same token grammar that write_entry parses.
std::string render_alt(const Entry& e, const unsigned char* base)
{
	const unsigned char* p = base + e.off;
	char buf[256];
	switch (e.tag) {
	case 'b':
	case 'w':
	case 'i': {
		int64_t v = 0;
		memcpy(&v, p, e.size);
		snprintf(buf, sizeof buf, "%lld", (long long)(v ^ 1));
		return buf;
	}
	case 'x': {
		bool on = (p[0] & e.mask) != 0;
		return on ? "0" : "1";
	}
	case 'f':
	case 'n': {
		unsigned u;
		memcpy(&u, p, 4);
		snprintf(buf, sizeof buf, "%s:%08x", e.tag == 'f' ? "f" : "n", u ^ 1u);
		return buf;
	}
	case 'v': {
		unsigned w[3];
		memcpy(w, p, 12);
		snprintf(buf, sizeof buf, "v:%08x,%08x,%08x", w[0] ^ 1u, w[1], w[2]);
		return buf;
	}
	case 'r': {
		int32_t r[3];
		memcpy(r, p, 12);
		snprintf(buf, sizeof buf, "R:%d,%d,%d", (int)(r[0] ^ 1), (int)r[1], (int)r[2]);
		return buf;
	}
	case 'p': {
		unsigned w[4];
		memcpy(w, p, 16);
		snprintf(buf, sizeof buf, "p:%08x,%08x,%08x,%08x", w[0] ^ 1u, w[1], w[2], w[3]);
		return buf;
	}
	case 'c': {
		snprintf(buf, sizeof buf, "c:%u,%u,%u,%u", p[0] ^ 1u, p[1], p[2], p[3]);
		return buf;
	}
	case 'o': {
		std::string out;
		for (size_t i = 0; i < e.count; ++i) {
			void* q;
			memcpy(&q, p + 8 * i, 8);
			if (i) out += ",";
			out += q ? "null" : "o";
		}
		return out;
	}
	case 's': {
		int32_t num;
		memcpy(&num, p + 8, 4);
		return num == 0 ? "s:x" : "s:";
	}
	}
	fail("alt tag", e.name);
}

void apply(int kind, const std::string& tok, Ctx& c)
{
	size_t eq = tok.find('=');
	if (eq == std::string::npos) fail("token without '='", tok);
	std::string key = tok.substr(0, eq);
	std::string val = tok.substr(eq + 1);
	if (key == "num") { c.num = atoi(val.c_str()); return; }
	if (key == "native") { c.native = hexu(val); return; }
	if (key == "viewer") { c.viewer = ptr_index(val); return; }
	if (key == "old") { c.old = ptr_index(val); return; }
	if (key == "reject") { c.reject = atoi(val.c_str()); return; }
	if (key == "vdir" || key == "vpos") {
		if (val.compare(0, 2, "v:") != 0) fail("vector knob", key);
		std::vector<unsigned> w = hexlist(val.substr(2), 3);
		unsigned* dst = key == "vdir" ? c.vdir : c.vpos;
		for (int i = 0; i < 3; ++i) dst[i] = w[i];
		return;
	}
	std::string prefix, field = key;
	size_t dot = key.find('.');
	if (dot != std::string::npos) { prefix = key.substr(0, dot); field = key.substr(dot + 1); }
	if (prefix.empty()) {
		write_entry(g_self, find_entry(kind, field), val);
		write_entry(g_recent, find_entry(kind, field), val);
	} else if (prefix == "t") {
		write_entry(g_self, find_entry(kind, field), val);
	} else if (prefix == "r") {
		write_entry(g_recent, find_entry(kind, field), val);
	} else if (prefix == "d") {
		write_entry(g_defobj, find_entry(kind, field), val);
	} else if (prefix == "o") {
		write_entry(g_pawn, find_entry(K_PAWN, field), val);
	} else {
		fail("target prefix", tok);
	}
}

void build_state(int kind, const std::vector<std::string>& toks, Ctx& c)
{
	memset(g_self, 0, kObjBytes);
	memset(g_recent, 0, kObjBytes);
	memset(g_pawn, 0, kObjBytes);
	memset(g_defobj, 0, kObjBytes);
	g_strnext = 0;
	reinterpret_cast<UObject*>(g_self)->Class = g_class[kind];
	reinterpret_cast<UObject*>(g_recent)->Class = g_class[kind];
	reinterpret_cast<UObject*>(g_pawn)->Class = g_class[K_PAWN];
	for (int k = 0; k < K_COUNT; ++k) {
		g_class[k]->Defaults.Data = (BYTE*)g_defobj;
		g_class[k]->Defaults.ArrayNum = (INT)sizeof(AActor);
		g_class[k]->Defaults.ArrayMax = (INT)sizeof(AActor);
	}
	for (const std::string& t : toks) apply(kind, t, c);
	for (int k = 0; k < K_COUNT; ++k)
		g_class[k]->ClassFlags = ((c.native >> k) & 1u) ? (DWORD)CLASS_NativeReplication : 0u;
	g_reject_obj = c.reject ? reinterpret_cast<UObject*>(g_pawn) : reinterpret_cast<UObject*>((uintptr_t)1);
}

// ---- engine boundary mocks (defined by the harness; the TU calls them) --------------------

class MockMap : public UPackageMap
{
public:
	UBOOL CanSerializeObject(UObject* Object) { return Object != g_reject_obj; }
};
alignas(16) unsigned char g_map_buf[sizeof(MockMap)];
UPackageMap* g_map = nullptr;

void run_rep(int kind, const Ctx& c)
{
	INT list[256];
	memset(list, 0, sizeof list);
	FPropertyRetirement retire;
	BYTE* recent = reinterpret_cast<BYTE*>(g_recent);
	INT* end = nullptr;
	switch (kind) {
	case K_ACTOR: end = reinterpret_cast<AActor*>(g_self)->AActor::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_PAWN: end = reinterpret_cast<APawn*>(g_self)->APawn::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_PLAYERPAWN: end = reinterpret_cast<APlayerPawn*>(g_self)->APlayerPawn::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_MOVER: end = reinterpret_cast<AMover*>(g_self)->AMover::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_ZONE: end = reinterpret_cast<AZoneInfo*>(g_self)->AZoneInfo::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_LEVEL: end = reinterpret_cast<ALevelInfo*>(g_self)->ALevelInfo::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_PRI: end = reinterpret_cast<APlayerReplicationInfo*>(g_self)->APlayerReplicationInfo::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_GRI: end = reinterpret_cast<AGameReplicationInfo*>(g_self)->AGameReplicationInfo::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_INV: end = reinterpret_cast<AInventory*>(g_self)->AInventory::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_WEAPON: end = reinterpret_cast<AWeapon*>(g_self)->AWeapon::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_AMMO: end = reinterpret_cast<AAmmo*>(g_self)->AAmmo::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	case K_PICKUP: end = reinterpret_cast<APickup*>(g_self)->APickup::GetOptimizedRepList(recent, &retire, list, g_map, c.num); break;
	default: fail("rep kind");
	}
	ptrdiff_t n = end - list;
	if (n < 0 || n > 256) fail("rep count out of range");
	printf("res n=%td idx=", n);
	for (ptrdiff_t i = 0; i < n; ++i) printf(i ? ",%d" : "%d", list[i]);
	printf("\n");
}

void run_freq(int kind, const Ctx& c)
{
	FVector dir(bitsf(c.vdir[0]), bitsf(c.vdir[1]), bitsf(c.vdir[2]));
	FVector pos(bitsf(c.vpos[0]), bitsf(c.vpos[1]), bitsf(c.vpos[2]));
	AActor* viewer = reinterpret_cast<AActor*>(ptr_by_index(c.viewer));
	FLOAT r = 0.0f;
	if (kind == K_ACTOR) r = reinterpret_cast<AActor*>(g_self)->AActor::UpdateFrequency(viewer, dir, pos);
	else if (kind == K_INV) r = reinterpret_cast<AInventory*>(g_self)->AInventory::UpdateFrequency(viewer, dir, pos);
	else fail("freq kind");
	printf("res f=%08x\n", fbits(r));
}

void run_script(const Ctx&)
{
	UBOOL b = reinterpret_cast<AInventory*>(g_self)->AInventory::ShouldDoScriptReplication();
	printf("res b=%d\n", b ? 1 : 0);
}

void run_novars(const Ctx& c)
{
	AActor* old = reinterpret_cast<AActor*>(ptr_by_index(c.old));
	UBOOL b = reinterpret_cast<AInventory*>(g_self)->AInventory::NoVariablesToReplicate(old);
	printf("res b=%d\n", b ? 1 : 0);
}

struct Fam { const char* name; int kind; int mode; };

const Fam kFams[] = {
	{ "actor_rep", K_ACTOR, M_REP },
	{ "actor_freq", K_ACTOR, M_FREQ },
	{ "pawn_rep", K_PAWN, M_REP },
	{ "player_pawn_rep", K_PLAYERPAWN, M_REP },
	{ "mover_rep", K_MOVER, M_REP },
	{ "zone_rep", K_ZONE, M_REP },
	{ "level_rep", K_LEVEL, M_REP },
	{ "pri_rep", K_PRI, M_REP },
	{ "gri_rep", K_GRI, M_REP },
	{ "inv_rep", K_INV, M_REP },
	{ "inv_freq", K_INV, M_FREQ },
	{ "inv_script", K_INV, M_SCRIPT },
	{ "inv_novars", K_INV, M_NOVARS },
	{ "weapon_rep", K_WEAPON, M_REP },
	{ "ammo_rep", K_AMMO, M_REP },
	{ "pickup_rep", K_PICKUP, M_REP },
};

void emit(const Fam& f, int& idx, const std::vector<std::string>& toks)
{
	Ctx c;
	build_state(f.kind, toks, c);
	printf("case %s %d", f.name, idx++);
	for (const std::string& t : toks) printf(" %s", t.c_str());
	printf("\n");
	switch (f.mode) {
	case M_REP: run_rep(f.kind, c); break;
	case M_FREQ: run_freq(f.kind, c); break;
	case M_SCRIPT: run_script(c); break;
	case M_NOVARS: run_novars(c); break;
	}
}

// Case-only string alt: the baseline text with ASCII letter case swapped. Empty when the
// value has fewer than two code units (num <= 1) or contains no letters.
std::string case_flipped(const Entry& e, const unsigned char* base)
{
	const unsigned char* p = base + e.off;
	const TCHAR* d;
	int32_t num;
	memcpy(&d, p, sizeof d);
	memcpy(&num, p + 8, 4);
	if (num <= 1) return "";
	std::string out = "s:";
	bool changed = false;
	for (int32_t i = 0; i < num - 1; ++i) {
		int ch = (int)d[i];
		if (ch < 0 || ch > 127) fail("non-ASCII string", e.name);
		if (ch >= 'a' && ch <= 'z') { ch -= 32; changed = true; }
		else if (ch >= 'A' && ch <= 'Z') { ch += 32; changed = true; }
		out += (char)ch;
	}
	return changed ? out : "";
}

// One case per toggle-eligible field of the family: the baseline plus `t.<field>=<alt>`.
// String fields also get a case-only alt (must not replicate). A final case applies every
// toggle at once, so DOREP emission order across fields is checked in one record.
void emit_toggles(const Fam& f, int& idx, const std::vector<std::string>& base)
{
	emit(f, idx, base);
	std::vector<std::string> all = base;
	for (const Entry& e : g_reg[f.kind]) {
		if (!e.toggle) continue;
		Ctx c;
		build_state(f.kind, base, c);
		std::string alt = "t." + e.name + "=" + render_alt(e, g_self);
		std::string flipped = e.tag == 's' ? case_flipped(e, g_self) : "";
		std::vector<std::string> toks = base;
		toks.push_back(alt);
		emit(f, idx, toks);
		all.push_back(alt);
		if (!flipped.empty()) {
			std::vector<std::string> ct = base;
			ct.push_back("t." + e.name + "=" + flipped);
			emit(f, idx, ct);
		}
	}
	if (all.size() > base.size()) emit(f, idx, all);
}

std::string fkv(float x)
{
	char b[24];
	snprintf(b, sizeof b, "f:%08x", fbits(x));
	return b;
}

std::string vkv(float x, float y, float z)
{
	char b[64];
	snprintf(b, sizeof b, "v:%08x,%08x,%08x", fbits(x), fbits(y), fbits(z));
	return b;
}

// Family-specific extras appended to every baseline of that family.
std::string extras(int kind)
{
	switch (kind) {
	case K_PAWN: return " bIsPlayer=1 bCanFly=1 bBehindView=0 Weapon=o";
	case K_PLAYERPAWN: return " bIsPlayer=1 bCanFly=1 bBehindView=0 Weapon=o bCheatsEnabled=1 bFixedCamera=0 bNeverAutoSwitch=1 bReadyToPlay=1 ViewTarget=o";
	case K_MOVER: return " SimInterpolate=" + vkv(1.5f, 0.f, 0.f) + " SimOldRotYaw=5";
	case K_ZONE: return " bReverbZone=1 TexUPanSpeed=" + fkv(0.25f);
	case K_LEVEL: return " bReverbZone=1 bAllowFOV=1 bNoCheating=0 TimeDilation=" + fkv(1.0f) + " Pauser=s:pause";
	case K_PRI: return " bAdmin=1 bIsABot=0 bWaitingPlayer=1 PlayerName=s:Alpha Team=1 Score=" + fkv(3.0f) + " Ping=50";
	case K_GRI: return " bTeamGame=1 NumPlayers=3 ServerName=s:srv GameName=s:Game ShortName=s:G";
	case K_INV: return " bActivatable=1 bActive=0 bIsAnArmor=1 bSteadyFlash3rd=0 Charge=50 o.Weapon=t";
	case K_WEAPON: return " bActivatable=1 bActive=0 bIsAnArmor=1 bSteadyFlash3rd=0 Charge=50 o.Weapon=t bHideWeapon=0 bLockedOn=1 AmmoType=o";
	case K_AMMO: return " AmmoAmount=10";
	case K_PICKUP: return " NumCopies=2";
	}
	return "";
}

const char* const kRepA = "native=FFF num=3 Role=4 RemoteRole=2 DrawType=2 LightType=0 Physics=0 LifeSpan=f:00000000 NetUpdateFrequency=f:00000000 Owner=null Instigator=null Inventory=null";
const char* const kRepB = "native=FFF num=5 Role=4 RemoteRole=2 DrawType=2 LightType=3 Physics=5 bNetTemporary=1 bNetOptional=1 bNetInitial=1 bNetOwner=1 bAlwaysRelevant=1 LifeSpan=f:40a00000 NetUpdateFrequency=f:40000000 Owner=o Instigator=o Inventory=o";
const char* const kRepC = "native=0 num=7 Role=2 RemoteRole=4 DrawType=3 bNetTemporary=1 bNetOptional=1 LifeSpan=f:41180000 NetUpdateFrequency=f:00000000";

std::vector<std::vector<std::string>> baselines(const Fam& f)
{
	std::vector<std::vector<std::string>> out;
	std::string ext = extras(f.kind);
	if (f.mode == M_FREQ) {
		std::string a = "bNetTemporary=0 LifeSpan=" + fkv(5) + " NetUpdateFrequency=" + fkv(20) + " d.LifeSpan=" + fkv(10) + " viewer=null";
		std::string b = "bNetTemporary=1 LifeSpan=" + fkv(5) + " NetUpdateFrequency=" + fkv(20) + " d.LifeSpan=" + fkv(10) + " Location=" + vkv(2000, 0, 0) + " viewer=t";
		std::string c = "bNetTemporary=1 bNetOptional=1 LifeSpan=" + fkv(9.9f) + " NetUpdateFrequency=" + fkv(20) + " d.LifeSpan=" + fkv(10) + " viewer=o";
		out.push_back(words(a + ext));
		out.push_back(words(b + ext));
		out.push_back(words(c + ext));
	} else if (f.mode == M_NOVARS) {
		out.push_back(words(std::string(kRepA) + ext + " old=r"));
		out.push_back(words(std::string(kRepB) + ext + " old=t"));
		out.push_back(words(std::string(kRepC) + ext + " old=o"));
	} else {
		out.push_back(words(std::string(kRepA) + ext));
		out.push_back(words(std::string(kRepB) + ext));
		out.push_back(words(std::string(kRepC) + ext));
	}
	return out;
}

void sweeps(const Fam& f, int& idx)
{
	const std::string sbase = "bNetTemporary=1 LifeSpan=" + fkv(5) + " NetUpdateFrequency=" + fkv(20) + " d.LifeSpan=" + fkv(10);
	// A: Location.x stepped by one ulp around 1000 (dist^2 crosses 1000000.f).
	float xs[25];
	xs[12] = 1000.0f;
	for (int i = 13; i < 25; ++i) xs[i] = nextafterf(xs[i - 1], INFINITY);
	for (int i = 11; i >= 0; --i) xs[i] = nextafterf(xs[i + 1], -INFINITY);
	for (int i = 0; i < 25; ++i) emit(f, idx, words(sbase + " Location=" + vkv(xs[i], 0, 0)));
	// B: same stepping with a non-zero y/z and a base just below 1000.
	float ys[25];
	ys[12] = 999.99984f;
	for (int i = 13; i < 25; ++i) ys[i] = nextafterf(ys[i - 1], INFINITY);
	for (int i = 11; i >= 0; --i) ys[i] = nextafterf(ys[i + 1], -INFINITY);
	for (int i = 0; i < 25; ++i) emit(f, idx, words(sbase + " Location=" + vkv(ys[i], 0.5f, 0.25f)));
	// C: view direction ⊥ offset to a sign flip of (Location-ViewPos)·ViewDir.
	for (int k = -8; k <= 8; ++k) {
		float a = (float)k * 1e-7f;
		emit(f, idx, words(sbase + " vdir=" + vkv(0.6f, 0.8f, 0) + " Location=" + vkv(a, -0.75f * a, 0)));
	}
}

void print_layout()
{
	for (int k = 0; k < K_COUNT; ++k)
		printf("sizeof %s %zu\n", kTypeName[k], kSize[k]);
	for (int k = 0; k < K_COUNT; ++k)
		for (const Entry& e : g_reg[k])
			printf("layout %s %s %c %zu %zu %zu %u\n", kTypeName[k], e.name.c_str(), e.tag, e.off, e.size, e.count, e.mask);
}

} // namespace

// ---- engine boundary definitions -----------------------------------------------------------

// The TU defines storage and accessors for these classes only in other translation units.
UClass* APawn::PrivateStaticClass = NULL;
UClass* APlayerPawn::PrivateStaticClass = NULL;
UClass* AMover::PrivateStaticClass = NULL;
UClass* UProperty::PrivateStaticClass = NULL;

UObject::UObject() {}

UObject* UObject::StaticFindObjectChecked(UClass* Class, UObject* InOuter, const TCHAR* Name, UBOOL ExactClass)
{
	int kind = -1;
	for (int k = 0; k < K_COUNT; ++k)
		if (reinterpret_cast<UObject*>(g_class[k]) == InOuter) kind = k;
	if (kind < 0) fail("FindObjectChecked outer is not an owned class");
	DWORD h = 2166136261u;
	for (const TCHAR* p = Name; *p; ++p) { h ^= (DWORD)*p; h *= 16777619u; }
	UProperty* prop = (UProperty*)calloc(1, sizeof(UProperty));
	int slot = kind * 4096 + (int)(h % 4055u) + 1;
	static std::string owner[K_COUNT * 4096 + 1];
	std::string key;
	for (const TCHAR* p = Name; *p; ++p) key += (char)*p;
	if (owner[slot].empty()) owner[slot] = key;
	else if (owner[slot] != key) fail("RepIndex collision", key + " vs " + owner[slot]);
	prop->RepIndex = (_WORD)slot;
	return prop;
}

INT appStricmp(const TCHAR* A, const TCHAR* B)
{
	for (;; ++A, ++B) {
		TCHAR a = *A, b = *B;
		if (a >= L'A' && a <= L'Z') a += 32;
		if (b >= L'A' && b <= L'Z') b += 32;
		if (a != b) return a < b ? -1 : 1;
		if (!a) return 0;
	}
}

void appMemcpy(void* Dest, const void* Src, INT Count)
{
	memcpy(Dest, Src, (size_t)Count);
}

int main()
{
	for (int k = 0; k < K_COUNT; ++k) g_class[k] = reinterpret_cast<UClass*>(g_cls_buf[k]);
	g_prop_class = reinterpret_cast<UClass*>(g_prop_cls_buf);
	for (int k = 0; k < K_COUNT; ++k)
		g_class[k]->SuperField = kParent[k] >= 0 ? (UField*)g_class[kParent[k]] : nullptr;

	AActor::PrivateStaticClass = g_class[K_ACTOR];
	APawn::PrivateStaticClass = g_class[K_PAWN];
	APlayerPawn::PrivateStaticClass = g_class[K_PLAYERPAWN];
	AMover::PrivateStaticClass = g_class[K_MOVER];
	AZoneInfo::PrivateStaticClass = g_class[K_ZONE];
	ALevelInfo::PrivateStaticClass = g_class[K_LEVEL];
	APlayerReplicationInfo::PrivateStaticClass = g_class[K_PRI];
	AGameReplicationInfo::PrivateStaticClass = g_class[K_GRI];
	AInventory::PrivateStaticClass = g_class[K_INV];
	AWeapon::PrivateStaticClass = g_class[K_WEAPON];
	AAmmo::PrivateStaticClass = g_class[K_AMMO];
	APickup::PrivateStaticClass = g_class[K_PICKUP];
	UProperty::PrivateStaticClass = g_prop_class;

	g_map = ::new (static_cast<void*>(g_map_buf)) MockMap();
	build_registry();
	print_layout();

	for (const Fam& f : kFams) {
		int idx = 0;
		for (const std::vector<std::string>& bt : baselines(f)) emit_toggles(f, idx, bt);
		if (f.kind == K_ACTOR && f.mode == M_FREQ) sweeps(f, idx);
	}
	return 0;
}

// Original-object UnLevel fixture harness. gen_level.sh compiles the pinned
// UnLevel.cpp (7ddb1d3) with its own flags and links it with this file. The
// ULevel object lives in zeroed memory, so every virtual entry point the oracle
// reaches is either called qualified or routed through a hook defined here.
// Archive, file manager, actor and log behaviour is driven by the case script.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#define private public
#define protected public
#include "EnginePrivate.h"
#include "UnNet.h"
#undef private
#undef protected

// Core globals referenced by the original object.
UBOOL GIsRunning = 1;
UBOOL GIsClient = 1;
UBOOL GIsEditor = 0;
UBOOL GIsRequestingExit = 0;
USystem* GSys = NULL;
FExec* GExec = NULL;

class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { (void)Tag; return malloc( Count ? Count : 1 ); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag ) { (void)Tag; return realloc( Original, Count ? Count : 1 ); }
	void Free( void* Original ) { free( Original ); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc g_harness_malloc;
FMalloc* GMalloc = &g_harness_malloc;

static std::string wide_to_utf8( const TCHAR* s )
{
	std::string out;
	for( ; s && *s; ++s )
	{
		unsigned c = (unsigned)*s;
		if( c < 128 ) out += (char)c;
		else out += '?';
	}
	return out;
}

// Records debugf text. Log output is recorded here, not in the sink itself.
struct EngSink : FOutputDevice
{
	void Serialize( const TCHAR* S, EName E )
	{
		char cat[32];
		if( E == NAME_DevNet ) snprintf( cat, sizeof(cat), "dev_net" );
		else if( E == NAME_NetComeGo ) snprintf( cat, sizeof(cat), "net_come_go" );
		else if( E == NAME_Log ) snprintf( cat, sizeof(cat), "log" );
		else if( E == NAME_Error ) snprintf( cat, sizeof(cat), "error" );
		else if( E == NAME_Critical ) snprintf( cat, sizeof(cat), "critical" );
		else snprintf( cat, sizeof(cat), "e%d", (int)E );
		std::string t = wide_to_utf8( S );
		// One record per line: escape the characters that would split or add fields.
		std::string esc;
		for( char c : t )
		{
			if( c == '\n' ) esc += "\\n";
			else if( c == '\r' ) esc += "\\r";
			else if( c == '\t' ) esc += "\\t";
			else esc += c;
		}
		printf( "debugf\t%s\t%s\n", cat, esc.c_str() );
	}
};
static EngSink g_sink;
FOutputDevice* GLog = &g_sink;
// Exceptions carry appErrorf / check failures back to the scenario driver.
struct HarnessAbort { int kind; };
// Error device: appErrorf expands to GError->Logf, and check() failures route there too. Logs, then unwinds to the driver.
struct EngErrorDevice : FOutputDeviceError
{
	void Serialize( const TCHAR* S, EName E )
	{
		(void)E;
		std::string t = wide_to_utf8( S );
		printf( "error\t%s\n", t.c_str() );
		throw HarnessAbort{ 1 };
	}
	void HandleError() { throw HarnessAbort{ 2 }; }
};
static EngErrorDevice g_err;
FOutputDeviceError* GError = &g_err;
// Null device: discards all output (oracle GNull is NullOutError in Core.cpp, which is not linked here).
struct EngNullDevice : FOutputDevice
{
	void Serialize( const TCHAR* S, EName E ) { (void)S; (void)E; }
};
static EngNullDevice g_null;
FOutputDevice* GNull = &g_null;


static std::vector<unsigned char> g_saved_bytes;
struct HArchive;
static HArchive* g_last_writer = NULL;
static bool g_fail_writer = false;
// Scripted in-memory archive. Writers append to Buffer; readers consume it.
struct HArchive : FArchive
{
	std::vector<unsigned char> Buffer;
	INT Pos;
	bool Writing;
	bool Error;
	HArchive( bool bWriting ) : Pos(0), Writing(bWriting), Error(false)
	{
		ArIsLoading = !bWriting;
		ArIsSaving = bWriting;
		ArIsPersistent = 1;
	}
	void Serialize( void* V, INT Length )
	{
		printf( "archive_serialize_bytes\t%d\t%s\n", (int)Length, Writing ? "W" : "R" );
		if( Writing )
		{
			const unsigned char* p = (const unsigned char*)V;
			if( Pos + Length > (INT)Buffer.size() ) Buffer.resize( Pos + Length );
			memcpy( &Buffer[Pos], p, Length );
			Pos += Length;
			return;
		}
		if( Pos + Length > (INT)Buffer.size() )
		{
			Error = true;
			memset( V, 0, Length );
			return;
		}
		memcpy( V, &Buffer[Pos], Length );
		Pos += Length;
	}
	INT Tell() { return Pos; }
	INT TotalSize() { return (INT)Buffer.size(); }
	void Seek( INT InPos ) { printf( "archive_seek\t%d\n", (int)InPos ); Pos = InPos; }
	UBOOL AtEnd() { return Pos >= (INT)Buffer.size(); }
	UBOOL GetError() { return Error; }
	~HArchive()
	{
		if( Writing )
		{
			g_saved_bytes = Buffer;
			printf( "archive_close\t%d\n", (int)Buffer.size() );
		}
		if( g_last_writer == this ) g_last_writer = NULL;
	}
};

static bool g_fail_reader = false;
static std::string g_writer_path;
static std::string g_reader_path;
static INT g_file_size = -1;
static std::vector<unsigned char> g_read_input;

class HarnessFileManager : public FFileManager
{
public:
	FArchive* CreateFileReader( const TCHAR* Filename, DWORD Flags, FOutputDevice* Error )
	{
		(void)Flags; (void)Error;
		g_reader_path = wide_to_utf8( Filename );
		printf( "archive_reader_create\t%s\n", g_reader_path.c_str() );
		if( g_fail_reader ) return NULL;
		HArchive* Ar = new HArchive( false );
		Ar->Buffer = g_read_input;
		return Ar;
	}
	FArchive* CreateFileWriter( const TCHAR* Filename, DWORD Flags, FOutputDevice* Error )
	{
		(void)Flags; (void)Error;
		g_writer_path = wide_to_utf8( Filename );
		printf( "archive_writer_create\t%s\n", g_writer_path.c_str() );
		if( g_fail_writer ) return NULL;
		g_last_writer = new HArchive( true );
		return g_last_writer;
	}
	INT FileSize( const TCHAR* Filename )
	{
		printf( "file_size\t%s\t%d\n", wide_to_utf8( Filename ).c_str(), (int)g_file_size );
		return g_file_size;
	}
	SQWORD GetGlobalTime( const TCHAR* Filename )
	{
		printf( "global_time\t%s\n", wide_to_utf8( Filename ).c_str() );
		return 0;
	}
	UBOOL Delete( const TCHAR* Filename, UBOOL RequireExists, UBOOL EvenReadOnly ) { (void)Filename; (void)RequireExists; (void)EvenReadOnly; return 0; }
	UBOOL Copy( const TCHAR* Dest, const TCHAR* Src, UBOOL Replace, UBOOL EvenIfReadOnly, UBOOL Attributes, void (*Progress)(FLOAT) ) { (void)Dest; (void)Src; (void)Replace; (void)EvenIfReadOnly; (void)Attributes; (void)Progress; return 0; }
	UBOOL Move( const TCHAR* Dest, const TCHAR* Src, UBOOL Replace, UBOOL EvenIfReadOnly, UBOOL Attributes ) { (void)Dest; (void)Src; (void)Replace; (void)EvenIfReadOnly; (void)Attributes; return 0; }
	UBOOL SetGlobalTime( const TCHAR* Filename ) { (void)Filename; return 0; }
	UBOOL MakeDirectory( const TCHAR* Path, UBOOL Tree ) { (void)Path; (void)Tree; return 0; }
	UBOOL DeleteDirectory( const TCHAR* Path, UBOOL RequireExists, UBOOL Tree ) { (void)Path; (void)RequireExists; (void)Tree; return 0; }
	TArray<FString> FindFiles( const TCHAR* Filename, UBOOL Files, UBOOL Directories ) { (void)Filename; (void)Files; (void)Directories; return TArray<FString>(); }
	UBOOL SetDefaultDirectory( const TCHAR* Filename ) { (void)Filename; return 0; }
	FString GetDefaultDirectory() { return FString(); }
};
static HarnessFileManager g_file_manager;

// Level, system and actor storage. Zeroed memory; virtuals are routed by hooks.
// Actor slots fit the largest actor subclass a scenario stores (pawn and camera fields exceed AActor).
enum { HARNESS_ACTOR_SLOT = sizeof(ACamera) > sizeof(APlayerPawn) ? sizeof(ACamera) : sizeof(APlayerPawn) };
alignas(16) static char g_level_mem[sizeof(ULevel)];
alignas(16) static char g_sys_mem[sizeof(USystem)];
alignas(16) static char g_actor_mem[8][HARNESS_ACTOR_SLOT];
// Fake class identity for actors: TObjectIterator<AActor> needs a non-NULL class, and IsA matches by pointer identity. Never dereferenced.
alignas(16) static char g_actor_class_mem[sizeof(UClass)];
static UClass* harness_actor_class() { return (UClass*)(void*)g_actor_class_mem; }
static ULevel* const g_level = (ULevel*)g_level_mem;
static USystem* const g_sys = (USystem*)g_sys_mem;
static const char* g_actor_names[8] = { "Pawn", "Trigger", "Mover", "Light", "Decal", "Note", "Gate", "Lamp" };
// Core hook state: model/level-info/brush storage and knobs for virtual/external hooks.
alignas(16) static char g_model_mem[sizeof(UModel)];
alignas(16) static char g_levelinfo_mem[sizeof(ALevelInfo)];
alignas(16) static char g_brush_mem[sizeof(ABrush)];
alignas(16) static char g_obj_pool[4][sizeof(UModel)];
static int g_obj_pool_next = 0;
static UBOOL g_fast_line_clear = 1;
static UBOOL g_single_line_clear = 1;
static UBOOL g_appsys_time_set = 0;
static AActor* g_trace_hit = NULL;
static INT g_trace_hit_item = 0;
// Trace knobs: zone the IsZoneVisible hook reports visible (-1 = none); hit at trace start so the loop repeats.
static INT g_visible_zone = -1;
static UBOOL g_trace_hit_at_start = 0;
// Engine, client, viewport, game and package-map storage for the view and net scenarios.
alignas(16) static char g_engine_mem[sizeof(UEngine)];
alignas(16) static char g_client_mem[sizeof(UClient)];
alignas(16) static char g_viewport_mem[sizeof(UViewport)];
alignas(16) static char g_game_mem[sizeof(AGameInfo)];
alignas(16) static char g_game_engine_mem[sizeof(UGameEngine)];
// Non-null sentinel for UObject::_Linker: Listen only tests it, never dereferences it, and ULinkerLoad is incomplete here.
alignas(16) static char g_linker_mem[sizeof(UObject)];
alignas(16) static char g_pkgmap_mem[sizeof(UPackageMap)];

static void begin( const char* name, const char* knobs = "" )
{
	static std::string last_family;
	static int case_idx = 0;
	const char* dot = strchr( name, '.' );
	std::string fam = dot ? std::string( name, dot - name ) : std::string( name );
	if( fam != last_family ) { last_family = fam; case_idx = 0; }
	printf( "case\t%s\t%d\t%s\n", fam.c_str(), case_idx++, knobs[0] ? knobs : "-" );
	printf( "scenario\t%s\n", name );
	g_obj_pool_next = 0;
	g_fast_line_clear = 1;
	g_single_line_clear = 1;
	g_appsys_time_set = 0;
	UObject::GObjObjects.Empty();
	g_fail_writer = false;
	g_fail_reader = false;
	g_file_size = -1;
	g_read_input.clear();
	GIsEditor = 0;
	memset( g_level_mem, 0, sizeof(g_level_mem) );
	memset( g_sys_mem, 0, sizeof(g_sys_mem) );
	memset( g_actor_mem, 0, sizeof(g_actor_mem) );
	memset( g_model_mem, 0, sizeof(g_model_mem) );
	memset( g_levelinfo_mem, 0, sizeof(g_levelinfo_mem) );
	memset( g_brush_mem, 0, sizeof(g_brush_mem) );
	memset( g_engine_mem, 0, sizeof(g_engine_mem) );
	memset( g_client_mem, 0, sizeof(g_client_mem) );
	memset( g_viewport_mem, 0, sizeof(g_viewport_mem) );
	memset( g_game_mem, 0, sizeof(g_game_mem) );
	memset( g_pkgmap_mem, 0, sizeof(g_pkgmap_mem) );
	g_trace_hit = NULL;
	g_trace_hit_item = 0;
	g_visible_zone = -1;
	g_trace_hit_at_start = 0;
	GSys = g_sys;
	new (&g_sys->SaveSlotPath) FString( TEXT("Save\\") );
	new (&g_sys->MapPath) FString( TEXT("Maps\\") );
	FURL::DefaultMapExt = TEXT("unr");
	FURL::DefaultSaveExt = TEXT("usa");
}

static AActor* make_actor( int i, bool persistent )
{
	char nm[32];
	snprintf( nm, sizeof(nm), "Actor%d", i );
	TCHAR wn[32];
	for( int k = 0; ; ++k ) { wn[k] = nm[k]; if( !nm[k] ) break; }
	AActor* a = (AActor*)g_actor_mem[i];
	a->Class = harness_actor_class();
	a->Name = FName( wn );
	a->bPersistent = persistent ? 1 : 0;
	return a;
}

// Actor persistence payload: index-derived, so the bytes are deterministic.
void AActor::SerializePersistence( FArchive& Ar )
{
	int idx = (int)(((char*)this - (char*)g_actor_mem) / (long)HARNESS_ACTOR_SLOT);
	printf( "actor_serialize_persistence\t%d\n", idx );
	INT v = 0x1000 + idx;
	Ar.Serialize( &v, sizeof(v) );
}

// Harness name table: FName is a link stub in this build; intern names here.
TArray<FNameEntry*> FName::Names;
TArray<INT> FName::Available;
FNameEntry* FName::NameHash[4096];
UBOOL FName::Initialized = 0;

FName::FName( const TCHAR* InName, EFindName FindType )
{
	(void)FindType;
	if( Names.Num() == 0 ) Names.AddItem( NULL );
	for( INT i = 1; i < Names.Num(); ++i )
		if( wcscmp( Names(i)->Name, InName ) == 0 ) { Index = (NAME_INDEX)i; return; }
	size_t n = wcslen( InName );
	FNameEntry* E = (FNameEntry*)calloc( 1, sizeof(FNameEntry) + (n + 1) * sizeof(TCHAR) );
	wcscpy( E->Name, InName );
	E->Flags = 0;
	E->HashNext = NULL;
	Names.AddItem( E );
	E->Index = (NAME_INDEX)(Names.Num() - 1);
	Index = E->Index;
}

// Port-side hook for the virtual ULevel::DestroyActor (see gen_level.sh rewrite).
UBOOL harness_destroy_actor( ULevel* Level, AActor* Actor )
{
	int idx = (int)(((char*)Actor - (char*)g_actor_mem) / (long)HARNESS_ACTOR_SLOT);
	printf( "level_destroy_actor\t%d\n", idx );
	for( INT i = 0; i < Level->Actors.Num(); ++i )
		if( Level->Actors(i) == Actor )
			Level->Actors(i) = NULL;
	return 1;
}

// Core hooks: definitions for externals and virtual targets reached by core exports.
static int actor_index( const AActor* a )
{
	if( !a ) return -1;
	if( (const char*)a == g_levelinfo_mem ) return 100;
	if( (const char*)a == g_brush_mem ) return 101;
	for( int i = 0; i < 8; ++i )
		if( (const char*)a == g_actor_mem[i] ) return i;
	return -2;
}

// Harness collision hash: the port-side FCollisionHashBase for GNewCollisionHash.
class HarnessHash : public FCollisionHashBase
{
public:
	virtual ~HarnessHash() { printf( "collision_hash_destroy\n" ); }
	virtual void Tick() {}
	virtual void AddActor( AActor* Actor ) { printf( "collision_hash_add_actor\t%d\n", actor_index( Actor ) ); }
	virtual void RemoveActor( AActor* Actor, bool bThorough ) { printf( "collision_hash_remove_actor\t%d\t%d\n", actor_index( Actor ), bThorough ? 1 : 0 ); }
	virtual FCheckResult* ActorLineCheck( FMemStack&, FVector, FVector, FVector, BYTE ) { return NULL; }
	virtual FCheckResult* ActorPointCheck( FMemStack&, FVector, FVector, DWORD ) { return NULL; }
	virtual FCheckResult* ActorRadiusCheck( FMemStack&, FVector, FLOAT, DWORD ) { return NULL; }
	virtual FCheckResult* ActorEncroachmentCheck( FMemStack&, AActor*, FVector, FRotator, DWORD ) { return NULL; }
	virtual void CheckActorNotReferenced( AActor* ) {}
};

FCollisionHashBase* GNewCollisionHash()
{
	printf( "collision_hash_new\n" );
	return new HarnessHash;
}

void UObject::Destroy() { printf( "uobject_destroy\n" ); }
void UObject::Modify() { printf( "uobject_modify\n" ); }
void UObject::PostLoad() { printf( "uobject_post_load\n" ); }
void UObject::Serialize( FArchive& Ar ) { (void)Ar; printf( "uobject_serialize\n" ); }
// URL stream format is outside the level boundary under test: record the call only.
FArchive& operator<<( FArchive& Ar, FURL& URL ) { (void)URL; printf( "url_serialize\t%d\n", (int)Ar.IsLoading() ); return Ar; }
// FURL string parsing is outside the level boundary; the level scenarios set URL fields explicitly.
FURL::FURL( const TCHAR* Filename ) { (void)Filename; }
// Base-object construction: fields live in the scenario's zeroed memory block (see begin()).
UObject::UObject() {}
// Class registration is outside the level boundary: each class gets a distinct sentinel identity; the allocator hook ignores the class.
// Per-class sentinel identity (never dereferenced): NULL would make IsA(NULL) true for every object.
#define HARNESS_NULL_CLASS(C) alignas(16) static char g_cls_##C[sizeof(UClass)]; UClass* C::GetPrivateStaticClass##C( const TCHAR* Package ) { (void)Package; return (UClass*)(void*)g_cls_##C; } void C::InitializePrivateStaticClass##C() {}
HARNESS_NULL_CLASS( UObject )
HARNESS_NULL_CLASS( UModel )
HARNESS_NULL_CLASS( ALevelInfo )
UClass* AActor::GetPrivateStaticClassAActor( const TCHAR* Package ) { (void)Package; return harness_actor_class(); }
void AActor::InitializePrivateStaticClassAActor() {}
HARNESS_NULL_CLASS( APawn )
HARNESS_NULL_CLASS( APlayerPawn )
HARNESS_NULL_CLASS( ACamera )
HARNESS_NULL_CLASS( UNetDriver )
HARNESS_NULL_CLASS( UGameEngine )
HARNESS_NULL_CLASS( UDemoPlayPendingLevel )
HARNESS_NULL_CLASS( UViewport )
HARNESS_NULL_CLASS( UState )
HARNESS_NULL_CLASS( UClass )
HARNESS_NULL_CLASS( UProperty )
HARNESS_NULL_CLASS( UFunction )
HARNESS_NULL_CLASS( UBoolProperty )
HARNESS_NULL_CLASS( UByteProperty )
HARNESS_NULL_CLASS( UObjectProperty )
TArray<UObject*> UObject::GObjObjects;

UObject* UObject::StaticAllocateObject( UClass* Class, UObject* InOuter, FName Name, DWORD SetFlags, UObject* Template, FOutputDevice* Error, UObject* Ptr )
{
	(void)Class; (void)InOuter; (void)Name; (void)SetFlags; (void)Template; (void)Error; (void)Ptr;
	printf( "uobject_allocate\n" );
	return (UObject*)(void*)g_obj_pool[g_obj_pool_next++];
}

UModel::UModel( ABrush* Owner, UBOOL InRootOutside )
	: RootOutside( InRootOutside ), Surfs( this ), Vectors( this ), Points( this ), Verts( this ), Nodes( this )
{
	printf( "model_construct\t%d\t%d\n", Owner ? 1 : 0, (int)InRootOutside );
}
void UModel::ShrinkModel() { printf( "model_shrink_model\n" ); }
void UModel::Modify( UBOOL DoTransArrays ) { printf( "model_modify\t%d\n", (int)DoTransArrays ); }
BYTE UModel::FastLineCheck( FVector End, FVector Start )
{
	(void)End; (void)Start;
	printf( "model_fast_line_check\t%d\n", (int)g_fast_line_clear );
	return (BYTE)g_fast_line_clear;
}

// Port-side hook for the virtual ULevel::SingleLineCheck (see gen_level.sh rewrites).
UBOOL harness_single_line_check( ULevel* Level, FCheckResult& Hit, AActor* SourceActor, const FVector& End, const FVector& Start, DWORD TraceFlags, FVector Extent = FVector(0,0,0) )
{
	(void)Level; (void)End; (void)Start; (void)Extent;
	printf( "level_single_line_check\t%08x\t%d\n", (unsigned)TraceFlags, actor_index( SourceActor ) );
	if( !g_single_line_clear && g_trace_hit )
	{
		// A scripted hit: the trace reports the scenario's actor at the end point with an explicit item index.
		Hit.Actor = g_trace_hit;
		Hit.Item = g_trace_hit_item;
		Hit.Location = g_trace_hit_at_start ? Start : End;
		Hit.Normal = FVector( 0, 0, 0 );
		Hit.Primitive = NULL;
		Hit.Time = 0.5f;
		Hit.Next = NULL;
	}
	return g_single_line_clear;
}

// Port-side hook for the virtual APlayerPawn::IsZoneVisible (see gen_level.sh rewrites): only zone g_visible_zone is visible.
bool harness_is_zone_visible( APlayerPawn* Player, INT iZone )
{
	(void)Player;
	printf( "zone_visible\t%d\t%d\n", (int)iZone, iZone == g_visible_zone ? 1 : 0 );
	return iZone == g_visible_zone;
}

// Port-side hooks for the virtual ULevel::SpawnActor and ULevel::SpawnBrush (see gen_level.sh rewrites).
AActor* harness_spawn_actor( ULevel* Level, UClass* Class )
{
	((AActor*)(void*)g_levelinfo_mem)->Class = Class;
	printf( "level_spawn_actor\t%d\n", actor_index( (AActor*)(void*)g_levelinfo_mem ) );
	Level->Actors.AddItem( (AActor*)(void*)g_levelinfo_mem );
	return (AActor*)(void*)g_levelinfo_mem;
}
ABrush* harness_spawn_brush( ULevel* Level )
{
	printf( "level_spawn_brush\t%d\n", actor_index( (AActor*)(void*)g_brush_mem ) );
	((AActor*)(void*)g_brush_mem)->Class = harness_actor_class();
	Level->Actors.AddItem( (AActor*)(void*)g_brush_mem );
	return (ABrush*)(void*)g_brush_mem;
}

// Port-side hook for the virtual ULevel::SpawnViewActor (see gen_level.sh rewrite): the view actor is a camera in slot 7.
void harness_spawn_view_actor( ULevel* Level, UViewport* Viewport )
{
	AActor* View = (AActor*)(void*)g_actor_mem[7];
	memset( View, 0, HARNESS_ACTOR_SLOT );
	View->Class = ACamera::StaticClass();
	((APlayerPawn*)View)->Player = Viewport;
	Viewport->Actor = (APlayerPawn*)View;
	printf( "level_spawn_view_actor\t%d\n", actor_index( View ) );
	Level->Actors.AddItem( View );
}
// Port-side hook for the virtual AActor::eventOnResolveGameState (see gen_level.sh rewrite).
void harness_on_resolve_game_state( AActor* Actor ) { printf( "resolve_game_state\t%d\n", actor_index( Actor ) ); }
// Port-side hook for the virtual AGameInfo::eventDetailChange (see gen_level.sh rewrite).
void harness_event_detail_change( AGameInfo* Game ) { (void)Game; printf( "event_detail_change\n" ); }
alignas(16) static char g_netdrv_mem[sizeof(UNetDriver)];
alignas(16) static char g_demodrv_mem[sizeof(UNetDriver)];
alignas(16) static char g_conn_mem[sizeof(UNetConnection)];
alignas(16) static char g_chan_mem[sizeof(UChannel)];
alignas(16) static char g_netdrv_class_mem[sizeof(UClass)];
alignas(16) static char g_demodrv_class_mem[sizeof(UClass)];
// Scenario knobs for the net hooks; net_begin() restores the defaults.
static UBOOL g_init_listen_fail = 0;
static const TCHAR* g_init_listen_error = TEXT("");
static float g_init_listen_pause = 0.0f;
static AActor* g_helper_actor = NULL;
static const TCHAR* g_prelogin_error = TEXT("");
static const TCHAR* g_prelogin_failcode = TEXT("");
// Port-side hook for the virtual UNetDriver::Exec (see gen_level.sh rewrite). Each driver claims its own token: the net driver NETHANDLED, the demo recorder DEMOHANDLED.
UBOOL harness_net_exec( UNetDriver* Driver, const TCHAR* Cmd, FOutputDevice& Ar )
{
	(void)Ar;
	bool demo = (void*)Driver == (void*)g_demodrv_mem;
	printf( "net_exec\t%s\t%s\n", demo ? "demo" : "net", wide_to_utf8( Cmd ).c_str() );
	return wcscmp( Cmd, demo ? TEXT("DEMOHANDLED") : TEXT("NETHANDLED") ) == 0;
}
// Class loading and object construction are outside the level boundary: the two device classes map to sentinel classes whose super is UNetDriver, and the constructed object is the matching zeroed buffer.
UClass* UObject::StaticLoadClass( UClass* BaseClass, UObject* InOuter, const TCHAR* Name, const TCHAR* Filename, DWORD LoadFlags, UPackageMap* Sandbox )
{
	(void)BaseClass; (void)InOuter; (void)Filename; (void)LoadFlags; (void)Sandbox;
	printf( "load_class\t%s\n", wide_to_utf8( Name ).c_str() );
	UClass* Cls = NULL;
	if( wcscmp( Name, TEXT("ini:Engine.Engine.NetworkDevice") ) == 0 ) Cls = (UClass*)(void*)g_netdrv_class_mem;
	else if( wcscmp( Name, TEXT("ini:Engine.Engine.DemoRecordingDevice") ) == 0 ) Cls = (UClass*)(void*)g_demodrv_class_mem;
	if( Cls ) Cls->SuperField = UNetDriver::StaticClass();
	return Cls;
}
// The transient package is a fixed sentinel object (never dereferenced): the oracle only uses it as the default outer.
alignas(16) static char g_transient_pkg[sizeof(UPackage)];
UPackage* UObject::GetTransientPackage() { return (UPackage*)(void*)g_transient_pkg; }
UObject* UObject::StaticConstructObject( UClass* Class, UObject* InOuter, FName Name, DWORD SetFlags, UObject* Template, FOutputDevice* Error )
{
	(void)InOuter; (void)Name; (void)SetFlags; (void)Template; (void)Error;
	bool demo = (void*)Class == (void*)g_demodrv_class_mem;
	char* Mem = demo ? g_demodrv_mem : g_netdrv_mem;
	printf( "construct_object\t%s\n", demo ? "demo" : "net" );
	memset( Mem, 0, sizeof(g_netdrv_mem) );
	UObject* Obj = (UObject*)(void*)Mem;
	Obj->Class = Class;
	return Obj;
}
// Port-side hooks for the UNetDriver and UGameEngine calls the Exec/Listen scenarios reach (see gen_level.sh rewrites).
void harness_delete_driver( UNetDriver* Driver ) { printf( "delete_driver\t%s\n", (void*)Driver == (void*)g_demodrv_mem ? "demo" : "net" ); }
UBOOL harness_init_listen( UNetDriver* Driver, ULevel* Level, FURL& URL, FString& Error )
{
	(void)Level;
	printf( "init_listen\t%s\t%s\t%d\n", (void*)Driver == (void*)g_demodrv_mem ? "demo" : "net", wide_to_utf8( *URL.Map ).c_str(), g_init_listen_fail ? 0 : 1 );
	if( g_init_listen_fail ) Error = g_init_listen_error;
	else Driver->ServerTravelPause = g_init_listen_pause;
	return !g_init_listen_fail;
}
// Port-side hooks for the ULevel::Listen calls (see gen_level.sh rewrites): localized text, the master-map build, the spawned helper and the config import.
const TCHAR* harness_localize_error( const ANSICHAR* Key )
{
	printf( "localize_error\t%s\n", Key );
	return strcmp( Key, "NetAlready" ) == 0 ? TEXT("Already listening") : TEXT("Listen failed");
}
void harness_build_master_map( UGameEngine* Game, UNetDriver* Driver, ULevel* Level ) { (void)Game; (void)Driver; (void)Level; printf( "build_master_map\n" ); }
AActor* harness_spawn_helper( ULevel* Level, UClass* Class ) { (void)Level; (void)Class; printf( "spawn_helper\t%s\n", g_helper_actor ? "actor" : "null" ); return g_helper_actor; }
void harness_import_text( UProperty* Property, const TCHAR* Value, BYTE* Data ) { (void)Data; printf( "import_text\t%s\t%s\n", wide_to_utf8( Property->GetName() ).c_str(), wide_to_utf8( Value ).c_str() ); }
// Net scenarios reset every buffer and knob they read, so no state leaks between cases.
static void net_begin( const char* name, const char* knobs )
{
	begin( name, knobs );
	memset( g_netdrv_mem, 0, sizeof(g_netdrv_mem) );
	memset( g_demodrv_mem, 0, sizeof(g_demodrv_mem) );
	memset( g_conn_mem, 0, sizeof(g_conn_mem) );
	memset( g_chan_mem, 0, sizeof(g_chan_mem) );
	g_init_listen_fail = 0;
	g_init_listen_error = TEXT("");
	g_init_listen_pause = 0.0f;
	g_helper_actor = NULL;
	memset( g_game_engine_mem, 0, sizeof(g_game_engine_mem) );
	g_prelogin_error = TEXT("");
	g_prelogin_failcode = TEXT("");
	harness_actor_class()->Children = NULL;
}
// Port-side hooks for the UNetConnection calls the net scenarios reach (see gen_level.sh rewrites). Addresses and clocks are fixed.
FString harness_remote_address( UNetConnection* Connection ) { (void)Connection; return FString( TEXT("192.0.2.7:7777") ); }
const TCHAR* harness_timestamp() { return TEXT("2004.05.06-14.31.07"); }
DWORD harness_cycles() { return 12345; }
void harness_flush_net( UNetConnection* Connection ) { (void)Connection; printf( "conn_flush\n" ); }
void harness_send_package_map( UNetConnection* Connection ) { (void)Connection; printf( "send_package_map\n" ); }
void harness_package_map_copy( UNetConnection* Connection ) { (void)Connection; printf( "package_map_copy\n" ); }
void harness_conn_logf( UNetConnection* Connection, const TCHAR* Fmt, ... )
{
	(void)Connection;
	TCHAR msg[1024];
	GET_VARARGS( msg, ARRAY_COUNT(msg), Fmt );
	printf( "conn_log\t%s\n", wide_to_utf8( msg ).c_str() );
}
void harness_channel_close( UChannel* Channel ) { (void)Channel; printf( "channel_close\n" ); }
void harness_set_client_travel( UEngine* Engine, UPlayer* Viewport, const TCHAR* URL, UBOOL Items, ETravelType Type )
{
	(void)Engine; (void)Viewport; (void)Items; (void)Type;
	printf( "client_travel\t%s\n", wide_to_utf8( URL ).c_str() );
}
INT harness_challenge_response( UEngine* Engine, INT Challenge )
{
	(void)Engine;
	printf( "challenge_response\t%d\t1\n", (int)Challenge );
	return 1;
}
void harness_pre_login( AGameInfo* Game, const FString& Options, const FString& Address, FString& Error, FString& FailCode )
{
	(void)Game;
	printf( "pre_login\t%s\t%s\n", wide_to_utf8( *Options ).c_str(), wide_to_utf8( *Address ).c_str() );
	Error = g_prelogin_error;
	FailCode = g_prelogin_failcode;
}

// Host wall clock is not reproducible: pin it. The oracle reads it only through this hook.
void appSystemTime( INT& Year, INT& Month, INT& DayOfWeek, INT& Day, INT& Hour, INT& Min, INT& Sec, INT& MSec )
{
	printf( "app_system_time\n" );
	Year = 2004; Month = 5; DayOfWeek = 3; Day = 6; Hour = 14; Min = 31; Sec = 7; MSec = 250;
}

static void setup_actors( const int* ids, int count )
{
	memset( g_level_mem, 0, sizeof(g_level_mem) );
	for( int i = 0; i < count; ++i )
		g_level->Actors.AddItem( make_actor( ids[i], true ) );
}

// Core helper: the level info is Actors(0), as GetLevelInfo() requires; the level carries a name for GetName().
static ALevelInfo* level_with_levelinfo()
{
	ALevelInfo* info = (ALevelInfo*)(void*)g_levelinfo_mem;
	info->Class = ALevelInfo::StaticClass();
	g_level->Actors.AddItem( info );
	g_level->Name = FName( TEXT("Level") );
	return info;
}
// Core helper: an engine whose client owns one viewport named Cam0 (RememberActors/ReconcileActors need Engine->Client).
static UViewport* setup_engine_viewport()
{
	UViewport* viewport = (UViewport*)(void*)g_viewport_mem;
	viewport->Class = UViewport::StaticClass();
	viewport->Name = FName( TEXT("Cam0") );
	UClient* client = (UClient*)(void*)g_client_mem;
	client->Viewports.AddItem( viewport );
	UEngine* engine = (UEngine*)(void*)g_engine_mem;
	engine->Client = client;
	g_level->Engine = engine;
	return viewport;
}

const TCHAR* UObject::GetFullName( TCHAR* Str ) const
{
	(void)Str;
	return TEXT("Engine.Level");
}

static void emit_inputs( const char* map, const int* ids, const int* persistent, int count, const std::vector<unsigned char>* input, INT file_size )
{
	printf( "input\tmap\t%s\n", map );
	for( int i = 0; i < count; ++i )
		printf( "input\tactor\t%d\tActor%d\t%d\t1\n", ids[i], ids[i], persistent[i] );
	if( input )
	{
		printf( "input\tpa_bytes\t%d\t", (int)input->size() );
		for( size_t i = 0; i < input->size(); ++i )
			printf( "%02x", (unsigned)(*input)[i] );
		printf( "\n" );
	}
	else
		printf( "input\tpa_bytes\t0\t\n" );
	printf( "input\tfile_size\t%d\n", (int)file_size );
	printf( "input\tglobal_time\t0\n" );
}

static void save_persist()
{
	begin( "persist.save_two_persistent" );
	const int ids[] = { 0, 2 };
	setup_actors( ids, 2 );
	g_level->Actors.AddItem( make_actor( 1, false ) );
	{ const int in_ids[] = { 0, 2, 1 }; const int in_p[] = { 1, 1, 0 }; emit_inputs( "Test.unr", in_ids, in_p, 3, NULL, -1 ); }
	printf( "=>\t%d\n", (int)g_level->ULevel::SavePersistentActors( FString( TEXT("Test.unr") ) ) );
	printf( "saved_bytes\t%d\n", (int)g_saved_bytes.size() );
}

static void load_case( const char* name, const int* ids, int count, const char* map, const std::vector<unsigned char>* input, INT file_size, bool fail_reader )
{
	begin( name );
	const int all_persist[] = { 1, 1, 1 };
	emit_inputs( map, ids, all_persist, count, input, file_size );
	printf( "input\treader_null\t%d\n", fail_reader ? 1 : 0 );
	setup_actors( ids, count );
	g_read_input = input ? *input : std::vector<unsigned char>();
	g_file_size = file_size;
	g_fail_reader = fail_reader;
	TCHAR wmap[64];
	for( int i = 0; ; ++i ) { wmap[i] = map[i]; if( !map[i] ) break; }
	printf( "=>\t%d\n", (int)g_level->ULevel::LoadPersistentActors( FString( wmap ) ) );
}

static void run_persist()
{
	save_persist();
	const std::vector<unsigned char> good = g_saved_bytes;
	const int two[] = { 0, 2 };
	const int extra[] = { 0, 2, 3 };
	const int one[] = { 0 };
	std::vector<unsigned char> bad_id = good;
	bad_id[0] ^= 0xFF;
	std::vector<unsigned char> truncated( good.begin(), good.end() - 4 );
	std::vector<unsigned char> bad_wire = good;
	bad_wire[2] = 0x7F;
	load_case( "persist.load_roundtrip", two, 2, "Test.unr", &good, -1, false );
	load_case( "persist.load_missing_file", two, 2, "Test.unr", &good, -1, true );
	load_case( "persist.load_empty_input", two, 2, "Test.unr", NULL, -1, false );
	load_case( "persist.load_bad_identifier", two, 2, "Test.unr", &bad_id, -1, false );
	load_case( "persist.load_map_mismatch", two, 2, "Other.unr", &good, -1, false );
	load_case( "persist.load_size_mismatch", two, 2, "Test.unr", &good, 99, false );
	load_case( "persist.load_missing_actor", one, 1, "Test.unr", &good, -1, false );
	load_case( "persist.load_leftover_destroy", extra, 3, "Test.unr", &good, -1, false );
	load_case( "persist.load_bad_eof", two, 2, "Test.unr", &truncated, -1, false );
	load_case( "persist.load_wire_error", two, 2, "Test.unr", &bad_wire, -1, false );
}

static AActor* actor_at( int i ) { return (AActor*)(void*)g_actor_mem[i]; }

static void run_audible( const char* name, const char* knobs, ESoundOcclusion occ, UBOOL clear )
{
	begin( name, knobs );
	g_level->Model = (UModel*)(void*)g_model_mem;
	g_fast_line_clear = clear;
	g_single_line_clear = clear;
	FVector Src( 0.f, 0.f, 0.f );
	FVector Lst( 100.f, 0.f, 0.f );
	printf( "=>\t%d\n", (int)g_level->ULevel::IsAudibleAt( Src, Lst, NULL, occ ) );
}

// Core helpers for ScreenActorsByGameState. Pawns carry Class = APawn so IsPlayer() passes.
static APlayerPawn* add_game_state_pawn( int slot, const TCHAR* cur, const TCHAR* master, INT token_len )
{
	AActor* a = make_actor( slot, true );
	a->Class = APawn::StaticClass();
	((APawn*)a)->bIsPlayer = 1;
	a->Group = FName( TEXT("PAWN") );
	APlayerPawn* p = (APlayerPawn*)a;
	p->GameStateTokenLen = token_len;
	p->GameStateMasterList = master;
	p->CurrentGameState = cur;
	g_level->Actors.AddItem( a );
	return p;
}
static AActor* add_group_actor( int slot, const TCHAR* group, const TCHAR* exclude )
{
	AActor* a = make_actor( slot, true );
	a->Group = FName( group );
	a->ExcludeGameStates = exclude;
	g_level->Actors.AddItem( a );
	return a;
}
// Observable GameState result per actor: index, bInCurrentGameState, bHiddenEd.
static void screen_and_report()
{
	g_level->ULevel::ScreenActorsByGameState();
	for( INT i = 0; i < g_level->Actors.Num(); ++i )
		if( g_level->Actors(i) )
			printf( "actor_flags\t%d\t%d\t%d\n", actor_index( g_level->Actors(i) ), g_level->Actors(i)->bInCurrentGameState ? 1 : 0, g_level->Actors(i)->bHiddenEd ? 1 : 0 );
	printf( "=>\t0\n" );
}
static void run_screen_actors_by_game_state()
{
	const TCHAR* master = TEXT("GSTATELOBYGSTATEMAIN");
	begin( "core.screen_actors_by_game_state_no_player", "player=0 group=GSTATEMAIN" );
	add_group_actor( 0, TEXT("GSTATEMAIN"), TEXT("") );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_none_state", "player_state=NONE group=GSTATEMAIN" );
	add_game_state_pawn( 0, TEXT("none"), master, 10 );
	add_group_actor( 1, TEXT("GSTATEMAIN"), TEXT("") );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_invalid_state", "player_state=GSTATEFOO master_has=0 group=GSTATEMAIN" );
	add_game_state_pawn( 0, TEXT("GSTATEFOO"), master, 10 );
	add_group_actor( 1, TEXT("GSTATEMAIN"), TEXT("") );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_null_slot", "null_slot=1 player_state=GSTATEMAIN group=GSTATEMAIN" );
	g_level->Actors.AddItem( (AActor*)NULL );
	add_group_actor( 1, TEXT("GSTATEMAIN"), TEXT("") );
	add_game_state_pawn( 0, TEXT("GSTATEMAIN"), master, 10 );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_exclude_hit", "player_state=GSTATEMAIN exclude=gstatemain group=GSTATEMAIN" );
	add_game_state_pawn( 0, TEXT("GSTATEMAIN"), master, 10 );
	add_group_actor( 1, TEXT("GSTATEMAIN"), TEXT("gstatemain") );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_exclude_miss", "player_state=GSTATEMAIN exclude=GSTATELOBY group=GSTATEMAIN" );
	add_game_state_pawn( 0, TEXT("GSTATEMAIN"), master, 10 );
	add_group_actor( 1, TEXT("GSTATEMAIN"), TEXT("GSTATELOBY") );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_no_gstate", "player_state=GSTATEMAIN group=PAWNLIGHT" );
	add_game_state_pawn( 0, TEXT("GSTATEMAIN"), master, 10 );
	add_group_actor( 1, TEXT("PAWNLIGHT"), TEXT("") );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_editor_hidden", "editor=1 player_state=GSTATEMAIN group=GSTATELOBY hidden_ed=1" );
	GIsEditor = 1;
	add_game_state_pawn( 0, TEXT("GSTATEMAIN"), master, 10 );
	add_group_actor( 1, TEXT("GSTATELOBY"), TEXT("") );
	actor_at( 1 )->bHiddenEd = 1;
	screen_and_report();
	begin( "core.screen_actors_by_game_state_match", "player_state=GSTATEMAIN group=GSTATEMAIN token_len=10 exclude=-" );
	add_game_state_pawn( 0, TEXT("GSTATEMAIN"), master, 10 );
	add_group_actor( 1, TEXT("GSTATEMAIN"), TEXT("") );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_multi_token", "player_state=GSTATEMAIN group=GSTATELOBY,GSTATEMAIN" );
	add_game_state_pawn( 0, TEXT("GSTATEMAIN"), master, 10 );
	add_group_actor( 1, TEXT("GSTATELOBY,GSTATEMAIN"), TEXT("") );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_bad_token_length", "player_state=GSTATEMAIN group=GSTATEX token_len=10" );
	add_game_state_pawn( 0, TEXT("GSTATEMAIN"), master, 10 );
	add_group_actor( 1, TEXT("GSTATEX"), TEXT("") );
	screen_and_report();
	begin( "core.screen_actors_by_game_state_bad_token_unknown", "player_state=GSTATEMAIN group=GSTATEXXXX token_len=10" );
	add_game_state_pawn( 0, TEXT("GSTATEMAIN"), master, 10 );
	add_group_actor( 1, TEXT("GSTATEXXXX"), TEXT("") );
	screen_and_report();
}

// ULevel::DetailChange: observable result is bHighDetailMode plus the eventDetailChange hook when a game is set.
static void run_detail_change()
{
	begin( "core.detail_change_no_game", "game=NULL new_detail=1" );
	ALevelInfo* info = level_with_levelinfo();
	g_level->ULevel::DetailChange( 1 );
	printf( "high_detail\t%d\n", info->bHighDetailMode ? 1 : 0 );
	printf( "=>\t0\n" );
	begin( "core.detail_change_game", "game=set new_detail=0 start_high=1" );
	info = level_with_levelinfo();
	info->bHighDetailMode = 1;
	info->Game = (AGameInfo*)g_game_mem;
	g_level->ULevel::DetailChange( 0 );
	printf( "high_detail\t%d\n", info->bHighDetailMode ? 1 : 0 );
	printf( "=>\t0\n" );
}

// Core helpers for RememberActors / ReconcileActors: a camera view actor and the engine/client chain.
static AActor* add_camera( int slot, const TCHAR* tag )
{
	AActor* a = make_actor( slot, true );
	a->Class = ACamera::StaticClass();
	a->Tag = FName( tag );
	g_level->Actors.AddItem( a );
	return a;
}
static UClient* setup_engine_client_only()
{
	UClient* client = (UClient*)(void*)g_client_mem;
	UEngine* engine = (UEngine*)(void*)g_engine_mem;
	engine->Client = client;
	g_level->Engine = engine;
	return client;
}
// Observable view state: the viewport's saved fields and the index of its linked actor (-1 when unlinked).
static void print_viewport_state( UViewport* v )
{
	printf( "viewport_saved\t%g\t%g\t%d\t%d\t%d\t%d\n", v->SavedOrthoZoom, v->SavedFovAngle, v->SavedShowFlags, v->SavedRendMap, v->SavedMisc1, v->SavedMisc2 );
	printf( "viewport_actor\t%d\n", v->Actor ? actor_index( v->Actor ) : -1 );
}
// ULevel::RememberActors: saves each viewport's actor view fields and clears the link.
static void run_remember_actors()
{
	begin( "core.remember_actors_no_client", "client=NULL viewports=1 actor=Cam0" );
	UViewport* viewport = setup_engine_viewport();
	viewport->Actor = (APlayerPawn*)add_camera( 1, TEXT("Cam0") );
	g_level->Engine->Client = NULL;
	g_level->ULevel::RememberActors();
	print_viewport_state( viewport );
	printf( "=>\t0\n" );
	begin( "core.remember_actors_no_viewports", "client=set viewports=0" );
	setup_engine_client_only();
	g_level->ULevel::RememberActors();
	printf( "=>\t0\n" );
	begin( "core.remember_actors_save_view", "client=set viewports=1 actor=Cam0" );
	viewport = setup_engine_viewport();
	APlayerPawn* view = (APlayerPawn*)add_camera( 1, TEXT("Cam0") );
	view->OrthoZoom = 1.5f;
	view->FovAngle = 90.0f;
	view->ShowFlags = 17;
	view->RendMap = 3;
	view->Misc1 = 4;
	view->Misc2 = 5;
	viewport->Actor = view;
	g_level->ULevel::RememberActors();
	print_viewport_state( viewport );
	printf( "=>\t0\n" );
}

static APlayerPawn* add_player_pawn( int slot, UPlayer* player )
{
	AActor* a = make_actor( slot, true );
	a->Class = APlayerPawn::StaticClass();
	((APlayerPawn*)a)->Player = player;
	g_level->Actors.AddItem( a );
	return (APlayerPawn*)a;
}
static void print_view_fields( APlayerPawn* v )
{
	printf( "view_fields\t%g\t%g\t%d\t%d\t%d\t%d\n", v->OrthoZoom, v->FovAngle, v->ShowFlags, v->RendMap, v->Misc1, v->Misc2 );
}
// ULevel::ReconcileActors: observable result is pawn Player links, viewport actor links, view fields, and destroys.
static void run_reconcile_actors()
{
	begin( "core.reconcile_actors_player_cleared", "editor=1 viewports=0 actors=null,nonpawn,pawn_player,pawn_free" );
	GIsEditor = 1;
	setup_engine_client_only();
	g_level->Actors.AddItem( (AActor*)NULL );
	g_level->Actors.AddItem( make_actor( 1, true ) );
	APlayerPawn* linked = add_player_pawn( 2, (UPlayer*)(void*)g_viewport_mem );
	APlayerPawn* free_pawn = add_player_pawn( 3, NULL );
	g_level->ULevel::ReconcileActors();
	printf( "pawn_player\t%d\t%d\n", actor_index( linked ), linked->Player ? 1 : 0 );
	printf( "pawn_player\t%d\t%d\n", actor_index( free_pawn ), free_pawn->Player ? 1 : 0 );
	printf( "=>\t0\n" );
	begin( "core.reconcile_actors_match_identical", "editor=1 viewports=Cam0 cameras=Other,Cam0" );
	GIsEditor = 1;
	UViewport* viewport = setup_engine_viewport();
	g_level->Actors.AddItem( make_actor( 0, true ) );
	add_camera( 1, TEXT("Other") );
	APlayerPawn* matched = (APlayerPawn*)add_camera( 2, TEXT("Cam0") );
	matched->OrthoZoom = 2.0f;
	matched->FovAngle = 45.0f;
	matched->ShowFlags = 9;
	matched->RendMap = 1;
	matched->Misc1 = 2;
	matched->Misc2 = 3;
	viewport->SavedOrthoZoom = 3.5f;
	viewport->SavedFovAngle = 70.0f;
	viewport->SavedShowFlags = 11;
	viewport->SavedRendMap = 2;
	viewport->SavedMisc1 = 6;
	viewport->SavedMisc2 = 7;
	g_level->ULevel::ReconcileActors();
	print_viewport_state( viewport );
	print_view_fields( matched );
	printf( "=>\t0\n" );
	begin( "core.reconcile_actors_spawn_unmatched", "editor=1 viewports=Cam0 cameras=Other" );
	GIsEditor = 1;
	viewport = setup_engine_viewport();
	viewport->SavedOrthoZoom = 3.5f;
	viewport->SavedFovAngle = 70.0f;
	viewport->SavedShowFlags = 11;
	viewport->SavedRendMap = 2;
	viewport->SavedMisc1 = 6;
	viewport->SavedMisc2 = 7;
	add_camera( 1, TEXT("Other") );
	g_level->ULevel::ReconcileActors();
	print_viewport_state( viewport );
	print_view_fields( viewport->Actor );
	printf( "=>\t0\n" );
	begin( "core.reconcile_actors_check_editor", "editor=0 viewports=0" );
	GIsEditor = 0;
	try { g_level->ULevel::ReconcileActors(); } catch( HarnessAbort& a ) { printf( "harness_abort\t%d\n", a.kind ); }
	printf( "=>\t0\n" );
	begin( "core.reconcile_actors_check_viewport_actor", "editor=1 viewports=Cam0 viewport_actor=Cam0" );
	GIsEditor = 1;
	viewport = setup_engine_viewport();
	viewport->Actor = (ACamera*)(void*)add_camera( 1, TEXT("Cam0") );
	try { g_level->ULevel::ReconcileActors(); } catch( HarnessAbort& a ) { printf( "harness_abort\t%d\n", a.kind ); }
	printf( "=>\t0\n" );
}

// Trace helpers. setup_trace_model builds node i with iZone[1]=i, so g_visible_zone selects the visible node.
static void setup_trace_model( int count )
{
	g_level->Model = (UModel*)(void*)g_model_mem;
	for( int i = 0; i < count; i++ )
	{
		FBspNode node;
		memset( &node, 0, sizeof(node) );
		node.iZone[1] = (BYTE)i;
		g_level->Model->Nodes.AddItem( node );
	}
}
static AActor* add_trace_actor( int slot, UClass* cls, int zone )
{
	AActor* a = make_actor( slot, true );
	a->Class = cls;
	a->Region.ZoneNumber = (BYTE)zone;
	return a;
}
static ALevelInfo* trace_levelinfo()
{
	ALevelInfo* info = level_with_levelinfo();
	info->XLevel = g_level;
	return info;
}
static void script_trace_hit( AActor* hit, INT item, bool at_start )
{
	g_single_line_clear = 0;
	g_trace_hit = hit;
	g_trace_hit_item = item;
	g_trace_hit_at_start = at_start ? 1 : 0;
}
static void print_trace_hit( const FCheckResult& Hit )
{
	printf( "hit_time\t%g\nhit_actor\t%d\nhit_item\t%d\n", Hit.Time, actor_index( Hit.Actor ), Hit.Item );
}
static void call_trace_visible( INT distance, AActor* source )
{
	FVector dir( 1, 0, 0 );
	FVector start( 0, 0, 0 );
	FCheckResult hit;
	hit.Item = -1;
	g_level->TraceVisible( dir, hit, source, start, TRACE_VisBlocking, distance );
	print_trace_hit( hit );
	printf( "=>\t0\n" );
}
static void call_trace_visible_objects( UClass* parent, INT distance, AActor* source, float dir_x = 1.0f )
{
	FVector dir( dir_x, 0, 0 );
	FVector start( 0, 0, 0 );
	FCheckResult hit;
	hit.Item = -1;
	g_level->TraceVisibleObjects( parent, dir, hit, source, start, TRACE_VisBlocking, distance );
	print_trace_hit( hit );
	printf( "=>\t0\n" );
}

// ULevel::TraceVisible branches: no distance, clear line, levelinfo item (0 = rock, else zone-gated node), actor zone gate with retry, owned skip, non-player hits.
static void run_trace_visible()
{
	begin( "core.trace_visible_no_distance", "distance=0 source=nonpawn" );
	AActor* src = add_trace_actor( 0, harness_actor_class(), 0 );
	call_trace_visible( 0, src );

	begin( "core.trace_visible_clear", "distance=100 single_line=clear source=nonpawn" );
	src = add_trace_actor( 0, harness_actor_class(), 0 );
	call_trace_visible( 100, src );

	begin( "core.trace_visible_levelinfo_visible", "distance=100 source=pawn hit=levelinfo item=5 zone=5 visible=5" );
	ALevelInfo* info = trace_levelinfo();
	src = add_trace_actor( 0, APlayerPawn::StaticClass(), 0 );
	setup_trace_model( 6 );
	g_visible_zone = 5;
	script_trace_hit( info, 5, false );
	call_trace_visible( 100, src );

	begin( "core.trace_visible_levelinfo_rock", "distance=100 source=pawn hit=levelinfo item=0" );
	info = trace_levelinfo();
	src = add_trace_actor( 0, APlayerPawn::StaticClass(), 0 );
	script_trace_hit( info, 0, false );
	call_trace_visible( 100, src );

	begin( "core.trace_visible_levelinfo_hidden", "distance=100 source=pawn hit=levelinfo item=5 zone=5 visible=3" );
	info = trace_levelinfo();
	src = add_trace_actor( 0, APlayerPawn::StaticClass(), 0 );
	setup_trace_model( 6 );
	g_visible_zone = 3;
	script_trace_hit( info, 5, false );
	call_trace_visible( 100, src );

	begin( "core.trace_visible_actor_visible", "distance=100 source=pawn hit=Actor1 zone=2 visible=2" );
	src = add_trace_actor( 0, APlayerPawn::StaticClass(), 0 );
	AActor* hit = add_trace_actor( 1, harness_actor_class(), 2 );
	g_visible_zone = 2;
	script_trace_hit( hit, 0, false );
	call_trace_visible( 100, src );

	begin( "core.trace_visible_actor_hidden_retry", "distance=48 source=pawn hit=Actor1 zone=2 visible=3 hit_at_start=1" );
	src = add_trace_actor( 0, APlayerPawn::StaticClass(), 0 );
	hit = add_trace_actor( 1, harness_actor_class(), 2 );
	g_visible_zone = 3;
	script_trace_hit( hit, 0, true );
	call_trace_visible( 48, src );

	begin( "core.trace_visible_owned_skip", "distance=100 source=pawn hit=Actor1 owned_by_source=1 zone=2 visible=2" );
	src = add_trace_actor( 0, APlayerPawn::StaticClass(), 0 );
	hit = add_trace_actor( 1, harness_actor_class(), 2 );
	src->Owner = hit;
	g_visible_zone = 2;
	script_trace_hit( hit, 0, false );
	call_trace_visible( 100, src );

	begin( "core.trace_visible_nonplayer_levelinfo", "distance=100 source=nonpawn hit=levelinfo item=5" );
	info = trace_levelinfo();
	src = add_trace_actor( 0, harness_actor_class(), 0 );
	setup_trace_model( 6 );
	script_trace_hit( info, 5, false );
	call_trace_visible( 100, src );

	begin( "core.trace_visible_nonplayer_actor", "distance=100 source=nonpawn hit=Actor1 zone=2" );
	src = add_trace_actor( 0, harness_actor_class(), 0 );
	hit = add_trace_actor( 1, harness_actor_class(), 2 );
	script_trace_hit( hit, 0, false );
	call_trace_visible( 100, src );
}

// ULevel::TraceVisibleObjects branches: no distance, clear line, class match vs mismatch, player zone gate with retry, owned skip.
static void run_trace_visible_objects()
{
	begin( "core.trace_visible_objects_no_distance", "distance=0 source=nonpawn parent=Actor" );
	AActor* src = add_trace_actor( 0, harness_actor_class(), 0 );
	call_trace_visible_objects( harness_actor_class(), 0, src );

	begin( "core.trace_visible_objects_clear", "distance=100 single_line=clear source=nonpawn parent=Actor" );
	src = add_trace_actor( 0, harness_actor_class(), 0 );
	call_trace_visible_objects( harness_actor_class(), 100, src );

	begin( "core.trace_visible_objects_nonplayer_match", "distance=100 source=nonpawn parent=Actor hit=Actor1 class=Actor" );
	src = add_trace_actor( 0, harness_actor_class(), 0 );
	AActor* hit = add_trace_actor( 1, harness_actor_class(), 2 );
	script_trace_hit( hit, 0, false );
	call_trace_visible_objects( harness_actor_class(), 100, src );

	begin( "core.trace_visible_objects_player_visible", "distance=100 source=pawn parent=Actor hit=Actor1 zone=2 visible=2" );
	src = add_trace_actor( 0, APlayerPawn::StaticClass(), 0 );
	hit = add_trace_actor( 1, harness_actor_class(), 2 );
	g_visible_zone = 2;
	script_trace_hit( hit, 0, false );
	call_trace_visible_objects( harness_actor_class(), 100, src );

	begin( "core.trace_visible_objects_player_hidden_retry", "distance=48 source=pawn parent=Actor hit=Actor1 zone=2 visible=3 hit_at_start=1" );
	src = add_trace_actor( 0, APlayerPawn::StaticClass(), 0 );
	hit = add_trace_actor( 1, harness_actor_class(), 2 );
	g_visible_zone = 3;
	script_trace_hit( hit, 0, true );
	call_trace_visible_objects( harness_actor_class(), 48, src );

	begin( "core.trace_visible_objects_class_mismatch", "distance=100 source=nonpawn parent=APlayerPawn hit=Actor1 class=Actor" );
	src = add_trace_actor( 0, harness_actor_class(), 0 );
	hit = add_trace_actor( 1, harness_actor_class(), 2 );
	script_trace_hit( hit, 0, false );
	call_trace_visible_objects( APlayerPawn::StaticClass(), 100, src );

	begin( "core.trace_visible_objects_owned_skip", "distance=100 source=nonpawn parent=Actor hit=Actor1 owned_by_source=1" );
	src = add_trace_actor( 0, harness_actor_class(), 0 );
	hit = add_trace_actor( 1, harness_actor_class(), 2 );
	src->Owner = hit;
	script_trace_hit( hit, 0, false );
	call_trace_visible_objects( harness_actor_class(), 100, src );
	begin( "core.trace_visible_objects_loop_band", "distance=47 dir_x=0.97 source=nonpawn parent=APlayerPawn hit=Actor1 class=Actor hit_at_start=1" );
	src = add_trace_actor( 0, harness_actor_class(), 0 );
	hit = add_trace_actor( 1, harness_actor_class(), 2 );
	script_trace_hit( hit, 0, true );
	call_trace_visible_objects( APlayerPawn::StaticClass(), 47, src, 0.97f );
}

static void run_core()
{
	const int two[] = { 0, 2 };
	const int three[] = { 0, 1, 2 };
	begin( "core.is_server_no_drivers" );
	printf( "=>\t%d\n", (int)g_level->ULevel::IsServer() );
	begin( "core.is_audible_occlusion_none" );
	FVector Src( 0.f, 0.f, 0.f );
	FVector Lst( 100.f, 0.f, 0.f );
	printf( "=>\t%d\n", (int)g_level->ULevel::IsAudibleAt( Src, Lst, NULL, OCCLUSION_None ) );
	begin( "core.init_stats" );
	g_level->ULevel::InitStats();
	printf( "=>\t0\n" );
	begin( "core.get_stats" );
	GSecondsPerCycle = 0.001f;
	GScriptCycles = 1;
	g_level->ActorTickCycles = 2;
	g_level->FindPathCycles = 3;
	g_level->SeePlayer = 4;
	g_level->Spawning = 5;
	g_level->AudioTickCycles = 6;
	g_level->Unused = 7;
	g_level->MoveCycles = 8;
	g_level->NumMoves = 9;
	g_level->NetTickCycles = 10;
	TCHAR stats[256];
	g_level->ULevel::GetStats( stats );
	printf( "stats\t%s\n", wide_to_utf8( stats ).c_str() );
	printf( "=>\t0\n" );

	begin( "core.base_serialize_roundtrip", "actors=2 path=save+load" );
	setup_actors( two, 2 );
	std::vector<unsigned char> saved;
	{
		HArchive w( true );
		g_level->ULevelBase::Serialize( w );
		saved = w.Buffer;
	}
	{
		HArchive r( false );
		r.Buffer = saved;
		g_level->ULevelBase::Serialize( r );
	}
	printf( "=>\t0\n" );

	begin( "core.base_destroy_no_drivers", "net=0 demo=0" );
	g_level->ULevelBase::Destroy();
	printf( "=>\t0\n" );

	begin( "core.init_level_ctor", "engine=null root_outside=0" );
	new ( (EInternal*)g_level_mem ) ULevel( NULL, 0 );
	printf( "=>\t0\n" );
	begin( "core.init_level_ctor_root", "engine=null root_outside=1" );
	new ( (EInternal*)g_level_mem ) ULevel( NULL, 1 );
	printf( "=>\t0\n" );

	begin( "core.shrink_model", "model=1" );
	g_level->Model = (UModel*)(void*)g_model_mem;
	g_level->ULevel::ShrinkLevel();
	printf( "=>\t0\n" );

	begin( "core.modify_model", "model=1 do_trans=0" );
	g_level->Model = (UModel*)(void*)g_model_mem;
	g_level->ULevel::Modify( 0 );
	printf( "=>\t0\n" );

	begin( "core.post_load_outer_match", "actors=2 outer=match" );
	setup_actors( two, 2 );
	for( int i = 0; i < 2; ++i ) UObject::GObjObjects.AddItem( (UObject*)(void*)g_actor_mem[two[i]] );
	g_level->ULevel::PostLoad();
	for( int i = 0; i < UObject::GObjObjects.Num(); ++i )
		printf( "xlevel\t%d\t%d\n", actor_index( (AActor*)(void*)UObject::GObjObjects(i) ), ((AActor*)(void*)UObject::GObjObjects(i))->XLevel == g_level ? 1 : 0 );
	printf( "=>\t0\n" );
	begin( "core.post_load_outer_mixed", "actors=2 outer=mixed" );
	setup_actors( two, 2 );
	for( int i = 0; i < 2; ++i ) UObject::GObjObjects.AddItem( (UObject*)(void*)g_actor_mem[two[i]] );
	actor_at( 2 )->Outer = (UObject*)(void*)g_sys;
	g_level->ULevel::PostLoad();
	for( int i = 0; i < UObject::GObjObjects.Num(); ++i )
		printf( "xlevel\t%d\t%d\n", actor_index( (AActor*)(void*)UObject::GObjObjects(i) ), ((AActor*)(void*)UObject::GObjObjects(i))->XLevel == g_level ? 1 : 0 );
	printf( "=>\t0\n" );

	begin( "core.set_collision_enable", "actors=3 hash=none enable=1" );
	setup_actors( three, 3 );
	actor_at( 0 )->bCollideActors = 1;
	actor_at( 2 )->bCollideActors = 1;
	g_level->ULevel::SetActorCollision( 1 );
	printf( "=>\t0\n" );
	begin( "core.set_collision_disable", "actors=3 hash=present enable=0" );
	setup_actors( three, 3 );
	actor_at( 0 )->bCollideActors = 1;
	actor_at( 2 )->bCollideActors = 1;
	g_level->Hash = GNewCollisionHash();
	g_level->ULevel::SetActorCollision( 0 );
	printf( "=>\t0\n" );
	begin( "core.set_collision_noop", "actors=3 hash=none enable=0" );
	setup_actors( three, 3 );
	g_level->ULevel::SetActorCollision( 0 );
	printf( "=>\t0\n" );

	begin( "core.serialize_save_ver64", "ver=64 writing=1" );
	g_level->Model = (UModel*)(void*)g_model_mem;
	{
		HArchive w( true );
		w.ArVer = 64;
		g_level->ULevel::Serialize( w );
	}
	printf( "=>\t0\n" );
	begin( "core.serialize_save_ver60", "ver=60 writing=1" );
	g_level->Model = (UModel*)(void*)g_model_mem;
	{
		HArchive w( true );
		w.ArVer = 60;
		g_level->ULevel::Serialize( w );
	}
	printf( "=>\t0\n" );
	begin( "core.serialize_load_ver64", "ver=64 writing=0" );
	g_level->Model = (UModel*)(void*)g_model_mem;
	{
		HArchive r( false );
		r.ArVer = 64;
		r.Buffer.assign( 256, 0 );
		g_level->ULevel::Serialize( r );
	}
	printf( "=>\t0\n" );

	begin( "core.destroy_hash_present", "hash=present" );
	g_level->Hash = GNewCollisionHash();
	g_level->ULevel::Destroy();
	printf( "=>\t0\n" );
	begin( "core.destroy_hash_none", "hash=none" );
	g_level->ULevel::Destroy();
	printf( "=>\t0\n" );

	begin( "core.update_time", "info=levelinfo" );
	g_level->ULevel::UpdateTime( (ALevelInfo*)(void*)g_levelinfo_mem );
	printf( "level_info_time\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%d\n", ((ALevelInfo*)(void*)g_levelinfo_mem)->Year, ((ALevelInfo*)(void*)g_levelinfo_mem)->Month, ((ALevelInfo*)(void*)g_levelinfo_mem)->DayOfWeek, ((ALevelInfo*)(void*)g_levelinfo_mem)->Day, ((ALevelInfo*)(void*)g_levelinfo_mem)->Hour, ((ALevelInfo*)(void*)g_levelinfo_mem)->Minute, ((ALevelInfo*)(void*)g_levelinfo_mem)->Second, ((ALevelInfo*)(void*)g_levelinfo_mem)->Millisecond );
	printf( "=>\t0\n" );

	begin( "core.radius_multiplier_same_zone", "zone1=1 zone2=1 distance=0" );
	g_level->ZoneDist[1][1] = 0;
	printf( "=>\t%.9g\n", (double)g_level->ULevel::CalculateRadiusMultiplier( 1, 1 ) );
	begin( "core.radius_multiplier_two_zones", "zone1=1 zone2=2 distance=2" );
	g_level->ZoneDist[1][2] = 2;
	printf( "=>\t%.9g\n", (double)g_level->ULevel::CalculateRadiusMultiplier( 1, 2 ) );

	run_audible( "core.is_audible_occlusion_default_clear", "occlusion=Default model_clear=1", OCCLUSION_Default, 1 );
	run_audible( "core.is_audible_occlusion_default_blocked", "occlusion=Default model_clear=0", OCCLUSION_Default, 0 );
	run_audible( "core.is_audible_occlusion_static_clear", "occlusion=StaticMeshes trace_clear=1", OCCLUSION_StaticMeshes, 1 );
	run_audible( "core.is_audible_occlusion_static_blocked", "occlusion=StaticMeshes trace_clear=0", OCCLUSION_StaticMeshes, 0 );
	run_screen_actors_by_game_state();
	run_detail_change();
	run_remember_actors();
	run_reconcile_actors();
	run_trace_visible();
	run_trace_visible_objects();
}


static void run_net()
{
	UNetDriver* drv = (UNetDriver*)g_netdrv_mem;
	UNetConnection* conn = (UNetConnection*)g_conn_mem;
	UChannel* chan = (UChannel*)g_chan_mem;
	memset( g_netdrv_mem, 0, sizeof(g_netdrv_mem) );
	memset( g_conn_mem, 0, sizeof(g_conn_mem) );
	memset( g_chan_mem, 0, sizeof(g_chan_mem) );
	begin( "net.notify_sending_file_server" );
	g_level->NetDriver = drv;
	printf( "=>\t%d\n", (int)g_level->ULevel::NotifySendingFile( conn, FGuid() ) );
	begin( "net.notify_sending_file_client" );
	g_level->NetDriver = drv;
	drv->ServerConnection = conn;
	printf( "=>\t%d\n", (int)g_level->ULevel::NotifySendingFile( conn, FGuid() ) );
	begin( "net.accepting_channel_client_refuse" );
	g_level->NetDriver = drv;
	drv->ServerConnection = conn;
	chan->Connection = conn;
	conn->Driver = drv;
	chan->ChType = CHTYPE_Control;
	printf( "=>\t%d\n", (int)g_level->ULevel::NotifyAcceptingChannel( chan ) );
	begin( "net.accepting_channel_server_control" );
	g_level->NetDriver = drv;
	drv->ServerConnection = NULL;
	chan->Connection = conn;
	conn->Driver = drv;
	chan->ChIndex = 0;
	chan->ChType = CHTYPE_Control;
	printf( "=>\t%d\n", (int)g_level->ULevel::NotifyAcceptingChannel( chan ) );
	begin( "net.accepting_channel_server_file", "server=1 type=file" );
	g_level->NetDriver = drv;
	drv->ServerConnection = NULL;
	chan->Connection = conn;
	conn->Driver = drv;
	chan->ChType = CHTYPE_File;
	printf( "=>\t%d\n", (int)g_level->ULevel::NotifyAcceptingChannel( chan ) );
	begin( "net.accepting_channel_server_refuse" );
	g_level->NetDriver = drv;
	drv->ServerConnection = NULL;
	chan->Connection = conn;
	conn->Driver = drv;
	chan->ChIndex = 1;
	chan->ChType = CHTYPE_Actor;
	printf( "=>\t%d\n", (int)g_level->ULevel::NotifyAcceptingChannel( chan ) );
}

// Exec scenarios: the net and demo drivers claim their own tokens; DEMOREC runs the oracle's ConstructObject/InitListen path through the hooks.
// DEMOPLAY with a filename is not covered: its FURL(Base, text, type) constructor is Rust-backed (hp2_rs_engine_url_construct), outside the harness.
static void run_net_exec()
{
	UNetDriver* drv = (UNetDriver*)(void*)g_netdrv_mem;
	UNetDriver* demo = (UNetDriver*)(void*)g_demodrv_mem;

	net_begin( "net.exec_net_handled", "cmd=NETHANDLED driver=net" );
	g_level->NetDriver = drv;
	printf( "=>\t%d\n", (int)g_level->ULevel::Exec( TEXT("NETHANDLED"), g_sink ) );
	net_begin( "net.exec_demo_handled", "cmd=DEMOHANDLED driver=demo" );
	g_level->DemoRecDriver = demo;
	printf( "=>\t%d\n", (int)g_level->ULevel::Exec( TEXT("DEMOHANDLED"), g_sink ) );
	net_begin( "net.exec_unhandled", "cmd=FOO driver=net" );
	g_level->NetDriver = drv;
	printf( "=>\t%d\n", (int)g_level->ULevel::Exec( TEXT("FOO"), g_sink ) );
	net_begin( "net.exec_demorec_no_filename", "cmd=DEMOREC" );
	printf( "=>\t%d\n", (int)g_level->ULevel::Exec( TEXT("DEMOREC"), g_sink ) );
	net_begin( "net.exec_demorec_init_fail", "cmd=DEMOREC demo1 init=fail" );
	g_init_listen_fail = 1;
	g_init_listen_error = TEXT("Port busy");
	printf( "=>\t%d\n", (int)g_level->ULevel::Exec( TEXT("DEMOREC demo1"), g_sink ) );
	net_begin( "net.exec_demorec_started", "cmd=DEMOREC demo2.dem init=ok" );
	printf( "=>\t%d\n", (int)g_level->ULevel::Exec( TEXT("DEMOREC demo2.dem"), g_sink ) );
	net_begin( "net.exec_demoplay_no_filename", "cmd=DEMOPLAY" );
	printf( "=>\t%d\n", (int)g_level->ULevel::Exec( TEXT("DEMOPLAY"), g_sink ) );
}

// IsServer scenarios: a driver counts as a client only when it holds a ServerConnection; a missing driver never does.
static void run_net_is_server()
{
	UNetDriver* drv = (UNetDriver*)(void*)g_netdrv_mem;
	UNetDriver* demo = (UNetDriver*)(void*)g_demodrv_mem;
	UNetConnection* conn = (UNetConnection*)(void*)g_conn_mem;

	net_begin( "net.is_server_driver_server", "net=present net_conn=none demo=none" );
	g_level->NetDriver = drv;
	printf( "=>\t%d\n", (int)g_level->ULevel::IsServer() );
	net_begin( "net.is_server_driver_client", "net=present net_conn=set demo=none" );
	g_level->NetDriver = drv;
	drv->ServerConnection = conn;
	printf( "=>\t%d\n", (int)g_level->ULevel::IsServer() );
	net_begin( "net.is_server_demo_client", "net=none demo=present demo_conn=set" );
	g_level->DemoRecDriver = demo;
	demo->ServerConnection = conn;
	printf( "=>\t%d\n", (int)g_level->ULevel::IsServer() );
	net_begin( "net.is_server_demo_present", "net=present net_conn=none demo=present demo_conn=none" );
	g_level->NetDriver = drv;
	g_level->DemoRecDriver = demo;
	printf( "=>\t%d\n", (int)g_level->ULevel::IsServer() );
}
// Sets the game engine that Listen's CastChecked and ServerActors loop read: Client decides ListenServer versus DedicatedServer.
static UGameEngine* setup_game_engine( UClient* client )
{
	UGameEngine* game = (UGameEngine*)(void*)g_game_engine_mem;
	game->Class = UGameEngine::StaticClass();
	game->Client = client;
	g_level->Engine = (UEngine*)(void*)game;
	return game;
}
// Success-path state: a linked level with level info, a game engine, and a listen URL.
static UGameEngine* listen_ready( UClient* client )
{
	g_level->_Linker = (ULinkerLoad*)(void*)g_linker_mem;
	g_level->URL.Map = FString( TEXT("Entry") );
	level_with_levelinfo();
	return setup_game_engine( client );
}
static void print_listen( UBOOL ok, const FString& Error )
{
	printf( "=>\t%d\n", (int)ok );
	printf( "error_text\t%s\n", Error.Len() ? wide_to_utf8( *Error ).c_str() : "" );
	printf( "net_driver\t%d\n", g_level->NetDriver ? 1 : 0 );
	if( ok )
	{
		printf( "net_mode\t%d\n", (int)g_level->GetLevelInfo()->NetMode );
		printf( "countdown\t%g\n", g_level->GetLevelInfo()->NextSwitchCountdown );
	}
}
// Fake config property for the ServerActors ImportText loop: TFieldIterator matches it through the class Children list.
alignas(16) static char g_prop_mem[2][sizeof(UProperty)];
static UProperty* make_prop( int i, const TCHAR* name, DWORD flags )
{
	UProperty* p = (UProperty*)(void*)g_prop_mem[i];
	memset( p, 0, sizeof(g_prop_mem[i]) );
	p->Class = UProperty::StaticClass();
	p->Name = FName( name );
	p->PropertyFlags = flags;
	return p;
}
// Listen scenarios: NetAlready and the linker check return before any driver exists; the success path builds the master map, spawns each ServerActors helper, and applies the config fields.
static void run_net_listen()
{
	UNetDriver* drv = (UNetDriver*)(void*)g_netdrv_mem;
	UClient* client = (UClient*)(void*)g_client_mem;

	net_begin( "net.listen_already_listening", "net_driver=present" );
	g_level->NetDriver = drv;
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_no_linker", "net_driver=none linker=none" );
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_init_fail", "net_driver=none linker=set init=fail" );
	g_level->_Linker = (ULinkerLoad*)(void*)g_linker_mem;
	g_level->URL.Map = FString( TEXT("Entry") );
	g_init_listen_fail = 1;
	g_init_listen_error = TEXT("Port busy");
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_server_ok", "client=set server_actors=none pause=2.5" );
	listen_ready( client );
	g_init_listen_pause = 2.5f;
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_dedicated_ok", "client=none server_actors=none pause=2.5" );
	listen_ready( NULL );
	g_init_listen_pause = 2.5f;
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_server_actor_blank", "client=set server_actors=blank" );
	UGameEngine* game = listen_ready( client );
	game->ServerActors.AddItem( FString( TEXT("   ") ) );
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_server_actor_spawn_null", "client=set server_actors=Engine.Helper_Flag=5 spawn=null" );
	game = listen_ready( client );
	game->ServerActors.AddItem( FString( TEXT("Engine.Helper Flag=5") ) );
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_server_actor_bare_token", "client=set server_actors=Engine.Helper_Bare spawn=actor" );
	game = listen_ready( client );
	g_helper_actor = make_actor( 0, false );
	game->ServerActors.AddItem( FString( TEXT("Engine.Helper Bare") ) );
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_server_actor_config_field", "client=set server_actors=Engine.Helper_Flag=5 spawn=actor prop=Flag:config" );
	game = listen_ready( client );
	g_helper_actor = make_actor( 0, false );
	harness_actor_class()->Children = make_prop( 0, TEXT("Flag"), CPF_Config );
	game->ServerActors.AddItem( FString( TEXT("Engine.Helper Flag=5") ) );
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_server_actor_non_config_field", "client=set server_actors=Engine.Helper_Flag=5 spawn=actor prop=Flag:plain" );
	game = listen_ready( client );
	g_helper_actor = make_actor( 0, false );
	harness_actor_class()->Children = make_prop( 0, TEXT("Flag"), 0 );
	game->ServerActors.AddItem( FString( TEXT("Engine.Helper Flag=5") ) );
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }

	net_begin( "net.listen_server_actor_name_mismatch", "client=set server_actors=Engine.Helper_Flag=5 spawn=actor prop=Other:config" );
	game = listen_ready( client );
	g_helper_actor = make_actor( 0, false );
	harness_actor_class()->Children = make_prop( 0, TEXT("Other"), CPF_Config );
	game->ServerActors.AddItem( FString( TEXT("Engine.Helper Flag=5") ) );
	{ FString Error; UBOOL ok = g_level->ULevel::Listen( Error ); print_listen( ok, Error ); }
}
// Connection lifecycle and message callbacks.
static void run_net_connection()
{
	UNetDriver* drv = (UNetDriver*)(void*)g_netdrv_mem;
	UNetConnection* conn = (UNetConnection*)(void*)g_conn_mem;
	
	ALevelInfo* info;

	net_begin( "net.notify_accepting_connection_client", "net=present server_conn=set" );
	g_level->NetDriver = drv;
	drv->ServerConnection = conn;
	printf( "=>\t%d\n", (int)g_level->ULevel::NotifyAcceptingConnection() );

	net_begin( "net.notify_accepting_connection_switching", "net=present server_conn=none next_url=set" );
	g_level->NetDriver = drv;
	info = level_with_levelinfo();
	info->NextURL = FString( TEXT("Next") );
	printf( "=>\t%d\n", (int)g_level->ULevel::NotifyAcceptingConnection() );

	net_begin( "net.notify_accepting_connection_accept", "net=present server_conn=none next_url=empty" );
	g_level->NetDriver = drv;
	level_with_levelinfo();
	printf( "=>\t%d\n", (int)g_level->ULevel::NotifyAcceptingConnection() );

	net_begin( "net.notify_accepted_connection", "net=present server_conn=none address=fixed timestamp=fixed" );
	g_level->NetDriver = drv;
	conn->Driver = drv;
	g_level->ULevel::NotifyAcceptedConnection( conn );

	net_begin( "net.welcome_player_default", "optional=empty lone=0" );
	conn->Driver = drv;
	conn->PackageMap = (UPackageMap*)(void*)g_pkgmap_mem;
	g_level->Outer = (UObject*)(void*)g_sys;
	g_level->Outer->Name = FName( TEXT("Level") );
	level_with_levelinfo();
	g_level->ULevel::WelcomePlayer( conn, TEXT("") );

	net_begin( "net.welcome_player_optional", "optional=set lone=1" );
	conn->Driver = drv;
	conn->PackageMap = (UPackageMap*)(void*)g_pkgmap_mem;
	g_level->Outer = (UObject*)(void*)g_sys;
	g_level->Outer->Name = FName( TEXT("Level") );
	level_with_levelinfo()->bLonePlayer = 1;
	g_level->ULevel::WelcomePlayer( conn, TEXT("extra") );

	net_begin( "net.notify_received_text_userflag", "net=none text=USERFLAG_7" );
	g_level->ULevel::NotifyReceivedText( conn, TEXT("USERFLAG 7") );
	printf( "user_flags\t%d\n", conn->UserFlags );

	net_begin( "net.notify_received_text_server_hello", "net=present server_conn=none text=HELLO" );
	g_level->NetDriver = drv;
	info = level_with_levelinfo();
	info->Game = (AGameInfo*)(void*)g_game_mem;
	conn->Channels[0] = (UChannel*)(void*)g_chan_mem;
	g_level->ULevel::NotifyReceivedText( conn, TEXT("HELLO MINVER=219 VER=219") );
	net_begin( "net.notify_received_text_client_failure", "net=present server_conn=set client=present text=FAILURE" );
	g_level->NetDriver = drv;
	drv->ServerConnection = conn;
	{
		UEngine* engine = (UEngine*)(void*)g_engine_mem;
		UClient* client = (UClient*)(void*)g_client_mem;
		client->Viewports.AddItem( (UViewport*)(void*)g_viewport_mem );
		engine->Client = client;
		g_level->Engine = engine;
	}
	g_level->ULevel::NotifyReceivedText( conn, TEXT("FAILURE rejected") );
	net_begin( "net.notify_received_text_netspeed_high", "net=present rate=7000 max=5000" );
	g_level->NetDriver = drv;
	drv->MaxClientRate = 5000;
	g_level->ULevel::NotifyReceivedText( conn, TEXT("NETSPEED 7000") );
	printf( "net_speed\t%d\n", conn->CurrentNetSpeed );
	net_begin( "net.notify_received_text_netspeed_low", "net=present rate=1000 initial=0" );
	g_level->NetDriver = drv;
	drv->MaxClientRate = 5000;
	g_level->ULevel::NotifyReceivedText( conn, TEXT("NETSPEED 1000") );
	printf( "net_speed\t%d\n", conn->CurrentNetSpeed );
	net_begin( "net.notify_received_text_have", "package_map=set list=empty" );
	g_level->NetDriver = drv;
	conn->PackageMap = (UPackageMap*)(void*)g_pkgmap_mem;
	g_level->ULevel::NotifyReceivedText( conn, TEXT("HAVE GUID=00000000000000000000000000000000") );
	net_begin( "net.notify_received_text_skip", "package_map=set list=empty" );
	g_level->NetDriver = drv;
	conn->PackageMap = (UPackageMap*)(void*)g_pkgmap_mem;
	g_level->ULevel::NotifyReceivedText( conn, TEXT("SKIP GUID=00000000000000000000000000000000") );
	net_begin( "net.notify_received_text_login_missing_response", "net=present response=missing" );
	g_level->NetDriver = drv;
	info = level_with_levelinfo();
	info->Game = (AGameInfo*)(void*)g_game_mem;
	conn->Channels[0] = (UChannel*)(void*)g_chan_mem;
	g_level->ULevel::NotifyReceivedText( conn, TEXT("LOGIN URL=?Name=Guest") );
	net_begin( "net.notify_received_text_login_precedence_false", "net=present challenge_result=1 response=0" );
	g_level->NetDriver = drv;
	g_level->Engine = (UEngine*)(void*)g_engine_mem;
	info = level_with_levelinfo();
	info->Game = (AGameInfo*)(void*)g_game_mem;
	conn->Channels[0] = (UChannel*)(void*)g_chan_mem;
	conn->RemoteVersion = 433;
	g_level->ULevel::NotifyReceivedText( conn, TEXT("LOGIN RESPONSE=0") );
	net_begin( "net.notify_received_text_login_success", "net=present challenge_result=1 response=1 prelogin=accept" );
	g_level->NetDriver = drv;
	g_level->Engine = (UEngine*)(void*)g_engine_mem;
	g_level->Outer = (UObject*)(void*)g_sys;
	g_level->Outer->Name = FName( TEXT("Level") );
	info = level_with_levelinfo();
	info->Game = (AGameInfo*)(void*)g_game_mem;
	conn->Driver = drv;
	conn->PackageMap = (UPackageMap*)(void*)g_pkgmap_mem;
	conn->RemoteVersion = 433;
	g_level->ULevel::NotifyReceivedText( conn, TEXT("LOGIN RESPONSE=1 URL=?Name=Guest") );
	net_begin( "net.notify_received_text_login_prelogin_reject", "net=present challenge_result=1 response=1 prelogin=reject failcode=Bad" );
	g_level->NetDriver = drv;
	g_level->Engine = (UEngine*)(void*)g_engine_mem;
	info = level_with_levelinfo();
	info->Game = (AGameInfo*)(void*)g_game_mem;
	conn->Driver = drv;
	conn->Channels[0] = (UChannel*)(void*)g_chan_mem;
	conn->RemoteVersion = 433;
	g_prelogin_error = TEXT("Denied");
	g_prelogin_failcode = TEXT("Bad");
	g_level->ULevel::NotifyReceivedText( conn, TEXT("LOGIN RESPONSE=1 URL=?Name=Guest") );

	net_begin( "net.notify_received_file", "package=0 error=empty skipped=0" );
	try { g_level->ULevel::NotifyReceivedFile( conn, 0, TEXT(""), 0 ); }
	catch( HarnessAbort& ) { printf( "=>\t0\n" ); }
}


int main()
{
	setvbuf( stdout, NULL, _IONBF, 0 );
	try
	{
		run_persist();
		run_core();
		run_net();
		run_net_exec();
		run_net_is_server();
		run_net_listen();
		run_net_connection();
	}
	catch( HarnessAbort& a )
	{
		printf( "harness_abort\t%d\n", a.kind );
		return 1;
	}
	return 0;
}
FFileManager* GFileManager = &g_file_manager;

// Original-object AStatLog / AStatLogFile / UCheckSumCommandlet fixture harness.
// gen_stat_log.sh rewrites the pinned AStatLog.cpp (P_GET_* marshalling becomes
// harness globals), links the ORIGINAL object against the boundary defined here,
// and runs the fixed case list. Every boundary call emits a trace token.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cwchar>
#include <cstdarg>
#include <string>
#include <vector>
#include <map>
#include <deque>
#include <csetjmp>
#define private public
#define protected public
#include "EnginePrivate.h"
#undef private
#undef protected

// ---------------------------------------------------------------- runtime globals
UBOOL GIsEditor = 0;
UBOOL GIsRequestingExit = 0;
BYTE GRegisterNative(INT, const Native&) { return 0; }

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

// Arguments read by the rewritten exec natives (stand in for P_GET_*).
const TCHAR* g_arg_str = NULL;
UObject* g_arg_obj = NULL;
FString* g_arg_strref = NULL;

struct HarnessAbort {};

// ---------------------------------------------------------------- trace + record
static std::string g_trace, g_in, g_out;
static std::string g_ret;
static int g_cpnull = 0;
static int g_gpr = 0;
static int g_gpr_rc = 0;
static int g_handle_seq = 0;
static int g_archive_seq = 0;

static void tok( const std::string& t )
{
	if( !g_trace.empty() ) g_trace += '|';
	g_trace += t;
}
static void kv( std::string& dst, const char* k, const std::string& v )
{
	if( !dst.empty() ) dst += ';';
	dst += k;
	dst += '=';
	dst += v;
}
static void IN( const char* k, const std::string& v ) { kv( g_in, k, v ); }
static void OUT( const char* k, const std::string& v ) { kv( g_out, k, v ); }

static std::string fmtS( const char* f, ... )
{
	char buf[8192];
	va_list ap;
	va_start( ap, f );
	vsnprintf( buf, sizeof buf, f, ap );
	va_end( ap );
	return buf;
}
static std::string nar( const TCHAR* s )
{
	std::string r;
	if( s ) for( ; *s; ++s ) r += (char)*s;
	return r;
}
static std::string nar( const std::string& s ) { return s; }
static std::wstring wide( const std::string& s )
{
	std::wstring w;
	for( unsigned char c : s ) w += (wchar_t)c;
	return w;
}
static std::string hexs( const unsigned char* p, size_t n )
{
	static const char* d = "0123456789abcdef";
	std::string r;
	for( size_t i = 0; i < n; ++i ) { r += d[p[i] >> 4]; r += d[p[i] & 15]; }
	return r;
}

// Printf-style formatter for the TU's TCHAR (wchar_t) format strings: %s takes a TCHAR*.
static std::wstring vfmt( const TCHAR* fmt, va_list ap )
{
	std::wstring f;
	for( const TCHAR* p = fmt; *p; ++p )
	{
		if( p[0] == L'%' && p[1] == L's' ) { f += L"%ls"; ++p; }
		else if( p[0] == L'%' && p[1] == L'%' ) { f += L"%%"; ++p; }
		else f += *p;
	}
	std::vector<wchar_t> buf( 16384 );
	vswprintf( buf.data(), buf.size(), f.c_str(), ap );
	return buf.data();
}

// ---------------------------------------------------------------- output devices
class HarnessNullDev : public FOutputDevice
{
public:
	void Serialize( const TCHAR* V, EName Event ) {}
};
static HarnessNullDev g_dev_null;

class HarnessLogDev : public FOutputDevice
{
public:
	void Serialize( const TCHAR* V, EName Event ) { tok( "DBG(" + nar( V ) + ")" ); }
};
static HarnessLogDev g_dev_log;

class HarnessWarnDev : public FFeedbackContext
{
public:
	void Serialize( const TCHAR* V, EName Event ) { tok( "LOG(" + nar( V ) + ")" ); }
	FString GetContext() { tok( "UNEXPECTED(GetContext)" ); throw HarnessAbort(); }
	UBOOL YesNof( const TCHAR* Fmt, ... ) { tok( "UNEXPECTED(YesNof)" ); throw HarnessAbort(); }
	void BeginSlowTask( const TCHAR* Task, UBOOL StatusWindow, UBOOL Cancelable ) { tok( "UNEXPECTED(BeginSlowTask)" ); throw HarnessAbort(); }
	void EndSlowTask() { tok( "UNEXPECTED(EndSlowTask)" ); throw HarnessAbort(); }
	UBOOL VARARGS StatusUpdatef( INT Numerator, INT Denominator, const TCHAR* Fmt, ... ) { tok( "UNEXPECTED(StatusUpdatef)" ); throw HarnessAbort(); }
	void SetContext( FContextSupplier* InSupplier ) { tok( "UNEXPECTED(SetContext)" ); throw HarnessAbort(); }
};
static HarnessWarnDev g_dev_warn;

class HarnessErrorDev : public FOutputDeviceError
{
public:
	void Serialize( const TCHAR* V, EName Event ) { tok( "ERR(" + nar( V ) + ")" ); throw HarnessAbort(); }
	void HandleError() { tok( "UNEXPECTED(HandleError)" ); throw HarnessAbort(); }
};
static HarnessErrorDev g_dev_error;

// ---------------------------------------------------------------- archives
class HarnessArchive : public FArchive
{
public:
	int id;
	explicit HarnessArchive( int i ) : id( i ) {}
	~HarnessArchive() { tok( fmtS( "ARD(a%d)", id ) ); }
	void Serialize( void* V, INT Length )
	{
		tok( fmtS( "SER(a%d,", id ) + hexs( (const unsigned char*)V, (size_t)Length ) + ")" );
	}
	void Flush() { tok( fmtS( "FLU(a%d)", id ) ); }
};

// ---------------------------------------------------------------- file manager
static std::map<std::string, INT> g_files;

class HarnessFileMgr : public FFileManager
{
public:
	FArchive* CreateFileReader( const TCHAR* Filename, DWORD ReadFlags, FOutputDevice* Error ) { tok( "UNEXPECTED(CreateFileReader)" ); throw HarnessAbort(); }
	FArchive* CreateFileWriter( const TCHAR* Filename, DWORD WriteFlags, FOutputDevice* Error )
	{
		int id = ++g_archive_seq;
		tok( "CFW(" + nar( Filename ) + ")=a" + fmtS( "%d", id ) );
		return new HarnessArchive( id );
	}
	INT FileSize( const TCHAR* Filename )
	{
		std::string n = nar( Filename );
		INT sz = -1;
		auto it = g_files.find( n );
		if( it != g_files.end() ) sz = it->second;
		tok( "FSZ(" + n + ")=" + fmtS( "%d", sz ) );
		return sz;
	}
	UBOOL Delete( const TCHAR* Filename, UBOOL RequireExists, UBOOL EvenReadOnly ) { tok( "UNEXPECTED(Delete)" ); throw HarnessAbort(); }
	UBOOL Copy( const TCHAR* Dest, const TCHAR* Src, UBOOL Replace, UBOOL EvenIfReadOnly, UBOOL Attributes, void (*Progress)(FLOAT Fraction) ) { tok( "UNEXPECTED(Copy)" ); throw HarnessAbort(); }
	UBOOL Move( const TCHAR* Dest, const TCHAR* Src, UBOOL Replace, UBOOL EvenIfReadOnly, UBOOL Attributes )
	{
		tok( "MV(" + nar( Dest ) + "," + nar( Src ) + ")" );
		return 1;
	}
	SQWORD GetGlobalTime( const TCHAR* Filename ) { tok( "UNEXPECTED(GetGlobalTime)" ); throw HarnessAbort(); }
	UBOOL SetGlobalTime( const TCHAR* Filename ) { tok( "UNEXPECTED(SetGlobalTime)" ); throw HarnessAbort(); }
	UBOOL MakeDirectory( const TCHAR* Path, UBOOL Tree )
	{
		tok( "MKD(" + nar( Path ) + ")" );
		return 1;
	}
	UBOOL DeleteDirectory( const TCHAR* Path, UBOOL RequireExists, UBOOL Tree ) { tok( "UNEXPECTED(DeleteDirectory)" ); throw HarnessAbort(); }
	TArray<FString> FindFiles( const TCHAR* Filename, UBOOL Files, UBOOL Directories ) { tok( "UNEXPECTED(FindFiles)" ); throw HarnessAbort(); }
	UBOOL SetDefaultDirectory( const TCHAR* Filename ) { tok( "UNEXPECTED(SetDefaultDirectory)" ); throw HarnessAbort(); }
	FString GetDefaultDirectory() { return FString( L"/work/" ); }
};
static HarnessFileMgr g_dev_files;

FOutputDevice* GLog = &g_dev_log;
FOutputDevice* GNull = &g_dev_null;
FFeedbackContext* GWarn = &g_dev_warn;
FOutputDeviceError* GError = &g_dev_error;
FFileManager* GFileManager = &g_dev_files;

// ---------------------------------------------------------------- formatting entry points
void FOutputDevice::Logf( const TCHAR* Fmt, ... )
{
	va_list ap;
	va_start( ap, Fmt );
	std::wstring s = vfmt( Fmt, ap );
	va_end( ap );
	Serialize( s.c_str(), NAME_None );
}
void FOutputDevice::Logf( EName Event, const TCHAR* Fmt, ... )
{
	va_list ap;
	va_start( ap, Fmt );
	std::wstring s = vfmt( Fmt, ap );
	va_end( ap );
	Serialize( s.c_str(), Event );
}
FString FString::Printf( const TCHAR* Fmt, ... )
{
	va_list ap;
	va_start( ap, Fmt );
	std::wstring s = vfmt( Fmt, ap );
	va_end( ap );
	return FString( s.c_str() );
}

// ---------------------------------------------------------------- process / URL boundary
void* appCreateProc( const TCHAR* URL, const TCHAR* Parms, UBOOL bShow )
{
	int h = ++g_handle_seq;
	bool isnull = g_cpnull != 0;
	tok( "CP(" + nar( URL ) + "," + nar( Parms ) + "," + fmtS( "%d", bShow ? 1 : 0 ) + ")=h" + fmtS( "%d", isnull ? 0 : h ) );
	return isnull ? NULL : (void*)(uintptr_t)h;
}
UBOOL appGetProcReturnCode( void* ProcHandle, INT* ReturnCode )
{
	std::string hid = fmtS( "h%d", (int)(uintptr_t)ProcHandle );
	if( !g_gpr )
	{
		tok( "GPR(" + hid + ")=0" );
		return 0;
	}
	tok( "GPR(" + hid + ")=1/" + fmtS( "%d", g_gpr_rc ) );
	if( ReturnCode ) *ReturnCode = g_gpr_rc;
	return 1;
}
void appLaunchURL( const TCHAR* URL, const TCHAR* Parms, FString* Error )
{
	tok( "LU(" + nar( URL ) + ")" );
}

// ---------------------------------------------------------------- assert / unwind
void VARARGS appFailAssert( const ANSICHAR* Expr, const ANSICHAR* File, INT Line )
{
	tok( std::string( "ASSERT(" ) + Expr + ")" );
	throw HarnessAbort();
}
void VARARGS appUnwindf( const TCHAR* Fmt, ... )
{
}

// ---------------------------------------------------------------- object model
static std::map<const void*, std::string> g_fullname;   // object -> "Class Pkg.Name" / "Package Pkg"
static void set_super( UClass* c, UClass* s ) { c->SuperField = (UField*)s; }
static std::map<std::string, UClass*> g_cls;
static std::map<std::string, UPackage*> g_pkg;
static std::deque<std::wstring> g_name_ring;

static UPackage* pkg_obj( const std::string& p )
{
	auto it = g_pkg.find( p );
	if( it != g_pkg.end() ) return it->second;
	UPackage* o = (UPackage*)calloc( 1, sizeof(UPackage) );
	o->Class = UPackage::PrivateStaticClass;
	g_fullname[o] = "Package " + p;
	g_pkg[p] = o;
	return o;
}
static void set_vptr( void* obj, void** vt );
static void** cls_vt();
static UClass* klass( const std::string& full )
{
	auto it = g_cls.find( full );
	if( it != g_cls.end() ) return it->second;
	size_t dot = full.find( '.' );
	std::string pkg = full.substr( 0, dot );
	UClass* c = (UClass*)calloc( 1, sizeof(UClass) );
	set_vptr( c, cls_vt() );
	c->Class = UClass::PrivateStaticClass;
	c->Outer = pkg_obj( pkg );
	g_fullname[c] = "Class " + full;
	g_cls[full] = c;
	return c;
}

// Engine objects: calloc'd, with a harness vtable where virtual calls reach them.
static void* g_vt[256];
static void harness_process_event( void* self, UFunction* Function, void* Parms, void* Result )
{
	uintptr_t idx = (uintptr_t)Function;
	if( idx == 1 )
	{
		AStatLog_eventLogGameSpecial_Parms* p = (AStatLog_eventLogGameSpecial_Parms*)Parms;
		tok( "EV(" + nar( *p->SpecialID ) + "," + nar( *p->SpecialParam ) + ")" );
	}
	else if( idx == 2 )
	{
		AStatLog_eventLogGameSpecial2_Parms* p = (AStatLog_eventLogGameSpecial2_Parms*)Parms;
		tok( "EV2(" + nar( *p->SpecialID ) + "," + nar( *p->SpecialParam ) + "," + nar( *p->SpecialParam2 ) + ")" );
	}
	else tok( "UNEXPECTED(ProcessEvent)" ), throw HarnessAbort();
}
static size_t pe_slot()
{
	void ( UObject::*pm )( UFunction*, void*, void* ) = &UObject::ProcessEvent;
	uintptr_t W[2];
	memcpy( W, &pm, sizeof W );
	return W[0] / sizeof(void*);
}
static void set_vptr( void* obj, void** vt ) { memcpy( obj, &vt, sizeof vt ); }
// UClass vtable: GetPropertiesSize is virtual (UStruct), so GetDefaultObject's check() dispatches through the vptr.
static INT cls_props_size( void* self )
{
	return ((UStruct*)self)->PropertiesSize;
}
static void cls_unexpected()
{
	fprintf( stderr, "STUB UClass virtual slot\n" );
	abort();
}
static size_t props_slot()
{
	INT ( UStruct::*pm )() = &UStruct::GetPropertiesSize;
	uintptr_t W[2];
	memcpy( W, &pm, sizeof W );
	if( !( W[1] & 1 ) || W[0] >= 256 * sizeof(void*) )
	{
		fprintf( stderr, "STUB GetPropertiesSize not virtual\n" );
		abort();
	}
	return W[0] / sizeof(void*);
}
static void** cls_vt()
{
	static void* vt[256];
	static bool init = false;
	if( !init )
	{
		for( void*& s : vt ) s = (void*)&cls_unexpected;
		vt[props_slot()] = (void*)&cls_props_size;
		init = true;
	}
	return vt;
}

static FName ENGINE_LogGameSpecial_dummy;

// ---- UObject boundary (virtual/non-virtual members used by the TU) ----
const TCHAR* UObject::GetFullName( TCHAR* Str ) const
{
	auto it = g_fullname.find( this );
	std::string n = it == g_fullname.end() ? std::string( "Object" ) : it->second;
	g_name_ring.push_back( wide( n ) );
	return g_name_ring.back().c_str();
}
UFunction* UObject::FindFunctionChecked( FName InName, UBOOL Global )
{
	return (UFunction*)(uintptr_t)InName.Index;
}
void UObject::SaveConfig( DWORD Flags, const TCHAR* Filename )
{
	tok( "SC" );
}
void UObject::ProcessEvent( UFunction* Function, void* Parms, void* Result )
{
	harness_process_event( this, Function, Parms, Result );
}

// ---------------------------------------------------------------- engine boundary
FName ENGINE_LogGameSpecial;
FName ENGINE_LogGameSpecial2;

// Class storage: pointer-form statics, filled in main().
UClass* UObject::PrivateStaticClass = NULL;
UClass* UClass::PrivateStaticClass = NULL;
UClass* UPackage::PrivateStaticClass = NULL;
UClass* UCommandlet::PrivateStaticClass = NULL;
UClass* AActor::PrivateStaticClass = NULL;
UClass* AInfo::PrivateStaticClass = NULL;
UClass* AStatLog::PrivateStaticClass = NULL;
UClass* AStatLogFile::PrivateStaticClass = NULL;
UClass* ALevelInfo::PrivateStaticClass = NULL;
UClass* AGameInfo::PrivateStaticClass = NULL;
UClass* APawn::PrivateStaticClass = NULL;
UClass* APlayerPawn::PrivateStaticClass = NULL;
UClass* APlayerReplicationInfo::PrivateStaticClass = NULL;
UClass* AMutator::PrivateStaticClass = NULL;
UPackage* g_engine_pkg = NULL;
TArray<UObject*> UObject::GObjObjects;
// Core globals read by the TU: real value from Core/Src/Core.cpp:13; Env is the __Context jmp_buf storage.
INT PACKAGE_FILE_VERSION = 79;
jmp_buf __Context::Env;

// ---------------------------------------------------------------- engine object construction
static AStatLog* g_stat = NULL;
static AStatLogFile* g_file[3] = { NULL, NULL, NULL };
static ALevelInfo* g_level = NULL;
static AGameInfo* g_game = NULL;
static APlayerPawn* g_pawn = NULL;
static APlayerReplicationInfo* g_pri = NULL;
static AMutator* g_mut = NULL;
static FString g_out_checksum;

static AStatLog* def_obj()
{
	return (AStatLog*)AStatLog::StaticClass()->GetDefaultObject();
}
static void setS( FString& d, const std::string& s ) { d = FString( wide( s ).c_str() ); }
static FString* g_fs_dummy = NULL;

static UObject* obj_of( UClass* c, size_t size, void** vt )
{
	void* o = calloc( 1, size );
	if( vt ) set_vptr( o, vt );
	(void)c;
	return (UObject*)o;
}

// Per-case frame for native calls: Object/Node/Code, vptr borrowed from a harness FOutputDevice.
static FFrame* frame_for( UObject* self )
{
	UStruct* node = (UStruct*)calloc( 1, sizeof(UStruct) );
	node->Script.Add( 1 );
	FFrame* f = (FFrame*)calloc( 1, sizeof(FFrame) );
	void* vt = *(void**)&g_dev_log;
	*(void**)f = vt;
	f->Object = self;
	f->Node = node;
	f->Code = &node->Script( 0 );
	return f;
}

// ---------------------------------------------------------------- case helpers
static void reset_case()
{
	g_trace.clear();
	g_in.clear();
	g_out.clear();
	g_ret = "-";
	g_cpnull = 0;
	g_gpr = 0;
	g_gpr_rc = 0;
	GIsRequestingExit = 0;
	g_files.clear();
	g_out_checksum = FString();
	g_arg_strref = &g_out_checksum;
	g_arg_obj = NULL;
	g_arg_str = L"";
	IN( "lparams", "lp" );
	IN( "bwatermark", "0" );
}
static void in_dedicated( int d )
{
	g_level->NetMode = d ? NM_DedicatedServer : NM_Standalone;
	IN( "dedicated", fmtS( "%d", d ) );
}
static void in_external( int e )
{
	g_game->bExternalBatcher = e ? 1 : 0;
	IN( "external", fmtS( "%d", e ) );
}
static void in_lfn( const char* v ) { setS( g_game->LocalLogFileName, v ); IN( "lfn", v ); }
static void in_lurl( const char* v ) { setS( g_stat->LocalBatcherURL, v ); IN( "lurl", v ); }
static void in_ldir( const char* v ) { setS( def_obj()->LocalLogDir, v ); IN( "ldir", v ); }
static void in_dlurl( const char* v ) { setS( def_obj()->LocalBatcherURL, v ); IN( "dlurl", v ); }
static void in_wurl( const char* v ) { setS( g_stat->WorldBatcherURL, v ); IN( "wurl", v ); }
static void in_wparams( const char* v ) { setS( g_stat->WorldBatcherParams, v ); IN( "wparams", v ); }
static void in_bwbe( int v ) { g_stat->bWorldBatcherError = v ? 1 : 0; IN( "bwbe", fmtS( "%d", v ) ); }
static void in_dbwbe( int v ) { def_obj()->bWorldBatcherError = v ? 1 : 0; IN( "dbwbe", fmtS( "%d", v ) ); }
static void in_cpnull( int v ) { g_cpnull = v; IN( "cpnull", fmtS( "%d", v ) ); }
static void in_gpr( int v ) { g_gpr = v; IN( "gpr", fmtS( "%d", v ) ); }
static void in_rc( int v ) { g_gpr_rc = v; IN( "rc", fmtS( "%d", v ) ); }
static void in_bworld( AStatLogFile* f, int v ) { f->bWorld = v ? 1 : 0; IN( "bworld", fmtS( "%d", v ) ); }
static void in_slfile( AStatLogFile* f, const char* v ) { setS( f->StatLogFile, v ); IN( "slfile", v ); }
static void in_slfinal( AStatLogFile* f, const char* v ) { setS( f->StatLogFinal, v ); IN( "slfinal", v ); }
static void in_ev( const char* v )
{
	static std::wstring buf;
	buf = wide( v );
	g_arg_str = buf.c_str();
	IN( "ev", v );
}
static void in_file_cls( const char* v ) { (void)v; }
static void in_files( const std::string& spec )
{
	IN( "files", spec );
	size_t pos = 0;
	while( pos < spec.size() )
	{
		size_t comma = spec.find( ',', pos );
		std::string item = spec.substr( pos, comma == std::string::npos ? std::string::npos : comma - pos );
		size_t colon = item.rfind( ':' );
		if( colon != std::string::npos )
			g_files[item.substr( 0, colon )] = atoi( item.substr( colon + 1 ).c_str() );
		if( comma == std::string::npos ) break;
		pos = comma + 1;
	}
}
static void in_classes( const std::string& spec )
{
	IN( "classes", spec );
	UObject::GObjObjects.Empty();
	size_t pos = 0;
	while( pos < spec.size() )
	{
		size_t comma = spec.find( ',', pos );
		std::string item = spec.substr( pos, comma == std::string::npos ? std::string::npos : comma - pos );
		UObject::GObjObjects.AddItem( klass( item ) );
		if( comma == std::string::npos ) break;
		pos = comma + 1;
	}
}
static void in_gameclass( const char* v )
{
	g_game->Class = klass( v );
	IN( "gameclass", v );
}

static void call_hgs( UObject* )
{
}

// ---------------------------------------------------------------- cases
static void c_local_show()
{
	in_dedicated( 0 );
	in_lfn( "Game.log" );
	in_lurl( "/bin/batcher" );
	g_stat->execExecuteLocalLogBatcher( *frame_for( g_stat ), NULL );
}
static void c_local_dedicated()
{
	in_dedicated( 1 );
	in_lfn( "Game.log" );
	in_lurl( "/bin/batcher" );
	g_stat->execExecuteLocalLogBatcher( *frame_for( g_stat ), NULL );
}
static void c_silent_show()
{
	in_dedicated( 0 );
	in_lfn( "Game.log" );
	in_lurl( "/bin/batcher" );
	g_stat->execExecuteSilentLogBatcher( *frame_for( g_stat ), NULL );
}
static void c_batch_local()
{
	in_dedicated( 0 );
	in_dlurl( "/bin/dl" );
	in_ldir( "/logs" );
	g_stat->execBatchLocal( *frame_for( g_stat ), NULL );
}
static void c_browse_relative()
{
	IN( "url", "Maps/A.u" );
	g_arg_str = L"Maps/A.u";
	g_stat->execBrowseRelativeLocalURL( *frame_for( g_stat ), NULL );
}
static void world_setup( int ext )
{
	in_dedicated( 0 );
	in_external( ext );
	in_wurl( "/bin/wb" );
	in_wparams( "wp" );
}
static void c_world_first()
{
	world_setup( 0 );
	in_bwbe( 0 );
	in_dbwbe( 0 );
	in_cpnull( 0 );
	g_stat->execExecuteWorldLogBatcher( *frame_for( g_stat ), NULL );
}
static void c_world_running()
{
	world_setup( 0 );
	in_bwbe( 0 );
	in_dbwbe( 0 );
	in_cpnull( 0 );
	in_gpr( 0 );
	g_stat->execExecuteWorldLogBatcher( *frame_for( g_stat ), NULL );
}
static void c_world_prev_fail()
{
	world_setup( 0 );
	in_bwbe( 0 );
	in_dbwbe( 0 );
	in_cpnull( 0 );
	in_gpr( 1 );
	in_rc( -1 );
	g_stat->execExecuteWorldLogBatcher( *frame_for( g_stat ), NULL );
}
static void c_world_prev_ok()
{
	world_setup( 0 );
	in_bwbe( 0 );
	in_dbwbe( 0 );
	in_cpnull( 0 );
	in_gpr( 1 );
	in_rc( 0 );
	g_stat->execExecuteWorldLogBatcher( *frame_for( g_stat ), NULL );
}
static void c_world_create_fail()
{
	world_setup( 0 );
	in_bwbe( 0 );
	in_dbwbe( 0 );
	in_cpnull( 1 );
	in_gpr( 1 );
	in_rc( 0 );
	g_stat->execExecuteWorldLogBatcher( *frame_for( g_stat ), NULL );
}
static void c_world_external()
{
	in_external( 1 );
	in_bwbe( 1 );
	in_dbwbe( 0 );
	g_stat->execExecuteWorldLogBatcher( *frame_for( g_stat ), NULL );
}
static void c_initial_basic()
{
	g_arg_obj = g_game;
	in_gameclass( "Engine.GameInfo" );
	in_classes( "Engine.GameInfo,Engine.Actor,Core.Object" );
	in_files( "Engine.u:1234,Engine.dylib:777,Core.u:55" );
	g_stat->execInitialCheck( *frame_for( g_stat ), NULL );
}
static void c_initial_dedup()
{
	g_arg_obj = g_game;
	in_gameclass( "Engine.GameInfo" );
	in_classes( "Engine.GameInfo,Engine.Actor" );
	in_files( "Engine.u:1234,Engine.dylib:777" );
	g_stat->execInitialCheck( *frame_for( g_stat ), NULL );
}
static void c_log_mutator()
{
	IN( "mutclass", "Engine.Mutator" );
	g_mut->Class = klass( "Engine.Mutator" );
	g_arg_obj = g_mut;
	g_stat->execLogMutator( *frame_for( g_stat ), NULL );
}
static void c_pcs_nosecret()
{
	setS( g_pawn->ngWorldSecret, "" );
	setS( g_pri->PlayerName, "Player One" );
	IN( "secret", "" );
	IN( "pname", "Player One" );
	g_arg_obj = g_pawn;
	g_stat->execGetPlayerChecksum( *frame_for( g_stat ), NULL );
	(void)0;
}
static void c_pcs_secret()
{
	setS( g_pawn->ngWorldSecret, "xyz" );
	setS( g_pri->PlayerName, "Player One" );
	IN( "secret", "xyz" );
	IN( "pname", "Player One" );
	g_arg_obj = g_pawn;
	g_stat->execGetPlayerChecksum( *frame_for( g_stat ), NULL );
}
static void file_setup( int inst )
{
	IN( "inst", fmtS( "%d", inst ) );
}
static void c_open_nonworld()
{
	file_setup( 1 );
	in_bworld( g_file[1], 0 );
	in_slfile( g_file[1], "Logs/x.log" );
	g_file[1]->execOpenLog( *frame_for( g_file[1] ), NULL );
}
static void c_filelog_nonworld()
{
	file_setup( 1 );
	in_bworld( g_file[1], 0 );
	in_ev( "Hello" );
	g_file[1]->execFileLog( *frame_for( g_file[1] ), NULL );
}
static void c_fileflush()
{
	file_setup( 1 );
	g_file[1]->execFileFlush( *frame_for( g_file[1] ), NULL );
}
static void c_close_nonworld()
{
	file_setup( 1 );
	in_bworld( g_file[1], 0 );
	in_slfile( g_file[1], "Logs/x.log" );
	in_slfinal( g_file[1], "Logs/x.final.log" );
	g_file[1]->execCloseLog( *frame_for( g_file[1] ), NULL );
}
static void c_open_world()
{
	file_setup( 1 );
	in_bworld( g_file[1], 1 );
	in_slfile( g_file[1], "Logs/w.log" );
	g_file[1]->execOpenLog( *frame_for( g_file[1] ), NULL );
}
static void c_watermark()
{
	file_setup( 1 );
	in_ev( "Mark" );
	g_file[1]->execWatermark( *frame_for( g_file[1] ), NULL );
}
static void c_get_checksum()
{
	file_setup( 1 );
	g_file[1]->execGetChecksum( *frame_for( g_file[1] ), NULL );
}
static void c_get_checksum_twice()
{
	file_setup( 1 );
	g_file[1]->execGetChecksum( *frame_for( g_file[1] ), NULL );
	g_file[1]->execGetChecksum( *frame_for( g_file[1] ), NULL );
}
static void c_filelog_world()
{
	file_setup( 1 );
	in_bworld( g_file[1], 1 );
	in_ev( "Hi" );
	g_file[1]->execFileLog( *frame_for( g_file[1] ), NULL );
}
static void c_watermark_nocontext()
{
	file_setup( 2 );
	in_ev( "Mark" );
	g_file[2]->execWatermark( *frame_for( g_file[2] ), NULL );
}
static void c_get_checksum_nocontext()
{
	file_setup( 2 );
	g_file[2]->execGetChecksum( *frame_for( g_file[2] ), NULL );
}
static void c_close_world()
{
	file_setup( 1 );
	in_bworld( g_file[1], 1 );
	in_slfile( g_file[1], "Logs/w.log" );
	in_slfinal( g_file[1], "Logs/w.final.log" );
	g_file[1]->execCloseLog( *frame_for( g_file[1] ), NULL );
}
INT HarnessChecksumCommandletMain( const TCHAR* Parms );
static void run_commandlet( const char* file, const std::string& files )
{
	in_files( files );
	IN( "file", file );
	static std::wstring buf;
	buf = wide( file );
	INT r = HarnessChecksumCommandletMain( buf.c_str() );
	g_ret = fmtS( "%d", r );
}
static void c_cs_backslash() { run_commandlet( "..\\Maps\\Engine.u", "..\\Maps\\Engine.u:4096" ); }
static void c_cs_slash() { run_commandlet( "Maps/Foo.u", "Maps/Foo.u:2048" ); }
static void c_cs_missing() { run_commandlet( "Nope.u", "Maps/Other.u:10" ); }

struct CaseDef
{
	const char* name;
	void ( *fn )();
	int defbwbe;
	int checksum;
};

// Engine state set once before the case loop.
static void setup_engine()
{
	// Class / package storage.
	UClass::PrivateStaticClass = (UClass*)calloc( 1, sizeof(UClass) );
	set_vptr( UClass::PrivateStaticClass, cls_vt() );
	UClass::PrivateStaticClass->Class = UClass::PrivateStaticClass;
	g_fullname[UClass::PrivateStaticClass] = "Class Core.Class";
	g_cls["Core.Class"] = UClass::PrivateStaticClass;
	UObject::PrivateStaticClass = klass( "Core.Object" );
	UPackage::PrivateStaticClass = klass( "Core.Package" );
	UCommandlet::PrivateStaticClass = klass( "Engine.Commandlet" );
	AActor::PrivateStaticClass = klass( "Engine.Actor" );
	AInfo::PrivateStaticClass = klass( "Engine.Info" );
	AStatLog::PrivateStaticClass = klass( "Engine.StatLog" );
	AStatLogFile::PrivateStaticClass = klass( "Engine.StatLogFile" );
	ALevelInfo::PrivateStaticClass = klass( "Engine.LevelInfo" );
	AGameInfo::PrivateStaticClass = klass( "Engine.GameInfo" );
	APawn::PrivateStaticClass = klass( "Engine.Pawn" );
	APlayerPawn::PrivateStaticClass = klass( "Engine.PlayerPawn" );
	APlayerReplicationInfo::PrivateStaticClass = klass( "Engine.PlayerReplicationInfo" );
	AMutator::PrivateStaticClass = klass( "Engine.Mutator" );
	g_engine_pkg = pkg_obj( "Engine" );
	for( auto& kv : g_pkg ) kv.second->Class = UPackage::PrivateStaticClass;

	set_super( klass( "Engine.StatLog" ), klass( "Engine.Info" ) );
	set_super( klass( "Engine.StatLogFile" ), klass( "Engine.StatLog" ) );
	set_super( klass( "Engine.Info" ), klass( "Engine.Actor" ) );
	set_super( klass( "Engine.Actor" ), klass( "Core.Object" ) );
	set_super( klass( "Engine.LevelInfo" ), klass( "Engine.Info" ) );
	set_super( klass( "Engine.GameInfo" ), klass( "Engine.Info" ) );
	set_super( klass( "Engine.Pawn" ), klass( "Engine.Actor" ) );
	set_super( klass( "Engine.PlayerPawn" ), klass( "Engine.Pawn" ) );
	set_super( klass( "Engine.PlayerReplicationInfo" ), klass( "Engine.Info" ) );
	set_super( klass( "Engine.Mutator" ), klass( "Engine.Info" ) );

	// Event function names: FindFunctionChecked returns the FName index as its sentinel.
	ENGINE_LogGameSpecial.Index = 1;
	ENGINE_LogGameSpecial2.Index = 2;

	// Vtable: only ProcessEvent is reached through the engine objects.
	g_vt[pe_slot()] = (void*)&harness_process_event;

	// Default object of AStatLog (class defaults buffer).
	UClass* sc = AStatLog::StaticClass();
	sc->PropertiesSize = sizeof(AStatLog);
	sc->Defaults.Add( sizeof(AStatLog) );
	memset( &sc->Defaults( 0 ), 0, sizeof(AStatLog) );
	set_vptr( &sc->Defaults( 0 ), g_vt );
	((UObject*)&sc->Defaults( 0 ))->Class = sc;

	g_stat = (AStatLog*)obj_of( sc, sizeof(AStatLog), g_vt );
	g_stat->Class = sc;
	g_file[1] = (AStatLogFile*)obj_of( AStatLogFile::StaticClass(), sizeof(AStatLogFile), g_vt );
	g_file[1]->Class = AStatLogFile::StaticClass();
	g_file[2] = (AStatLogFile*)obj_of( AStatLogFile::StaticClass(), sizeof(AStatLogFile), g_vt );
	g_file[2]->Class = AStatLogFile::StaticClass();
	g_level = (ALevelInfo*)obj_of( ALevelInfo::StaticClass(), sizeof(ALevelInfo), NULL );
	g_level->Class = ALevelInfo::StaticClass();
	g_game = (AGameInfo*)obj_of( AGameInfo::StaticClass(), sizeof(AGameInfo), NULL );
	g_game->Class = AGameInfo::StaticClass();
	g_pawn = (APlayerPawn*)obj_of( APlayerPawn::StaticClass(), sizeof(APlayerPawn), NULL );
	g_pawn->Class = APlayerPawn::StaticClass();
	g_pri = (APlayerReplicationInfo*)obj_of( APlayerReplicationInfo::StaticClass(), sizeof(APlayerReplicationInfo), NULL );
	g_pri->Class = APlayerReplicationInfo::StaticClass();
	g_mut = (AMutator*)obj_of( AMutator::StaticClass(), sizeof(AMutator), NULL );
	g_mut->Class = AMutator::StaticClass();

	g_stat->Level = g_level;
	g_level->Game = g_game;
	g_pawn->PlayerReplicationInfo = g_pri;
	g_stat->LocalBatcherParams = FString();
	setS( g_stat->LocalBatcherParams, "lp" );
	setS( g_stat->WorldBatcherURL, "/bin/wb" );
	setS( g_stat->WorldBatcherParams, "wp" );
	setS( def_obj()->LocalBatcherURL, "/bin/dl" );
	setS( def_obj()->LocalLogDir, "/logs" );
}

int main()
{
	setup_engine();

	static const CaseDef cases[] = {
		{ "LocalBatcher_show", c_local_show, 0, 0 },
		{ "LocalBatcher_dedicated", c_local_dedicated, 0, 0 },
		{ "SilentBatcher_show", c_silent_show, 0, 0 },
		{ "BatchLocal", c_batch_local, 0, 0 },
		{ "BrowseRelative", c_browse_relative, 0, 0 },
		{ "WorldBatcher_first", c_world_first, 1, 0 },
		{ "WorldBatcher_running", c_world_running, 1, 0 },
		{ "WorldBatcher_prev_fail", c_world_prev_fail, 1, 0 },
		{ "WorldBatcher_prev_ok", c_world_prev_ok, 1, 0 },
		{ "WorldBatcher_create_fail", c_world_create_fail, 1, 0 },
		{ "WorldBatcher_external", c_world_external, 1, 0 },
		{ "InitialCheck_basic", c_initial_basic, 0, 0 },
		{ "InitialCheck_dedup", c_initial_dedup, 0, 0 },
		{ "LogMutator", c_log_mutator, 0, 0 },
		{ "GetPlayerChecksum_nosecret", c_pcs_nosecret, 0, 1 },
		{ "GetPlayerChecksum_secret", c_pcs_secret, 0, 1 },
		{ "OpenLog_nonworld", c_open_nonworld, 0, 0 },
		{ "FileLog_nonworld", c_filelog_nonworld, 0, 0 },
		{ "FileFlush", c_fileflush, 0, 0 },
		{ "CloseLog_nonworld", c_close_nonworld, 0, 0 },
		{ "OpenLog_world", c_open_world, 0, 0 },
		{ "Watermark", c_watermark, 0, 0 },
		{ "GetChecksum", c_get_checksum, 0, 1 },
		{ "GetChecksum_twice", c_get_checksum_twice, 0, 1 },
		{ "FileLog_world", c_filelog_world, 0, 0 },
		{ "Watermark_nocontext", c_watermark_nocontext, 0, 0 },
		{ "GetChecksum_nocontext", c_get_checksum_nocontext, 0, 0 },
		{ "CloseLog_world", c_close_world, 0, 0 },
		{ "Checksum_backslash", c_cs_backslash, 0, 0 },
		{ "Checksum_slash", c_cs_slash, 0, 0 },
		{ "Checksum_missing", c_cs_missing, 0, 0 },
	};

	for( const CaseDef& c : cases )
	{
		reset_case();
		try
		{
			c.fn();
		}
		catch( HarnessAbort& )
		{
		}
		catch( ... )
		{
			tok( "UNEXPECTED(exception)" );
		}
		if( c.checksum )
			OUT( "Checksum", nar( *g_out_checksum ) );
		if( c.defbwbe )
			OUT( "DefBWBE", fmtS( "%d", def_obj()->bWorldBatcherError ? 1 : 0 ) );
		OUT( "GIsRequestingExit", fmtS( "%d", GIsRequestingExit ? 1 : 0 ) );
		printf( "case=%s in=%s ret=%s out=%s trace=%s\n",
			c.name,
			g_in.c_str(),
			g_ret.empty() ? "-" : g_ret.c_str(),
			g_out.c_str(),
			g_trace.empty() ? "-" : g_trace.c_str() );
	}
	return 0;
}


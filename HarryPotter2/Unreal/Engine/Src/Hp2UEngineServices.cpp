/*=============================================================================
	Hp2UEngineServices.cpp: UEngine services called by the Rust engine port
	(crates/hp2-engine/src/uengine.rs). Each adapter performs one operation
	the Rust code cannot express without C++ types, in the original order.
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "Hp2UEngineServices.h"
#include <string.h>

// Globals.

static INT EngineIsClient( void )
{
	return GIsClient;
}

static INT EngineIsEditor( void )
{
	return GIsEditor;
}

static INT EngineIsRunning( void )
{
	return GIsRunning;
}

static void EngineSetIsRunning( INT Value )
{
	GIsRunning = Value;
}

static INT EngineIsRequestingExit( void )
{
	return GIsRequestingExit;
}

static void* EngineGLog( void )
{
	return GLog;
}

// UEngine fields and virtual calls.

static INT EngineCacheSizeMegs( void* Engine )
{
	return ((UEngine*)Engine)->CacheSizeMegs;
}

static INT EngineUseSound( void* Engine )
{
	return ((UEngine*)Engine)->UseSound;
}

static void* EngineAudio( void* Engine )
{
	return ((UEngine*)Engine)->Audio;
}

static void EngineSetAudio( void* Engine, void* Value )
{
	((UEngine*)Engine)->Audio = (UAudioSubsystem*)Value;
}

static void* EngineRender( void* Engine )
{
	return ((UEngine*)Engine)->Render;
}

static void EngineSetRender( void* Engine, void* Value )
{
	((UEngine*)Engine)->Render = (URenderBase*)Value;
}

static void* EngineClient( void* Engine )
{
	return ((UEngine*)Engine)->Client;
}

static void EngineSetClient( void* Engine, void* Value )
{
	((UEngine*)Engine)->Client = (UClient*)Value;
}

static void EngineSetCurrentTickRate( void* Engine, FLOAT Value )
{
	((UEngine*)Engine)->CurrentTickRate = Value;
}

static void EngineCreateCylinder( void* Engine )
{
	((UEngine*)Engine)->Cylinder = new UCylinder;
}

static void EngineCreateOCylinder( void* Engine )
{
	((UEngine*)Engine)->OCylinder = new UOrientedCylinder;
}

static void EngineCreateOvalCylinder( void* Engine )
{
	((UEngine*)Engine)->OvalCylinder = new UAlignedOvalCylinder;
}

static void EngineCreateOOvalCylinder( void* Engine )
{
	((UEngine*)Engine)->OOvalCylinder = new UOrientedOvalCylinder;
}

static void EngineCreateBox( void* Engine )
{
	((UEngine*)Engine)->Box = new UBox;
}

static void EngineAddToRoot( void* Engine )
{
	((UEngine*)Engine)->AddToRoot();
}

static void EngineRemoveFromRoot( void* Engine )
{
	((UEngine*)Engine)->RemoveFromRoot();
}

static void EngineInit( void* Engine )
{
	((UEngine*)Engine)->Init();
}

static void EngineTick( void* Engine, FLOAT DeltaSeconds )
{
	guard(UpdateWorld);
	((UEngine*)Engine)->Tick( DeltaSeconds );
	unguard;
}

static FLOAT EngineGetMaxTickRate( void* Engine )
{
	return ((UEngine*)Engine)->GetMaxTickRate();
}

static void EngineFlush( void* Engine, INT AllowPrecache )
{
	((UEngine*)Engine)->Flush( AllowPrecache );
}

// Class loading and object construction.

static void* EngineLoadEngineClass( void )
{
	return UObject::StaticLoadClass( UEngine::StaticClass(), NULL, TEXT("ini:Engine.Engine.GameEngine"), NULL, LOAD_NoFail | LOAD_DisallowFiles, NULL );
}

static void* EngineConstructEngine( void* Class )
{
	return ConstructObject<UEngine>( (UClass*)Class );
}

static void* EngineLoadAudioClass( void )
{
	return UObject::StaticLoadClass( UAudioSubsystem::StaticClass(), NULL, TEXT("ini:Engine.Engine.AudioDevice"), NULL, LOAD_NoFail, NULL );
}

static void* EngineConstructAudio( void* Class )
{
	return ConstructObject<UAudioSubsystem>( (UClass*)Class );
}

static void EngineAudioDelete( void* Audio )
{
	guard(ExitSound);
	delete (UAudioSubsystem*)Audio;
	unguard;
}

static INT EngineAudioInit( void* Audio )
{
	return ((UAudioSubsystem*)Audio)->Init();
}

static void EngineLogAudioInitFailed( void )
{
	debugf( NAME_Log, TEXT("Audio initialization failed.") );
}

static void EngineLogEngineInitialized( void )
{
	debugf( NAME_Init, TEXT("Unreal engine initialized") );
}

static INT EngineParseCmdlineParam( const uint32_t* Param )
{
	return ParseParam( appCmdLine(), (const TCHAR*)Param );
}

// Subsystems.

static void* EngineGSys( void )
{
	return GSys;
}

static INT EngineGSysExec( void* Sys, const uint32_t* Cmd, void* Ar )
{
	return ((USystem*)Sys)->Exec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar );
}

static INT EngineStaticExec( const uint32_t* Cmd, void* Ar )
{
	return UObject::StaticExec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar );
}

static void EngineGCacheInit( INT Bytes, INT MaxItems )
{
	GCache.Init( Bytes, MaxItems );
}

static void EngineGCacheExit( INT FreeMemory )
{
	GCache.Exit( FreeMemory );
}

static void EngineGCacheFlush( void )
{
	GCache.Flush();
}

static INT EngineGCacheExec( const uint32_t* Cmd, void* Ar )
{
	return GCache.Exec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar );
}

static void* EngineGExec( void )
{
	return GExec;
}

static INT EngineGExecExec( void* Exec, const uint32_t* Cmd, void* Ar )
{
	return ((FExec*)Exec)->Exec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar );
}

static INT EngineClientExec( void* Client, const uint32_t* Cmd, void* Ar )
{
	return ((UClient*)Client)->Exec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar );
}

static void EngineClientFlush( void* Client, INT AllowPrecache )
{
	((UClient*)Client)->Flush( AllowPrecache );
}

static INT EngineRenderExec( void* Render, const uint32_t* Cmd, void* Ar )
{
	return ((URenderBase*)Render)->Exec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar );
}

static INT EngineAudioExec( void* Audio, const uint32_t* Cmd, void* Ar )
{
	return ((UAudioSubsystem*)Audio)->Exec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar );
}

static void EngineEngineMemInit( INT Bytes, const uint32_t* Name )
{
	GEngineMem.Init( Bytes, (const TCHAR*)Name );
}

static void EngineEngineMemExit( void )
{
	GEngineMem.Exit();
}

static void EngineUrlStaticInit( void )
{
	FURL::StaticInit();
}

static void EngineUrlStaticExit( void )
{
	FURL::StaticExit();
}

// Text, config and time.

static const uint32_t* EngineLocalize( const char* Section, const char* Key, const uint32_t* Package )
{
	return (const uint32_t*)Localize( Section, Key, (const TCHAR*)Package );
}

static uint32_t EngineAppToUpper( uint32_t Char )
{
	return (uint32_t)appToUpper( (TCHAR)Char );
}

static INT EngineAppStricmp( const uint32_t* A, const uint32_t* B )
{
	return appStricmp( (const TCHAR*)A, (const TCHAR*)B );
}

static INT EngineConfigGetLanguage( uint32_t* Buf, INT Len )
{
	return GConfig->GetString( TEXT("Engine.Engine"), TEXT("Language"), (TCHAR*)Buf, Len );
}

static void EngineSetLanguage( const uint32_t* Text )
{
	UObject::SetLanguage( (const TCHAR*)Text );
}

// The memcpy round-trips below assume FTime is exactly one 64-bit fixed-point word.
static_assert( sizeof(FTime) == sizeof(int64_t), "FTime layout" );

static int64_t EngineAppSeconds( void )
{
	FTime Now = appSeconds();
	int64_t Raw;
	memcpy( &Raw, &Now, sizeof(Raw) );
	return Raw;
}

static FLOAT EngineFTimeSub( int64_t A, int64_t B )
{
	FTime TimeA, TimeB;
	memcpy( (void*)&TimeA, &A, sizeof(A) );
	memcpy( (void*)&TimeB, &B, sizeof(B) );
	return TimeA - TimeB;
}

static void EngineAppSleep( FLOAT Seconds )
{
	guard(EnforceTickRate);
	appSleep( Seconds );
	unguard;
}

// Viewport, actor, HUD and input.

static void* EngineViewportConsole( void* Viewport )
{
	return ((UViewport*)Viewport)->Console;
}

static void* EngineViewportActor( void* Viewport )
{
	return ((UViewport*)Viewport)->Actor;
}

static void* EngineViewportInput( void* Viewport )
{
	return ((UViewport*)Viewport)->Input;
}

static void* EngineActorHud( void* Actor )
{
	return ((APlayerPawn*)Actor)->myHUD;
}

static void* EngineHudMainMenu( void* Hud )
{
	return ((AHUD*)Hud)->MainMenu;
}

static void* EngineObjectClass( void* Object )
{
	return ((UObject*)Object)->GetClass();
}

static void* EngineClassSuper( void* Class )
{
	return ((UClass*)Class)->GetSuperClass();
}

static const uint32_t* EngineClassName( void* Class )
{
	return (const uint32_t*)((UClass*)Class)->GetName();
}

static INT EngineConsoleEventKeyType( void* Console, INT Key )
{
	return ((UConsole*)Console)->eventKeyType( (BYTE)Key );
}

static INT EngineConsoleEventKeyEvent( void* Console, INT Key, INT State, FLOAT Delta )
{
	return ((UConsole*)Console)->eventKeyEvent( (BYTE)Key, (BYTE)State, Delta );
}

static void* EngineConsoleAsOutput( void* Console )
{
	return static_cast<FOutputDevice*>((UConsole*)Console);
}

static INT EngineInputPreProcess( void* Input, INT Key, INT State, FLOAT Delta )
{
	return ((UInput*)Input)->PreProcess( (EInputKey)Key, (EInputAction)State, Delta );
}

static INT EngineInputProcess( void* Input, void* Ar, INT Key, INT State, FLOAT Delta )
{
	return ((UInput*)Input)->Process( *(FOutputDevice*)Ar, (EInputKey)Key, (EInputAction)State, Delta );
}

// Output and command parsing.

static INT EngineParseCommand( const uint32_t** Stream, const uint32_t* Match )
{
	return ParseCommand( (const TCHAR**)Stream, (const TCHAR*)Match );
}

static void EngineOutputLog( void* Ar, const uint32_t* Text )
{
	((FOutputDevice*)Ar)->Log( (const TCHAR*)Text );
}

static void EngineOutputLogf( void* Ar, const uint32_t* Fmt )
{
	((FOutputDevice*)Ar)->Logf( (const TCHAR*)Fmt );
}

static void EngineOutputLogfS( void* Ar, const uint32_t* Fmt, const uint32_t* S )
{
	((FOutputDevice*)Ar)->Logf( (const TCHAR*)Fmt, (const TCHAR*)S );
}

static void EngineOutputLogfI( void* Ar, const uint32_t* Fmt, INT I )
{
	((FOutputDevice*)Ar)->Logf( (const TCHAR*)Fmt, I );
}

static void EngineOutputLogfIs( void* Ar, const uint32_t* Fmt, INT I, const uint32_t* S )
{
	((FOutputDevice*)Ar)->Logf( (const TCHAR*)Fmt, I, (const TCHAR*)S );
}

// URL handles. A handle owns the FURL built by the CRACKURL command and the
// String() result, so the text pointers returned below stay valid until
// EngineUrlFree.

struct EngineUrlHandle
{
	FURL URL;
	FString Str;

	EngineUrlHandle( const TCHAR* Cmd )
		: URL( NULL, Cmd, TRAVEL_Absolute )
	{
		if( URL.Valid )
			Str = URL.String();
	}
};

static void* EngineUrlCrack( const uint32_t* Cmd )
{
	return new EngineUrlHandle( (const TCHAR*)Cmd );
}

static void EngineUrlFree( void* Url )
{
	delete (EngineUrlHandle*)Url;
}

static INT EngineUrlValid( void* Url )
{
	return ((EngineUrlHandle*)Url)->URL.Valid;
}

static const uint32_t* EngineUrlProtocol( void* Url )
{
	return (const uint32_t*)*((EngineUrlHandle*)Url)->URL.Protocol;
}

static const uint32_t* EngineUrlHost( void* Url )
{
	return (const uint32_t*)*((EngineUrlHandle*)Url)->URL.Host;
}

static INT EngineUrlPort( void* Url )
{
	return ((EngineUrlHandle*)Url)->URL.Port;
}

static const uint32_t* EngineUrlMap( void* Url )
{
	return (const uint32_t*)*((EngineUrlHandle*)Url)->URL.Map;
}

static INT EngineUrlNumOptions( void* Url )
{
	return ((EngineUrlHandle*)Url)->URL.Op.Num();
}

static const uint32_t* EngineUrlOption( void* Url, INT Index )
{
	return (const uint32_t*)*((EngineUrlHandle*)Url)->URL.Op(Index);
}

static const uint32_t* EngineUrlPortal( void* Url )
{
	return (const uint32_t*)*((EngineUrlHandle*)Url)->URL.Portal;
}

// Original CRACKURL printed **URL.String(), which passes one TCHAR to %s
// (undefined behaviour). This passes the string itself.
static const uint32_t* EngineUrlString( void* Url )
{
	return (const uint32_t*)*((EngineUrlHandle*)Url)->Str;
}

// Every field is assigned by name, so a swapped entry cannot compile silently.
// constexpr keeps GHp2UEngineServices constant-initialized (no static-init order dependency).
static constexpr Hp2UEngineServices MakeServices()
{
	Hp2UEngineServices s = {};
	s.is_client = EngineIsClient;
	s.is_editor = EngineIsEditor;
	s.is_running = EngineIsRunning;
	s.set_is_running = EngineSetIsRunning;
	s.is_requesting_exit = EngineIsRequestingExit;
	s.glog = EngineGLog;
	s.cache_size_megs = EngineCacheSizeMegs;
	s.use_sound = EngineUseSound;
	s.audio = EngineAudio;
	s.set_audio = EngineSetAudio;
	s.render = EngineRender;
	s.set_render = EngineSetRender;
	s.client = EngineClient;
	s.set_client = EngineSetClient;
	s.set_current_tick_rate = EngineSetCurrentTickRate;
	s.create_cylinder = EngineCreateCylinder;
	s.create_ocylinder = EngineCreateOCylinder;
	s.create_oval_cylinder = EngineCreateOvalCylinder;
	s.create_ooval_cylinder = EngineCreateOOvalCylinder;
	s.create_box = EngineCreateBox;
	s.add_to_root = EngineAddToRoot;
	s.remove_from_root = EngineRemoveFromRoot;
	s.engine_init = EngineInit;
	s.engine_tick = EngineTick;
	s.engine_get_max_tick_rate = EngineGetMaxTickRate;
	s.engine_flush = EngineFlush;
	s.load_engine_class = EngineLoadEngineClass;
	s.construct_engine = EngineConstructEngine;
	s.load_audio_class = EngineLoadAudioClass;
	s.construct_audio = EngineConstructAudio;
	s.audio_delete = EngineAudioDelete;
	s.audio_init = EngineAudioInit;
	s.log_audio_init_failed = EngineLogAudioInitFailed;
	s.log_engine_initialized = EngineLogEngineInitialized;
	s.parse_cmdline_param = EngineParseCmdlineParam;
	s.gsys = EngineGSys;
	s.gsys_exec = EngineGSysExec;
	s.static_exec = EngineStaticExec;
	s.gcache_init = EngineGCacheInit;
	s.gcache_exit = EngineGCacheExit;
	s.gcache_flush = EngineGCacheFlush;
	s.gcache_exec = EngineGCacheExec;
	s.gexec = EngineGExec;
	s.gexec_exec = EngineGExecExec;
	s.client_exec = EngineClientExec;
	s.client_flush = EngineClientFlush;
	s.render_exec = EngineRenderExec;
	s.audio_exec = EngineAudioExec;
	s.engine_mem_init = EngineEngineMemInit;
	s.engine_mem_exit = EngineEngineMemExit;
	s.url_static_init = EngineUrlStaticInit;
	s.url_static_exit = EngineUrlStaticExit;
	s.localize = EngineLocalize;
	s.app_to_upper = EngineAppToUpper;
	s.app_stricmp = EngineAppStricmp;
	s.config_get_language = EngineConfigGetLanguage;
	s.set_language = EngineSetLanguage;
	s.app_seconds = EngineAppSeconds;
	s.ftime_sub = EngineFTimeSub;
	s.app_sleep = EngineAppSleep;
	s.viewport_console = EngineViewportConsole;
	s.viewport_actor = EngineViewportActor;
	s.viewport_input = EngineViewportInput;
	s.actor_hud = EngineActorHud;
	s.hud_main_menu = EngineHudMainMenu;
	s.object_class = EngineObjectClass;
	s.class_super = EngineClassSuper;
	s.class_name = EngineClassName;
	s.console_event_key_type = EngineConsoleEventKeyType;
	s.console_event_key_event = EngineConsoleEventKeyEvent;
	s.console_as_output = EngineConsoleAsOutput;
	s.input_pre_process = EngineInputPreProcess;
	s.input_process = EngineInputProcess;
	s.parse_command = EngineParseCommand;
	s.output_log = EngineOutputLog;
	s.output_logf = EngineOutputLogf;
	s.output_logf_s = EngineOutputLogfS;
	s.output_logf_i = EngineOutputLogfI;
	s.output_logf_is = EngineOutputLogfIs;
	s.url_crack = EngineUrlCrack;
	s.url_free = EngineUrlFree;
	s.url_valid = EngineUrlValid;
	s.url_protocol = EngineUrlProtocol;
	s.url_host = EngineUrlHost;
	s.url_port = EngineUrlPort;
	s.url_map = EngineUrlMap;
	s.url_num_options = EngineUrlNumOptions;
	s.url_option = EngineUrlOption;
	s.url_portal = EngineUrlPortal;
	s.url_string = EngineUrlString;
	return s;
}

const Hp2UEngineServices GHp2UEngineServices = MakeServices();
// Compile-time proof that the table is constant-initialized; fails to compile otherwise.
[[maybe_unused]] static constexpr Hp2UEngineServices kServicesConstantCheck = MakeServices();

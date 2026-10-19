// Fixture driver for the ORIGINAL UnCamMgr.cpp (UClient) at the pinned ref.
// Each record prints the exact inputs and outputs so the Rust replay can assert
// bit-exact results. The TU code (GetEffectiveFovAngle, PostEditChange, Exec,
// Flush) is the original; the doubles below stand in for engine callees:
//   - ParseCommand: Core command match (skip blanks, case-insensitive keyword,
//     then blank or end of string).
//   - UEngine::Flush, URenderDevice::Flush, AActor::ProcessEvent: vtable doubles.
//   - UObject::SaveConfig, UObject::PostEditChange, UObject::FindFunctionChecked:
//     direct doubles.
//   - FString::Printf: formats the wide text with the original argument. The
//     ProcessEvent double records the text eventClientMessage delivers.
//   - appTan/appAtan/appStrlen/appStrcpy/appMemcpy: Core helpers (libm/libc).

#include "EnginePrivate.h"
#include "UnRender.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdarg.h>
#include <wchar.h>
#include <math.h>
#include <limits.h>

class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { return malloc( Count ? Count : 1 ); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag ) { return realloc( Original, Count ? Count : 1 ); }
	void Free( void* Original ) { free( Original ); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc GHarnessMalloc;
FMalloc* GMalloc = &GHarnessMalloc;

// Core helpers the TU reaches through FString and appTan/appAtan.
void appMemcpy( void* Dest, const void* Src, INT Count ) { memcpy( Dest, Src, Count ); }
INT appStrlen( const TCHAR* String ) { return (INT)wcslen( String ); }
TCHAR* appStrcpy( TCHAR* Dest, const TCHAR* Src ) { return wcscpy( Dest, Src ); }
DOUBLE appTan( DOUBLE Value ) { return tan( Value ); }
DOUBLE appAtan( DOUBLE Value ) { return atan( Value ); }

// ---- trace of engine-side effects, one comma-separated token per effect ----
static char g_trace[512];
static void trace_clear( void ) { g_trace[0] = 0; }
static void trace_add( const char* Tok )
{
	size_t Len = strlen( g_trace );
	if( Len ) { g_trace[Len++] = ','; g_trace[Len] = 0; }
	strncat( g_trace, Tok, sizeof(g_trace) - Len - 1 );
}

// ---- doubles for Core/Engine callees ----
UBOOL ParseCommand( const TCHAR** Stream, const TCHAR* Match )
{
	const TCHAR* s = *Stream;
	while( *s == ' ' || *s == '\t' ) s++;
	size_t n = wcslen( Match );
	for( size_t i = 0; i < n; i++ )
	{
		TCHAR a = s[i];
		if( a >= 'a' && a <= 'z' ) a -= 32;
		if( a != Match[i] ) return 0;
	}
	if( s[n] != 0 && s[n] != ' ' && s[n] != '\t' ) return 0;
	s += n;
	while( *s == ' ' || *s == '\t' ) s++;
	*Stream = s;
	return 1;
}

void UObject::SaveConfig( DWORD Flags, const TCHAR* Filename ) { trace_add( "SC" ); }
void UObject::PostEditChange() {}
static char g_fn_sentinel[16];
UFunction* UObject::FindFunctionChecked( FName InName, UBOOL Global ) { return (UFunction*)g_fn_sentinel; }

FString FString::Printf( const TCHAR* Fmt, ... )
{
	TCHAR Buf[256];
	va_list Ap;
	va_start( Ap, Fmt );
	vswprintf( Buf, 256, Fmt, Ap );
	va_end( Ap );
	return FString( Buf );
}

static void* g_eng_vt[512];
static void* g_rd_vt[512];
static void* g_pawn_vt[512];

static void eng_flush( UEngine* Self, UBOOL Allow )
{
	char Tok[16];
	snprintf( Tok, sizeof(Tok), "EF%d", (int)Allow );
	trace_add( Tok );
}

static URenderDevice* g_rd[3];
static void rd_flush( URenderDevice* Self, UBOOL Allow )
{
	int Idx = -1;
	for( int i = 0; i < 3; i++ ) if( Self == g_rd[i] ) Idx = i;
	char Tok[16];
	snprintf( Tok, sizeof(Tok), "RF%d%d", Idx, (int)Allow );
	trace_add( Tok );
}

static void pawn_process_event( AActor* Self, UFunction* Function, void* Parms, void* Result )
{
	APawn_eventClientMessage_Parms* P = (APawn_eventClientMessage_Parms*)Parms;
	const TCHAR* S = *P->S;
	char Msg[256], Tok[320];
	INT i = 0;
	for( ; S[i] && i < 255; i++ ) Msg[i] = (char)S[i];
	Msg[i] = 0;
	snprintf( Tok, sizeof(Tok), "MSG:%s/b%u", Msg, (unsigned)P->bBeep );
	trace_add( Tok );
}

// Vtable slot of a virtual member function pointer (ARM member-pointer ABI: word0 is the
// byte offset of the vtable entry, word1 bit0 marks a virtual member).
static size_t vt_slot( const void* Pm )
{
	uintptr_t W[2];
	memcpy( W, Pm, sizeof W );
	if( !(W[1] & 1) )
		abort();
	return W[0] / sizeof(void*);
}

static void set_vptr( void* Obj, void** Vt )
{
	memcpy( Obj, &Vt, sizeof Vt );
}

// ---- deterministic inputs ----
static uint64_t gState = 0;
static uint64_t next64( void )
{
	uint64_t z = (gState += 0x9E3779B97F4A7C15ULL);
	z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
	z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
	return z ^ (z >> 31);
}
static uint32_t rnd32( void ) { return (uint32_t)next64(); }
static float rnd_float( float Lo, float Hi )
{
	double U = (rnd32() >> 8) * (1.0 / 16777216.0);
	return (float)(Lo + (Hi - Lo) * U);
}
static uint32_t fbits( float F ) { uint32_t B; memcpy( &B, &F, 4 ); return B; }
static float bits_f( uint32_t B ) { float F; memcpy( &F, &B, 4 ); return F; }

// ---- objects ----
static UEngine* g_eng;
static UViewport* g_vp[3];
static APlayerPawn* g_actor;
static UClient* g_cl;

static const TCHAR* g_cmds[] = {
	TEXT("BRIGHTNESS"), TEXT("brightness"), TEXT("BRIGHTNESS now"),
	TEXT("  BRIGHTNESS"), TEXT("BRIGHTNESSX"), TEXT("NOPE"), TEXT("")
};
#define NCMD (int)(sizeof(g_cmds) / sizeof(g_cmds[0]))

int main( void )
{
	// vtables
	auto PmEngFlush = &UEngine::Flush;
	g_eng_vt[vt_slot( &PmEngFlush )] = (void*)&eng_flush;
	auto PmRdFlush = &URenderDevice::Flush;
	g_rd_vt[vt_slot( &PmRdFlush )] = (void*)&rd_flush;
	auto PmPe = &AActor::ProcessEvent;
	g_pawn_vt[vt_slot( &PmPe )] = (void*)&pawn_process_event;

	g_eng = (UEngine*)calloc( 1, sizeof(UEngine) );
	set_vptr( g_eng, g_eng_vt );
	for( int i = 0; i < 3; i++ )
	{
		g_rd[i] = (URenderDevice*)calloc( 1, sizeof(URenderDevice) );
		set_vptr( g_rd[i], g_rd_vt );
		g_vp[i] = (UViewport*)calloc( 1, sizeof(UViewport) );
	}
	g_actor = (APlayerPawn*)calloc( 1, sizeof(APlayerPawn) );
	set_vptr( g_actor, g_pawn_vt );
	g_cl = (UClient*)calloc( 1, sizeof(UClient) );
	g_cl->Engine = g_eng;

	printf( "# UnCamMgr.cpp (UClient) fixture generated by gen_cam_mgr.sh from the original TU at the pinned ref.\n" );
	printf( "# kinds: fov (GetEffectiveFovAngle), postedit (PostEditChange), exec (Exec, cmd index into the table below), flush (Flush).\n" );
	printf( "# cmd table:" );
	for( int i = 0; i < NCMD; i++ ) printf( " %d=\"%ls\"", i, g_cmds[i] );
	printf( "\n" );

	float SpecNaN = bits_f( 0x7fc00000u );
	float SpecInf = bits_f( 0x7f800000u );

	// ---- GetEffectiveFovAngle ----
	gState = 0x5EED0001ULL;
	for( int n = 0; n < 240; n++ )
	{
		float Auth;
		if( n < 8 )
		{
			static const float Tab[8] = { 0.0f, -0.0f, 90.0f, 179.0f, 170.0f, 0.1f, 360.0f, 1.0f };
			Auth = Tab[n];
		}
		else if( n == 8 ) Auth = SpecNaN;
		else if( n == 9 ) Auth = SpecInf;
		else if( n % 5 == 0 ) Auth = bits_f( rnd32() );
		else Auth = rnd_float( -30.0f, 200.0f );
		static const INT VxTab[10] = { 0, -1, 1, 640, 800, 1024, 1280, 1920, 2560, 3840 };
		INT Vx = (n < 10) ? VxTab[n] : (INT)(rnd32() % 8000) - 2;
		INT Vy = (n < 10) ? VxTab[(n + 3) % 10] : (INT)(rnd32() % 6000) - 2;
		INT Maintain = (n % 7 == 6) ? 0 : 1;
		float Ret = UClient::GetEffectiveFovAngle( Auth, Vx, Vy, Maintain );
		printf( "fov auth=%08x vx=%d vy=%d maintain=%d -> ret=%08x\n",
			fbits( Auth ), Vx, Vy, Maintain, fbits( Ret ) );
	}

	// ---- PostEditChange ----
	gState = 0x5EED0002ULL;
	for( int n = 0; n < 160; n++ )
	{
		static const float BTab[8] = { 0.0f, -0.0f, 1.0f, 0.5f, 2.0f, -1.0f, 1.5f, 0.99f };
		static const float MTab[8] = { 0.0f, -0.0f, 3.0f, -3.0f, 3.5f, -3.5f, 2.75f, -2.75f };
		static const INT DTab[8] = { 0, -1, 7, INT_MIN, INT_MAX, 100, -100, 3 };
		float B, M;
		INT D;
		if( n < 8 ) { B = BTab[n]; M = MTab[(n + 3) % 8]; D = DTab[(n + 5) % 8]; }
		else if( n == 8 ) { B = SpecNaN; M = SpecNaN; D = 1; }
		else if( n == 9 ) { B = SpecInf; M = SpecInf; D = -5; }
		else if( n == 10 ) { B = -SpecInf; M = -SpecInf; D = 0; }
		else { B = rnd_float( -2.0f, 3.0f ); M = rnd_float( -5.0f, 5.0f ); D = (INT)(rnd32() % 101) - 50; }
		g_cl->Brightness = B;
		g_cl->MipFactor = M;
		g_cl->ParticleDensity = D;
		g_cl->UClient::PostEditChange();
		printf( "postedit bright=%08x mip=%08x density=%d -> bright=%08x mip=%08x density=%d\n",
			fbits( B ), fbits( M ), D,
			fbits( g_cl->Brightness ), fbits( g_cl->MipFactor ), g_cl->ParticleDensity );
	}

	// ---- Exec ----
	gState = 0x5EED0003ULL;
	static char g_ar_buf[64] __attribute__((aligned(16)));
	FOutputDevice& Ar = *(FOutputDevice*)g_ar_buf;
	for( int n = 0; n < 280; n++ )
	{
		static const float BTab[8] = { 0.0f, 0.9f, 0.85f, 0.95f, 0.999f, 1.0f, -0.5f, 0.1f };
		float Bright = (n < 8) ? BTab[n] : rnd_float( 0.0f, 1.2f );
		INT Cmd = n % NCMD;
		INT Nv = (n / NCMD) % 3;
		INT Act = (n / 21) % 2;
		g_cl->Viewports.Empty();
		for( INT i = 0; i < Nv; i++ ) g_cl->Viewports.AddItem( g_vp[i] );
		g_vp[0]->Actor = Act ? g_actor : NULL;
		g_cl->Brightness = Bright;
		trace_clear();
		UBOOL Ret = g_cl->UClient::Exec( g_cmds[Cmd], Ar );
		printf( "exec cmd=%d bright=%08x nv=%d act=%d -> ret=%d bright=%08x trace=%s\n",
			Cmd, fbits( Bright ), Nv, Act, Ret, fbits( g_cl->Brightness ),
			g_trace[0] ? g_trace : "-" );
	}

	// ---- Flush ----
	gState = 0x5EED0004ULL;
	for( int n = 0; n < 120; n++ )
	{
		INT Allow = n & 1;
		INT Nv = (n / 2) % 4;
		INT Ids[3];
		g_cl->Viewports.Empty();
		for( INT i = 0; i < Nv; i++ )
		{
			Ids[i] = (INT)(rnd32() % 4) - 1;
			g_vp[i]->RenDev = (Ids[i] < 0) ? NULL : g_rd[Ids[i]];
			g_cl->Viewports.AddItem( g_vp[i] );
		}
		char Vps[64] = "";
		for( INT i = 0; i < Nv; i++ )
		{
			char Tok[16];
			snprintf( Tok, sizeof(Tok), "%s%d", i ? "," : "", Ids[i] );
			strcat( Vps, Tok );
		}
		trace_clear();
		g_cl->UClient::Flush( Allow );
		printf( "flush allow=%d vps=%s -> trace=%s\n",
			Allow, Nv ? Vps : "-", g_trace[0] ? g_trace : "-" );
	}
	return 0;
}

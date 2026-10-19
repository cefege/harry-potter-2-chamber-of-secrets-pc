// Oracle driver for UViewport::Exec and its sibling paths (ScreenShot, Lock, Unlock, SetDrag,
// IsWire, ReadInput) in the ORIGINAL UnCamera.cpp at 69c3263. gen_camera_exec.sh compiles that
// TU unmodified; every callee outside it is a recording double defined here.
// Output per case:
//   case <family> <idx> <knobs>
//     CMD / RET / LOG / CREATE / FILE / FILESIZE / READPIX / RDLOCK / RDUNLOCK / INEXEC / INREAD /
//     UPDATEINPUT / CONSOLE / PLAYER_EXEC / SAVECONFIG trace lines (two-space indent)
//   STATE ...
// Families: bitmap, exec, lock, unlock, setdrag, iswire, readinput.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cwchar>
#include <cstdarg>
#include <cmath>
#include <string>
#include <vector>
#define private public
#define protected public
#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnNet.h"
#undef private
#undef protected

enum
{
	BITMAP_CASES   = 6,
	EXEC_CASES     = 35,
	LOCK_CASES     = 5,
	UNLOCK_CASES   = 3,
	SETDRAG_CASES  = 3,
	ISWIRE_CASES   = 8,
	READINPUT_CASES = 3,
};

UBOOL GIsEditor = 0;
// Native registration runs in static initializers of the original TU; no table is needed here.
BYTE GRegisterNative(INT, const Native&) { return 0; }

// Static initializers of the original TU allocate through GMalloc; back it with libc.
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
static HarnessMalloc g_harness_malloc;
FMalloc* GMalloc = &g_harness_malloc;

// Core globals the original TU reaches; Core/Src is not linked.
FMemStack GMem;

// PushBytes ignores the return value and reads Top/End afterwards, so the new
// chunk must publish Top/End itself. FMemMark::Pop restores only Top, so
// FreeChunks must republish End from the surviving top chunk.
BYTE* FMemStack::AllocateNewChunk( INT MinSize )
{
	INT DataSize = MinSize > 65536 ? MinSize : 65536;
	FTaggedMemory* Chunk = (FTaggedMemory*)malloc( sizeof(FTaggedMemory) - 1 + DataSize );
	Chunk->Next     = TopChunk;
	Chunk->DataSize = DataSize;
	TopChunk        = Chunk;
	Top             = Chunk->Data;
	End             = Chunk->Data + DataSize;
	return Top;
}
void FMemStack::FreeChunks( FTaggedMemory* NewTopChunk )
{
	while( TopChunk != NewTopChunk )
	{
		FTaggedMemory* Next = TopChunk->Next;
		free( TopChunk );
		TopChunk = Next;
	}
	End = NewTopChunk ? NewTopChunk->Data + NewTopChunk->DataSize : NULL;
}
FFileManager* GFileManager = NULL;
USystem* GSys = NULL;
UBOOL GIsRunning = 0;
FTransactionBase* GUndo = NULL;   // TTransArray::Add skips SaveArray when this is null

class HarnessOut : public FOutputDevice
{
public:
	virtual void Serialize( const TCHAR* V, EName Event ) {}
};
static HarnessOut g_ar_obj;    // the Ar passed to UViewport::Exec
static HarnessOut g_null_obj;
FOutputDevice* GNull = &g_null_obj;
FOutputDevice* GLog = &g_null_obj;

static const FOutputDevice* g_conn_fod = NULL;   // FOutputDevice base of the server connection
static APlayerPawn* g_pawn = NULL;               // Actor, compared against ScriptConsoleExec's Executor
static UViewport* g_cur_vp = NULL;              // viewport whose size ReadPixels fills
static INT g_in_ret = 0, g_con_ret = 0, g_plr_ret = 0, g_exist = 0;
static uint32_t g_lcg = 0;

// ---- deterministic pixel source: one LCG step per pixel, reset per case ----
static uint32_t next_u32( void ) { g_lcg = g_lcg * 1664525u + 1013904223u; return g_lcg; }
static void seed( int Family, int Idx ) { g_lcg = 0x2545F491u + 1000u * (uint32_t)Family + (uint32_t)Idx; }
static uint32_t bits_of( FLOAT F ) { uint32_t U; memcpy( &U, &F, 4 ); return U; }

// ---- text helpers ----
// Escaped ASCII form of a TCHAR string: \r and \n shown as escapes, '"' as ', other non-printables as ?.
static std::string esc( const TCHAR* S )
{
	std::string R;
	for( ; *S; S++ )
	{
		TCHAR C = *S;
		if( C == '\r' )      R += "\\r";
		else if( C == '\n' ) R += "\\n";
		else if( C == '"' )  R += "'";
		else if( C < 32 || C > 126 ) R += "?";
		else R += (char)C;
	}
	return R;
}
static void to_wide( const char* S, TCHAR* Out, INT Max )
{
	INT I = 0;
	for( ; S[I] && I + 1 < Max; I++ ) Out[I] = (TCHAR)(unsigned char)S[I];
	Out[I] = 0;
}
// Core formats %s as TCHAR*; the C wide printf reads %s as char*, so rewrite %s to %ls.
static void wide_fmt( const TCHAR* Fmt, TCHAR* Out, INT Max )
{
	INT J = 0;
	for( INT I = 0; Fmt[I] && J + 3 < Max; I++ )
	{
		Out[J++] = Fmt[I];
		if( Fmt[I] == '%' && Fmt[I + 1] == 's' ) Out[J++] = 'l';
	}
	Out[J] = 0;
}
static const char* label_of( const FOutputDevice* D )
{
	if( D == (const FOutputDevice*)&g_ar_obj ) return "AR";
	if( D == g_conn_fod ) return "CONN";
	return "OUT";
}

// ---- pure Core helpers (real algorithms, not recorders) ----
const TCHAR* appUserDir() { return TEXT("UserDir\\"); }
// Clock for UViewport::Lock/Unlock. The value is never printed; the TU's inline form only exists under ASMLINUX.
FTime appSeconds() { return FTime( 1.0 ); }
// UnMisc.cpp:1107-1134, verbatim.
UBOOL ParseCommand( const TCHAR** Stream, const TCHAR* Match )
{
	while( (**Stream==' ')||(**Stream==9) )
		(*Stream)++;

	if( appStrnicmp(*Stream,Match,appStrlen(Match))==0 )
	{
		*Stream += appStrlen(Match);
		if( !appIsAlnum(**Stream) )
		{
			while ((**Stream==' ')||(**Stream==9)) (*Stream)++;
			return 1; // Success.
		}
		else
		{
			*Stream -= appStrlen(Match);
			return 0; // Only found partial match.
		}
	}
	else return 0; // No match.
}
// UnMisc.cpp:1164-1202, verbatim.
UBOOL ParseToken( const TCHAR*& Str, TCHAR* Result, INT MaxLen, UBOOL UseEscape )
{
	INT Len=0;

	// Skip spaces and tabs.
	while( *Str==' ' || *Str==9 )
		Str++;
	if( *Str == 34 )
	{
		// Get quoted string.
		Str++;
		while( *Str && *Str!=34 && (Len+1)<MaxLen )
		{
			TCHAR c = *Str++;
			if( c=='\\' && UseEscape )
			{
				// Get escape.
				c = *Str++;
				if( !c )
					break;
			}
			if( (Len+1)<MaxLen )
				Result[Len++] = c;
		}
		if( *Str==34 )
			Str++;
	}
	else
	{
		// Get unquoted string.
		for( ; *Str && *Str!=' ' && *Str!=9; Str++ )
			if( (Len+1)<MaxLen )
				Result[Len++] = *Str;
	}
	Result[Len]=0;
	return Len!=0;
}
// Parse(Stream, "x=", Value): the integer following the first occurrence of Match.
UBOOL Parse( const TCHAR* Stream, const TCHAR* Match, INT& Value )
{
	const TCHAR* Found = wcsstr( Stream, Match );
	if( !Found ) return 0;
	Value = appAtoi( Found + wcslen( Match ) );
	return 1;
}

// ---- recording doubles for engine callees outside the TU ----
void FOutputDevice::Logf( const TCHAR* Fmt, ... )
{
	TCHAR Conv[1024], Buf[2048];
	wide_fmt( Fmt, Conv, 1024 );
	va_list Ap;
	va_start( Ap, Fmt );
	vswprintf( Buf, 2048, Conv, Ap );
	va_end( Ap );
	printf( "  LOG %s \"%s\"\n", label_of( this ), esc( Buf ).c_str() );
}
void UObject::SaveConfig( DWORD Flags, const TCHAR* Filename )
{
	printf( "  SAVECONFIG flags=%u\n", (unsigned)Flags );
}
UBOOL UPlayer::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	printf( "  PLAYER_EXEC \"%s\" -> %d\n", esc( Cmd ).c_str(), (int)g_plr_ret );
	return g_plr_ret;
}

// Itanium/ARM64 member-pointer ABI: word0 is the vtable byte offset (plus virtual-bit in word1).
template< class M > static size_t vt_slot( M Pm )
{
	uintptr_t W[2];
	memcpy( W, &Pm, sizeof W );
	if( !(W[1] & 1) ) abort();
	return W[0] / sizeof(void*);
}
static void set_vptr( void* Obj, void** Vt )
{
	memcpy( Obj, &Vt, sizeof Vt );
}
static void* g_vp_vt[512];
static void* g_in_vt[512];
static void* g_rd_vt[512];
static void* g_con_vt[512];
static void* g_fm_vt[512];

// UViewport::UpdateInput (pure virtual, called from ReadInput).
static void vp_update_input( UViewport* Self, UBOOL Reset )
{
	printf( "  UPDATEINPUT reset=%d\n", (int)Reset );
}

// UInput::Exec / ReadInput.
static UBOOL in_exec( UInput* Self, const TCHAR* Cmd, FOutputDevice& Ar )
{
	printf( "  INEXEC \"%s\" -> %d\n", esc( Cmd ).c_str(), (int)g_in_ret );
	return g_in_ret;
}
static void in_read_input( UInput* Self, FLOAT DeltaSeconds, FOutputDevice& Ar )
{
	printf( "  INREAD delta=%08x ar=%s\n", bits_of( DeltaSeconds ), label_of( &Ar ) );
}

// URenderDevice: Exec dispatches to the original body; Lock/Unlock/ReadPixels record.
static UBOOL rd_exec( URenderDevice* Self, const TCHAR* Cmd, FOutputDevice& Ar )
{
	return Self->URenderDevice::Exec( Cmd, Ar );
}
static void print_plane( const char* Tag, const FPlane& P )
{
	printf( " %s=%08x,%08x,%08x,%08x", Tag, bits_of( P.X ), bits_of( P.Y ), bits_of( P.Z ), bits_of( P.W ) );
}
static void rd_lock( URenderDevice* Self, FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData, INT* HitSize )
{
	printf( "  RDLOCK" );
	print_plane( "flash", FlashScale );
	print_plane( "fog", FlashFog );
	print_plane( "clear", ScreenClear );
	printf( " flags=%u hit=%d hitsize=%d\n", (unsigned)RenderLockFlags, HitData != NULL ? 1 : 0, HitSize ? (int)*HitSize : -1 );
}
static void rd_unlock( URenderDevice* Self, UBOOL Blit )
{
	printf( "  RDUNLOCK blit=%d\n", (int)Blit );
}
static void rd_read_pixels( URenderDevice* Self, FColor* Pixels )
{
	INT W = g_cur_vp->SizeX, H = g_cur_vp->SizeY;
	printf( "  READPIX %dx%d\n", (int)W, (int)H );
	for( INT I = 0; I < W * H; I++ )
	{
		uint32_t V = next_u32();
		Pixels[I].R = (BYTE)( V & 255 );
		Pixels[I].G = (BYTE)( (V >> 8) & 255 );
		Pixels[I].B = (BYTE)( (V >> 16) & 255 );
		Pixels[I].A = (BYTE)( (V >> 24) & 255 );
	}
}

// UObject::ScriptConsoleExec (virtual) on the console object.
static UBOOL con_script_exec( UObject* Self, const TCHAR* Cmd, FOutputDevice& Ar, UObject* Executor )
{
	printf( "  CONSOLE \"%s\" executor=%s -> %d\n", esc( Cmd ).c_str(), (void*)Executor == (void*)g_pawn ? "actor" : "other", (int)g_con_ret );
	return g_con_ret;
}

// Bitmap file writer: the bytes the original TU serializes are captured and printed at destruction.
class RecArchive : public FArchive
{
public:
	std::string Path;
	std::vector<BYTE> Bytes;
	explicit RecArchive( const std::string& InPath ) : Path( InPath ) {}
	virtual ~RecArchive()
	{
		printf( "  FILE \"%s\" n=%u ", Path.c_str(), (unsigned)Bytes.size() );
		for( size_t I = 0; I < Bytes.size(); I++ ) printf( "%02x", (unsigned)Bytes[I] );
		printf( "\n" );
	}
	virtual void Serialize( void* V, INT Length )
	{
		const BYTE* P = (const BYTE*)V;
		Bytes.insert( Bytes.end(), P, P + Length );
	}
};
static FArchive* fm_create_writer( FFileManager* Self, const TCHAR* Filename, DWORD WriteFlags, FOutputDevice* Error )
{
	std::string P = esc( Filename );
	printf( "  CREATE \"%s\"\n", P.c_str() );
	return new RecArchive( P );
}
// Shot files with index below the knob exist; the size is reported only for those.
static INT fm_file_size( FFileManager* Self, const TCHAR* Filename )
{
	std::string P = esc( Filename );
	INT R = -1;
	size_t At = P.find( "Shot" );
	if( At != std::string::npos && P.size() >= At + 8 )
	{
		INT Index = (INT)atoi( P.c_str() + At + 4 );
		if( Index < g_exist ) R = 54;
	}
	printf( "  FILESIZE \"%s\" -> %d\n", P.c_str(), (int)R );
	return R;
}

// ---- case knobs ----
struct Knob
{
	INT    w, h;
	DWORD  show;      // Actor->ShowFlags
	INT    rend;      // Actor->RendMap
	INT    in;        // -1: no Input object; else Input->Exec return value
	INT    rd;        // 1: RenDev present
	INT    detail;    // RenDev->DetailTextures
	INT    con;       // -1: no Console object; else ScriptConsoleExec return value
	INT    plr;       // UPlayer::Exec return value
	INT    conn;      // 0: no NetDriver; 1: NetDriver without connection; 2: NetDriver + connection
	INT    exist;     // Shot0000..Shot(exist-1) exist
	INT    nodes;     // UModel Nodes count
	INT    actor;     // 1: Actor present
	INT    hit;       // lock: HitData non-null
	INT    hitsize;   // lock: HitSize pointer non-null (value 5)
	DWORD  flags;     // lock: RenderLockFlags
	INT    blit;      // unlock: Blit
	FLOAT  delta;     // readinput: DeltaSeconds
	INT    newdrag;   // setdrag: NewDrag
	INT    prevdrag;  // setdrag: prior Dragging
};
static Knob mk( void )
{
	Knob k;
	k.w = 7; k.h = 5; k.show = 0; k.rend = 0;
	k.in = -1; k.rd = 0; k.detail = 0; k.con = -1; k.plr = 0;
	k.conn = 0; k.exist = 0; k.nodes = 0; k.actor = 1;
	k.hit = 0; k.hitsize = 0; k.flags = 0; k.blit = 0; k.delta = 0.0f;
	k.newdrag = 0; k.prevdrag = 0;
	return k;
}
static void print_case( const char* Family, int Idx, const Knob& k, const char* const* C )
{
	std::string Cmds;
	for( int I = 0; I < 3 && C[I]; I++ ) { if( I ) Cmds += "|"; Cmds += C[I]; }
	if( Cmds.empty() ) Cmds = "-";
	printf( "case %s %d w=%d h=%d show=%u rend=%d in=%d rd=%d detail=%d con=%d plr=%d conn=%d exist=%d nodes=%d actor=%d hit=%d hitsize=%d flags=%u blit=%d delta=%08x newdrag=%d prevdrag=%d cmds=%s\n",
		Family, Idx, (int)k.w, (int)k.h, (unsigned)k.show, (int)k.rend, (int)k.in, (int)k.rd, (int)k.detail,
		(int)k.con, (int)k.plr, (int)k.conn, (int)k.exist, (int)k.nodes, (int)k.actor, (int)k.hit, (int)k.hitsize,
		(unsigned)k.flags, (int)k.blit, bits_of( k.delta ), (int)k.newdrag, (int)k.prevdrag, Cmds.c_str() );
}

// Builds a fresh viewport world for a case. Globals read by the doubles are set here.
static UViewport* build_world( const Knob& k )
{
	UViewport* vp = (UViewport*)calloc( 1, sizeof(UViewport) );
	set_vptr( vp, g_vp_vt );
	vp->SizeX = k.w;
	vp->SizeY = k.h;
	g_cur_vp = vp;
	g_in_ret = k.in;
	g_con_ret = k.con;
	g_plr_ret = k.plr;
	g_exist = k.exist;
	g_pawn = NULL;
	g_conn_fod = NULL;
	if( k.actor )
	{
		APlayerPawn* Pawn = (APlayerPawn*)calloc( 1, sizeof(APlayerPawn) );
		ULevel* Level = (ULevel*)calloc( 1, sizeof(ULevel) );
		UModel* Model = (UModel*)calloc( 1, sizeof(UModel) );
		AActor* A0 = (AActor*)calloc( 1, sizeof(AActor) );
		AActor* A2 = (AActor*)calloc( 1, sizeof(AActor) );
		A0->bHidden = 1;
		A2->bHidden = 1;
		Level->Actors.AddItem( A0 );
		Level->Actors.AddItem( (AActor*)NULL );
		Level->Actors.AddItem( A2 );
		if( k.nodes > 0 ) Model->Nodes.Add( k.nodes );
		Level->Model = Model;
		Pawn->XLevel = Level;
		Pawn->ShowFlags = k.show;
		Pawn->RendMap = k.rend;
		vp->Actor = Pawn;
		g_pawn = Pawn;
		if( k.conn >= 1 )
		{
			UNetDriver* Drv = (UNetDriver*)calloc( 1, sizeof(UNetDriver) );
			Level->NetDriver = Drv;
			if( k.conn == 2 )
			{
				UNetConnection* Conn = (UNetConnection*)calloc( 1, sizeof(UNetConnection) );
				Drv->ServerConnection = Conn;
				g_conn_fod = static_cast<FOutputDevice*>( Conn );
			}
		}
	}
	if( k.in >= 0 )
	{
		UInput* In = (UInput*)calloc( 1, sizeof(UInput) );
		set_vptr( In, g_in_vt );
		vp->Input = In;
	}
	if( k.rd )
	{
		URenderDevice* Rd = (URenderDevice*)calloc( 1, sizeof(URenderDevice) );
		set_vptr( Rd, g_rd_vt );
		Rd->DetailTextures = k.detail;
		vp->RenDev = Rd;
	}
	if( k.con >= 0 )
	{
		UObject* Con = (UObject*)calloc( 1, sizeof(UObject) );
		set_vptr( Con, g_con_vt );
		vp->Console = (UConsole*)Con;
	}
	return vp;
}

// One STATE line per case; the Rust replay recomputes exactly these fields.
static void print_state( UViewport* vp )
{
	INT Hidden = 0;
	DWORD Show = 0;
	INT Rend = -1;
	if( vp->Actor )
	{
		Show = vp->Actor->ShowFlags;
		Rend = vp->Actor->RendMap;
		ULevel* Level = vp->Actor->GetLevel();
		if( Level )
			for( INT I = 0; I < Level->Actors.Num(); I++ )
				if( Level->Actors(I) && Level->Actors(I)->bHidden ) Hidden++;
	}
	INT Detail = vp->RenDev ? (INT)vp->RenDev->DetailTextures : -1;
	uint32_t Fnv = 2166136261u;
	for( INT I = 0; I < vp->snapBuff.Num(); I++ )
	{
		FColor C = vp->snapBuff(I);
		BYTE B[4] = { C.R, C.G, C.B, C.A };
		for( INT K = 0; K < 4; K++ ) { Fnv ^= B[K]; Fnv *= 16777619u; }
	}
	printf( "  STATE show=%08x rend=%d hidden=%d detail=%d snap=%dx%d n=%d fnv=%08x dragging=%d hitsizes=%d frame=%u render=%08x extra=%08x hit=%d\n",
		(unsigned)Show, (int)Rend, (int)Hidden, (int)Detail, (int)vp->snapU, (int)vp->snapV, (int)vp->snapBuff.Num(),
		(unsigned)Fnv, (int)vp->Dragging, (int)vp->HitSizes.Num(), (unsigned)vp->FrameCount, (unsigned)vp->RenderFlags,
		(unsigned)vp->ExtraPolyFlags, (int)vp->HitTesting );
}

static void run_exec( UViewport* vp, const char* Cmd )
{
	TCHAR W[512];
	to_wide( Cmd, W, 512 );
	printf( "  CMD \"%s\"\n", Cmd );
	UBOOL R = vp->UViewport::Exec( W, g_ar_obj );
	printf( "  RET %d\n", (int)R );
}

static void run_lock( UViewport* vp, const Knob& k )
{
	static BYTE HitBuf[16];
	INT HitSize = 5;
	BYTE* Hit = k.hit ? HitBuf : NULL;
	INT* HitSizeP = k.hitsize ? &HitSize : NULL;
	UBOOL R = vp->UViewport::Lock( FPlane( 0.3f, 0.7f, 0.11f, 1.0f ), FPlane( 0.05f, 0.9f, 0.25f, 0.5f ),
		FPlane( 0.2f, 0.4f, 0.6f, 0.8f ), k.flags, Hit, HitSizeP );
	printf( "  RET %d\n", (int)R );
}

// ---- families ----
static void case_bitmap( int Idx, INT W, INT H )
{
	Knob k = mk();
	k.w = W; k.h = H; k.rd = 1;
	char Cmd[64];
	snprintf( Cmd, sizeof Cmd, "SHOT Bitmap%04d.bmp", Idx );
	const char* C[3] = { Cmd, NULL, NULL };
	seed( 1, Idx );
	print_case( "bitmap", Idx, k, C );
	UViewport* vp = build_world( k );
	run_exec( vp, Cmd );
	print_state( vp );
}

static void exec_spec( int Idx, Knob& k, const char** C )
{
	switch( Idx )
	{
	case 0:  C[0] = "GETPING"; break;
	case 1:  k.in = 1; C[0] = "GETPING"; break;
	case 2:  k.in = 0; k.rd = 1; C[0] = "GETPING"; break;
	case 3:  k.conn = 2; C[0] = "INJECT hello world"; break;
	case 4:  k.conn = 1; C[0] = "INJECT hello"; break;
	case 5:  k.conn = 0; k.plr = 1; C[0] = "INJECT hello"; break;
	case 6:  C[0] = "SHOWALL"; break;
	case 7:  C[0] = "SHOWACTORS"; break;
	case 8:  k.show = 0x8008; C[0] = "HIDEACTORS"; break;
	case 9:  C[0] = "RMODE 4"; break;
	case 10: k.rend = 2; C[0] = "RMODE 0"; break;
	case 11: k.rend = 2; C[0] = "RMODE 21"; break;
	case 12: C[0] = "SHOWBONES 1"; break;
	case 13: k.show = 0x10000; C[0] = "SHOWBONES 0"; break;
	case 14: C[0] = "SHOWBOUNDS 1"; break;
	case 15: k.show = 0x8000; C[0] = "SHOWBOUNDS 0"; break;
	case 16: k.rd = 1; C[0] = "SHOT shot_a.bmp"; break;
	case 17: k.rd = 1; k.exist = 2; C[0] = "SHOT"; break;
	case 18: k.rd = 1; k.exist = 0; C[0] = "SHOT"; break;
	case 19: k.rd = 1; C[0] = "SNAP 1"; break;
	case 20: k.rd = 1; C[0] = "SNAP 0"; break;
	case 21: k.rd = 1; k.w = 13; k.h = 9; C[0] = "SNAP 2"; break;
	case 22: k.rd = 1; k.w = 320; k.h = 240; C[0] = "THUMBNAIL"; break;
	case 23: k.rd = 1; k.w = 320; k.h = 240; C[0] = "THUMBNAIL x=8 y=6"; break;
	case 24: k.rd = 1; k.w = 320; k.h = 240; C[0] = "THUMBNAIL x=8 y=6"; C[1] = "SAVETHUMBNAIL t.bmp"; break;
	case 25: k.rd = 1; C[0] = "SNAP 1"; C[1] = "SAVESNAP s.bmp"; break;
	case 26: k.rd = 1; C[0] = "SAVETHUMBNAIL t.bmp"; break;
	case 27: k.rd = 1; C[0] = "SAVESNAP s.bmp"; break;
	case 28: k.rd = 1; C[0] = "GetDetailTextures"; break;
	case 29: k.rd = 1; C[0] = "ToggleDetailTextures"; break;
	case 30: k.con = 1; C[0] = "CONSOLE_CMD foo"; break;
	case 31: k.con = 0; k.plr = 1; C[0] = "CONSOLE_CMD foo"; break;
	case 32: k.plr = 1; C[0] = "PLAYER_CMD"; break;
	case 33: C[0] = "NOPE_CMD"; break;
	case 34: k.in = 1; k.show = 0x8; C[0] = "SHOWALL"; C[1] = "SHOWACTORS"; break;
	}
}

static void case_exec( int Idx )
{
	Knob k = mk();
	const char* C[3] = { NULL, NULL, NULL };
	exec_spec( Idx, k, C );
	seed( 2, Idx );
	print_case( "exec", Idx, k, C );
	UViewport* vp = build_world( k );
	for( int I = 0; I < 3 && C[I]; I++ ) run_exec( vp, C[I] );
	print_state( vp );
}

static void lock_spec( int Idx, Knob& k )
{
	k.rd = 1;
	switch( Idx )
	{
	case 0: break;
	case 1: k.hit = 1; k.hitsize = 1; k.flags = 3; k.rend = REN_PolyCuts; break;
	case 2: k.flags = 0x10; k.rend = REN_Zones; break;
	case 3: k.flags = 0xFFFFFFFFu; k.rend = REN_Wire; break;
	case 4: k.hit = 1; break;
	}
}
static void case_lock( int Idx )
{
	Knob k = mk();
	lock_spec( Idx, k );
	seed( 3, Idx );
	const char* C[3] = { NULL, NULL, NULL };
	print_case( "lock", Idx, k, C );
	UViewport* vp = build_world( k );
	run_lock( vp, k );
	print_state( vp );
}

static void case_unlock( int Idx )
{
	Knob k = mk();
	k.rd = 1;
	k.blit = Idx != 0;
	seed( 4, Idx );
	const char* C[3] = { NULL, NULL, NULL };
	print_case( "unlock", Idx, k, C );
	UViewport* vp = build_world( k );
	if( Idx == 2 ) run_lock( vp, k );
	vp->UViewport::Unlock( k.blit );
	print_state( vp );
}

static void case_setdrag( int Idx )
{
	Knob k = mk();
	k.newdrag = Idx != 1;
	k.prevdrag = Idx != 0;
	seed( 5, Idx );
	const char* C[3] = { NULL, NULL, NULL };
	print_case( "setdrag", Idx, k, C );
	UViewport* vp = build_world( k );
	vp->Dragging = k.prevdrag;
	UBOOL R = vp->UViewport::SetDrag( k.newdrag );
	printf( "  RET %d\n", (int)R );
	print_state( vp );
}

static void iswire_spec( int Idx, Knob& k )
{
	switch( Idx )
	{
	case 0: k.nodes = 0; k.rend = REN_PolyCuts; break;
	case 1: k.nodes = 3; k.rend = REN_PolyCuts; break;
	case 2: k.nodes = 3; k.rend = REN_Wire; break;
	case 3: k.nodes = 3; k.rend = REN_OrthXY; break;
	case 4: k.nodes = 3; k.rend = REN_OrthXZ; break;
	case 5: k.nodes = 3; k.rend = REN_OrthYZ; break;
	case 6: k.nodes = 3; k.rend = REN_Zones; break;
	case 7: k.actor = 0; break;
	}
}
static void case_iswire( int Idx )
{
	Knob k = mk();
	iswire_spec( Idx, k );
	seed( 6, Idx );
	const char* C[3] = { NULL, NULL, NULL };
	print_case( "iswire", Idx, k, C );
	UViewport* vp = build_world( k );
	UBOOL R = vp->UViewport::IsWire();
	printf( "  RET %d\n", (int)R );
	print_state( vp );
}

static void case_readinput( int Idx )
{
	Knob k = mk();
	k.in = 0;
	k.delta = Idx == 0 ? -1.0f : ( Idx == 1 ? 0.5f : -0.25f );
	seed( 7, Idx );
	const char* C[3] = { NULL, NULL, NULL };
	print_case( "readinput", Idx, k, C );
	UViewport* vp = build_world( k );
	vp->UViewport::ReadInput( k.delta );
	print_state( vp );
}

int main()
{
	g_vp_vt[vt_slot( &UViewport::UpdateInput )] = (void*)vp_update_input;
	g_in_vt[vt_slot( &UInput::Exec )] = (void*)in_exec;
	g_in_vt[vt_slot( &UInput::ReadInput )] = (void*)in_read_input;
	g_rd_vt[vt_slot( &URenderDevice::Exec )] = (void*)rd_exec;
	g_rd_vt[vt_slot( &URenderDevice::Lock )] = (void*)rd_lock;
	g_rd_vt[vt_slot( &URenderDevice::Unlock )] = (void*)rd_unlock;
	g_rd_vt[vt_slot( &URenderDevice::ReadPixels )] = (void*)rd_read_pixels;
	g_con_vt[vt_slot( &UObject::ScriptConsoleExec )] = (void*)con_script_exec;
	g_fm_vt[vt_slot( &FFileManager::CreateFileWriter )] = (void*)fm_create_writer;
	g_fm_vt[vt_slot( &FFileManager::FileSize )] = (void*)fm_file_size;

	FFileManager* Fm = (FFileManager*)calloc( 1, sizeof(FFileManager) );
	set_vptr( Fm, g_fm_vt );
	GFileManager = Fm;
	GSys = (USystem*)calloc( 1, sizeof(USystem) );
	GSys->SaveSlotPath = FString( TEXT("Slot\\") );

	static const INT BitmapDims[BITMAP_CASES][2] = { {3,1}, {3,2}, {7,5}, {13,9}, {31,17}, {40,30} };
	for( int I = 0; I < BITMAP_CASES; I++ ) case_bitmap( I, BitmapDims[I][0], BitmapDims[I][1] );
	for( int I = 0; I < EXEC_CASES; I++ ) case_exec( I );
	for( int I = 0; I < LOCK_CASES; I++ ) case_lock( I );
	for( int I = 0; I < UNLOCK_CASES; I++ ) case_unlock( I );
	for( int I = 0; I < SETDRAG_CASES; I++ ) case_setdrag( I );
	for( int I = 0; I < ISWIRE_CASES; I++ ) case_iswire( I );
	for( int I = 0; I < READINPUT_CASES; I++ ) case_readinput( I );
	return 0;
}

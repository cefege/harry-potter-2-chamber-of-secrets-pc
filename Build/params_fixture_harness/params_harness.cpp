// Fixture driver for the ORIGINAL UnParams.cpp (GetFVECTOR, GetSUBSTRING, GetFROTATOR)
// at the pinned ref. Each record prints the exact inputs and the exact outputs
// (return values, written components, sentinel-prefilled slots left untouched), so
// the Rust replay asserts bit-exact results without re-deriving any PRNG stream.
//
// Core helpers below are verbatim copies of the pinned bodies (5782e7f), minus the
// guard/unguard scaffolding. Fixture inputs are ASCII only, so ToAnsi is identity.
#include "EnginePrivate.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <string>
#include <vector>

// Real allocator for GMalloc (referenced by engine inline code); mirrors palette_harness.cpp.
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

// ---- Core helper copies -----------------------------------------------------

INT appStrlen( const TCHAR* String )
{
	return appCheckedIntSize( wcslen(String) );
}

TCHAR* appStrchr( const TCHAR* String, int c )
{
	return const_cast<TCHAR*>(wcschr(String, c));
}

TCHAR* appStrncpy( TCHAR* Dest, const TCHAR* Src, INT MaxLen )
{
	wcsncpy( Dest, Src, MaxLen );
	Dest[MaxLen-1]=0;
	return Dest;
}

INT appStrnicmp( const TCHAR* A, const TCHAR* B, INT Count )
{
	return wcsncasecmp( A, B, Count );
}

const TCHAR* appStrfind( const TCHAR* Str, const TCHAR* Find )
{
	UBOOL Alnum  = 0;
	TCHAR f      = (*Find<'a' || *Find>'z') ? (*Find) : (*Find+'A'-'a');
	INT   Length = appStrlen(Find++)-1;
	TCHAR c      = *Str++;
	while( c )
	{
		if( c>='a' && c<='z' )
			c += 'A'-'a';
		if( !Alnum && c==f && !appStrnicmp(Str,Find,Length) )
			return Str-1;
		Alnum = (c>='A' && c<='Z') || (c>='0' && c<='9');
		c = *Str++;
	}
	return NULL;
}

ANSICHAR* appAnsiStaticString1024()
{
	static ANSICHAR Results[128][1024];
	static INT Count=0;
	ANSICHAR* Result = Results[Count++ & 127];
	return Result;
}

const ANSICHAR* appToAnsi( const TCHAR* Str )
{
	if( !Str )
		return NULL;
	ANSICHAR* ACh = appAnsiStaticString1024();
	INT Count;
	for( Count=0; Count<1024-1 && Str[Count]; Count++ )
		ACh[Count] = ToAnsi( Str[Count] );
	ACh[Count] = 0;
	return ACh;
}

FLOAT appAtof( const TCHAR* Str )
{
	return atof( appToAnsi(Str) );
}

UBOOL Parse( const TCHAR* Stream, const TCHAR* Match, FLOAT& Value )
{
	const TCHAR* Temp = appStrfind( Stream, Match );
	if( Temp==NULL )
		return 0;
	Value = appAtof( Temp+appStrlen(Match) );
	return 1;
}

// Prototypes for the ORIGINAL UnParams.cpp entry points (defined in the TU, not in a header).
UBOOL ENGINE_API GetFVECTOR( const TCHAR* Stream, FVector& Value );
UBOOL ENGINE_API GetSUBSTRING( const TCHAR* Stream, const TCHAR* Match, TCHAR* Value, INT MaxLen );
UBOOL ENGINE_API GetFVECTOR( const TCHAR* Stream, const TCHAR* Match, FVector& Value );
UBOOL ENGINE_API GetFROTATOR( const TCHAR* Stream, FRotator& Rotation, INT ScaleFactor );
UBOOL ENGINE_API GetFROTATOR( const TCHAR* Stream, const TCHAR* Match, FRotator& Value, INT ScaleFactor );

// ---- Deterministic generator ------------------------------------------------

static uint64_t gState = 0;
static uint64_t next64()
{
	uint64_t z = (gState += 0x9E3779B97F4A7C15ULL);
	z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
	z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
	return z ^ (z >> 31);
}
static unsigned pick( unsigned n ) { return (unsigned)(next64() % n); }

static std::string randNum()
{
	static const char* specials[] = { "0", "-0", "1", "-1", "3.5", "-2.25", "1e3", "-1.5e-2", "0.001",
		"123456789", "2.5e10", "nan", "inf", "-inf", "x", ".5", "7.", "+4", "1e-40", "9", "65535" };
	char b[96];
	switch( pick(4) )
	{
	case 0:
		return specials[pick(sizeof(specials)/sizeof(specials[0]))];
	case 1:
		snprintf( b, sizeof b, "%d", (int)(next64() % 2000) - 1000 );
		return b;
	case 2:
	{
		double d = ((double)(next64() % 2000001) - 1000000.0) / (double)(1u << (next64() % 12));
		snprintf( b, sizeof b, "%.6f", d );
		return b;
	}
	default:
		snprintf( b, sizeof b, "%g", ((double)(next64() % 100000) - 50000.0) * 1.0e4 );
		return b;
	}
}

static const char* kAlpha = "abcdefgXYZ0123456789.-,=()+ ";

static std::string randGarbage( unsigned maxLen )
{
	std::string s;
	unsigned n = pick( maxLen + 1 );
	for( unsigned i = 0; i < n; i++ )
		s += kAlpha[pick( (unsigned)strlen(kAlpha) )];
	return s;
}

// Old-format components "<key><num>" for a random subset, shuffled, joined by sep.
static std::string randKeyed( const char* const* keys, unsigned nkeys )
{
	std::vector<unsigned> order;
	for( unsigned i = 0; i < nkeys; i++ ) order.push_back( i );
	for( unsigned i = nkeys; i > 1; i-- ) std::swap( order[i-1], order[pick(i)] );
	unsigned take = pick( nkeys + 1 );
	std::string s;
	for( unsigned i = 0; i < take; i++ )
	{
		if( i ) s += pick(2) ? " " : ",";
		s += keys[order[i]];
		s += randNum();
	}
	return s;
}

// New-format "a,b,c" with 0-3 commas.
static std::string randCsv()
{
	std::string s = randNum();
	unsigned commas = pick( 4 );
	for( unsigned i = 0; i < commas; i++ ) s += "," + randNum();
	return s;
}

static std::string wrapMatch( const std::string& match, const std::string& inner )
{
	std::string s = randGarbage( 3 );
	unsigned style = pick( 10 );
	if( style == 0 )
		s += match + inner;                 // missing parentheses
	else if( style == 1 )
		s += match + "(" + inner;           // missing close paren
	else if( style == 2 )
		s += match + inner + ")";           // missing open paren
	else
	{
		std::string m = match;
		if( pick(3) == 0 )
			for( auto& ch : m ) if( ch >= 'A' && ch <= 'Z' ) ch += 'a' - 'A'; // case-insensitive find
		s += m + "(" + inner + ")";
	}
	if( pick(3) == 0 ) s += randGarbage( 4 );
	return s;
}

static const char* kVecKeys[]  = { "X=", "Y=", "Z=" };
static const char* kRotKeys[]  = { "PITCH=", "YAW=", "ROLL=" };
static const char* kVecMatch[] = { "Loc=", "Pos=", "Vel=", "Loc=" };
static const char* kRotMatch[] = { "Rot=", "rotation=", "Ang=" };
static const int   kScales[]   = { 1, 10, 100, 65536, -1, 0, 3, 182 };
static const int   kMaxLens[]  = { 1, 2, 3, 4, 8, 16, 40, 80 };

// ---- Records ----------------------------------------------------------------

static void putNarrow( const TCHAR* s )
{
	for( ; *s; s++ )
		putchar( (int)*s );
}

static void putBits( float f )
{
	uint32_t u;
	memcpy( &u, &f, 4 );
	printf( "%08x", u );
}

static void putTChars( const TCHAR* s, INT count )
{
	for( INT i = 0; i < count; i++ )
		printf( "%s%d", i ? "," : "", (int)s[i] );
}

static TCHAR* toTChar( const std::string& s, TCHAR* out, size_t cap )
{
	size_t n = s.size() < cap - 1 ? s.size() : cap - 1;
	for( size_t i = 0; i < n; i++ )
		out[i] = (TCHAR)(unsigned char)s[i];
	out[n] = 0;
	return out;
}

static const FVector kVecSentinel = { 1.25f, -2.5f, 3.75f };
static const INT     kRotSentinelP = 111, kRotSentinelY = -222, kRotSentinelR = 333;
static const TCHAR   kBufSentinel = 0x5A;   // 'Z', prefilled into the substring buffer

static void emitVecStream( const std::string& stream )
{
	TCHAR sbuf[1024];
	toTChar( stream, sbuf, 1024 );
	FVector v = kVecSentinel;
	UBOOL ret = GetFVECTOR( sbuf, v );
	printf( "fvec_stream\t%s\t-\t-\t%d\t", stream.c_str(), ret );
	putBits( v.X ); printf( "\t" ); putBits( v.Y ); printf( "\t" ); putBits( v.Z ); printf( "\n" );
}

static void emitVecMatch( const std::string& stream, const std::string& match )
{
	TCHAR sbuf[1024], mbuf[64];
	toTChar( stream, sbuf, 1024 );
	toTChar( match, mbuf, 64 );
	FVector v = kVecSentinel;
	UBOOL ret = GetFVECTOR( sbuf, mbuf, v );
	printf( "fvec_match\t%s\t%s\t-\t%d\t", stream.c_str(), match.c_str(), ret );
	putBits( v.X ); printf( "\t" ); putBits( v.Y ); printf( "\t" ); putBits( v.Z ); printf( "\n" );
}

static void emitSubstring( const std::string& stream, const std::string& match, INT maxLen )
{
	TCHAR sbuf[1024], mbuf[64];
	TCHAR buf[80];
	toTChar( stream, sbuf, 1024 );
	toTChar( match, mbuf, 64 );
	for( INT i = 0; i < 80; i++ ) buf[i] = kBufSentinel;
	UBOOL ret = GetSUBSTRING( sbuf, mbuf, buf, maxLen );
	printf( "substring\t%s\t%s\t%d\t%d\t", stream.c_str(), match.c_str(), maxLen, ret );
	putTChars( buf, maxLen );
	printf( "\n" );
}

static void emitRotStream( const std::string& stream, INT scale )
{
	TCHAR sbuf[1024];
	toTChar( stream, sbuf, 1024 );
	FRotator r;
	r.Pitch = kRotSentinelP; r.Yaw = kRotSentinelY; r.Roll = kRotSentinelR;
	UBOOL ret = GetFROTATOR( sbuf, r, scale );
	printf( "frot_stream\t%s\t-\t%d\t%d\t%d\t%d\t%d\n", stream.c_str(), scale, ret, r.Pitch, r.Yaw, r.Roll );
}

static void emitRotMatch( const std::string& stream, const std::string& match, INT scale )
{
	TCHAR sbuf[1024], mbuf[64];
	toTChar( stream, sbuf, 1024 );
	toTChar( match, mbuf, 64 );
	FRotator r;
	r.Pitch = kRotSentinelP; r.Yaw = kRotSentinelY; r.Roll = kRotSentinelR;
	UBOOL ret = GetFROTATOR( sbuf, mbuf, r, scale );
	printf( "frot_match\t%s\t%s\t%d\t%d\t%d\t%d\t%d\n", stream.c_str(), match.c_str(), scale, ret, r.Pitch, r.Yaw, r.Roll );
}

int main()
{
	gState = 0x5EEDF00DULL;
	printf( "# params fixture: ORIGINAL UnParams.cpp at the pinned ref, generated by gen_params.sh\n" );
	printf( "# fields (tab-separated): kind, stream, match|-, arg|-, ret, outputs\n" );
	printf( "# sentinels: FVector = 1.25,-2.5,3.75; FRotator = 111,-222,333; substring buffer prefill = 90 ('Z')\n" );

	const int kRandomPerKind = 400;

	// Vector, no match: old "X=/Y=/Z=" format or new "a,b,c" format.
	for( int i = 0; i < kRandomPerKind; i++ )
	{
		std::string s = pick(2) ? randGarbage(4) + randKeyed( kVecKeys, 3 ) : randGarbage(2) + randCsv();
		emitVecStream( s );
	}
	// Vector, with match: "<match>(<inner>)".
	for( int i = 0; i < kRandomPerKind; i++ )
	{
		const char* match = kVecMatch[pick(4)];
		std::string inner = pick(2) ? randKeyed( kVecKeys, 3 ) : randCsv();
		emitVecMatch( wrapMatch( match, inner ), match );
	}
	// Substring: arbitrary inner text, truncated by MaxLen.
	for( int i = 0; i < kRandomPerKind; i++ )
	{
		const char* match = kVecMatch[pick(4)];
		std::string inner = randGarbage( 120 );
		emitSubstring( wrapMatch( match, inner ), match, kMaxLens[pick(8)] );
	}
	// Rotation, no match.
	for( int i = 0; i < kRandomPerKind; i++ )
	{
		std::string s = pick(2) ? randGarbage(4) + randKeyed( kRotKeys, 3 ) : randGarbage(2) + randCsv();
		emitRotStream( s, kScales[pick(8)] );
	}
	// Rotation, with match.
	for( int i = 0; i < kRandomPerKind; i++ )
	{
		const char* match = kRotMatch[pick(3)];
		std::string inner = pick(2) ? randKeyed( kRotKeys, 3 ) : randCsv();
		emitRotMatch( wrapMatch( match, inner ), match, kScales[pick(8)] );
	}

	// Hand-picked edges: partial old format, missing commas, empty input, sentinel survival.
	emitVecStream( "" );
	emitVecStream( "X=1" );
	emitVecStream( "X=1 Z=3" );
	emitVecStream( "1,2" );
	emitVecStream( "1,2,3" );
	emitVecStream( "1,2,3,4" );
	emitVecStream( "x=5,6,7" );
	emitVecMatch( "Loc=(1,2)", "Loc=" );
	emitVecMatch( "Loc=(X=1,Y=2)", "Loc=" );
	emitVecMatch( "loc=(X=1,Y=2,Z=3)", "Loc=" );
	emitVecMatch( "Loc=1,2,3", "Loc=" );
	emitSubstring( "Loc=(abc)tail", "Loc=", 80 );
	emitSubstring( "Loc=(abcdef)", "Loc=", 4 );
	emitSubstring( "Loc=abc", "Loc=", 8 );
	emitRotStream( "", 1 );
	emitRotStream( "PITCH=1.5", 10 );
	emitRotStream( "PITCH=1.5 YAW=2", 10 );
	emitRotStream( "1.5,2.5", 10 );
	emitRotStream( "1.5,2.5,3.5", 65536 );
	emitRotStream( "nan,1,2", 10 );
	emitRotStream( "inf,1,2", 10 );
	emitRotMatch( "Rot=(PITCH=2)", "Rot=", 100 );
	emitRotMatch( "Rot=(4,5,6)", "Rot=", 1 );
	emitRotMatch( "Rot=(4,5)", "Rot=", -1 );
	return 0;
}

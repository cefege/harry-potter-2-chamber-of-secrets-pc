/*=============================================================================
	UnGnuG.h: Unreal definitions for Gnu G++. Unfinished. Unsupported.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.
=============================================================================*/

/*----------------------------------------------------------------------------
	Platform compiler definitions.
----------------------------------------------------------------------------*/

#include <cstddef>
#include <cstdint>
#include <cwchar>
#include <cstring>
#include <cstdarg>
#if defined(__APPLE__)
	#include <alloca.h>
#endif

#if defined(__APPLE__) && (defined(__aarch64__) || defined(__arm64__))
	#ifndef __UNIX__
		#define __UNIX__ 1
	#endif
	#define __MACOS__ 1
	#define __ARM64__ 1
	#define __INTEL_BYTE_ORDER__ 1
	#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
		#error Apple arm64 builds require little-endian byte order.
	#endif
	#undef ASM
	#undef ASM3DNOW
	#undef ASMKNI
	#undef ASMLINUX
	#define ASM 0
	#define ASM3DNOW 0
	#define ASMKNI 0
	#define ASMLINUX 0
	#define COMPILER "Compiled with Apple Clang (" __clang_version__ ")"
#elif defined(__LINUX_X86__)
	#define __UNIX__  1
	#define __LINUX__ 1
	#define __INTEL__ 1
	#define __INTEL_BYTE_ORDER__ 1
	#undef ASM
	#undef ASM3DNOW
	#undef ASMKNI
	#define ASMLINUX 1
	#define COMPILER "Compiled with GNU g++ ("__VERSION__")"
#elif defined(__PSX2_EE__)
	#define __UNIX__ 1
	#define __LINUX__ 1
	#define __INTEL__ 1
	#define __INTEL_BYTE_ORDER__ 1
	#undef ASM
	#undef ASM3DNOW
	#undef ASMKNI
	#undef ASMLINUX
	#define ASMPSX2 1
	#define COMPILER "Compiled with PSX2-EE g++ ("__VERSION__")"
#else
	#error Unsupported platform.
#endif

// Stack control.
#include <sys/wait.h>
#include <signal.h>
#include <setjmp.h>
class __Context
{
public:
	__Context() { std::memcpy( Last, Env, sizeof(Last) ); }
	~__Context() { std::memcpy( Env, Last, sizeof(Env) ); }
	static void StaticInit();
	static jmp_buf Env;

protected:
	static void HandleSignal( int Sig );
	static struct sigaction Act_SIGHUP;
	static struct sigaction Act_SIGQUIT;
	static struct sigaction Act_SIGILL;
	static struct sigaction Act_SIGTRAP;
	static struct sigaction Act_SIGIOT;
	static struct sigaction Act_SIGBUS;
	static struct sigaction Act_SIGFPE;
	static struct sigaction Act_SIGSEGV;
	static struct sigaction Act_SIGTERM;
	jmp_buf Last;
};

/*----------------------------------------------------------------------------
	Platform specifics types and defines.
----------------------------------------------------------------------------*/

// Undo any Windows defines.
#undef BYTE
#undef WORD
#undef DWORD
#undef INT
#undef FLOAT
#undef MAXBYTE
#undef MAXWORD
#undef LONG
#undef MAXDWORD
#undef MAXINT
#undef VOID
#undef CDECL

// Opaque operating-system handles always retain host pointer width.
typedef void* HANDLE;
typedef void* HINSTANCE;
typedef void* HMODULE;

// Sizes.
enum {DEFAULT_ALIGNMENT = 16}; // Default boundary to align memory allocations on.
enum {CACHE_LINE_SIZE   = 32}; // Cache line size.
#define GCC_PACK(n) __attribute__((packed,aligned(n)))
//#define GCC_MOVE_ALIGN(n) __attribute__((aligned(n))) __attribute__((section (".bss")))
#define GCC_ALIGN(n) __attribute__((aligned(n)))
#define GCC_MOVE_ALIGN(n)
//#define GCC_ALIGN(n)

// Optimization macros
#define DISABLE_OPTIMIZATION
#define ENABLE_OPTIMIZATION

// Function type macros.
#define DLL_IMPORT
#define DLL_EXPORT			extern "C"
#define DLL_EXPORT_CLASS
#define VARARGS
#define CDECL
#define STDCALL
#define FORCEINLINE /* Force code to be inline */
#define ZEROARRAY 0 /* Zero-length arrays in structs */
#define __cdecl

// MSVC's wide printf treats an unqualified %s as a wide string, and the
// original engine relies on that contract throughout config, paths, and
// logging. POSIX vswprintf cannot honor it: %ls/%lc convert through the
// current LC_CTYPE, so any codepoint above U+00FF fails (EILSEQ) under the C
// locale and vswprintf returns -1 leaving the destination unspecified —
// FString::Printf then silently propagates stale buffer bytes (the root cause
// of ANSI-folded ini Flush output). This formatter therefore never routes
// strings or chars through the locale: wide strings/chars are spliced
// verbatim into the destination; only numeric conversions are delegated to
// vswprintf, one conversion at a time through a va_copy snapshot.
enum AppVswLength { APPVSW_NONE, APPVSW_H, APPVSW_HH, APPVSW_L, APPVSW_LL, APPVSW_Z, APPVSW_J, APPVSW_T, APPVSW_BIGL };

static inline bool appVswprintfPut( wchar_t* Dest, std::size_t Count, std::size_t& Out, wchar_t Ch )
{
	if( Out + 1 >= Count )
		return false;
	Dest[Out++] = Ch;
	return true;
}
static inline bool appVswprintfAppendText( wchar_t* Dest, std::size_t Count, std::size_t& Out, const wchar_t* Text )
{
	for( std::size_t Index = 0; Text[Index]; ++Index )
		if( !appVswprintfPut(Dest, Count, Out, Text[Index]) )
			return false;
	return true;
}
static inline bool appVswprintfAppendInt( wchar_t* Dest, std::size_t Count, std::size_t& Out, int Value )
{
	wchar_t Digits[16];
	std::size_t DigitCount = 0;
	unsigned int Magnitude = Value < 0 ? 0u - static_cast<unsigned int>(Value) : static_cast<unsigned int>(Value);
	do
	{
		Digits[DigitCount++] = static_cast<wchar_t>(L'0' + Magnitude % 10u);
		Magnitude /= 10u;
	} while( Magnitude );
	if( Value < 0 && !appVswprintfPut(Dest, Count, Out, L'-') )
		return false;
	while( DigitCount )
		if( !appVswprintfPut(Dest, Count, Out, Digits[--DigitCount]) )
			return false;
	return true;
}
// Advances Args past exactly one argument of the promoted type the pending
// numeric conversion consumes (mirrors the single-conversion va_copy handed
// to vswprintf above).
static inline void appVswprintfSkipNumeric( va_list& Args, AppVswLength Length, wchar_t Conversion )
{
	switch( Conversion )
	{
		case L'd': case L'i': case L'o': case L'u': case L'x': case L'X':
			switch( Length )
			{
				case APPVSW_LL: (void)va_arg(Args, long long);      break;
				case APPVSW_L:  (void)va_arg(Args, long);           break;
				case APPVSW_Z:  (void)va_arg(Args, std::size_t);    break;
				case APPVSW_J:  (void)va_arg(Args, std::intmax_t);  break;
				case APPVSW_T:  (void)va_arg(Args, std::ptrdiff_t); break;
				default:        (void)va_arg(Args, int);            break;
			}
			break;
		case L'f': case L'F': case L'e': case L'E': case L'g': case L'G': case L'a': case L'A':
			(void)va_arg(Args, double);
			break;
		case L'p':
			(void)va_arg(Args, void*);
			break;
		case L'n':
			(void)va_arg(Args, int*);
			break;
		default:
			break;
	}
}
static inline int appVswprintfCompat( wchar_t* Dest, std::size_t Count, const wchar_t* Format, va_list Args )
{
	if( !Count )
		return -1;
	std::size_t Out = 0;
	for( std::size_t In = 0; Format[In]; )
	{
		if( Format[In] != L'%' )
		{
			if( !appVswprintfPut(Dest, Count, Out, Format[In++]) )
				goto Overflow;
			continue;
		}
		++In;
		if( Format[In] == L'%' )
		{
			if( !appVswprintfPut(Dest, Count, Out, L'%') )
				goto Overflow;
			++In;
			continue;
		}
		{
			wchar_t Segment[64];
			std::size_t SegOut = 0;
			auto PutSeg = [&](wchar_t Ch)
			{
				if( SegOut + 1 < sizeof(Segment) / sizeof(Segment[0]) )
					Segment[SegOut++] = Ch;
			};
			PutSeg( L'%' );
			// Flags.
			for(;; ++In)
			{
				const wchar_t Ch = Format[In];
				if( Ch != L'-' && Ch != L'+' && Ch != L' ' && Ch != L'#' && Ch != L'0' )
					break;
				PutSeg( Ch );
			}
			// Width ('*' consumes an int argument and is resolved literally).
			if( Format[In] == L'*' )
			{
				++In;
				const int Width = va_arg(Args, int);
				if( Width < 0 )
					PutSeg( L'-' );
				if( !appVswprintfAppendInt(Segment, sizeof(Segment)/sizeof(Segment[0]), SegOut, Width < 0 ? -Width : Width) )
					goto Overflow;
			}
			else for(; Format[In] >= L'0' && Format[In] <= L'9'; ++In)
				PutSeg( Format[In] );
			// Precision ('*' likewise; a negative precision means "none").
			if( Format[In] == L'.' )
			{
				++In;
				PutSeg( L'.' );
				if( Format[In] == L'*' )
				{
					++In;
					const int Precision = va_arg(Args, int);
					if( Precision >= 0
						&& !appVswprintfAppendInt(Segment, sizeof(Segment)/sizeof(Segment[0]), SegOut, Precision) )
						goto Overflow;
				}
				else for(; Format[In] >= L'0' && Format[In] <= L'9'; ++In)
					PutSeg( Format[In] );
			}
			// Length modifier.
			AppVswLength Length = APPVSW_NONE;
			for(;; ++In)
			{
				const wchar_t Ch = Format[In];
				if( Ch == L'l' ) { if( Length == APPVSW_L ) Length = APPVSW_LL; else Length = APPVSW_L; }
				else if( Ch == L'h' ) { if( Length == APPVSW_H ) Length = APPVSW_HH; else Length = APPVSW_H; }
				else if( Ch == L'z' ) Length = APPVSW_Z;
				else if( Ch == L'j' ) Length = APPVSW_J;
				else if( Ch == L't' ) Length = APPVSW_T;
				else if( Ch == L'L' ) Length = APPVSW_BIGL;
				else break;
				PutSeg( Ch );
			}
			const wchar_t Conversion = Format[In];
			if( !Conversion )
				goto Overflow;
			PutSeg( Conversion );
			Segment[SegOut] = 0;
			++In;
			switch( Conversion )
			{
				case L's':
					if( Length == APPVSW_H || Length == APPVSW_HH )
					{
						// Narrow string widened byte-for-byte (ASCII contract,
						// no locale): matches MSVC %hs semantics closely enough
						// for this engine's ASCII-only narrow literals.
						for( const char* Narrow = va_arg(Args, const char*); Narrow && *Narrow; ++Narrow )
							if( !appVswprintfPut(Dest, Count, Out, static_cast<wchar_t>(*Narrow)) )
								goto Overflow;
					}
					else
					{
						const wchar_t* Wide = va_arg(Args, const wchar_t*);
						if( !Wide )
							Wide = L"(null)";
						if( !appVswprintfAppendText(Dest, Count, Out, Wide) )
							goto Overflow;
					}
					break;
				case L'c':
				{
					const int Code = va_arg(Args, int);
					wchar_t Wide = 0;
					if( Length == APPVSW_H || Length == APPVSW_HH )
						Wide = static_cast<wchar_t>(static_cast<char>(Code));
					else
						Wide = static_cast<wchar_t>(Code);
					if( !appVswprintfPut(Dest, Count, Out, Wide) )
						goto Overflow;
					break;
				}
				case L'd': case L'i': case L'u': case L'o': case L'x': case L'X':
				case L'f': case L'F': case L'e': case L'E': case L'g': case L'G':
				case L'a': case L'A': case L'p': case L'n':
				{
					wchar_t Number[512];
					va_list Snapshot;
					va_copy(Snapshot, Args);
					const int Written = std::vswprintf(Number, sizeof(Number)/sizeof(Number[0]), Segment, Snapshot);
					va_end(Snapshot);
					if( Written < 0 || !appVswprintfAppendText(Dest, Count, Out, Number) )
						goto Overflow;
					appVswprintfSkipNumeric(Args, Length, Conversion);
					break;
				}
				default:
					// Unknown conversion: pass it through untouched rather than
					// consuming arguments we cannot classify.
					if( !appVswprintfPut(Dest, Count, Out, L'%') )
						goto Overflow;
					if( SegOut > 1 && !appVswprintfAppendText(Dest, Count, Out, Segment + 1) )
						goto Overflow;
					if( !appVswprintfPut(Dest, Count, Out, Conversion) )
						goto Overflow;
					break;
			}
		}
	}
	Dest[Out] = 0;
	return static_cast<int>(Out);
Overflow:
	Dest[0] = 0;
	return -1;
}

#if defined(__MACOS__)
	#define GET_VARARGS(msg,len,fmt) \
	{ \
		va_list ArgPtr; \
		va_start( ArgPtr, fmt ); \
		appVswprintfCompat( msg, len, fmt, ArgPtr ); \
		va_end( ArgPtr ); \
	}
	#define GET_VARARGS_RESULT(msg,len,fmt,result) \
	{ \
		va_list ArgPtr; \
		va_start( ArgPtr, fmt ); \
		result = appVswprintfCompat( msg, len, fmt, ArgPtr ); \
		va_end( ArgPtr ); \
	}
#else
	#define GET_VARARGS(msg,len,fmt) \
	{ \
		va_list ArgPtr; \
		va_start( ArgPtr, fmt ); \
		vsprintf( msg, fmt, ArgPtr ); \
		va_end( ArgPtr ); \
	}
	#define GET_VARARGS_RESULT(msg,len,fmt,result) \
	{ \
		va_list ArgPtr; \
		va_start( ArgPtr, fmt ); \
		result = vsprintf( msg, fmt, ArgPtr ); \
		va_end( ArgPtr ); \
	}
#endif

// Unsigned base types.
typedef std::uint8_t		BYTE;		// 8-bit  unsigned.
typedef std::uint16_t		_WORD;		// 16-bit unsigned.
typedef std::uint32_t		DWORD;		// 32-bit unsigned.
typedef std::uint64_t		QWORD;		// 64-bit unsigned.
#if defined(__SIZEOF_INT128__)
typedef unsigned __int128	OWORD;
#endif

// Signed base types.
typedef std::int8_t			SBYTE;		// 8-bit  signed.
typedef std::int16_t		SWORD;		// 16-bit signed.
typedef std::int32_t		INT;		// 32-bit signed.
typedef std::int32_t		LONG;		// Windows-compatible 32-bit signed long.
typedef std::int64_t		SQWORD;		// 64-bit signed.

// Host pointer and size types.
typedef std::intptr_t		PTRINT;
typedef std::uintptr_t		UPTRINT;
typedef std::size_t			SIZE_T;

// Character types. UNICHAR is always one serialized UTF-16 code unit.
typedef char				ANSICHAR;
typedef std::uint16_t		UNICHAR;
typedef std::uint8_t		ANSICHARU;
typedef std::uint16_t		UNICHARU;

// Other base types.
typedef std::int32_t		UBOOL;		// Boolean 0 (false) or 1 (true).
typedef float				FLOAT;		// 32-bit IEEE floating point.
typedef double				DOUBLE;		// 64-bit IEEE floating point.

// Bitfield type.
typedef std::uint32_t		BITFIELD;

static_assert(sizeof(BYTE) == 1, "BYTE must remain 8-bit");
static_assert(sizeof(_WORD) == 2, "_WORD must remain 16-bit");
static_assert(sizeof(DWORD) == 4, "DWORD must remain 32-bit");
static_assert(sizeof(QWORD) == 8, "QWORD must remain 64-bit");
static_assert(sizeof(SBYTE) == 1, "SBYTE must remain 8-bit");
static_assert(sizeof(SWORD) == 2, "SWORD must remain 16-bit");
static_assert(sizeof(INT) == 4, "INT must remain 32-bit");
static_assert(sizeof(LONG) == 4, "LONG must remain 32-bit");
static_assert(sizeof(SQWORD) == 8, "SQWORD must remain 64-bit");
static_assert(sizeof(UBOOL) == 4, "UBOOL must remain 32-bit");
static_assert(sizeof(FLOAT) == 4, "FLOAT must remain 32-bit");
static_assert(sizeof(BITFIELD) == 4, "BITFIELD must remain 32-bit");
static_assert(sizeof(UNICHAR) == 2, "UNICHAR must remain one UTF-16 code unit");
static_assert(sizeof(PTRINT) == sizeof(void*), "PTRINT must retain host pointer width");
static_assert(sizeof(UPTRINT) == sizeof(void*), "UPTRINT must retain host pointer width");
static_assert(sizeof(SIZE_T) == sizeof(void*), "SIZE_T must retain host pointer width");
static_assert(sizeof(HANDLE) == sizeof(void*), "HANDLE must retain host pointer width");
static_assert(sizeof(HINSTANCE) == sizeof(void*), "HINSTANCE must retain host pointer width");
static_assert(sizeof(HMODULE) == sizeof(void*), "HMODULE must retain host pointer width");
#if defined(__APPLE__) && (defined(__aarch64__) || defined(__arm64__))
static_assert(sizeof(void*) == 8, "Apple arm64 requires an LP64 host ABI");
static_assert(sizeof(wchar_t) == 4, "Apple arm64 wchar_t must be UTF-32 width");
#endif

// Make sure characters are unsigned.
#ifdef __CHAR_UNSIGNED__
	#error "Bad compiler option: Characters must be signed"
#endif

// Strings.
#if defined(__APPLE__)
#define LINE_TERMINATOR TEXT("\n")
#define PATH_SEPARATOR TEXT("/")
#define DLLEXT TEXT(".dylib")
#elif __UNIX__
#define LINE_TERMINATOR TEXT("\n")
#define PATH_SEPARATOR TEXT("/")
#define DLLEXT TEXT(".so")
#else
#define LINE_TERMINATOR TEXT("\r\n")
#define PATH_SEPARATOR TEXT("\\")
#define DLLEXT TEXT(".dll")
#endif

// NULL.
#undef NULL
#define NULL 0

// Package implementation.
#define IMPLEMENT_PACKAGE_PLATFORM(pkgname) \
	BYTE GLoaded##pkgname;

// Platform support options.
#define PLATFORM_NEEDS_ARRAY_NEW 1
#define FORCE_ANSI_LOG           0

// OS unicode function calling.
#define TCHAR_CALL_OS(funcW,funcA) (funcA)
#if defined(__APPLE__)
	#define TCHAR_TO_ANSI(str) appToAnsi(str)
	#define ANSI_TO_TCHAR(str) appFromAnsi(str)
#else
	#define TCHAR_TO_ANSI(str) str
	#define ANSI_TO_TCHAR(str) str
#endif

// !! Fixme: This is a workaround.
#define GCC_OPT_INLINE

// Memory
#define appAlloca(size) alloca((size+7)&~7)

extern "C" { extern CORE_API UBOOL GTimestamp; }
extern CORE_API FLOAT GSecondsPerCycle;
CORE_API FTime appSecondsSlow();

//
// Round a floating point number to an integer.
// Note that (int+.5) is rounded to (int+1).
//
#define DEFINED_appRound 1
inline INT appRound( FLOAT f )
{
#if __PSX2_EE__
	register int r;
	__asm__ __volatile__(
	"
		cvt.w.s %1,%1
		mfc1 %0,%1
	"
	:"=r"(r)
	:"$f"(f)
	);
	return r;
#else
	return static_cast<INT>(f);
#endif
}

//
// Converts to integer equal to or less than.
//
#define DEFINED_appFloor 1
inline INT appFloor( FLOAT f )
{
#if __PSX2_EE__
	register int r;
	__asm__ __volatile__(
	"
		cvt.w.s %1,%1
		mfc1 %0,%1
	"
	:"=r"(r)
	:"$f"(f)
	);
	return r;
#else
	return static_cast<INT>(f);
#endif
}

//
// CPU cycles, related to GSecondsPerCycle.
//
#if ASMLINUX
#define DEFINED_appCycles 1
inline DWORD appCycles()
{
	if( GTimestamp )
	{
		DWORD r;
		asm("rdtsc" : "=a" (r) : "d" (r));
		return r;
	}
}
#endif

//
// Seconds, arbitrarily based.
//
#if ASMLINUX
#define DEFINED_appSeconds 1
inline FTime appSeconds()
{
	if( GTimestamp )
	{
		DWORD L,H;
		asm("rdtsc" : "=a" (L), "=d" (H));
		return ((double)L +  4294967296.0 * (double)H) * GSecondsPerCycle;
	}
	else return appSecondsSlow();
}
#endif

//
// Memory copy.
//
#if ASMLINUX
#define DEFINED_appMemcpy 1
inline void appMemcpy( void* Dest, const void* Src, INT Count )
{
	asm volatile("
		pushl %%ebx;
		pushl %%ecx;
		pushl %%esi;
		pushl %%edi;
		mov %%ecx, %%ebx;
		shr $2, %%ecx;
		and $3, %%ebx;
		rep;
		movsl;
		mov %%ebx, %%ecx;
		rep;
		movsb;
		popl %%edi;
		popl %%esi;
		popl %%ecx;
		popl %%ebx;
	"
	:
	: "S" (Src),
	  "D" (Dest),
	  "c" (Count)
	);
}
#endif

//
// Memory zero.
//
#define DEFINED_appMemzero 1
inline void appMemzero( void* Dest, INT Count )
{
	memset( Dest, 0, Count );
}

/*----------------------------------------------------------------------------
	Globals.
----------------------------------------------------------------------------*/

// System identification.
extern "C"
{
	extern HINSTANCE      hInstance;
	extern CORE_API UBOOL GIsMMX;
	extern CORE_API UBOOL GIsPentiumPro;
	extern CORE_API UBOOL GIsKatmai;
	extern CORE_API UBOOL GIsK6;
	extern CORE_API UBOOL GIs3DNow;
	extern CORE_API UBOOL GTimestamp;
}

// Module name
extern ANSICHAR GModule[32];

/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/

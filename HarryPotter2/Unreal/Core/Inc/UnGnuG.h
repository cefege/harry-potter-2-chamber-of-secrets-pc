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

// MSVC's wide printf treats an unqualified %s as a wide string. The original
// engine relies on that contract throughout config, paths, and logging, while
// POSIX vswprintf requires %ls. Translate only unqualified string conversions.
static inline int appVswprintfCompat(wchar_t* Dest, std::size_t Count, const wchar_t* Format, va_list Args)
{
	wchar_t Converted[4096];
	std::size_t Out = 0;
	for (std::size_t In = 0; Format[In];)
	{
		if (Out + 2 >= sizeof(Converted) / sizeof(Converted[0]))
		{
			if (Count) Dest[0] = 0;
			return -1;
		}
		if (Format[In] != L'%')
		{
			Converted[Out++] = Format[In++];
			continue;
		}
		Converted[Out++] = Format[In++];
		if (Format[In] == L'%')
		{
			Converted[Out++] = Format[In++];
			continue;
		}
		bool HasNarrowModifier = false;
		bool HasLongModifier = false;
		for (;;)
		{
			const wchar_t Ch = Format[In++];
			if (!Ch)
			{
				if (Count) Dest[0] = 0;
				return -1;
			}
			const bool IsConversion = std::wcschr(L"diouxXfFeEgGaAcspn", Ch) != nullptr;
			if (IsConversion)
			{
				if (Ch == L's' && !HasLongModifier && !HasNarrowModifier)
					Converted[Out++] = L'l';
				Converted[Out++] = Ch;
				break;
			}
			if (Ch == L'l')
				HasLongModifier = true;
			if (Ch == L'h')
				HasNarrowModifier = true;
			Converted[Out++] = Ch;
			if (Out + 2 >= sizeof(Converted) / sizeof(Converted[0]))
			{
				if (Count) Dest[0] = 0;
				return -1;
			}
		}
	}
	Converted[Out] = 0;
	return std::vswprintf(Dest, Count, Converted, Args);
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

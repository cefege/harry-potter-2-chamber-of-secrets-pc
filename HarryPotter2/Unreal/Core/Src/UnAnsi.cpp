/*=============================================================================
	UnFile.cpp: ANSI C core.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

// Core includes.
#include "CorePrivate.h"

// To help ANSI out.
#undef clock
#undef unclock

// ANSI C++ includes.
#include <math.h>
#include <float.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#include <wctype.h>
#include <sys/types.h>
#include <sys/stat.h>

#ifdef _WIN32
#include <io.h>
#endif

/*-----------------------------------------------------------------------------
	Time.
-----------------------------------------------------------------------------*/

//
// String timestamp.
// !! Note to self: Move to UnVcWin32.cpp
// !! Make Linux version.
//
#if _MSC_VER
CORE_API const TCHAR* appTimestamp()
{
	guard(appTimestamp);
	static TCHAR Result[1024];
	*Result = 0;
#if UNICODE
	if( GUnicodeOS )
	{
		_wstrdate( Result );
		appStrcat( Result, TEXT(" ") );
		_wstrtime( Result + appStrlen(Result) );
	}
	else
#endif
	{
		ANSICHAR Temp[1024]="";
		_strdate( Temp );
		appStrcpy( Result, appFromAnsi(Temp) );
		appStrcat( Result, TEXT(" ") );
		_strtime( Temp );
		appStrcat( Result, appFromAnsi(Temp) );
	}
	return Result;
	unguard;
}
#endif

//
// Get a GMT Ref
//
CORE_API FString appGetGMTRef()
{
	#if __PSX2_EE__
	#else
	guard(appGetGMTRef);
	struct tm *newtime;
	TCHAR* GMTRef = appStaticString1024();
	time_t ltime, gtime;
	FLOAT diff;
	time( &ltime );
	newtime = gmtime( &ltime );
	gtime = mktime(newtime);
	diff = (ltime - gtime) / 3600;
	appSprintf( GMTRef, (diff>0)?TEXT("+%1.1f"):TEXT("%1.1f"), diff );
	return GMTRef;
	unguard;
	#endif
}

/*-----------------------------------------------------------------------------
	Math functions.
-----------------------------------------------------------------------------*/

CORE_API DOUBLE appExp( DOUBLE Value )
{
	return exp(Value);
}
CORE_API DOUBLE appLoge( DOUBLE Value )
{
	return log(Value);
}
CORE_API DOUBLE appFmod( DOUBLE Y, DOUBLE X )
{
	return fmod(Y,X);
}
CORE_API DOUBLE appSin( DOUBLE Value )
{
	return sin(Value);
}
CORE_API DOUBLE appCos( DOUBLE Value )
{
	return cos(Value);
}
CORE_API DOUBLE appAcos( DOUBLE Value )
{
	return acos(Value);
}
CORE_API DOUBLE appTan( DOUBLE Value )
{
	return tan(Value);
}
CORE_API DOUBLE appAtan( DOUBLE Value )
{
	return atan(Value);
}
CORE_API DOUBLE appAtan2( DOUBLE Y, DOUBLE X )
{
	return atan2(Y,X);
}
CORE_API DOUBLE appSqrt( DOUBLE Value )
{
	return sqrt(Value);
}
CORE_API DOUBLE appPow( DOUBLE A, DOUBLE B )
{
	return pow(A,B);
}
CORE_API UBOOL appIsNan( DOUBLE A )
{
#if _MSC_VER
	return _isnan(A)==1;
#else
	return isnan(A)==1;
#endif
}
CORE_API INT appRand()
{
	return rand();
}
CORE_API FLOAT appFrand()
{
	return rand() / (FLOAT)RAND_MAX;
}						  
CORE_API FLOAT appFrand(FLOAT From, FLOAT To)
{
	FLOAT r = FLOAT(rand()) / FLOAT(RAND_MAX);
	return (To - From) * r + From;
}
#if 1 //Fix added by Legend on 4/12/2000
CORE_API FLOAT appRandRange( FLOAT Min, FLOAT Max )
{
	return Min + (Max - Min) * appFrand();
}
CORE_API INT appRandRange( INT Min, INT Max )
{
	INT Range = Max - Min;
	INT R = Range>0 ? appRand() % Range : 0;
	return R + Min;
}
#endif

#if !DEFINED_appFloor
CORE_API INT appFloor( FLOAT Value )
{
	return (INT)floor(Value);
}
#endif

#if !DEFINED_appCeil
CORE_API INT appCeil( FLOAT Value )
{
	return (INT)ceil(Value);
}
#endif

#if !DEFINED_appRound
CORE_API INT appRound( FLOAT Value )
{
	return (INT)floor(Value + 0.5);
}
#endif

/*-----------------------------------------------------------------------------
	Memory functions.
-----------------------------------------------------------------------------*/

CORE_API INT appMemcmp( const void* Buf1, const void* Buf2, INT Count )
{
	return memcmp( Buf1, Buf2, Count );
}

CORE_API UBOOL appMemIsZero( const void* V, int Count )
{
	guardSlow(appMemIsZero);
	BYTE* B = (BYTE*)V;
	while( Count-- > 0 )
		if( *B++ != 0 )
			return 0;
	return 1;
	unguardSlow;
}

CORE_API void* appMemmove( void* Dest, const void* Src, INT Count )
{
	return memmove( Dest, Src, Count );
}

CORE_API void appMemset( void* Dest, INT C, INT Count )
{
	memset( Dest, C, Count );
}

#ifndef DEFINED_appMemzero
CORE_API void appMemzero( void* Dest, INT Count )
{
	memset( Dest, 0, Count );
}
#endif

#ifndef DEFINED_appMemcpy
CORE_API void appMemcpy( void* Dest, const void* Src, INT Count )
{
	memcpy( Dest, Src, Count );
}
#endif

/*-----------------------------------------------------------------------------
	String functions.
-----------------------------------------------------------------------------*/

//
// Copy a string with length checking.
//warning: Behavior differs from strncpy; last character is zeroed.
//
TCHAR* appStrncpy( TCHAR* Dest, const TCHAR* Src, INT MaxLen )
{
	guard(appStrncpy);

#if UNICODE
	wcsncpy( Dest, Src, MaxLen );
#else
	strncpy( Dest, Src, MaxLen );
#endif
	Dest[MaxLen-1]=0;
	return Dest;

	unguard;
}

//
// Concatenate a string with length checking
//
TCHAR* appStrncat( TCHAR* Dest, const TCHAR* Src, INT MaxLen )
{
	guard(appStrncat);
	INT Len = appStrlen(Dest);
	TCHAR* NewDest = Dest + Len;
	if( (MaxLen-=Len) > 0 )
	{
		appStrncpy( NewDest, Src, MaxLen );
		NewDest[MaxLen-1] = 0;
	}
	return Dest;
	unguard;
}

//
// Standard string functions.
//
CORE_API INT appSprintf( TCHAR* Dest, const TCHAR* Fmt, ... )
{
#if _MSC_VER
	return GET_VARARGS(Dest,1024/*!!*/,Fmt);
#else
	int Result;
	GET_VARARGS_RESULT(Dest,1024,Fmt,Result);
#endif
	return Result;
}

#if _MSC_VER
CORE_API INT appGetVarArgs( TCHAR* Dest, INT Count, const TCHAR*& Fmt )
{
	va_list ArgPtr;
	va_start( ArgPtr, Fmt );
#if UNICODE
	INT Result = _vsnwprintf( Dest, Count, Fmt, ArgPtr );
#else
	INT Result = _vsnprintf( Dest, Count, Fmt, ArgPtr );
#endif
	va_end( ArgPtr );
	return Result;
}
#endif

CORE_API INT appStrlen( const TCHAR* String )
{
#if UNICODE
	return appCheckedIntSize( wcslen(String) );
#else
	return strlen( String );
#endif
}

CORE_API TCHAR* appStrstr( const TCHAR* String, const TCHAR* Find )
{
#if UNICODE
	return const_cast<TCHAR*>(wcsstr(String, Find));
#else
	return const_cast<TCHAR*>(strstr(String, Find));
#endif
}

CORE_API TCHAR* appStrchr( const TCHAR* String, int c )
{
#if __PSX2_EE__
	FString s = FString::Printf(TEXT("%c"), c);
	return appStrstr( String, *s );
#elif UNICODE
	return const_cast<TCHAR*>(wcschr(String, c));
#else
	return const_cast<TCHAR*>(strchr(String, c));
#endif
}

CORE_API TCHAR* appStrcat( TCHAR* Dest, const TCHAR* Src )
{
#if UNICODE
	return wcscat( Dest, Src );
#else
	return strcat( Dest, Src );
#endif
}

CORE_API INT appStrcmp( const TCHAR* String1, const TCHAR* String2 )
{
#if UNICODE
	return wcscmp( String1, String2 );
#else
	return strcmp( String1, String2 );
#endif
}

CORE_API INT appStricmp( const TCHAR* String1, const TCHAR* String2 )
{
#if UNICODE
	return wcscasecmp( String1, String2 );
#else
	return stricmp( String1, String2 );
#endif
}

CORE_API TCHAR* appStrcpy( TCHAR* Dest, const TCHAR* Src )
{
#if UNICODE
	return wcscpy( Dest, Src );
#else
	return strcpy( Dest, Src );
#endif
}

CORE_API TCHAR* appStrupr( TCHAR* String )
{
#if UNICODE
	for( TCHAR* Ch=String; *Ch; ++Ch )
		*Ch = static_cast<TCHAR>(towupper(*Ch));
	return String;
#else
	return strupr( String );
#endif
}

CORE_API INT appAtoi( const TCHAR* Str )
{
#if UNICODE
	const long Value = wcstol( Str, NULL, 10 );
	check( Value>=-static_cast<long>(MAXINT)-1 && Value<=static_cast<long>(MAXINT) );
	return static_cast<INT>(Value);
#else
	return atoi( Str );
#endif
}

CORE_API FLOAT appAtof( const TCHAR* Str )
{
	return atof( appToAnsi(Str) );
}

CORE_API INT appStrtoi( const TCHAR* Start, TCHAR** End, INT Base )
{
#if UNICODE
	const long Value = wcstol( Start, End, Base );
	check( Value>=-static_cast<long>(MAXINT)-1 && Value<=static_cast<long>(MAXINT) );
	return static_cast<INT>(Value);
#else
	return strtol( Start, End, Base );
#endif
}

CORE_API INT appStrncmp( const TCHAR* A, const TCHAR* B, INT Count )
{
#if UNICODE
	return wcsncmp( A, B, Count );
#else
	return strncmp( A, B, Count );
#endif
}

CORE_API INT appStrnicmp( const TCHAR* A, const TCHAR* B, INT Count )
{
#if UNICODE
	return wcsncasecmp( A, B, Count );
#else
	return strnicmp( A, B, Count );
#endif
}

CORE_API UBOOL appToUtf8InPlace( ANSICHAR* Dest, const TCHAR* Src, INT DestCapacity )
{
	if( !Dest || !Src || DestCapacity<=0 )
		return 0;
	INT DestIndex = 0;
	for( ; *Src; ++Src )
	{
		const DWORD Scalar = static_cast<DWORD>(*Src);
		ANSICHAR Encoded[4];
		INT EncodedLength;
		if( Scalar<=0x7f )
		{
			Encoded[0] = static_cast<ANSICHAR>(Scalar);
			EncodedLength = 1;
		}
		else if( Scalar<=0x7ff )
		{
			Encoded[0] = static_cast<ANSICHAR>(0xc0 | (Scalar >> 6));
			Encoded[1] = static_cast<ANSICHAR>(0x80 | (Scalar & 0x3f));
			EncodedLength = 2;
		}
		else if( Scalar<=0xffff && !(Scalar>=0xd800 && Scalar<=0xdfff) )
		{
			Encoded[0] = static_cast<ANSICHAR>(0xe0 | (Scalar >> 12));
			Encoded[1] = static_cast<ANSICHAR>(0x80 | ((Scalar >> 6) & 0x3f));
			Encoded[2] = static_cast<ANSICHAR>(0x80 | (Scalar & 0x3f));
			EncodedLength = 3;
		}
		else if( Scalar<=0x10ffff )
		{
			Encoded[0] = static_cast<ANSICHAR>(0xf0 | (Scalar >> 18));
			Encoded[1] = static_cast<ANSICHAR>(0x80 | ((Scalar >> 12) & 0x3f));
			Encoded[2] = static_cast<ANSICHAR>(0x80 | ((Scalar >> 6) & 0x3f));
			Encoded[3] = static_cast<ANSICHAR>(0x80 | (Scalar & 0x3f));
			EncodedLength = 4;
		}
		else
		{
			Dest[0] = 0;
			return 0;
		}
		if( EncodedLength>=DestCapacity-DestIndex )
		{
			Dest[0] = 0;
			return 0;
		}
		for( INT Index=0; Index<EncodedLength; ++Index )
			Dest[DestIndex++] = Encoded[Index];
	}
	Dest[DestIndex] = 0;
	return 1;
}

CORE_API UBOOL appFromUtf8InPlace( TCHAR* Dest, const ANSICHAR* Src, INT DestCapacity )
{
	if( !Dest || !Src || DestCapacity<=0 )
		return 0;
	INT DestIndex = 0;
	while( *Src )
	{
		const BYTE First = static_cast<BYTE>(*Src++);
		DWORD Scalar;
		INT Continuations;
		DWORD Minimum;
		if( First<0x80 )
		{
			Scalar = First;
			Continuations = 0;
			Minimum = 0;
		}
		else if( (First & 0xe0)==0xc0 )
		{
			Scalar = First & 0x1f;
			Continuations = 1;
			Minimum = 0x80;
		}
		else if( (First & 0xf0)==0xe0 )
		{
			Scalar = First & 0x0f;
			Continuations = 2;
			Minimum = 0x800;
		}
		else if( (First & 0xf8)==0xf0 )
		{
			Scalar = First & 0x07;
			Continuations = 3;
			Minimum = 0x10000;
		}
		else
		{
			Dest[0] = 0;
			return 0;
		}
		for( INT Index=0; Index<Continuations; ++Index )
		{
			const BYTE Next = static_cast<BYTE>(*Src++);
			if( !Next || (Next & 0xc0)!=0x80 )
			{
				Dest[0] = 0;
				return 0;
			}
			Scalar = (Scalar << 6) | (Next & 0x3f);
		}
		if( Scalar<Minimum || Scalar>0x10ffff || (Scalar>=0xd800 && Scalar<=0xdfff)
		||	DestIndex>=DestCapacity-1 )
		{
			Dest[0] = 0;
			return 0;
		}
		Dest[DestIndex++] = static_cast<TCHAR>(Scalar);
	}
	Dest[DestIndex] = 0;
	return 1;
}

/*-----------------------------------------------------------------------------
	Sorting.
-----------------------------------------------------------------------------*/

CORE_API void appQsort( void* Base, INT Num, INT Width, int(CDECL *Compare)(const void* A, const void* B ) )
{
	qsort( Base, Num, Width, Compare );
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

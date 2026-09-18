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
#include <limits.h>
#include <atomic>
#include <mutex>
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

/*-----------------------------------------------------------------------------
	Random tracing.
-----------------------------------------------------------------------------*/

namespace
{
	enum
	{
		HP2_RAND_TRACE_DEFAULT_LIMIT       = 16384,
		HP2_RAND_TRACE_MAX_LIMIT           = 262144,
		HP2_RAND_TRACE_MAX_CONTEXT_BYTES   = 32 * 1024 * 1024,
		HP2_RAND_TRACE_PHASE_CAPACITY      = 48,
		HP2_RAND_TRACE_MAP_CAPACITY        = 256,
		HP2_RAND_TRACE_MAP_URL_CAPACITY    = 1024,
	};

	struct FAppRandTraceContext
	{
		FAppRandTraceContext* Previous;
		UObject* Object;
		UObject* Function;
		const ANSICHAR* Native;
		INT NativeSlot;
		INT CallsiteOffset;
	};

	struct FAppRandTraceSample
	{
		unsigned long long Ordinal;
		INT Value;
		unsigned long long TickIndex;
		char Phase[HP2_RAND_TRACE_PHASE_CAPACITY];
		UBOOL HasConsumer;
		char* Actor;
		char* Class;
		char* Function;
		char* Native;
		INT NativeSlot;
		INT CallsiteOffset;
		UBOOL HasOuterConsumer;
		char* OuterActor;
		char* OuterClass;
		char* OuterFunction;
	};

	struct FAppRandTraceTickBoundary
	{
		unsigned long long Index;
		char Phase[HP2_RAND_TRACE_PHASE_CAPACITY];
	};

	struct FAppRandTraceFunctionEvent
	{
		unsigned long long CallId;
		unsigned long long EntryOrdinal;
		unsigned long long ExitOrdinal;
		UBOOL HasExit;
		char* Function;
		char* Receiver;
		char* Actor;
		char* RootActor;
		char* RootClass;
		char* RootFunction;
	};

	struct FAppRandTraceRelevanceEvent
	{
		FAppRandTraceRelevanceEvent* Previous;
		unsigned long long EntryOrdinal;
		unsigned long long ExitOrdinal;
		UBOOL HasExit;
		UBOOL HasAlwaysKeepResult;
		UBOOL AlwaysKeepResult;
		UBOOL HasIsRelevantResult;
		UBOOL IsRelevantResult;
		UBOOL HasSuperRelevant;
		BYTE SuperRelevant;
		UBOOL HasFinalResult;
		UBOOL FinalResult;
		INT ReturnSourceLine;
		const char* EarlyReturnBranch;
		INT DirectCallDepth;
		char* Function;
		char* Receiver;
		char* Other;
		char* OtherClass;
		char* BaseMutator;
		char* BaseMutatorClass;
	};

	struct FAppRandTracePreBeginEvent
	{
		FAppRandTracePreBeginEvent* Previous;
		UObject* ActorObject;
		unsigned long long EntryOrdinal;
		unsigned long long ExitOrdinal;
		UBOOL HasExit;
		UBOOL GameRelevant;
		UBOOL RelevantInSoftwareRenderer;
		UBOOL DeleteMeAtEntry;
		UBOOL DeleteMeAtExit;
		UBOOL HasNetMode;
		INT NetMode;
		UBOOL HasSoftwareRendering;
		UBOOL IsSoftwareRendering;
		UBOOL RelevanceWillDispatch;
		UBOOL RelevanceDispatched;
		UBOOL RelevanceCompleted;
		UBOOL HasRelevanceResult;
		UBOOL RelevanceResult;
		UBOOL HasRelevanceOrdinals;
		unsigned long long RelevanceEntryOrdinal;
		unsigned long long RelevanceExitOrdinal;
		UBOOL RelevanceAppRandConsumed;
		UBOOL HasLevelGame;
		char* Actor;
		char* Function;
		char* LevelGame;
	};
	struct FAppRandTracePhaseEvent
	{
		char Phase[HP2_RAND_TRACE_PHASE_CAPACITY];
		char MapToken[HP2_RAND_TRACE_MAP_CAPACITY];
		char MapURL[HP2_RAND_TRACE_MAP_URL_CAPACITY];
		char LifecycleBoundary[HP2_RAND_TRACE_PHASE_CAPACITY];
		unsigned long long LoadOrdinal;
		unsigned long long OrdinalBefore;
		unsigned long long OrdinalAfter;
	};

	enum { HP2_RAND_TRACE_MAX_STARTUP_PHASES = 32 };


	static std::atomic<UBOOL> GAppRandTraceInitialized( 0 );
	static std::mutex GAppRandTraceMutex;
	static UBOOL GAppRandTraceEnabled = 0;
	// Global Tick diagnostics need the same appRand ordinal even when retaining
	// the full RNG sample stream is not requested.
	static UBOOL GAppRandTraceOrdinalEnabled = 0;
	static UBOOL GAppRandTraceStorageAvailable = 0;
	static UBOOL GAppRandTraceTruncated = 0;
	static UBOOL GAppRandTraceFlushed = 0;
	static UBOOL GAppRandTraceReplaySeeded = 0;
	static UBOOL GAppRandTraceDiagnosticSeeded = 0;
	static unsigned GAppRandTraceDiagnosticSeed = 0;
	static unsigned GAppRandTraceDiagnosticRequestedSeed = 0;
	static char GAppRandTracePath[PATH_MAX];
	static size_t GAppRandTraceLimit = 0;
	static size_t GAppRandTraceCount = 0;
	static size_t GAppRandTraceTickBoundaryCount = 0;
	static size_t GAppRandTraceContextBytes = 0;
	static unsigned long long GAppRandTraceCallCount = 0;
	static unsigned long long GAppRandTraceTickIndex = 0;
	static char GAppRandTracePhase[HP2_RAND_TRACE_PHASE_CAPACITY];
	static FAppRandTraceSample* GAppRandTraceSamples = NULL;
	static FAppRandTraceTickBoundary* GAppRandTraceTickBoundaries = NULL;
	static thread_local FAppRandTraceContext* GAppRandTraceContext = NULL;
	static UBOOL GAppRandTraceStartupPhaseEnabled = 0;
	static char GAppRandTraceStartupPhasePath[PATH_MAX];
	static size_t GAppRandTraceStartupPhaseCount = 0;
	static FAppRandTracePhaseEvent GAppRandTraceStartupPhases[HP2_RAND_TRACE_MAX_STARTUP_PHASES];
	static char GAppRandTraceStartupMapToken[HP2_RAND_TRACE_MAP_CAPACITY];
	static char GAppRandTraceStartupMapURL[HP2_RAND_TRACE_MAP_URL_CAPACITY];
	static char GAppRandTraceStartupLifecycleBoundary[HP2_RAND_TRACE_PHASE_CAPACITY] = "map_loaded";
	static unsigned long long GAppRandTraceStartupLoadOrdinal = 0;
	static size_t GAppRandTraceFunctionEventCount = 0;
	static unsigned long long GAppRandTraceFunctionCallCount = 0;
	static FAppRandTraceFunctionEvent* GAppRandTraceFunctionEvents = NULL;
	static size_t GAppRandTracePreBeginEventCount = 0;
	static FAppRandTracePreBeginEvent* GAppRandTracePreBeginEvents = NULL;
	static thread_local FAppRandTracePreBeginEvent* GAppRandTracePreBeginEvent = NULL;
	static size_t GAppRandTraceRelevanceEventCount = 0;
	static FAppRandTraceRelevanceEvent* GAppRandTraceRelevanceEvents = NULL;
	static thread_local FAppRandTraceRelevanceEvent* GAppRandTraceRelevanceEvent = NULL;

	static void appCopyRandTraceString( char* Destination, size_t Capacity, const ANSICHAR* Source, const ANSICHAR* Default )
	{
		if( !Source || !*Source )
			Source = Default;
		size_t Index = 0;
		for( ; Index+1<Capacity && Source[Index]; ++Index )
			Destination[Index] = Source[Index];
		Destination[Index] = 0;
	}

	static void appSetRandTracePhase( char* Destination, const ANSICHAR* Source )
	{
		appCopyRandTraceString( Destination, HP2_RAND_TRACE_PHASE_CAPACITY, Source, "startup" );
	}

	static void appSetRandTraceMapToken( char* Destination, size_t Capacity, const ANSICHAR* Source )
	{
		const ANSICHAR* Token = Source ? Source : "";
		for( const ANSICHAR* It=Token; *It; ++It )
			if( *It=='/' || *It=='\\' )
				Token = It+1;
		size_t Length = 0;
		while( Token[Length] && Token[Length]!='.' )
			++Length;
		if( Length>=Capacity )
			Length = Capacity-1;
		memcpy( Destination, Token, Length );
		Destination[Length] = 0;
	}

	static size_t appRandTraceLimit()
	{
		const char* Value = getenv( "HP2_RNG_TRACE_LIMIT" );
		if( !Value || !*Value )
			return HP2_RAND_TRACE_DEFAULT_LIMIT;

		for( const char* It=Value; *It; ++It )
			if( *It<'0' || *It>'9' )
				return HP2_RAND_TRACE_DEFAULT_LIMIT;

		errno = 0;
		const unsigned long long Parsed = strtoull( Value, NULL, 10 );
		if( errno == ERANGE || Parsed>HP2_RAND_TRACE_MAX_LIMIT )
			return HP2_RAND_TRACE_MAX_LIMIT;
		return (size_t)Parsed;
	}

	static char* appDuplicateRandTraceString( const ANSICHAR* Value )
	{
		if( !Value )
			Value = "None";
		const size_t Length = strlen( Value ) + 1;
		if( Length>HP2_RAND_TRACE_MAX_CONTEXT_BYTES-GAppRandTraceContextBytes )
			return NULL;
		char* Result = (char*)malloc( Length );
		if( !Result )
			return NULL;
		memcpy( Result, Value, Length );
		GAppRandTraceContextBytes += Length;
		return Result;
	}

	static char* appDuplicateRandTraceObjectPath( UObject* Object )
	{
		return Object
			? appDuplicateRandTraceString( appToAnsi(Object->GetPathName()) )
			: appDuplicateRandTraceString( "None" );
	}

	static void appFreeRandTraceString( char*& Value )
	{
		if( Value )
		{
			GAppRandTraceContextBytes -= strlen(Value) + 1;
			free( Value );
			Value = NULL;
		}
	}

	static void appFreeRandTraceSample( FAppRandTraceSample& Sample )
	{
		appFreeRandTraceString( Sample.Actor );
		appFreeRandTraceString( Sample.Class );
		appFreeRandTraceString( Sample.Function );
		appFreeRandTraceString( Sample.Native );
		appFreeRandTraceString( Sample.OuterActor );
		appFreeRandTraceString( Sample.OuterClass );
		appFreeRandTraceString( Sample.OuterFunction );
		memset( &Sample, 0, sizeof(Sample) );
	}

	static void appFreeRandTraceFunctionEvent( FAppRandTraceFunctionEvent& Event )
	{
		appFreeRandTraceString( Event.Function );
		appFreeRandTraceString( Event.Receiver );
		appFreeRandTraceString( Event.Actor );
		appFreeRandTraceString( Event.RootActor );
		appFreeRandTraceString( Event.RootClass );
		appFreeRandTraceString( Event.RootFunction );
		memset( &Event, 0, sizeof(Event) );
	}

	static void appFreeRandTraceRelevanceEvent( FAppRandTraceRelevanceEvent& Event )
	{
		appFreeRandTraceString( Event.Function );
		appFreeRandTraceString( Event.Receiver );
		appFreeRandTraceString( Event.Other );
		appFreeRandTraceString( Event.OtherClass );
		appFreeRandTraceString( Event.BaseMutator );
		appFreeRandTraceString( Event.BaseMutatorClass );
		memset( &Event, 0, sizeof(Event) );
	}

	static void appFreeRandTracePreBeginEvent( FAppRandTracePreBeginEvent& Event )
	{
		appFreeRandTraceString( Event.Actor );
		appFreeRandTraceString( Event.Function );
		appFreeRandTraceString( Event.LevelGame );
		memset( &Event, 0, sizeof(Event) );
	}

	static void appAppendRandTraceTickBoundary()
	{
		if( GAppRandTraceTickBoundaryCount>=GAppRandTraceLimit || !GAppRandTraceTickBoundaries )
		{
			if( GAppRandTraceLimit )
				GAppRandTraceTruncated = 1;
			return;
		}
		FAppRandTraceTickBoundary& Boundary = GAppRandTraceTickBoundaries[GAppRandTraceTickBoundaryCount++];
		Boundary.Index = GAppRandTraceTickIndex;
		appSetRandTracePhase( Boundary.Phase, GAppRandTracePhase );
	}

	static void appResetRandTraceTickState()
	{
		GAppRandTraceTickIndex = 0;
		appSetRandTracePhase( GAppRandTracePhase, "startup" );
		GAppRandTraceTickBoundaryCount = 0;
		appAppendRandTraceTickBoundary();
	}

	static void appInitRandTrace()
	{
		if( GAppRandTraceInitialized.load(std::memory_order_acquire) )
			return;

		std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
		if( GAppRandTraceInitialized.load(std::memory_order_relaxed) )
			return;

		const char* Path = getenv( "HP2_RNG_TRACE" );
		const char* GlobalTickPath = getenv( "HP2_GLOBAL_TICK_TRACE" );
		const char* FireTexturePath = getenv( "HP2_FIRE_TEXTURE_TRACE" );
		const char* StartupPhasePath = getenv( "HP2_STARTUP_RNG_PHASE_TRACE" );
		if( StartupPhasePath && *StartupPhasePath )
		{
			const size_t PathLength = strlen( StartupPhasePath );
			if( PathLength<sizeof(GAppRandTraceStartupPhasePath) )
			{
				memcpy( GAppRandTraceStartupPhasePath, StartupPhasePath, PathLength+1 );
				GAppRandTraceStartupPhaseEnabled = 1;
				GAppRandTraceOrdinalEnabled = 1;
			}
		}
		// Fire's InitTables oracle needs the real global appRand interval but
		// intentionally does not request a separate RNG artifact.
		GAppRandTraceOrdinalEnabled = (GlobalTickPath && *GlobalTickPath)
			|| (FireTexturePath && *FireTexturePath)
			|| GAppRandTraceStartupPhaseEnabled;
		if( Path && *Path )
		{
			const size_t PathLength = strlen( Path );
			if( PathLength<sizeof(GAppRandTracePath) )
			{
				memcpy( GAppRandTracePath, Path, PathLength+1 );
				GAppRandTraceLimit = appRandTraceLimit();
				GAppRandTraceEnabled = 1;
				GAppRandTraceOrdinalEnabled = 1;
				if( GAppRandTraceLimit )
				{
					GAppRandTraceSamples = (FAppRandTraceSample*)calloc( GAppRandTraceLimit, sizeof(FAppRandTraceSample) );
					GAppRandTraceTickBoundaries = (FAppRandTraceTickBoundary*)calloc( GAppRandTraceLimit, sizeof(FAppRandTraceTickBoundary) );
					GAppRandTraceFunctionEvents = (FAppRandTraceFunctionEvent*)calloc( GAppRandTraceLimit, sizeof(FAppRandTraceFunctionEvent) );
					GAppRandTracePreBeginEvents = (FAppRandTracePreBeginEvent*)calloc( GAppRandTraceLimit, sizeof(FAppRandTracePreBeginEvent) );
					GAppRandTraceRelevanceEvents = (FAppRandTraceRelevanceEvent*)calloc( GAppRandTraceLimit, sizeof(FAppRandTraceRelevanceEvent) );
				}
				GAppRandTraceStorageAvailable = !GAppRandTraceLimit || (GAppRandTraceSamples!=NULL && GAppRandTraceTickBoundaries!=NULL && GAppRandTraceFunctionEvents!=NULL && GAppRandTracePreBeginEvents!=NULL && GAppRandTraceRelevanceEvents!=NULL);
				appResetRandTraceTickState();
			}
		}

		GAppRandTraceInitialized.store( 1, std::memory_order_release );
	}

	static FAppRandTraceContext* appFindRandTraceNativeContext()
	{
		return GAppRandTraceContext && GAppRandTraceContext->Native
			? GAppRandTraceContext
			: NULL;
	}

	static FAppRandTraceContext* appFindRandTraceRootContext()
	{
		FAppRandTraceContext* Root = NULL;
		for( FAppRandTraceContext* Context=GAppRandTraceContext; Context; Context=Context->Previous )
			if( !Context->Native )
				Root = Context;
		return Root;
	}

	static UBOOL appCaptureRandTraceConsumer( FAppRandTraceSample& Sample )
	{
		FAppRandTraceContext* NativeContext = appFindRandTraceNativeContext();
		if( NativeContext )
		{
			Sample.HasConsumer = 1;
			Sample.Actor = appDuplicateRandTraceObjectPath( NativeContext->Object );
			Sample.Class = appDuplicateRandTraceObjectPath( NativeContext->Object ? NativeContext->Object->GetClass() : NULL );
			Sample.Function = appDuplicateRandTraceObjectPath( NativeContext->Function );
			Sample.Native = appDuplicateRandTraceString( NativeContext->Native );
			Sample.NativeSlot = NativeContext->NativeSlot;
			Sample.CallsiteOffset = NativeContext->CallsiteOffset;
			if( !Sample.Actor || !Sample.Class || !Sample.Function || !Sample.Native )
			{
				appFreeRandTraceSample( Sample );
				return 0;
			}
		}

		FAppRandTraceContext* RootContext = appFindRandTraceRootContext();
		if( RootContext )
		{
			Sample.HasOuterConsumer = 1;
			Sample.OuterActor = appDuplicateRandTraceObjectPath( RootContext->Object );
			Sample.OuterClass = appDuplicateRandTraceObjectPath( RootContext->Object ? RootContext->Object->GetClass() : NULL );
			Sample.OuterFunction = appDuplicateRandTraceObjectPath( RootContext->Function );
			if( !Sample.OuterActor || !Sample.OuterClass || !Sample.OuterFunction )
			{
				appFreeRandTraceSample( Sample );
				return 0;
			}
		}
		return 1;
	}

	static UBOOL appIsGameInfoIsRelevant( UFunction* Function )
	{
		UObject* Owner = Function ? Function->GetOuter() : NULL;
		return Owner
			&& appStrcmp(Function->GetName(),TEXT("IsRelevant"))==0
			&& appStrcmp(Owner->GetName(),TEXT("GameInfo"))==0;
	}

	static UObject* appFindGameInfoIsRelevantActor( UFunction* Function, FFrame& Stack )
	{
		UProperty* Property = Function ? (UProperty*)Function->Children : NULL;
		if( !Property
		|| appStrcmp(Property->GetName(),TEXT("Other"))!=0
		|| !Cast<UObjectProperty>(Property) )
			return NULL;
		return *(UObject**)(Stack.Locals + Property->Offset);
	}

	static UBOOL appIsActorPreBeginPlay( UFunction* Function )
	{
		UObject* Owner = Function ? Function->GetOuter() : NULL;
		return Owner
			&& appStrcmp(Function->GetName(),TEXT("PreBeginPlay"))==0
			&& appStrcmp(Owner->GetName(),TEXT("Actor"))==0;
	}

	static UBOOL appReadRandTraceBool( UObject* Object, const TCHAR* Name )
	{
		UBoolProperty* Property = Object ? FindField<UBoolProperty>(Object->GetClass(),Name) : NULL;
		return Property && ((*(BITFIELD*)((BYTE*)Object + Property->Offset) & Property->BitMask) != 0);
	}

	static UObject* appReadRandTraceObject( UObject* Object, const TCHAR* Name )
	{
		UObjectProperty* Property = Object ? FindField<UObjectProperty>(Object->GetClass(),Name) : NULL;
		return Property ? *(UObject**)((BYTE*)Object + Property->Offset) : NULL;
	}

	static UBOOL appReadRandTraceByte( UObject* Object, const TCHAR* Name, INT& Value )
	{
		UByteProperty* Property = Object ? FindField<UByteProperty>(Object->GetClass(),Name) : NULL;
		if( !Property )
			return 0;
		Value = *(BYTE*)((BYTE*)Object + Property->Offset);
		return 1;
	}

	static FAppRandTracePreBeginEvent* appBeginActorPreBeginTrace( UObject* Object, UFunction* Function )
	{
		if( !appIsActorPreBeginPlay(Function)
		|| GAppRandTracePreBeginEventCount>=GAppRandTraceLimit
		|| !GAppRandTracePreBeginEvents )
		{
			if( appIsActorPreBeginPlay(Function) && GAppRandTraceLimit )
				GAppRandTraceTruncated = 1;
			return NULL;
		}

		FAppRandTracePreBeginEvent& Event = GAppRandTracePreBeginEvents[GAppRandTracePreBeginEventCount];
		appMemzero( &Event, sizeof(Event) );
		UObject* Level = appReadRandTraceObject( Object, TEXT("Level") );
		UObject* LevelGame = appReadRandTraceObject( Level, TEXT("Game") );
		Event.HasLevelGame = LevelGame != NULL;
		Event.ActorObject = Object;
		Event.EntryOrdinal = GAppRandTraceCallCount;
		Event.GameRelevant = appReadRandTraceBool( Object, TEXT("bGameRelevant") );
		Event.RelevantInSoftwareRenderer = appReadRandTraceBool( Object, TEXT("bRelevantInSoftwareRenderer") );
		Event.DeleteMeAtEntry = appReadRandTraceBool( Object, TEXT("bDeleteMe") );
		Event.HasNetMode = appReadRandTraceByte( Level, TEXT("NetMode"), Event.NetMode );
		Event.RelevanceWillDispatch = !Event.GameRelevant && Event.HasNetMode && Event.NetMode!=3;
		Event.Actor = appDuplicateRandTraceObjectPath( Object );
		Event.Function = appDuplicateRandTraceObjectPath( Function );
		Event.LevelGame = appDuplicateRandTraceObjectPath( LevelGame );
		if( !Event.Actor || !Event.Function || !Event.LevelGame )
		{
			appFreeRandTracePreBeginEvent( Event );
			GAppRandTraceTruncated = 1;
			return NULL;
		}
		Event.Previous = GAppRandTracePreBeginEvent;
		GAppRandTracePreBeginEvent = &Event;
		GAppRandTracePreBeginEventCount++;
		return &Event;
	}
	static FAppRandTracePreBeginEvent* appBeginPreBeginRelevanceTrace( UFunction* Function, FFrame& Stack )
	{
		if( !appIsGameInfoIsRelevant(Function) )
			return NULL;
		UObject* Actor = appFindGameInfoIsRelevantActor( Function, Stack );
		FAppRandTracePreBeginEvent* Event = GAppRandTracePreBeginEvent;
		if( Event && Event->ActorObject==Actor )
		{
			Event->RelevanceDispatched = 1;
			return Event;
		}
		return NULL;
	}

	static FAppRandTraceFunctionEvent* appBeginGameInfoIsRelevantTrace( UObject* Object, UFunction* Function, FFrame& Stack )
	{
		if( !appIsGameInfoIsRelevant(Function)
		|| GAppRandTraceFunctionEventCount>=GAppRandTraceLimit
		|| !GAppRandTraceFunctionEvents )
		{
			if( appIsGameInfoIsRelevant(Function) && GAppRandTraceLimit )
				GAppRandTraceTruncated = 1;
			return NULL;
		}

		FAppRandTraceFunctionEvent& Event = GAppRandTraceFunctionEvents[GAppRandTraceFunctionEventCount];
		Event.CallId = GAppRandTraceFunctionCallCount++;
		Event.EntryOrdinal = GAppRandTraceCallCount;
		Event.Function = appDuplicateRandTraceObjectPath( Function );
		Event.Receiver = appDuplicateRandTraceObjectPath( Object );
		Event.Actor = appDuplicateRandTraceObjectPath( appFindGameInfoIsRelevantActor(Function,Stack) );

		FAppRandTraceContext* RootContext = appFindRandTraceRootContext();
		Event.RootActor = appDuplicateRandTraceObjectPath( RootContext ? RootContext->Object : NULL );
		Event.RootClass = appDuplicateRandTraceObjectPath( RootContext && RootContext->Object ? RootContext->Object->GetClass() : NULL );
		Event.RootFunction = appDuplicateRandTraceObjectPath( RootContext ? RootContext->Function : NULL );
		if( !Event.Function || !Event.Receiver || !Event.Actor || !Event.RootActor || !Event.RootClass || !Event.RootFunction )
		{
			appFreeRandTraceFunctionEvent( Event );
			GAppRandTraceTruncated = 1;
			return NULL;
		}
		GAppRandTraceFunctionEventCount++;
		return &Event;
	}

	static FAppRandTraceRelevanceEvent* appBeginGameInfoRelevanceTrace( UObject* Object, UFunction* Function, FFrame& Stack )
	{
		if( !appIsGameInfoIsRelevant(Function)
		|| GAppRandTraceRelevanceEventCount>=GAppRandTraceLimit
		|| !GAppRandTraceRelevanceEvents )
		{
			if( appIsGameInfoIsRelevant(Function) && GAppRandTraceLimit )
				GAppRandTraceTruncated = 1;
			return NULL;
		}

		FAppRandTraceRelevanceEvent& Event = GAppRandTraceRelevanceEvents[GAppRandTraceRelevanceEventCount];
		UObject* Other = appFindGameInfoIsRelevantActor( Function, Stack );
		UObject* BaseMutator = appReadRandTraceObject( Object, TEXT("BaseMutator") );
		Event.EntryOrdinal = GAppRandTraceCallCount;
		Event.ReturnSourceLine = INDEX_NONE;
		Event.Function = appDuplicateRandTraceObjectPath( Function );
		Event.Receiver = appDuplicateRandTraceObjectPath( Object );
		Event.Other = appDuplicateRandTraceObjectPath( Other );
		Event.OtherClass = appDuplicateRandTraceObjectPath( Other ? Other->GetClass() : NULL );
		Event.BaseMutator = appDuplicateRandTraceObjectPath( BaseMutator );
		Event.BaseMutatorClass = appDuplicateRandTraceObjectPath( BaseMutator ? BaseMutator->GetClass() : NULL );
		if( !Event.Function || !Event.Receiver || !Event.Other || !Event.OtherClass || !Event.BaseMutator || !Event.BaseMutatorClass )
		{
			appFreeRandTraceRelevanceEvent( Event );
			GAppRandTraceTruncated = 1;
			return NULL;
		}
		Event.Previous = GAppRandTraceRelevanceEvent;
		GAppRandTraceRelevanceEvent = &Event;
		GAppRandTraceRelevanceEventCount++;
		return &Event;
	}

	static const char* appGetGameInfoRelevanceReturnBranch( const FAppRandTraceRelevanceEvent& Event )
	{
		if( Event.HasAlwaysKeepResult && Event.AlwaysKeepResult )
			return "always_keep";
		if( Event.HasIsRelevantResult && !Event.IsRelevantResult )
			return "mutator_is_relevant_rejected";
		if( Event.HasIsRelevantResult && Event.IsRelevantResult && Event.HasSuperRelevant && Event.SuperRelevant==1 )
			return "mutator_super_relevant";

		switch( Event.ReturnSourceLine )
		{
		case 453: return "always_keep";
		case 457: return "mutator_super_relevant";
		case 459: return "mutator_is_relevant_rejected";
		case 469: return "difficulty_or_network_filter";
		case 472: return "no_monsters_filter";
		case 475: return "odds_of_appearing";
		case 487: return "relevant";
		default:  return "unknown";
		}
	}

	static void appRecordRandTrace( INT Value )
	{
		const unsigned long long Ordinal = GAppRandTraceCallCount++;
		if( GAppRandTraceCount>=GAppRandTraceLimit || !GAppRandTraceSamples )
		{
			GAppRandTraceTruncated = 1;
			return;
		}

		FAppRandTraceSample& Sample = GAppRandTraceSamples[GAppRandTraceCount];
		Sample.Ordinal = Ordinal;
		Sample.Value = Value;
		Sample.TickIndex = GAppRandTraceTickIndex;
		appSetRandTracePhase( Sample.Phase, GAppRandTracePhase );
		if( !appCaptureRandTraceConsumer(Sample) )
		{
			GAppRandTraceTruncated = 1;
			return;
		}
		GAppRandTraceCount++;
	}

	static UBOOL appWriteRandTraceJsonString( FILE* Trace, const char* Value )
	{
		if( fputc('"',Trace)==EOF )
			return 0;
		for( const unsigned char* It=(const unsigned char*)Value; *It; ++It )
		{
			switch( *It )
			{
			case '"':  if( fputs("\\\"",Trace)==EOF ) return 0; break;
			case '\\': if( fputs("\\\\",Trace)==EOF ) return 0; break;
			case '\b': if( fputs("\\b",Trace)==EOF ) return 0; break;
			case '\f': if( fputs("\\f",Trace)==EOF ) return 0; break;
			case '\n': if( fputs("\\n",Trace)==EOF ) return 0; break;
			case '\r': if( fputs("\\r",Trace)==EOF ) return 0; break;
			case '\t': if( fputs("\\t",Trace)==EOF ) return 0; break;
			default:
				if( *It<0x20 )
				{
					if( fprintf(Trace,"\\u%04x",(unsigned int)*It)<0 )
						return 0;
				}
				else if( fputc(*It,Trace)==EOF )
					return 0;
			}
		}
		return fputc('"',Trace)!=EOF;
	}

	static UBOOL appWriteStartupRandTrace()
	{
		char TemporaryPath[PATH_MAX+5];
		const int TemporaryLength = snprintf( TemporaryPath, sizeof(TemporaryPath), "%s.tmp", GAppRandTraceStartupPhasePath );
		if( TemporaryLength<0 || (size_t)TemporaryLength>=sizeof(TemporaryPath) )
			return 0;
		FILE* Trace = fopen( TemporaryPath, "wb" );
		if( !Trace )
			return 0;
		UBOOL WriteSucceeded = fputs("{\"version\":2,\"enabled\":true,\"phases\":[",Trace)!=EOF;
		for( size_t Index=0; WriteSucceeded && Index<GAppRandTraceStartupPhaseCount; ++Index )
		{
			const FAppRandTracePhaseEvent& Event = GAppRandTraceStartupPhases[Index];
			WriteSucceeded = fprintf(
				Trace,
				"%s{\"phase\":\"%s\",\"map_token\":",
				Index ? "," : "",
				Event.Phase
			)>=0
				&& appWriteRandTraceJsonString(Trace,Event.MapToken)
				&& fputs(",\"map_url\":",Trace)!=EOF
				&& appWriteRandTraceJsonString(Trace,Event.MapURL)
				&& fprintf(
					Trace,
					",\"load_ordinal\":%llu,\"lifecycle_boundary\":\"%s\",\"ordinal_before\":%llu,\"ordinal_after\":%llu,\"delta\":%llu}",
					Event.LoadOrdinal,
					Event.LifecycleBoundary,
					Event.OrdinalBefore,
					Event.OrdinalAfter,
					Event.OrdinalAfter-Event.OrdinalBefore
				)>=0;
		}
		WriteSucceeded = WriteSucceeded && fputs("]}\n",Trace)!=EOF && fclose(Trace)==0;
		if( !WriteSucceeded )
		{
			fclose( Trace );
			remove( TemporaryPath );
			return 0;
		}
		return rename(TemporaryPath,GAppRandTraceStartupPhasePath)==0;
	}

	static UBOOL appWriteRandTraceNullableString( FILE* Trace, const char* Value )
	{
		return Value ? appWriteRandTraceJsonString(Trace,Value) : fputs("null",Trace)!=EOF;
	}

	static UBOOL appWriteRandTrace()
	{
		char TemporaryPath[PATH_MAX+5];
		const int TemporaryLength = snprintf( TemporaryPath, sizeof(TemporaryPath), "%s.tmp", GAppRandTracePath );
		if( TemporaryLength<0 || (size_t)TemporaryLength>=sizeof(TemporaryPath) )
			return 0;

		FILE* Trace = fopen( TemporaryPath, "wb" );
		if( !Trace )
			return 0;

		char SeedMetadata[112];
		SeedMetadata[0] = 0;
		if( GAppRandTraceReplaySeeded )
			snprintf( SeedMetadata, sizeof(SeedMetadata), ",\"seed\":1,\"start_reason\":\"replay\"" );
		else if( GAppRandTraceDiagnosticSeeded )
			snprintf(
				SeedMetadata,
				sizeof(SeedMetadata),
				",\"seed\":%u,\"requested_seed\":%u,\"start_reason\":\"diagnostic\"",
				GAppRandTraceDiagnosticSeed,
				GAppRandTraceDiagnosticRequestedSeed
			);
		UBOOL WriteSucceeded = fprintf(
			Trace,
			"{\"version\":2,\"enabled\":true,\"rand_max\":%d,\"count\":%lu,\"call_count\":%llu,\"limit\":%lu,\"capture_available\":%s,\"truncated\":%s%s,\"outputs\":[",
			RAND_MAX,
			(unsigned long)GAppRandTraceCount,
			GAppRandTraceCallCount,
			(unsigned long)GAppRandTraceLimit,
			GAppRandTraceStorageAvailable ? "true" : "false",
			GAppRandTraceTruncated ? "true" : "false",
			SeedMetadata
		)>=0;
		for( size_t Index=0; WriteSucceeded && Index<GAppRandTraceCount; ++Index )
			WriteSucceeded = fprintf(
				Trace,
				"%s{\"ordinal\":%llu,\"raw\":%d}",
				Index ? "," : "",
				GAppRandTraceSamples[Index].Ordinal,
				GAppRandTraceSamples[Index].Value
			)>=0;
		if( WriteSucceeded )
			WriteSucceeded = fputs( "],\"tick_boundaries\":[", Trace )!=EOF;
		for( size_t Index=0; WriteSucceeded && Index<GAppRandTraceTickBoundaryCount; ++Index )
		{
			const FAppRandTraceTickBoundary& Boundary = GAppRandTraceTickBoundaries[Index];
			WriteSucceeded = fprintf(Trace,"%s{\"index\":%llu,\"phase\":",Index ? "," : "",Boundary.Index)>=0
				&& appWriteRandTraceJsonString(Trace,Boundary.Phase)
				&& fputc('}',Trace)!=EOF;
		}
		if( WriteSucceeded )
			WriteSucceeded = fputs( "],\"samples\":[", Trace )!=EOF;
		for( size_t Index=0; WriteSucceeded && Index<GAppRandTraceCount; ++Index )
		{
			const FAppRandTraceSample& Sample = GAppRandTraceSamples[Index];
			WriteSucceeded = fprintf(
				Trace,
				"%s{\"ordinal\":%llu,\"raw\":%d,\"tick\":{\"index\":%llu,\"phase\":",
				Index ? "," : "",
				Sample.Ordinal,
				Sample.Value,
				Sample.TickIndex
			)>=0
				&& appWriteRandTraceJsonString(Trace,Sample.Phase)
				&& fputs("},\"consumer\":",Trace)!=EOF;
			if( WriteSucceeded && !Sample.HasConsumer )
				WriteSucceeded = fputs("null",Trace)!=EOF;
			if( WriteSucceeded && Sample.HasConsumer )
			{
				WriteSucceeded = fputs("{\"kind\":\"native\",\"actor\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Sample.Actor)
					&& fputs(",\"class\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Sample.Class)
					&& fputs(",\"function\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Sample.Function)
					&& fputs(",\"native\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Sample.Native)
					&& fprintf(Trace,",\"native_slot\":%d,\"callsite_offset\":%d}",Sample.NativeSlot,Sample.CallsiteOffset)>=0;
			}
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"outer_consumer\":",Trace)!=EOF;
			if( WriteSucceeded && !Sample.HasOuterConsumer )
				WriteSucceeded = fputs("null",Trace)!=EOF;
			if( WriteSucceeded && Sample.HasOuterConsumer )
			{
				WriteSucceeded = fputs("{\"kind\":\"function\",\"actor\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Sample.OuterActor)
					&& fputs(",\"class\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Sample.OuterClass)
					&& fputs(",\"function\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Sample.OuterFunction)
					&& fputs(",\"native\":null,\"native_slot\":null,\"callsite_offset\":null}",Trace)!=EOF;
			}
			if( WriteSucceeded )
				WriteSucceeded = fputc('}',Trace)!=EOF;
		}
		if( WriteSucceeded )
			WriteSucceeded = fputs( "],\"function_events\":[", Trace )!=EOF;
		for( size_t Index=0; WriteSucceeded && Index<GAppRandTraceFunctionEventCount; ++Index )
		{
			const FAppRandTraceFunctionEvent& Event = GAppRandTraceFunctionEvents[Index];
			WriteSucceeded = fprintf(
				Trace,
				"%s{\"call_id\":%llu,\"function\":",
				Index ? "," : "",
				Event.CallId
			)>=0
				&& appWriteRandTraceNullableString(Trace,Event.Function)
				&& fputs(",\"receiver\":",Trace)!=EOF
				&& appWriteRandTraceNullableString(Trace,Event.Receiver)
				&& fputs(",\"actor\":",Trace)!=EOF
				&& appWriteRandTraceNullableString(Trace,Event.Actor)
				&& fprintf(Trace,",\"ordinal_at_entry\":%llu,\"ordinal_at_exit\":",Event.EntryOrdinal)>=0;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasExit
					? fprintf(Trace,"%llu",Event.ExitOrdinal)>=0
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"root_context\":{\"actor\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Event.RootActor)
					&& fputs(",\"class\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Event.RootClass)
					&& fputs(",\"function\":",Trace)!=EOF
					&& appWriteRandTraceNullableString(Trace,Event.RootFunction)
					&& fputs("}}",Trace)!=EOF;
		}
		if( WriteSucceeded )
			WriteSucceeded = fputs( "],\"relevance_events\":[", Trace )!=EOF;
		for( size_t Index=0; WriteSucceeded && Index<GAppRandTraceRelevanceEventCount; ++Index )
		{
			const FAppRandTraceRelevanceEvent& Event = GAppRandTraceRelevanceEvents[Index];
			WriteSucceeded = fprintf(
				Trace,
				"%s{\"function\":",
				Index ? "," : ""
			)>=0
				&& appWriteRandTraceNullableString(Trace,Event.Function)
				&& fputs(",\"receiver\":",Trace)!=EOF
				&& appWriteRandTraceNullableString(Trace,Event.Receiver)
				&& fputs(",\"other\":{\"actor\":",Trace)!=EOF
				&& appWriteRandTraceNullableString(Trace,Event.Other)
				&& fputs(",\"class\":",Trace)!=EOF
				&& appWriteRandTraceNullableString(Trace,Event.OtherClass)
				&& fputs("},\"base_mutator\":{\"path\":",Trace)!=EOF
				&& appWriteRandTraceNullableString(Trace,Event.BaseMutator)
				&& fputs(",\"class\":",Trace)!=EOF
				&& appWriteRandTraceNullableString(Trace,Event.BaseMutatorClass)
				&& fputs("},\"ordinal_at_entry\":",Trace)!=EOF
				&& fprintf(Trace,"%llu",Event.EntryOrdinal)>=0
				&& fputs(",\"ordinal_at_exit\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasExit
					? fprintf(Trace,"%llu",Event.ExitOrdinal)>=0
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"always_keep\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasAlwaysKeepResult
					? fputs(Event.AlwaysKeepResult ? "true" : "false",Trace)!=EOF
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"is_relevant\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasIsRelevantResult
					? fputs(Event.IsRelevantResult ? "true" : "false",Trace)!=EOF
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"b_super_relevant\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasSuperRelevant
					? fprintf(Trace,"%u",(unsigned int)Event.SuperRelevant)>=0
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"return_source_line\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.ReturnSourceLine!=INDEX_NONE
					? fprintf(Trace,"%d",Event.ReturnSourceLine)>=0
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"early_return_branch\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasFinalResult
					? appWriteRandTraceJsonString(Trace,Event.EarlyReturnBranch)
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"result\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasFinalResult
					? fputs(Event.FinalResult ? "true" : "false",Trace)!=EOF
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputc('}',Trace)!=EOF;
		}
		if( WriteSucceeded )
			WriteSucceeded = fputs( "],\"prebegin_events\":[", Trace )!=EOF;
		for( size_t Index=0; WriteSucceeded && Index<GAppRandTracePreBeginEventCount; ++Index )
		{
			const FAppRandTracePreBeginEvent& Event = GAppRandTracePreBeginEvents[Index];
			WriteSucceeded = fprintf(
				Trace,
				"%s{\"actor\":",
				Index ? "," : ""
			)>=0
				&& appWriteRandTraceNullableString(Trace,Event.Actor)
				&& fputs(",\"function\":",Trace)!=EOF
				&& appWriteRandTraceNullableString(Trace,Event.Function)
				&& fprintf(
					Trace,
					",\"ordinal_at_entry\":%llu,\"ordinal_at_exit\":",
					Event.EntryOrdinal
				)>=0;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasExit
					? fprintf(Trace,"%llu",Event.ExitOrdinal)>=0
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fprintf(
					Trace,
					",\"b_game_relevant\":%s,\"b_relevant_in_software_renderer\":%s,\"b_delete_me_at_entry\":%s,\"b_delete_me_at_exit\":%s,\"is_software_rendering\":",
					Event.GameRelevant ? "true" : "false",
					Event.RelevantInSoftwareRenderer ? "true" : "false",
					Event.DeleteMeAtEntry ? "true" : "false",
					Event.DeleteMeAtExit ? "true" : "false"
				)>=0;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasSoftwareRendering
					? fputs(Event.IsSoftwareRendering ? "true" : "false",Trace)!=EOF
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"net_mode\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasNetMode
					? fprintf(Trace,"%d",Event.NetMode)>=0
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"level_game\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasLevelGame
					? appWriteRandTraceNullableString(Trace,Event.LevelGame)
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"relevance_result\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasRelevanceResult
					? fputs(Event.RelevanceResult ? "true" : "false",Trace)!=EOF
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"relevance_ordinal_at_entry\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasRelevanceOrdinals
					? fprintf(Trace,"%llu",Event.RelevanceEntryOrdinal)>=0
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"relevance_ordinal_at_exit\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasRelevanceOrdinals
					? fprintf(Trace,"%llu",Event.RelevanceExitOrdinal)>=0
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fputs(",\"relevance_app_rand_consumed\":",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = Event.HasRelevanceOrdinals
					? fputs(Event.RelevanceAppRandConsumed ? "true" : "false",Trace)!=EOF
					: fputs("null",Trace)!=EOF;
			if( WriteSucceeded )
				WriteSucceeded = fprintf(
					Trace,
					",\"relevance_will_dispatch\":%s,\"relevance_dispatched\":%s,\"relevance_completed\":%s}",
					Event.RelevanceWillDispatch ? "true" : "false",
					Event.RelevanceDispatched ? "true" : "false",
					Event.RelevanceCompleted ? "true" : "false"
				)>=0;
		}
		if( WriteSucceeded )
			WriteSucceeded = fputs( "]}\n", Trace )!=EOF;
		if( WriteSucceeded && fflush(Trace)!=0 )
			WriteSucceeded = 0;
		const int CloseResult = fclose( Trace );
		if( !WriteSucceeded || CloseResult!=0 || rename(TemporaryPath,GAppRandTracePath)!=0 )
		{
			remove( TemporaryPath );
			return 0;
		}
		return 1;
	}
}

/*-----------------------------------------------------------------------------
	Selected patrol-state tracing.
-----------------------------------------------------------------------------*/

namespace
{
	static std::atomic<UBOOL> GPatrolStateTraceInitialized( 0 );
	static std::mutex GPatrolStateTraceMutex;
	static UBOOL GPatrolStateTraceEnabled = 0;
	static char GPatrolStateTracePath[PATH_MAX];
	static char GPatrolStateTraceState[PATH_MAX];
	static char GPatrolStateTraceActor[PATH_MAX];
	static char GPatrolStateTraceStateOwner[PATH_MAX];

	static void appInitPatrolStateTrace()
	{
		if( GPatrolStateTraceInitialized.load(std::memory_order_acquire) )
			return;

		std::lock_guard<std::mutex> Lock( GPatrolStateTraceMutex );
		if( GPatrolStateTraceInitialized.load(std::memory_order_relaxed) )
			return;

		const char* Path = getenv( "HP2_PATROL_STATE_TRACE" );
		if( Path && *Path && strlen(Path)<sizeof(GPatrolStateTracePath) )
		{
			memcpy( GPatrolStateTracePath, Path, strlen(Path)+1 );
			const char* Actor = getenv( "HP2_PATROL_STATE_TRACE_ACTOR" );
			if( Actor && *Actor && strlen(Actor)<sizeof(GPatrolStateTraceActor) )
				memcpy( GPatrolStateTraceActor, Actor, strlen(Actor)+1 );
			const char* StateOwner = getenv( "HP2_PATROL_STATE_TRACE_STATE_OWNER" );
			const char* State = getenv( "HP2_PATROL_STATE_TRACE_STATE" );
			if( State && *State && strlen(State)<sizeof(GPatrolStateTraceState) )
				memcpy( GPatrolStateTraceState, State, strlen(State)+1 );
			if( StateOwner && *StateOwner && strlen(StateOwner)<sizeof(GPatrolStateTraceStateOwner) )
				memcpy( GPatrolStateTraceStateOwner, StateOwner, strlen(StateOwner)+1 );
			FILE* Trace = fopen( GPatrolStateTracePath, "wb" );
			if( Trace )
			{
				fclose( Trace );
				GPatrolStateTraceEnabled = 1;
			}
		}

		GPatrolStateTraceInitialized.store( 1, std::memory_order_release );
	}

	static UBOOL appWritePatrolStateTraceString( FILE* Trace, const char* Value )
	{
		if( !Value )
			return fputs("null",Trace)!=EOF;
		if( fputc('"',Trace)==EOF )
			return 0;
		for( const unsigned char* It=(const unsigned char*)Value; *It; ++It )
		{
			switch( *It )
			{
			case '"':  if( fputs("\\\"",Trace)==EOF ) return 0; break;
			case '\\': if( fputs("\\\\",Trace)==EOF ) return 0; break;
			case '\b': if( fputs("\\b",Trace)==EOF ) return 0; break;
			case '\f': if( fputs("\\f",Trace)==EOF ) return 0; break;
			case '\n': if( fputs("\\n",Trace)==EOF ) return 0; break;
			case '\r': if( fputs("\\r",Trace)==EOF ) return 0; break;
			case '\t': if( fputs("\\t",Trace)==EOF ) return 0; break;
			default:
				if( *It<0x20 )
				{
					if( fprintf(Trace,"\\u%04x",(unsigned int)*It)<0 )
						return 0;
				}
				else if( fputc(*It,Trace)==EOF )
					return 0;
			}
		}
		return fputc('"',Trace)!=EOF;
	}

	static UBOOL appGetPatrolStateTraceState( FFrame& Stack, UObject* Object, UState*& State )
	{
		State = Cast<UState>( Stack.Node );
		if( !Object || !State )
			return 0;

		appInitPatrolStateTrace();
		if( !GPatrolStateTraceEnabled )
			return 0;

		if( GPatrolStateTraceState[0]
			? strcmp(GPatrolStateTraceState,appToAnsi(State->GetName()))!=0
				&& strcmp(GPatrolStateTraceState,appToAnsi(State->GetPathName()))!=0
			: appStricmp(State->GetName(),TEXT("patrol"))!=0
				&& appStricmp(State->GetName(),TEXT("stateIdle"))!=0 )
			return 0;
		if( GPatrolStateTraceActor[0]
		&& strcmp(GPatrolStateTraceActor,appToAnsi(Object->GetName()))!=0
		&& strcmp(GPatrolStateTraceActor,appToAnsi(Object->GetPathName()))!=0 )
			return 0;

		UObject* StateOwner = State->GetOuter();
		return !GPatrolStateTraceStateOwner[0]
			|| (StateOwner && strcmp(GPatrolStateTraceStateOwner,appToAnsi(StateOwner->GetName()))==0)
			|| (StateOwner && strcmp(GPatrolStateTraceStateOwner,appToAnsi(StateOwner->GetPathName()))==0);
	}

	static UBOOL appWritePatrolStateTracePrefix( FILE* Trace, const char* Kind, UObject* Object, UState* State )
	{
		UObject* StateOwner = State ? State->GetOuter() : NULL;
		return fprintf(Trace,"{\"version\":1,\"kind\":")>=0
			&& appWritePatrolStateTraceString(Trace,Kind)
			&& fputs(",\"actor\":",Trace)!=EOF
			&& appWritePatrolStateTraceString(Trace,Object ? appToAnsi(Object->GetPathName()) : NULL)
			&& fputs(",\"class\":",Trace)!=EOF
			&& appWritePatrolStateTraceString(Trace,Object ? appToAnsi(Object->GetClass()->GetPathName()) : NULL)
			&& fputs(",\"state\":",Trace)!=EOF
			&& appWritePatrolStateTraceString(Trace,State ? appToAnsi(State->GetPathName()) : NULL)
			&& fputs(",\"state_owner\":",Trace)!=EOF
			&& appWritePatrolStateTraceString(Trace,StateOwner ? appToAnsi(StateOwner->GetPathName()) : NULL);
	}
}

CORE_API void appRecordPatrolStateTraceProperty( FFrame& Stack, UObject* Object, UProperty* Property, const BYTE* Value, INT OpcodeOffset, const ANSICHAR* Access )
{
	UState* State = NULL;
	if( !Property || !Value || !appGetPatrolStateTraceState(Stack,Object,State) )
		return;

	TCHAR ExportedValue[1024];
	ExportedValue[0] = 0;
	if( !Property->ExportText(0,ExportedValue,(BYTE*)Value,(BYTE*)Value,PPF_Delimited) )
		appStrcpy( ExportedValue, TEXT("<unavailable>") );

	std::lock_guard<std::mutex> Lock( GPatrolStateTraceMutex );
	FILE* Trace = fopen( GPatrolStateTracePath, "ab" );
	if( !Trace )
		return;
	const UBOOL Wrote = appWritePatrolStateTracePrefix(Trace,"property",Object,State)
		&& fprintf(Trace,",\"opcode_offset\":%d,\"access\":",OpcodeOffset)>=0
		&& appWritePatrolStateTraceString(Trace,Access)
		&& fputs(",\"property\":",Trace)!=EOF
		&& appWritePatrolStateTraceString(Trace,appToAnsi(Property->GetName()))
		&& fputs(",\"property_owner\":",Trace)!=EOF
		&& appWritePatrolStateTraceString(Trace,Property->GetOuter() ? appToAnsi(Property->GetOuter()->GetPathName()) : NULL)
		&& fputs(",\"property_type\":",Trace)!=EOF
		&& appWritePatrolStateTraceString(Trace,appToAnsi(Property->GetClass()->GetName()))
		&& fputs(",\"value\":",Trace)!=EOF
		&& appWritePatrolStateTraceString(Trace,appToAnsi(ExportedValue))
		&& fputs("}\n",Trace)!=EOF;
	if( Wrote )
		fflush( Trace );
	fclose( Trace );
}

CORE_API void appRecordPatrolStateTraceBranch( FFrame& Stack, UObject* Object, INT OpcodeOffset, INT TargetOffset, INT NextOffset, UBOOL Condition )
{
	UState* State = NULL;
	if( !appGetPatrolStateTraceState(Stack,Object,State) )
		return;

	std::lock_guard<std::mutex> Lock( GPatrolStateTraceMutex );
	FILE* Trace = fopen( GPatrolStateTracePath, "ab" );
	if( !Trace )
		return;
	const UBOOL Wrote = appWritePatrolStateTracePrefix(Trace,"jump_if_not",Object,State)
		&& fprintf(
			Trace,
			",\"opcode_offset\":%d,\"target_offset\":%d,\"next_offset\":%d,\"condition\":%s,\"taken\":%s}\n",
			OpcodeOffset,
			TargetOffset,
			NextOffset,
			Condition ? "true" : "false",
			Condition ? "false" : "true"
		)>=0;
	if( Wrote )
		fflush( Trace );
	fclose( Trace );
}

/*-----------------------------------------------------------------------------
	Selected patrol-point iterator tracing.
-----------------------------------------------------------------------------*/

namespace
{
	static std::atomic<UBOOL> GPatrolPointTraceInitialized( 0 );
	static std::mutex GPatrolPointTraceMutex;
	static UBOOL GPatrolPointTraceEnabled = 0;
	static char GPatrolPointTracePath[PATH_MAX];
	static char GPatrolPointTraceActor[PATH_MAX];

	static void appInitPatrolPointTrace()
	{
		if( GPatrolPointTraceInitialized.load(std::memory_order_acquire) )
			return;

		std::lock_guard<std::mutex> Lock( GPatrolPointTraceMutex );
		if( GPatrolPointTraceInitialized.load(std::memory_order_relaxed) )
			return;

		const char* Path = getenv( "HP2_PATROL_POINT_TRACE" );
		if( Path && *Path && strlen(Path)<sizeof(GPatrolPointTracePath) )
		{
			memcpy( GPatrolPointTracePath, Path, strlen(Path)+1 );
			const char* Actor = getenv( "HP2_PATROL_POINT_TRACE_ACTOR" );
			if( Actor && *Actor && strlen(Actor)<sizeof(GPatrolPointTraceActor) )
				memcpy( GPatrolPointTraceActor, Actor, strlen(Actor)+1 );
			FILE* Trace = fopen( GPatrolPointTracePath, "wb" );
			if( Trace )
			{
				fclose( Trace );
				GPatrolPointTraceEnabled = 1;
			}
		}
		GPatrolPointTraceInitialized.store( 1, std::memory_order_release );
	}

	static UBOOL appPatrolPointTraceString( FILE* Trace, const char* Value )
	{
		return appWritePatrolStateTraceString( Trace, Value );
	}

	static UBOOL appPatrolPointTraceSelected( UObject* Object )
	{
		if( !Object )
			return 0;
		if( !GPatrolPointTraceActor[0] )
			return 1;
		return strcmp(GPatrolPointTraceActor,appToAnsi(Object->GetName()))==0
			|| strcmp(GPatrolPointTraceActor,appToAnsi(Object->GetPathName()))==0;
	}

	static const char* appPatrolPointTraceProperty( UObject* Object, const TCHAR* PropertyName, TCHAR* Buffer, INT BufferCount )
	{
		UProperty* Property = Object ? FindField<UProperty>(Object->GetClass(),PropertyName) : NULL;
		if( !Property || !Property->ExportText(0,Buffer,(BYTE*)Object+Property->Offset,(BYTE*)Object+Property->Offset,PPF_Delimited) )
			return NULL;
		return appToAnsi(Buffer);
	}
}

CORE_API void appRecordPatrolPointIterator( UObject* Requestor, UClass* RequestedClass, const FName& MatchTag, INT Slot, UObject* Yielded, INT Order )
{
	appInitPatrolPointTrace();
	if( !GPatrolPointTraceEnabled || !Requestor || !RequestedClass || !Yielded || !appPatrolPointTraceSelected(Requestor) )
		return;

	const TCHAR* ClassName = RequestedClass->GetName();
	if( appStricmp(ClassName,TEXT("PatrolPoint"))!=0 && appStricmp(ClassName,TEXT("NavigationPoint"))!=0 )
		return;

	TCHAR RequestorFirstPatrolPoint[1024];
	TCHAR YieldedTag[1024];
	const char* FirstPatrolPoint = appPatrolPointTraceProperty(
		Requestor,
		TEXT("FirstPatrolPoint_ObjectName"),
		RequestorFirstPatrolPoint,
		ARRAY_COUNT(RequestorFirstPatrolPoint)
	);
	if( !FirstPatrolPoint )
		FirstPatrolPoint = appPatrolPointTraceProperty(
			Requestor,
			TEXT("firstPatrolPointObjectName"),
			RequestorFirstPatrolPoint,
			ARRAY_COUNT(RequestorFirstPatrolPoint)
		);
	const char* Tag = appPatrolPointTraceProperty(
		Yielded,
		TEXT("Tag"),
		YieldedTag,
		ARRAY_COUNT(YieldedTag)
	);

	std::lock_guard<std::mutex> Lock( GPatrolPointTraceMutex );
	FILE* Trace = fopen( GPatrolPointTracePath, "ab" );
	if( !Trace )
		return;
	const UBOOL Wrote = fprintf(Trace,"{\"version\":1,\"kind\":\"all_actors\",\"actor\":")>=0
		&& appPatrolPointTraceString(Trace,appToAnsi(Requestor->GetPathName()))
		&& fputs(",\"class\":",Trace)!=EOF
		&& appPatrolPointTraceString(Trace,appToAnsi(Requestor->GetClass()->GetPathName()))
		&& fputs(",\"requested_class\":",Trace)!=EOF
		&& appPatrolPointTraceString(Trace,appToAnsi(RequestedClass->GetPathName()))
		&& fputs(",\"match_tag\":",Trace)!=EOF
		&& appPatrolPointTraceString(Trace,MatchTag==NAME_None ? NULL : appToAnsi(*MatchTag))
		&& fputs(",\"firstPatrolPointObjectName\":",Trace)!=EOF
		&& appPatrolPointTraceString(Trace,FirstPatrolPoint)
		&& fprintf(Trace,",\"yielded\":{\"order\":%d,\"slot\":%d,\"path\":",Order,Slot)>=0
		&& appPatrolPointTraceString(Trace,appToAnsi(Yielded->GetPathName()))
		&& fputs(",\"name\":",Trace)!=EOF
		&& appPatrolPointTraceString(Trace,appToAnsi(Yielded->GetName()))
		&& fputs(",\"tag\":",Trace)!=EOF
		&& appPatrolPointTraceString(Trace,Tag)
		&& fputs("}}\n",Trace)!=EOF;
	if( Wrote )
		fflush(Trace);
	fclose(Trace);
}

FAppRandTraceOuterScope::FAppRandTraceOuterScope( UObject* Object, UObject* Function )
: Context( NULL )
, Pushed( 0 )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed )
		return;

	FAppRandTraceContext* NewContext = new FAppRandTraceContext;
	if( !NewContext )
		return;
	NewContext->Previous = GAppRandTraceContext;
	NewContext->Object = Object;
	NewContext->Function = Function;
	NewContext->Native = NULL;
	NewContext->NativeSlot = INDEX_NONE;
	NewContext->CallsiteOffset = INDEX_NONE;
	GAppRandTraceContext = NewContext;
	Context = NewContext;
	Pushed = 1;
}

FAppRandTraceOuterScope::~FAppRandTraceOuterScope()
{
	if( Pushed )
	{
		FAppRandTraceContext* PushedContext = (FAppRandTraceContext*)Context;
		GAppRandTraceContext = PushedContext->Previous;
		delete PushedContext;
	}
}

FAppRandTraceNativeScope::FAppRandTraceNativeScope( FFrame& Stack, const ANSICHAR* Native, INT NativeSlot, INT CallsiteOffset )
: Context( NULL )
, Pushed( 0 )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed )
		return;

	FAppRandTraceContext* NewContext = new FAppRandTraceContext;
	if( !NewContext )
		return;
	NewContext->Previous = GAppRandTraceContext;
	NewContext->Object = Stack.Object;
	NewContext->Function = Cast<UFunction>( Stack.Node );
	NewContext->Native = Native;
	NewContext->NativeSlot = NativeSlot;
	NewContext->CallsiteOffset = CallsiteOffset;
	GAppRandTraceContext = NewContext;
	Context = NewContext;
	Pushed = 1;
}

FAppRandTraceNativeScope::~FAppRandTraceNativeScope()
{
	if( Pushed )
	{
		FAppRandTraceContext* PushedContext = (FAppRandTraceContext*)Context;
		GAppRandTraceContext = PushedContext->Previous;
		delete PushedContext;
	}
}

FAppRandTraceNativeObjectScope::FAppRandTraceNativeObjectScope( UObject* Object, const ANSICHAR* Native, INT NativeSlot, INT CallsiteOffset )
: Context( NULL )
, Pushed( 0 )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed )
		return;

	FAppRandTraceContext* NewContext = new FAppRandTraceContext;
	if( !NewContext )
		return;
	NewContext->Previous = GAppRandTraceContext;
	NewContext->Object = Object;
	NewContext->Function = NULL;
	NewContext->Native = Native;
	NewContext->NativeSlot = NativeSlot;
	NewContext->CallsiteOffset = CallsiteOffset;
	GAppRandTraceContext = NewContext;
	Context = NewContext;
	Pushed = 1;
}

FAppRandTraceNativeObjectScope::~FAppRandTraceNativeObjectScope()
{
	if( Pushed )
	{
		FAppRandTraceContext* PushedContext = (FAppRandTraceContext*)Context;
		GAppRandTraceContext = PushedContext->Previous;
		delete PushedContext;
	}
}

FAppRandTracePreBeginScope::FAppRandTracePreBeginScope( UObject* Object, UFunction* Function )
: Event( NULL )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed || !appIsActorPreBeginPlay(Function) )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( !GAppRandTraceFlushed )
		Event = appBeginActorPreBeginTrace( Object, Function );
}

FAppRandTracePreBeginScope::~FAppRandTracePreBeginScope()
{
	if( Event )
	{
		std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
		if( !GAppRandTraceFlushed )
		{
			FAppRandTracePreBeginEvent* RecordedEvent = (FAppRandTracePreBeginEvent*)Event;
			RecordedEvent->DeleteMeAtExit = appReadRandTraceBool( RecordedEvent->ActorObject, TEXT("bDeleteMe") );
			RecordedEvent->ExitOrdinal = GAppRandTraceCallCount;
			RecordedEvent->HasExit = 1;
			GAppRandTracePreBeginEvent = RecordedEvent->Previous;
			RecordedEvent->ActorObject = NULL;
		}
	}
}

FAppRandTracePreBeginRelevanceScope::FAppRandTracePreBeginRelevanceScope( FFrame& CallerStack, UObject* Object, UFunction* Function, void* InResult )
: Event( NULL )
, Result( InResult )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed || !appIsGameInfoIsRelevant(Function) )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	FAppRandTracePreBeginEvent* ActiveEvent = GAppRandTracePreBeginEvent;
	if( GAppRandTraceFlushed
	|| !ActiveEvent
	|| !appIsActorPreBeginPlay(Cast<UFunction>(CallerStack.Node)) )
		return;

	UObject* Level = appReadRandTraceObject( ActiveEvent->ActorObject, TEXT("Level") );
	if( Object!=appReadRandTraceObject(Level,TEXT("Game")) )
		return;

	Event = ActiveEvent;
	ActiveEvent->RelevanceDispatched = 1;
	ActiveEvent->HasRelevanceOrdinals = 1;
	ActiveEvent->RelevanceEntryOrdinal = GAppRandTraceCallCount;
}

FAppRandTracePreBeginRelevanceScope::~FAppRandTracePreBeginRelevanceScope()
{
	if( !Event || !GAppRandTraceEnabled || GAppRandTraceFlushed )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	FAppRandTracePreBeginEvent* RecordedEvent = (FAppRandTracePreBeginEvent*)Event;
	if( GAppRandTraceFlushed )
		return;

	RecordedEvent->RelevanceExitOrdinal = GAppRandTraceCallCount;
	RecordedEvent->RelevanceAppRandConsumed = RecordedEvent->RelevanceExitOrdinal!=RecordedEvent->RelevanceEntryOrdinal;
	RecordedEvent->HasRelevanceResult = Result!=NULL;
	RecordedEvent->RelevanceResult = Result && *(BITFIELD*)Result;
	RecordedEvent->RelevanceCompleted = 1;
}

CORE_API void appRecordRandTracePreBeginSoftwareRendering( UObject* Object, UBOOL IsSoftwareRendering )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	FAppRandTracePreBeginEvent* Event = GAppRandTracePreBeginEvent;
	if( !GAppRandTraceFlushed && Event && Event->ActorObject==Object )
	{
		Event->HasSoftwareRendering = 1;
		Event->IsSoftwareRendering = IsSoftwareRendering;
	}
}

FAppRandTraceFunctionScope::FAppRandTraceFunctionScope( UObject* Object, UFunction* Function, FFrame& Stack )
: Event( NULL )
, PreBeginEvent( NULL )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed || !appIsGameInfoIsRelevant(Function) )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( !GAppRandTraceFlushed )
	{
		PreBeginEvent = appBeginPreBeginRelevanceTrace( Function, Stack );
		Event = appBeginGameInfoIsRelevantTrace( Object, Function, Stack );
	}
}

FAppRandTraceFunctionScope::~FAppRandTraceFunctionScope()
{
	if( Event || PreBeginEvent )
	{
		std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
		if( !GAppRandTraceFlushed )
		{
			if( Event )
			{
				FAppRandTraceFunctionEvent* RecordedEvent = (FAppRandTraceFunctionEvent*)Event;
				RecordedEvent->ExitOrdinal = GAppRandTraceCallCount;
				RecordedEvent->HasExit = 1;
			}
			if( PreBeginEvent )
				((FAppRandTracePreBeginEvent*)PreBeginEvent)->RelevanceCompleted = 1;
		}
	}
}

FAppRandTraceRelevanceScope::FAppRandTraceRelevanceScope( UObject* Object, UFunction* Function, FFrame& Stack )
: Event( NULL )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed || !appIsGameInfoIsRelevant(Function) )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( !GAppRandTraceFlushed )
		Event = appBeginGameInfoRelevanceTrace( Object, Function, Stack );
}

FAppRandTraceRelevanceScope::~FAppRandTraceRelevanceScope()
{
	if( Event )
	{
		std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
		if( !GAppRandTraceFlushed )
		{
			FAppRandTraceRelevanceEvent* RecordedEvent = (FAppRandTraceRelevanceEvent*)Event;
			RecordedEvent->ExitOrdinal = GAppRandTraceCallCount;
			RecordedEvent->HasExit = 1;
			GAppRandTraceRelevanceEvent = RecordedEvent->Previous;
		}
	}
}

FAppRandTraceRelevanceCallScope::FAppRandTraceRelevanceCallScope( FFrame& CallerStack, UObject*, UFunction* Function, void* InResult )
: Event( NULL )
, Result( InResult )
, SuperRelevant( NULL )
, CaptureKind( 0 )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	FAppRandTraceRelevanceEvent* ActiveEvent = GAppRandTraceRelevanceEvent;
	if( GAppRandTraceFlushed || !ActiveEvent )
		return;

	Event = ActiveEvent;
	if( ActiveEvent->DirectCallDepth==0
	&& Function
	&& appStrcmp(Function->GetName(),TEXT("IsRelevant"))==0 )
	{
		CaptureKind = 2;
		UByteProperty* Property = FindField<UByteProperty>( Cast<UFunction>(CallerStack.Node), TEXT("bSuperRelevant") );
		if( Property )
			SuperRelevant = CallerStack.Locals + Property->Offset;
	}
	else if( ActiveEvent->DirectCallDepth==0
	&& Function
	&& appStrcmp(Function->GetName(),TEXT("AlwaysKeep"))==0 )
	{
		CaptureKind = 1;
	}
	ActiveEvent->DirectCallDepth++;
}

FAppRandTraceRelevanceCallScope::~FAppRandTraceRelevanceCallScope()
{
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	FAppRandTraceRelevanceEvent* RecordedEvent = (FAppRandTraceRelevanceEvent*)Event;
	if( GAppRandTraceFlushed || !RecordedEvent )
		return;

	if( CaptureKind==2 )
	{
		RecordedEvent->HasIsRelevantResult = Result!=NULL;
		RecordedEvent->IsRelevantResult = Result && *(BITFIELD*)Result;
		if( SuperRelevant )
		{
			RecordedEvent->HasSuperRelevant = 1;
			RecordedEvent->SuperRelevant = *SuperRelevant;
		}
	}
	else if( CaptureKind==1 && Result )
	{
		RecordedEvent->HasAlwaysKeepResult = 1;
		RecordedEvent->AlwaysKeepResult = *(BITFIELD*)Result;
	}
	check( RecordedEvent->DirectCallDepth>0 );
	RecordedEvent->DirectCallDepth--;
}

CORE_API void appRecordRandTraceRelevanceReturnDebugInfo( FFrame& Stack, INT LineNumber )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed || !appIsGameInfoIsRelevant(Cast<UFunction>(Stack.Node)) )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( !GAppRandTraceFlushed && GAppRandTraceRelevanceEvent )
		GAppRandTraceRelevanceEvent->ReturnSourceLine = LineNumber;
}

CORE_API void appRecordRandTraceRelevanceReturn( FFrame& Stack, void* Result )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled || GAppRandTraceFlushed || !appIsGameInfoIsRelevant(Cast<UFunction>(Stack.Node)) )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	FAppRandTraceRelevanceEvent* Event = GAppRandTraceRelevanceEvent;
	if( !GAppRandTraceFlushed && Event && Result )
	{
		Event->HasFinalResult = 1;
		Event->FinalResult = *(BITFIELD*)Result;
		Event->EarlyReturnBranch = appGetGameInfoRelevanceReturnBranch( *Event );
	}
}

FAppRandTracePhaseScope::FAppRandTracePhaseScope( const ANSICHAR* Phase )
: Event( NULL )
{
	appInitRandTrace();
	if( !GAppRandTraceStartupPhaseEnabled || GAppRandTraceFlushed )
		return;
	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( GAppRandTraceFlushed || GAppRandTraceStartupPhaseCount>=HP2_RAND_TRACE_MAX_STARTUP_PHASES )
		return;
	if( strcmp(Phase,"pre_render_fglobalrandoms")==0 )
	{
		UBOOL HasFireInit = 0;
		for( size_t Index=0; Index<GAppRandTraceStartupPhaseCount; ++Index )
		{
			if( strcmp(GAppRandTraceStartupPhases[Index].Phase,"fire_init_tables")==0 )
				HasFireInit = 1;
			if( strcmp(GAppRandTraceStartupPhases[Index].Phase,"pre_render_fglobalrandoms")==0
				&& GAppRandTraceStartupPhases[Index].LoadOrdinal==GAppRandTraceStartupLoadOrdinal )
				return;
		}
		if( !HasFireInit )
			return;
	}
	FAppRandTracePhaseEvent* NewEvent = &GAppRandTraceStartupPhases[GAppRandTraceStartupPhaseCount++];
	appSetRandTracePhase( NewEvent->Phase, Phase );
	appCopyRandTraceString( NewEvent->MapToken, sizeof(NewEvent->MapToken), GAppRandTraceStartupMapToken, "" );
	appCopyRandTraceString( NewEvent->MapURL, sizeof(NewEvent->MapURL), GAppRandTraceStartupMapURL, "" );
	appCopyRandTraceString( NewEvent->LifecycleBoundary, sizeof(NewEvent->LifecycleBoundary), GAppRandTraceStartupLifecycleBoundary, "map_loaded" );
	NewEvent->LoadOrdinal = GAppRandTraceStartupLoadOrdinal;
	NewEvent->OrdinalBefore = GAppRandTraceCallCount;
	Event = NewEvent;
}

FAppRandTracePhaseScope::~FAppRandTracePhaseScope()
{
	if( !Event )
		return;
	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( !GAppRandTraceFlushed )
		((FAppRandTracePhaseEvent*)Event)->OrdinalAfter = GAppRandTraceCallCount;
}

CORE_API void appSetStartupRandTraceMap( const TCHAR* MapToken, const TCHAR* MapURL )
{
	appInitRandTrace();
	if( !GAppRandTraceStartupPhaseEnabled || GAppRandTraceFlushed )
		return;
	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( GAppRandTraceFlushed )
		return;
	appSetRandTraceMapToken( GAppRandTraceStartupMapToken, sizeof(GAppRandTraceStartupMapToken), appToAnsi(MapToken) );
	appCopyRandTraceString( GAppRandTraceStartupMapURL, sizeof(GAppRandTraceStartupMapURL), appToAnsi(MapURL), "" );
	appCopyRandTraceString( GAppRandTraceStartupLifecycleBoundary, sizeof(GAppRandTraceStartupLifecycleBoundary), "map_loaded", "map_loaded" );
	++GAppRandTraceStartupLoadOrdinal;
}

CORE_API void appSetStartupRandTraceLifecycleBoundary( const ANSICHAR* Boundary )
{
	appInitRandTrace();
	if( !GAppRandTraceStartupPhaseEnabled || GAppRandTraceFlushed )
		return;
	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( !GAppRandTraceFlushed )
		appCopyRandTraceString( GAppRandTraceStartupLifecycleBoundary, sizeof(GAppRandTraceStartupLifecycleBoundary), Boundary, "map_loaded" );
}

CORE_API void appSetRandTraceTick( unsigned long long TickIndex, const ANSICHAR* Phase )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( GAppRandTraceFlushed )
		return;
	GAppRandTraceTickIndex = TickIndex;
	appSetRandTracePhase( GAppRandTracePhase, Phase );
	appAppendRandTraceTickBoundary();
}

void appFlushRandTrace()
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled && !GAppRandTraceStartupPhaseEnabled )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( GAppRandTraceFlushed )
		return;

	GAppRandTraceFlushed = 1;
	if( GAppRandTraceStartupPhaseEnabled )
		appWriteStartupRandTrace();
	if( GAppRandTraceEnabled )
		appWriteRandTrace();
	for( size_t Index=0; Index<GAppRandTraceCount; ++Index )
		appFreeRandTraceSample( GAppRandTraceSamples[Index] );
	for( size_t Index=0; Index<GAppRandTraceFunctionEventCount; ++Index )
		appFreeRandTraceFunctionEvent( GAppRandTraceFunctionEvents[Index] );
	for( size_t Index=0; Index<GAppRandTraceRelevanceEventCount; ++Index )
		appFreeRandTraceRelevanceEvent( GAppRandTraceRelevanceEvents[Index] );
	for( size_t Index=0; Index<GAppRandTracePreBeginEventCount; ++Index )
		appFreeRandTracePreBeginEvent( GAppRandTracePreBeginEvents[Index] );
	free( GAppRandTraceSamples );
	free( GAppRandTraceTickBoundaries );
	GAppRandTraceSamples = NULL;
	GAppRandTraceTickBoundaries = NULL;
	free( GAppRandTraceFunctionEvents );
	GAppRandTraceFunctionEvents = NULL;
	free( GAppRandTracePreBeginEvents );
	GAppRandTracePreBeginEvents = NULL;
	free( GAppRandTraceRelevanceEvents );
	GAppRandTraceRelevanceEvents = NULL;
}
CORE_API void appSetRandTraceDiagnosticSeed( unsigned RequestedSeed, unsigned Seed )
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled && !GAppRandTraceOrdinalEnabled )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( !GAppRandTraceFlushed )
	{
		GAppRandTraceDiagnosticSeed = Seed;
		GAppRandTraceDiagnosticRequestedSeed = RequestedSeed;
		GAppRandTraceDiagnosticSeeded = 1;
	}
}
CORE_API void appResetRandTraceForReplay()
{
	if( !GAppRandTraceInitialized.load(std::memory_order_acquire) )
	{
		const char* Path = getenv( "HP2_RNG_TRACE" );
		const char* GlobalTickPath = getenv( "HP2_GLOBAL_TICK_TRACE" );
		const char* StartupPhasePath = getenv( "HP2_STARTUP_RNG_PHASE_TRACE" );
		if( (!Path || !*Path) && (!GlobalTickPath || !*GlobalTickPath) && (!StartupPhasePath || !*StartupPhasePath) )
			return;
		appInitRandTrace();
	}
	if( !GAppRandTraceEnabled && !GAppRandTraceOrdinalEnabled )
		return;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	if( GAppRandTraceFlushed )
		return;

	for( size_t Index=0; Index<GAppRandTraceCount; ++Index )
		appFreeRandTraceSample( GAppRandTraceSamples[Index] );
	GAppRandTraceCount = 0;
	GAppRandTraceCallCount = 0;
	GAppRandTraceTruncated = 0;
	for( size_t Index=0; Index<GAppRandTraceFunctionEventCount; ++Index )
		appFreeRandTraceFunctionEvent( GAppRandTraceFunctionEvents[Index] );
	GAppRandTraceFunctionEventCount = 0;
	GAppRandTraceFunctionCallCount = 0;
	for( size_t Index=0; Index<GAppRandTraceRelevanceEventCount; ++Index )
		appFreeRandTraceRelevanceEvent( GAppRandTraceRelevanceEvents[Index] );
	GAppRandTraceRelevanceEventCount = 0;
	GAppRandTraceRelevanceEvent = NULL;
	for( size_t Index=0; Index<GAppRandTracePreBeginEventCount; ++Index )
		appFreeRandTracePreBeginEvent( GAppRandTracePreBeginEvents[Index] );
	GAppRandTracePreBeginEventCount = 0;
	GAppRandTracePreBeginEvent = NULL;
	GAppRandTraceReplaySeeded = 1;
	appResetRandTraceTickState();
}
CORE_API unsigned long long appGetRandTraceOrdinal()
{
	appInitRandTrace();
	if( !GAppRandTraceOrdinalEnabled )
		return 0;

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	return GAppRandTraceCallCount;
}
CORE_API INT appRand()
{
	appInitRandTrace();
	if( !GAppRandTraceEnabled )
	{
		const INT Value = rand();
		if( GAppRandTraceOrdinalEnabled )
		{
			std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
			if( !GAppRandTraceFlushed )
				GAppRandTraceCallCount++;
		}
		return Value;
	}

	std::lock_guard<std::mutex> Lock( GAppRandTraceMutex );
	const INT Value = rand();
	if( !GAppRandTraceFlushed )
		appRecordRandTrace( Value );
	return Value;
}

CORE_API FLOAT appFrand()
{
	return appRand() / (FLOAT)RAND_MAX;
}						  
CORE_API FLOAT appFrand(FLOAT From, FLOAT To)
{
	FLOAT r = FLOAT(appRand()) / FLOAT(RAND_MAX);
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

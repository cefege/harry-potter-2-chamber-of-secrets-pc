/*=============================================================================
	Core.h: Unreal core public header file.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#ifndef _INC_CORE
#define _INC_CORE

/*----------------------------------------------------------------------------
	Low level includes.
----------------------------------------------------------------------------*/

// API definition.
#ifndef CORE_API
#define CORE_API DLL_IMPORT
#endif

// Build options.
#include "UnBuild.h"
#include <array>
#include <limits>
#include <cstdio>
#include <string>

// Time.
#define FIXTIME 4294967296.f
class FTime
{
#if __GNUG__
#define TIMETYP long long
#else
#define TIMETYP __int64
#endif
public:

	static TIMETYP FromSeconds( long double Seconds )
	{
		const long double Scaled = Seconds * static_cast<long double>(FIXTIME);
		if( Scaled >= static_cast<long double>(std::numeric_limits<TIMETYP>::max()) )
			return std::numeric_limits<TIMETYP>::max();
		if( Scaled <= static_cast<long double>(std::numeric_limits<TIMETYP>::min()) )
			return std::numeric_limits<TIMETYP>::min();
		return static_cast<TIMETYP>(Scaled);
	}
	        FTime      ()               {v=0;}
	        FTime      (float f)        {v=FromSeconds(f);}
	        FTime      (double d)       {v=FromSeconds(d);}
	float   GetFloat   ()               {return v/FIXTIME;}
	FTime   operator+  (float f) const  {return FTime(v+(TIMETYP)(f*FIXTIME));}
	float   operator-  (FTime t) const  {return (v-t.v)/FIXTIME;}
	FTime   operator*  (float f) const  {return FTime(v*f);}
	FTime   operator/  (float f) const  {return FTime(v/f);}
	FTime&  operator+= (float f)        {v=v+(TIMETYP)(f*FIXTIME); return *this;}
	FTime&  operator*= (float f)        {v=(TIMETYP)(v*f); return *this;}
	FTime&  operator/= (float f)        {v=(TIMETYP)(v/f); return *this;}
	int     operator== (FTime t)        {return v==t.v;}
	int     operator!= (FTime t)        {return v!=t.v;}
	int     operator>  (FTime t)        {return v>t.v;}
	FTime&  operator=  (const FTime& t) {v=t.v; return *this;}
private:
	FTime (TIMETYP i) {v=i;}
	TIMETYP v;
};

#if _MSC_VER || __ICC || __LINUX__ || defined(__APPLE__)
	#define SUPPORTS_PRAGMA_PACK 1
#else
	#define SUPPORTS_PRAGMA_PACK 0
#endif

// Compiler specific include.
#if _MSC_VER
	#include "UnVcWin32.h"
#elif __GNUG__
	#include <string.h>
	#include "UnGnuG.h"
#else
	#error Unknown Compiler
#endif

// If no asm, redefine __asm to cause compile-time error.
#if !ASM && !__GNUG__
	#define __asm ERROR_ASM_NOT_ALLOWED
#endif

// OS specific include.
#if __UNIX__
	#include "UnUnix.h"
	#include <signal.h>
#endif

// Global constants.
enum {MAXBYTE		= 0xff       };
enum {MAXWORD		= 0xffffU    };
enum {MAXDWORD		= 0xffffffffU};
enum {MAXSBYTE		= 0x7f       };
enum {MAXSWORD		= 0x7fff     };
enum {MAXINT		= 0x7fffffff };
enum {INDEX_NONE	= -1         };
enum {UNICODE_BOM   = 0xfeff     };
enum ENoInit {E_NoInit = 0};

// Host and serialized character mappings.
#if defined(__APPLE__) && (defined(__aarch64__) || defined(__arm64__))
	#ifndef _TCHAR_DEFINED
		typedef wchar_t TCHAR;
		typedef wchar_t TCHARU;
		#define _TCHAR_DEFINED 1
	#endif
	#undef TEXT
	#define TEXT(s) L##s
	#undef US
	#define US FString(L"")
	inline TCHAR FromAnsi( ANSICHAR In )
	{
		return static_cast<TCHAR>(static_cast<ANSICHARU>(In));
	}
	inline TCHAR FromUnicode( UNICHAR In )
	{
		return static_cast<TCHAR>(In);
	}
	inline ANSICHAR ToAnsi( TCHAR In )
	{
		return In >= 0 && static_cast<UPTRINT>(In) < 0x100u
			? static_cast<ANSICHAR>(In)
			: static_cast<ANSICHAR>(MAXSBYTE);
	}
	inline UNICHAR ToUnicode( TCHAR In )
	{
		return In >= 0 && static_cast<UPTRINT>(In) <= 0xffffu
			? static_cast<UNICHAR>(In)
			: static_cast<UNICHAR>(0xfffd);
	}
	static_assert(sizeof(TCHAR) == 4, "Apple host TCHAR must use 32-bit wchar_t");
	static_assert(sizeof(TCHARU) == 4, "Apple host TCHARU must use 32-bit wchar_t");
	static_assert(sizeof(UNICHAR) == 2, "Serialized Unicode must remain UTF-16");
#elif defined(_UNICODE)
	#ifndef _TCHAR_DEFINED
		typedef UNICHAR  TCHAR;
		typedef UNICHARU TCHARU;
	#endif
	#undef TEXT
	#define TEXT(s) L##s
	#undef US
	#define US FString(L"")
	inline TCHAR    FromAnsi   ( ANSICHAR In ) { return (BYTE)In;                                            }
	inline TCHAR    FromUnicode( UNICHAR In  ) { return In;                                                  }
	inline ANSICHAR ToAnsi     ( TCHAR In    ) { return (_WORD)In<0x100 ? (ANSICHAR)In : (ANSICHAR)MAXSBYTE; }
	inline UNICHAR  ToUnicode  ( TCHAR In    ) { return In;                                                  }
#else
	#ifndef _TCHAR_DEFINED
		typedef ANSICHAR  TCHAR;
		typedef ANSICHARU TCHARU;
	#endif
	#undef TEXT
	#define TEXT(s) s
	#undef US
	#define US FString("")
	inline TCHAR    FromAnsi   ( ANSICHAR In ) { return In;                              }
	inline TCHAR    FromUnicode( UNICHAR In  ) { return (_WORD)In<0x100 ? In : MAXSBYTE; }
	inline ANSICHAR ToAnsi     ( TCHAR In    ) { return (_WORD)In<0x100 ? In : MAXSBYTE; }
	inline UNICHAR  ToUnicode  ( TCHAR In    ) { return (BYTE)In;                        }
#endif

/*----------------------------------------------------------------------------
	Forward declarations.
----------------------------------------------------------------------------*/

// Objects.
class	UObject;
class		UExporter;
class		UFactory;
class		UField;
class			UConst;
class			UEnum;
class			UProperty;
class				UByteProperty;
class				UIntProperty;
class				UBoolProperty;
class				UFloatProperty;
class				UObjectProperty;
class					UClassProperty;
class				UNameProperty;
class				UStructProperty;
class               UStrProperty;
class               UArrayProperty;
class			UStruct;
class				UFunction;
class				UState;
class					UClass;
class		ULinker;
class			ULinkerLoad;
class			ULinkerSave;
class		UPackage;
class		USubsystem;
class			USystem;
class		UTextBuffer;
class       URenderDevice;
class		UPackageMap;
class		UDebugger; //DEBUGGER


// Structs.
class FName;
class FArchive;
class FCompactIndex;
class FExec;
class FGuid;
class FMemCache;
class FMemStack;
class FPackageInfo;
class FTransactionBase;
class FUnknown;
class FRepLink;
class FArray;
class FLazyLoader;
class FString;
class FMalloc;

// Templates.
template<class T> class TArray;
template<class T> class TTransArray;
template<class T> class TLazyArray;
template<class TK, class TI> class TMap;
template<class TK, class TI> class TMultiMap;

// Globals.
CORE_API extern class FOutputDevice* GNull;

// EName definition.
#include "UnNames.h"

/*-----------------------------------------------------------------------------
	Abstract interfaces.
-----------------------------------------------------------------------------*/

// An output device.
class CORE_API FOutputDevice
{
public:
	// FOutputDevice interface.
	virtual void Serialize( const TCHAR* V, EName Event )=0;

	// Simple text printing.
	void Log( const TCHAR* S );
	void Log( enum EName Type, const TCHAR* S );
	void Log( const FString& S );
	void Log( enum EName Type, const FString& S );
	void Logf( const TCHAR* Fmt, ... );
	void Logf( enum EName Type, const TCHAR* Fmt, ... );
};

// Error device.
class CORE_API FOutputDeviceError : public FOutputDevice
{
public:
	virtual void HandleError()=0;
};

// Memory allocator.
class CORE_API FMalloc
{
public:
	virtual void* Malloc( DWORD Count, const TCHAR* Tag )=0;
	virtual void* Realloc( void* Original, DWORD Count, const TCHAR* Tag )=0;
	virtual void Free( void* Original )=0;
	virtual INT MemSize( void* Mem )  { return 0; }
	virtual void SetTag( const TCHAR* Tag ) {}
	virtual const TCHAR* GetTag() { return NULL; }
	virtual void DumpAllocs()=0;
	virtual void HeapCheck()=0;
	virtual void Init()=0;
	virtual void Exit()=0;
};

// Configuration database cache.
class FConfigCache
{
public:
	virtual UBOOL GetBool( const TCHAR* Section, const TCHAR* Key, UBOOL& Value, const TCHAR* Filename=NULL )=0;
	virtual UBOOL GetInt( const TCHAR* Section, const TCHAR* Key, INT& Value, const TCHAR* Filename=NULL )=0;
	virtual UBOOL GetFloat( const TCHAR* Section, const TCHAR* Key, FLOAT& Value, const TCHAR* Filename=NULL )=0;
	virtual UBOOL GetString( const TCHAR* Section, const TCHAR* Key, TCHAR* Value, INT Size, const TCHAR* Filename=NULL )=0;
	virtual UBOOL GetString( const TCHAR* Section, const TCHAR* Key, class FString& Str, const TCHAR* Filename=NULL )=0;
	virtual const TCHAR* GetStr( const TCHAR* Section, const TCHAR* Key, const TCHAR* Filename=NULL )=0;
	virtual UBOOL GetSection( const TCHAR* Section, TCHAR* Value, INT Size, const TCHAR* Filename=NULL )=0;
	virtual TMultiMap<FString,FString>* GetSectionPrivate( const TCHAR* Section, UBOOL Force, UBOOL Const, const TCHAR* Filename=NULL )=0;
	virtual void EmptySection( const TCHAR* Section, const TCHAR* Filename=NULL )=0;
	virtual void SetBool( const TCHAR* Section, const TCHAR* Key, UBOOL Value, const TCHAR* Filename=NULL )=0;
	virtual void SetInt( const TCHAR* Section, const TCHAR* Key, INT Value, const TCHAR* Filename=NULL )=0;
	virtual void SetFloat( const TCHAR* Section, const TCHAR* Key, FLOAT Value, const TCHAR* Filename=NULL )=0;
	virtual void SetString( const TCHAR* Section, const TCHAR* Key, const TCHAR* Value, const TCHAR* Filename=NULL )=0;
	virtual void Flush( UBOOL Read, const TCHAR* Filename=NULL )=0;
	virtual void Detach( const TCHAR* Filename )=0;
	virtual void Init( const TCHAR* InSystem, const TCHAR* InUser, UBOOL RequireConfig )=0;
	virtual void Exit()=0;
	virtual void Dump( FOutputDevice& Ar )=0;
	virtual ~FConfigCache() {};
};

// Any object that is capable of taking commands.
class CORE_API FExec
{
public:
	virtual UBOOL Exec( const TCHAR* Cmd, FOutputDevice& Ar )=0;
};

// Notification hook.
class CORE_API FNotifyHook
{
public:
	virtual void NotifyDestroy( void* Src ) {}
	virtual void NotifyPreChange( void* Src ) {}
	virtual void NotifyPostChange( void* Src ) {}
	virtual void NotifyExec( void* Src, const TCHAR* Cmd ) {}
};

// Interface for returning a context string.
class FContextSupplier
{
public:
	virtual FString GetContext()=0;
};

// A context for displaying modal warning messages.
class CORE_API FFeedbackContext : public FOutputDevice
{
public:
	virtual UBOOL YesNof( const TCHAR* Fmt, ... )=0;
	virtual void BeginSlowTask( const TCHAR* Task, UBOOL StatusWindow, UBOOL Cancelable )=0;
	virtual void EndSlowTask()=0;
	virtual UBOOL VARARGS StatusUpdatef( INT Numerator, INT Denominator, const TCHAR* Fmt, ... )=0;
	virtual void SetContext( FContextSupplier* InSupplier )=0;
};

// Class for handling undo/redo transactions among objects.
typedef void( *STRUCT_AR )( FArchive& Ar, void* TPtr );
typedef void( *STRUCT_DTOR )( void* TPtr );
class CORE_API FTransactionBase
{
public:
	virtual void SaveObject( UObject* Object )=0;
	virtual void SaveArray( UObject* Object, FArray* Array, INT Index, INT Count, INT Oper, INT ElementSize, STRUCT_AR Serializer, STRUCT_DTOR Destructor )=0;
	virtual void Apply()=0;
};

// File manager.
enum EFileTimes
{
	FILETIME_Create      = 0,
	FILETIME_LastAccess  = 1,
	FILETIME_LastWrite   = 2,
};
enum EFileWrite
{
	FILEWRITE_NoFail            = 0x01,
	FILEWRITE_NoReplaceExisting = 0x02,
	FILEWRITE_EvenIfReadOnly    = 0x04,
	FILEWRITE_Unbuffered        = 0x08,
	FILEWRITE_Append			= 0x10,
	FILEWRITE_AllowRead         = 0x20,
};
enum EFileRead
{
	FILEREAD_NoFail             = 0x01,
};
class CORE_API FFileManager
{
public:
	virtual FArchive* CreateFileReader( const TCHAR* Filename, DWORD ReadFlags=0, FOutputDevice* Error=GNull )=0;
	virtual FArchive* CreateFileWriter( const TCHAR* Filename, DWORD WriteFlags=0, FOutputDevice* Error=GNull )=0;
	virtual INT FileSize( const TCHAR* Filename )=0;
	virtual UBOOL Delete( const TCHAR* Filename, UBOOL RequireExists=0, UBOOL EvenReadOnly=0 )=0;
	virtual UBOOL Copy( const TCHAR* Dest, const TCHAR* Src, UBOOL Replace=1, UBOOL EvenIfReadOnly=0, UBOOL Attributes=0, void (*Progress)(FLOAT Fraction)=NULL )=0;
	virtual UBOOL Move( const TCHAR* Dest, const TCHAR* Src, UBOOL Replace=1, UBOOL EvenIfReadOnly=0, UBOOL Attributes=0 )=0;
	virtual SQWORD GetGlobalTime( const TCHAR* Filename )=0;
	virtual UBOOL SetGlobalTime( const TCHAR* Filename )=0;
	virtual UBOOL MakeDirectory( const TCHAR* Path, UBOOL Tree=0 )=0;
	virtual UBOOL DeleteDirectory( const TCHAR* Path, UBOOL RequireExists=0, UBOOL Tree=0 )=0;
	virtual TArray<FString> FindFiles( const TCHAR* Filename, UBOOL Files, UBOOL Directories )=0;
	virtual UBOOL SetDefaultDirectory( const TCHAR* Filename )=0;
	virtual FString GetDefaultDirectory()=0;
	virtual void Init(UBOOL Startup) {}
};


//	Begin addition for initial port of audio system.  -tg
//
// TG ALPHA
//
// File Streaming.
//

enum EFileStreamType
{
	ST_Regular		= 0,
	ST_Ogg			= 1,
	ST_OggLooping	= 2,
	ST_XA			= 3,
	ST_XALooping	= 4
};

struct FStreamCompletion
{
	void*	Destination;
	UBOOL	Terminal;
	UBOOL	Ready;
};

struct FStream;

class CORE_API FFileStream
{
public:
	static FFileStream* Init( INT MaxStreams );
	static void Destroy();

	// Interface functions.
	// Successful Ogg creation adopts TDD; failed creation leaves it with the caller.
	// XA creation copies RawData into stream-owned storage before returning.
	// Initial data is synchronously primed; queued destinations must remain valid
	// until completed, cancelled, or DestroyStream has returned.
	INT CreateStream( const TCHAR* Filename, INT ChunkSize, INT InitialChunks, void* Data, EFileStreamType Type, void* TDD );
	INT CreateStream( TLazyArray<BYTE> *RawData, INT NumSamples, INT ChunkSize, INT InitialChunks, void* Data, EFileStreamType Type, void* TDD );
	UBOOL RequestChunk( INT StreamId, void* Destination );
	UBOOL PopCompletedChunk( INT StreamId, void*& OutDestination, UBOOL& OutTerminal );
	UBOOL IsStreamAlive( INT StreamId );
	void DestroyStream( INT StreamId, UBOOL ReadQueuedChunks );

	// Only use the below functions in the thread's main loop.
	UBOOL Create( INT StreamId, const TCHAR* Filename );
	UBOOL Create( INT StreamId, TLazyArray<BYTE> *RawData, INT NumSamples );
	UBOOL Destroy( INT StreamId );
	void Enter(INT StreamId);
	void Leave(INT StreamId);

	static FFileStream* Instance;
	static INT MaxStreams;
	static FStream* Streams;
	static INT Destroyed;

private:
	FFileStream() {}
	~FFileStream() {}
};

// Allocation-free decoder for mono EA-XA blocks.
class CORE_API FEAXABlockDecoder
{
public:
	FEAXABlockDecoder();

	UBOOL Feed( const BYTE* Data, INT Bytes, INT NumSamples );
	INT Decode( SWORD* Dest, INT Samples );
	void ResetState( SWORD Sample1=0, SWORD Sample2=0 );

private:
	const BYTE* EncodedData;
	INT EncodedBytes;
	INT EncodedOffset;
	INT SamplesRemaining;
	SWORD History1;
	SWORD History2;
	SWORD Residue[28];
	INT ResidueOffset;
	INT ResidueSamples;
};

//	End addition for initial port of audio system.  -tg
// TG ALPHA


/*----------------------------------------------------------------------------
	Global variables.
----------------------------------------------------------------------------*/

// Core globals.
CORE_API extern FMemStack				GMem;
CORE_API extern FOutputDevice*			GLog;
CORE_API extern FOutputDevice*			GNull;
CORE_API extern FOutputDevice*		    GThrow;
CORE_API extern FOutputDeviceError*		GError;
CORE_API extern FFeedbackContext*		GWarn;
CORE_API extern FConfigCache*			GConfig;
CORE_API extern FTransactionBase*		GUndo;
CORE_API extern FOutputDevice*			GLogHook;
CORE_API extern FExec*					GExec;
CORE_API extern FMalloc*				GMalloc;
CORE_API extern FFileManager*			GFileManager;
CORE_API extern USystem*				GSys;
CORE_API extern UProperty*				GProperty;
CORE_API extern BYTE*					GPropAddr;
CORE_API extern USubsystem*				GWindowManager;
CORE_API extern TCHAR				    GErrorHist[4096];
CORE_API extern TCHAR                   GTrue[64], GFalse[64], GYes[64], GNo[64], GNone[64];
CORE_API extern TCHAR					GCdPath[];
CORE_API extern	FLOAT					GSecondsPerCycle;
CORE_API extern	FTime					GTempTime;
CORE_API extern void					(*GTempFunc)(void*);
CORE_API extern SQWORD					GTicks;
CORE_API extern INT                     GScriptCycles;
CORE_API extern DWORD					GPageSize;
CORE_API extern DWORD					GProcessorCount;
CORE_API extern DWORD					GPhysicalMemory;
CORE_API extern DWORD                   GUglyHackFlags;
CORE_API extern UBOOL					GIsScriptable;
CORE_API extern UBOOL					GIsEditor;
CORE_API extern UBOOL					GIsClient;
CORE_API extern UBOOL					GIsServer;
CORE_API extern UBOOL					GIsCriticalError;
CORE_API extern UBOOL					GIsStarted;
CORE_API extern UBOOL					GIsRunning;
CORE_API extern UBOOL					GIsSlowTask;
CORE_API extern UBOOL					GIsGuarded;
CORE_API extern UBOOL					GIsRequestingExit;
CORE_API extern UBOOL					GIsStrict;
CORE_API extern UBOOL                   GScriptEntryTag;
CORE_API extern UBOOL                   GLazyLoad;
CORE_API extern UBOOL					GUnicode;
CORE_API extern UBOOL					GUnicodeOS;
CORE_API extern class FGlobalMath		GMath;
CORE_API extern	URenderDevice*			GRenderDevice;
CORE_API extern class FArchive*         GDummySave;
CORE_API extern DWORD					GCurrentViewport;
CORE_API extern UBOOL					GEncryptLoad;
CORE_API extern UBOOL					GCurrentEncrypt;
CORE_API extern	UDebugger*				GDebugger; //DEBUGGER
CORE_API extern FFileStream*			GFileStream;				// TG ALPHA
CORE_API extern FLOAT					GAudioMaxRadiusMultiplier;	// TG ALPHA
CORE_API extern FLOAT					GAudioDefaultRadius;		// TG ALPHA

// Per module globals.
#if _MSC_VER
extern "C" DLL_EXPORT TCHAR GPackage[];
#else
DLL_EXPORT TCHAR GPackage[];
#endif

// Normal includes.
#include "UnFile.h"			// Low level utility code.
#include "UnObjVer.h"		// Object version info.
#include "UnArc.h"			// Archive class.
#include "UnTemplate.h"     // Dynamic arrays.

static constexpr size_t FILE_STREAM_REQUEST_CAPACITY = 4096;

struct FStream
{
	void*	Handle;
	void*	TDD;		// type dependent data
	INT		FileSeek;
	INT		ChunkSize;
	std::array<void*, FILE_STREAM_REQUEST_CAPACITY> PendingDestinations;
	size_t	PendingHead;
	size_t	PendingCount;
	std::array<FStreamCompletion, FILE_STREAM_REQUEST_CAPACITY> CompletedChunks;
	size_t	CompletionHead;
	size_t	CompletionCount;
	INT		Locked;
	INT		Used;
	INT		EndOfFile;
	INT		NumSamples;		// used for looping streaming XA files to re-feed data when at end.
	EFileStreamType	Type;
};
#include "UnName.h"			// Global name subsystem.
#include "UnStack.h"		// Script stack definition.
#include "UnObjBas.h"		// Object base class.
#include "UnCoreNet.h"		// Core networking.
#include "UnCorObj.h"		// Core object class definitions.
#include "UnClass.h"		// Class definition.
#include "UnType.h"			// Base property type.
#include "UnScript.h"		// Script class.
#include "UFactory.h"		// Factory definition.
#include "UExporter.h"		// Exporter definition.
#include "UnCache.h"		// Cache based memory management.
#include "UnMem.h"			// Stack based memory management.
#include "UnCId.h"          // Cache ID's.
#include "UnBits.h"         // Bitstream archiver.
#include "UnMath.h"         // Vector math functions.

#if __STATIC_LINK
#include "UnCoreNative.h"
#endif


// Very basic abstract debugger class.
class UDebugger //DEBUGGER
{
public:
	virtual void DebugInfo( UObject* Debugee, FFrame* Stack, FString InfoType, int LineNumber, int InputPos )=0;
};



/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
#endif

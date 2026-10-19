// C callbacks behind the Rust UnURL port (crates/hp2-engine/src/url.rs).
// Each thunk performs one static, config, string or FURL member access from the
// original UnURL.cpp code, so the Rust side never re-implements FString layout,
// config storage or libc case folding. FURL objects arrive as void*.

#include "EnginePrivate.h"
#include "Hp2UrlServices.h"

static FString& Hp2UrlStatic( int Id )
{
	switch( Id )
	{
		case 0:  return FURL::DefaultProtocol;
		case 1:  return FURL::DefaultProtocolDescription;
		case 2:  return FURL::DefaultName;
		case 3:  return FURL::DefaultMap;
		case 4:  return FURL::DefaultLocalMap;
		case 5:  return FURL::DefaultHost;
		case 6:  return FURL::DefaultPortal;
		case 7:  return FURL::DefaultMapExt;
		case 8:  return FURL::DefaultSaveExt;
		default: appErrorf( TEXT("Hp2UrlServices: bad static id %i"), Id ); return FURL::DefaultSaveExt;
	}
}

static const FString& Hp2UrlField( const FURL* Url, int Field )
{
	switch( Field )
	{
		case 0:  return Url->Protocol;
		case 1:  return Url->Host;
		case 2:  return Url->Map;
		case 3:  return Url->Portal;
		default: appErrorf( TEXT("Hp2UrlServices: bad field id %i"), Field ); return Url->Protocol;
	}
}

static FString& Hp2UrlFieldMut( FURL* Url, int Field )
{
	switch( Field )
	{
		case 0:  return Url->Protocol;
		case 1:  return Url->Host;
		case 2:  return Url->Map;
		case 3:  return Url->Portal;
		default: appErrorf( TEXT("Hp2UrlServices: bad field id %i"), Field ); return Url->Protocol;
	}
}

static const void* Hp2UrlStaticChars( int Id )
{
	return *Hp2UrlStatic( Id );
}

static void Hp2UrlStaticSetChars( int Id, const void* Chars )
{
	Hp2UrlStatic( Id ) = static_cast<const TCHAR*>(Chars);
}

static int Hp2UrlDefaultPort( void )
{
	return FURL::DefaultPort;
}

static void Hp2UrlDefaultPortSet( int Port )
{
	FURL::DefaultPort = Port;
}

static const void* Hp2UrlConfigGetStr( const void* Section, const void* Key )
{
	return GConfig->GetStr( static_cast<const TCHAR*>(Section), static_cast<const TCHAR*>(Key) );
}

static void Hp2UrlConfigGetSection( const void* Section, void* Value, int Size, const void* Filename )
{
	GConfig->GetSection( static_cast<const TCHAR*>(Section), static_cast<TCHAR*>(Value), Size, static_cast<const TCHAR*>(Filename) );
}

static void Hp2UrlConfigSetString( const void* Section, const void* Key, const void* Value, const void* Filename )
{
	GConfig->SetString( static_cast<const TCHAR*>(Section), static_cast<const TCHAR*>(Key), static_cast<const TCHAR*>(Value), static_cast<const TCHAR*>(Filename) );
}

static int Hp2UrlStrIcmp( const void* A, const void* B )
{
	return appStricmp( static_cast<const TCHAR*>(A), static_cast<const TCHAR*>(B) );
}

static int Hp2UrlStrNicmp( const void* A, const void* B, int Count )
{
	return appStrnicmp( static_cast<const TCHAR*>(A), static_cast<const TCHAR*>(B), Count );
}

static int Hp2UrlStrAtoi( const void* Str )
{
	return appAtoi( static_cast<const TCHAR*>(Str) );
}

static const void* Hp2UrlFieldRead( const void* Url, int Field )
{
	return *Hp2UrlField( static_cast<const FURL*>(Url), Field );
}

static int Hp2UrlPort( const void* Url )
{
	return static_cast<const FURL*>(Url)->Port;
}

static int Hp2UrlValid( const void* Url )
{
	return static_cast<const FURL*>(Url)->Valid;
}

static int Hp2UrlOpCount( const void* Url )
{
	return static_cast<const FURL*>(Url)->Op.Num();
}

static const void* Hp2UrlOp( const void* Url, int Index )
{
	return *static_cast<const FURL*>(Url)->Op(Index);
}

static void Hp2UrlSetField( void* Url, int Field, const void* Chars )
{
	Hp2UrlFieldMut( static_cast<FURL*>(Url), Field ) = static_cast<const TCHAR*>(Chars);
}

static void Hp2UrlSetPort( void* Url, int Port )
{
	static_cast<FURL*>(Url)->Port = Port;
}

static void Hp2UrlSetValid( void* Url, int Valid )
{
	static_cast<FURL*>(Url)->Valid = Valid;
}

static void Hp2UrlOpClear( void* Url )
{
	static_cast<FURL*>(Url)->Op.Empty();
}

static void Hp2UrlOpPush( void* Url, const void* Chars )
{
	FURL* U = static_cast<FURL*>(Url);
	new(U->Op)FString( static_cast<const TCHAR*>(Chars) );
}

static void Hp2UrlOpSet( void* Url, int Index, const void* Chars )
{
	static_cast<FURL*>(Url)->Op(Index) = static_cast<const TCHAR*>(Chars);
}

static void Hp2UrlFStrSet( void* Str, const void* Chars )
{
	*static_cast<FString*>(Str) = static_cast<const TCHAR*>(Chars);
}

static void Hp2UrlLogDefaultOption( const void* Chars )
{
	debugf( TEXT("URL: Adding default option %s"), static_cast<const TCHAR*>(Chars) );
}

const Hp2UrlServices GHp2UrlServices =
{
	Hp2UrlStaticChars,
	Hp2UrlStaticSetChars,
	Hp2UrlDefaultPort,
	Hp2UrlDefaultPortSet,
	Hp2UrlConfigGetStr,
	Hp2UrlConfigGetSection,
	Hp2UrlConfigSetString,
	Hp2UrlStrIcmp,
	Hp2UrlStrNicmp,
	Hp2UrlStrAtoi,
	Hp2UrlFieldRead,
	Hp2UrlPort,
	Hp2UrlValid,
	Hp2UrlOpCount,
	Hp2UrlOp,
	Hp2UrlSetField,
	Hp2UrlSetPort,
	Hp2UrlSetValid,
	Hp2UrlOpClear,
	Hp2UrlOpPush,
	Hp2UrlOpSet,
	Hp2UrlFStrSet,
	Hp2UrlLogDefaultOption,
};

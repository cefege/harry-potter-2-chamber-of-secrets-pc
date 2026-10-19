/*=============================================================================
	UnURL.cpp: Various file-management functions.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "hp2_rs.h"
#include "Hp2UrlServices.h"

/*-----------------------------------------------------------------------------
	FURL Statics.
-----------------------------------------------------------------------------*/

// Variables.
FString FURL::DefaultProtocol;
FString FURL::DefaultProtocolDescription;
FString FURL::DefaultName;
FString FURL::DefaultMap;
FString FURL::DefaultLocalMap;
FString FURL::DefaultHost;
FString FURL::DefaultPortal;
FString FURL::DefaultMapExt;
FString FURL::DefaultSaveExt;
INT		FURL::DefaultPort=0;

// Static init.
void FURL::StaticInit()
{
	guard(FURL::StaticInit);
	hp2_rs_engine_url_static_init( &GHp2UrlServices );
	unguard;
}
void FURL::StaticExit()
{
	guard(FURL::StaticExit);
	hp2_rs_engine_url_static_exit( &GHp2UrlServices );
	unguard;
}

ENGINE_API FArchive& operator<<( FArchive& Ar, FURL& U )
{
	guard(FURL<<);
	Ar << U.Protocol << U.Host << U.Map << U.Portal << U.Op << U.Port << U.Valid;
	return Ar;
	unguard;
}

UBOOL appFilePathToFURLToken( const TCHAR* Path, TCHAR* Out, INT OutCapacity )
{
	return hp2_rs_engine_url_file_path_to_token( Path, Out, OutCapacity );
}


/*-----------------------------------------------------------------------------
	Constructors.
-----------------------------------------------------------------------------*/

//
// Constuct a purely default, local URL from an optional filename.
//
FURL::FURL( const TCHAR* LocalFilename )
:	Protocol	( DefaultProtocol )
,	Host		( DefaultHost )
,	Map			( LocalFilename ? LocalFilename : DefaultMap )
,	Portal		( DefaultPortal )
,	Port		( DefaultPort )
,	Op			()
,	Valid		( 1 )
{}

//
// Construct a URL from text and an optional relative base.
//
FURL::FURL( FURL* Base, const TCHAR* TextURL, ETravelType Type )
{
	guard(FURL::FURL);
	check(TextURL);
	hp2_rs_engine_url_construct( &GHp2UrlServices, this, Base, TextURL, Type );
	unguard;
}

/*-----------------------------------------------------------------------------
	Conversion to text.
-----------------------------------------------------------------------------*/

//
// Convert this URL to text.
//
FString FURL::String( UBOOL FullyQualified ) const
{
	guard(FURL::String);
	FString Result;
	hp2_rs_engine_url_string( &GHp2UrlServices, this, FullyQualified, &Result );
	return Result;
	unguard;
}

/*-----------------------------------------------------------------------------
	Informational.
-----------------------------------------------------------------------------*/

//
// Return whether this URL corrsponds to an internal object, i.e. an Unreal
// level which this app can try to connect to locally or on the net. If this
// is fals, the URL refers to an object that a remote application like Internet
// Explorer can execute.
//
UBOOL FURL::IsInternal() const
{
	guard(FURL::IsInternal);
	return hp2_rs_engine_url_is_internal( &GHp2UrlServices, this );
	unguard;
}

//
// Return whether this URL corresponds to an internal object on this local 
// process. In this case, no Internet use is necessary.
//
UBOOL FURL::IsLocalInternal() const
{
	guard(FURL::IsLocalInternal);
	return hp2_rs_engine_url_is_local_internal( &GHp2UrlServices, this );
	unguard;
}

//
// Add a unique option to the URL, replacing any existing one.
//
void FURL::AddOption( const TCHAR* Str )
{
	guard(FURL::AddOption);
	hp2_rs_engine_url_add_option( &GHp2UrlServices, this, Str );
	unguard;
}

//
// Load URL from config.
//
void FURL::LoadURLConfig( const TCHAR* Section, const TCHAR* Filename )
{
	guard(FURL::LoadURLConfig);
	hp2_rs_engine_url_load_config( &GHp2UrlServices, this, Section, Filename );
	unguard;
}

//
// Save URL to config.
//
void FURL::SaveURLConfig( const TCHAR* Section, const TCHAR* Item, const TCHAR* Filename ) const
{
	guard(FURL::SaveURLConfig);
	hp2_rs_engine_url_save_config( &GHp2UrlServices, this, Section, Item, Filename );
	unguard;
}

//
// See if the URL contains an option string.
//
UBOOL FURL::HasOption( const TCHAR* Test ) const
{
	guard(FURL::HasOption);
	return hp2_rs_engine_url_has_option( &GHp2UrlServices, this, Test );
	unguard;
}

//
// Return an option if it exists.
//
const TCHAR* FURL::GetOption( const TCHAR* Match, const TCHAR* Default ) const
{
	guard(FURL::GetOption);
	return static_cast<const TCHAR*>( hp2_rs_engine_url_get_option( &GHp2UrlServices, this, Match, Default ) );
	unguard;
}

/*-----------------------------------------------------------------------------
	Comparing.
-----------------------------------------------------------------------------*/

//
// Compare two URL's to see if they refer to the same exact thing.
//
UBOOL FURL::operator==( const FURL& Other ) const
{
	guard(FURL::operator==);
	return hp2_rs_engine_url_equal( &GHp2UrlServices, this, &Other );
	unguard;
}


/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

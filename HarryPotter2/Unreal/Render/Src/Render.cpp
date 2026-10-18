/*=============================================================================
	Render.cpp: Unreal rendering package.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.
=============================================================================*/

#include "Render.h"
#include "hp2_rs.h"

#include <new>
#include <string>
#include <vector>

/*-----------------------------------------------------------------------------
	Package implementation.
-----------------------------------------------------------------------------*/

RENDER_API FMemStack GDynMem, GSceneMem;

IMPLEMENT_PACKAGE(Render);

/*-----------------------------------------------------------------------------
	Services for the Rust Render port (crates/hp2-render). Rust never calls
	the engine directly; it receives this table on every export call.
-----------------------------------------------------------------------------*/

static FLOAT RenderFrand()
{
	return appFrand();
}

static BYTE* RenderMemPush( void* Mem, INT Size, INT Align )
{
	return static_cast<FMemStack*>(Mem)->PushBytes( Size, Align );
}

static void RenderMemMarkSave( void* Mem, void* Mark )
{
	new(Mark) FMemMark( *static_cast<FMemStack*>(Mem) );
}

static void RenderMemMarkPop( void* Mark )
{
	static_cast<FMemMark*>(Mark)->Pop();
}

static void RenderLogWarning( const uint8_t* Text, size_t Length )
{
	const std::string Utf8( reinterpret_cast<const char*>(Text), Length );
	std::vector<TCHAR> Converted( Length + 1 );
	if( appFromUtf8InPlace( Converted.data(), Utf8.c_str(), static_cast<INT>(Converted.size()) ) )
		debugf( NAME_Warning, TEXT("%s"), Converted.data() );
	else
		debugf( NAME_Warning, TEXT("HP2Rust: <log record failed UTF-8 conversion>") );
}

static FLOAT RenderSinTab( INT Angle )
{
	return GMath.SinTab( Angle );
}

static FLOAT RenderCosTab( INT Angle )
{
	return GMath.CosTab( Angle );
}

static FLOAT RenderSqrtApprox( FLOAT F )
{
	return SqrtApprox( F );
}

static FLOAT RenderDivSqrtApprox( FLOAT F )
{
	return DivSqrtApprox( F );
}

static FLOAT RenderDivApprox( FLOAT F )
{
	return DivApprox( F );
}

static FLOAT RenderGMathDivSqrtApprox( FLOAT F )
{
	return GMath.DivSqrtApprox( F );
}

static FLOAT RenderGMathSqrtApprox( FLOAT F )
{
	return GMath.SqrtApprox( F );
}

static const Hp2RenderServices RenderServicesTable =
{
	RenderFrand,
	RenderMemPush,
	RenderMemMarkSave,
	RenderMemMarkPop,
	RenderLogWarning,
	RenderSinTab,
	RenderCosTab,
	RenderSqrtApprox,
	RenderDivSqrtApprox,
	RenderDivApprox,
	RenderGMathDivSqrtApprox,
	RenderGMathSqrtApprox,
};
static_assert( sizeof(Hp2RenderServices) == 12*sizeof(void*), "Hp2RenderServices field count must match RenderServicesTable" );

const Hp2RenderServices* GRenderServices = &RenderServicesTable;

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

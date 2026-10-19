/*=============================================================================
	UnParams.cpp: Functions to help parse commands.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	What's happening: When the Visual Basic level editor is being used,
	this code exchanges messages with Visual Basic.  This lets Visual Basic
	affect the world, and it gives us a way of sending world information back
	to Visual Basic.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "hp2_rs.h"
#include "Hp2ParamsServices.h"

/*-----------------------------------------------------------------------------
	Getters.
	All of these functions return 1 if the appropriate item was
	fetched, or 0 if not.
-----------------------------------------------------------------------------*/

//
// Get a floating-point vector (X=, Y=, Z=), return number of components parsed (0-3).
//
UBOOL ENGINE_API GetFVECTOR( const TCHAR* Stream, FVector& Value )
{
	guard(GetFVECTOR);
	return hp2_rs_engine_params_get_fvector_stream( &GHp2ParamsServices, Stream, &Value );
	unguard;
}

//
// Get a string enclosed in parenthesis.
//
UBOOL ENGINE_API GetSUBSTRING
(
	const TCHAR*	Stream, 
	const TCHAR*	Match,
	TCHAR*			Value,
	INT				MaxLen
)
{
	guard(GetSUBSTRING);
	return hp2_rs_engine_params_get_substring( &GHp2ParamsServices, Stream, Match, Value, MaxLen );
	unguard;
}

//
// Get a floating-point vector (X=, Y=, Z=), return number of components parsed (0-3).
//
UBOOL ENGINE_API GetFVECTOR
(
	const TCHAR*	Stream, 
	const TCHAR*	Match, 
	FVector&		Value
)
{
	guard(GetFVECTOR);

	return hp2_rs_engine_params_get_fvector( &GHp2ParamsServices, Stream, Match, &Value );

	unguard;
}

//
// Get a set of rotations (PITCH=, YAW=, ROLL=), return number of components parsed (0-3).
//
UBOOL ENGINE_API GetFROTATOR
(
	const TCHAR*	Stream, 
	FRotator&		Rotation,
	INT				ScaleFactor
)
{
	guard(GetFROTATOR);

	return hp2_rs_engine_params_get_frotator_stream( &GHp2ParamsServices, Stream, &Rotation, ScaleFactor );
	unguard;
}

//
// Get a rotation value, return number of components parsed (0-3).
//
UBOOL ENGINE_API GetFROTATOR
(
	const TCHAR*	Stream, 
	const TCHAR*	Match, 
	FRotator&		Value,
	INT				ScaleFactor
)
{
	guard(GetFROTATOR);

	return hp2_rs_engine_params_get_frotator( &GHp2ParamsServices, Stream, Match, &Value, ScaleFactor );

	unguard;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

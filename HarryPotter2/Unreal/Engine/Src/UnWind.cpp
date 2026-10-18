//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999.
//
//  File:       UnWind.cpp
//
//  Contents:   Implementation of AWind.h
//
//  History:    00-Jan-18	SPeter created
//
//	To do:		Make fluctuation both spatially and temporally variant, but
//				locally coherent.
//
//---------------------------------------------------------------------------

#include "EnginePrivate.h"
#include "HP2EngineServices.h"

/*-----------------------------------------------------------------------------
	AWind implementation.
	Wind registry, gust integration, and sampling live in crates/hp2-engine
	(wind.rs); this file keeps the UObject shell.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(AWind);

AWind::AWind()
{
	hp2_rs_engine_wind_register( this );
}

void AWind::Destroy()
{
	hp2_rs_engine_wind_unregister( this );
	Super::Destroy();
}

UBOOL AWind::Tick( FLOAT DeltaTime, enum ELevelTick TickType )
{
	Super::Tick( DeltaTime, TickType );
	hp2_rs_engine_wind_tick( &GHp2EngineServices, this, DeltaTime );
	return 1;
}

FVector AWind::GetWind( const FVector& Loc )
{
	FVector Result;
	hp2_rs_native_AWind_GetWind( &GHp2EngineServices, this, &Loc, &Result );
	return Result;
}

FVector AWind::GetTotalWind( ULevel* Level, const FVector& Loc )
{
	FVector TotalWind;
	hp2_rs_engine_wind_get_total_wind( &GHp2EngineServices, &Loc, &TotalWind );
	return TotalWind;
}

// Script interfaces.

HP2_RUST_NATIVE( AWind, GetWind, P_GET_VECTOR (Loc), &Loc, (FVector*)Result )

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

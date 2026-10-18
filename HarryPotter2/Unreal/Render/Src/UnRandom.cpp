/*=============================================================================
	UnRandom.cpp: Random number generation code.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Description:
	Maintains a set of random numbers from 0.0-1.0 updated per tick.  One 
	set of random numbers is discontinuous and is regenerated every tick,
	and the other set is continuous and is smoothly interpolated every tick.

	The algorithm lives in crates/hp2-render/src/random.rs. This file owns the
	storage, the engine RNG callback table, and the UnrealScript-facing shell.

Notes:
*	Subsystem is designed for single use.

Revision history:
    10-21-96, Tim: Rewritten from the ground up.
=============================================================================*/

#include "Render.h"
#include "hp2_rs.h"

/*------------------------------------------------------------------------------------
	FGlobalRandoms definition
------------------------------------------------------------------------------------*/

class FGlobalRandoms : public FGlobalRandomsBase
{
private:

	// Functions from FGlobalRandomsBase.
	void Init();
	void Exit();
	void Tick(FLOAT TimeSeconds);

	// Variables.
	static FLOAT RandomDeltas[N_RANDS]; // Deltas used to update Randoms.
	static DWORD LastTicks;
};

//
// FGlobalRandomsBase statics.
//
FLOAT FGlobalRandomsBase::RandomBases[N_RANDS];
FLOAT FGlobalRandomsBase::Randoms[N_RANDS];

//
// FGlobalRandoms statics.
//
FLOAT FGlobalRandoms::RandomDeltas[N_RANDS];
DWORD FGlobalRandoms::LastTicks=0;

/*------------------------------------------------------------------------------------
	FGlobalRandoms Implementation
------------------------------------------------------------------------------------*/

//
// Initialize random number subsytem.
//
void FGlobalRandoms::Init()
{
	guard(FGlobalRandoms::Init);
	unguard;
}

//
// Shut down random number subsytem.
//
void FGlobalRandoms::Exit()
{
	guard(FGlobalRandoms::Exit);
	unguard;
}

//
// Update random number tables.
//
void FGlobalRandoms::Tick( FLOAT TimeSeconds )
{
	guard(FGlobalRandoms::Tick);
	FAppRandTracePhaseScope StartupPhaseTrace( "pre_render_fglobalrandoms" );
	hp2_rs_render_random_tick( GRenderServices, TimeSeconds, RandomBases, Randoms, RandomDeltas, &LastTicks );
	unguard;
}

/*------------------------------------------------------------------------------------
	Random subsystem instantiation
------------------------------------------------------------------------------------*/

static FGlobalRandoms GRandomsInstance;
FGlobalRandomsBase* GRandoms = &GRandomsInstance;

/*------------------------------------------------------------------------------------
	The End
------------------------------------------------------------------------------------*/

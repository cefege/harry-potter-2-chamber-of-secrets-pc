//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999
//
//  File:       UnParticle.cpp
//
//  Contents:   Implementation of UnParticle.cpp
//
//  History:    23-June-99   PKeet  Created
//
//	To do:		
//				Add bLanded and disable anymore collision checks
//
//---------------------------------------------------------------------------

#include "EnginePrivate.h"
#include "UnParticle.h"
#include "Hp2ParticleServices.h"
#include "hp2_rs.h"

/*-----------------------------------------------------------------------------
	UParticle object implementation.
-----------------------------------------------------------------------------*/

UParticle::UParticle(const FVector& Pos, const FVector& Vel, const AParticleFX* System)
	:	LastPosition(Pos), Position(Pos), Velocity(Vel), Age(0.0f), Id(-1) 
{
}


bool UParticle::Update( const FVector& SystemGravity, const FLOAT SystemDamping, const FVector& SystemWind, ULevel* Level, FLOAT Timestep, AParticleFX* System)
{
	return hp2_rs_engine_particle_update( this, &SystemGravity, SystemDamping, &SystemWind, Level, Timestep, System, &GHp2ParticleServices ) != 0;
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

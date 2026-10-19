//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999.
//
//  File:       UnParticleFX.cpp
//
//  Contents:   Implementation of AParticleFX
//
//  History:    26-April-99   PKeet  Created
//
//	To do:		
//				Put checks on AlphaDelay, ColorDelay and SizeDelay
//
//---------------------------------------------------------------------------

#include "EnginePrivate.h"
#include "UnParticle.h"
#include "UnParticleList.h"
#include "UnMesh.h"
#include "UnRender.h"
#include "UnStat.h"
#include "Hp2ParticleFxServices.h"
#include "hp2_rs.h"

/*-----------------------------------------------------------------------------
	AParticleFX implementation.

	The particle logic lives in crates/hp2-engine/src/particle_fx.rs and is
	reached through hp2_rs_engine_particle_fx_*. This file keeps the UObject
	shell, the render-trace scopes, and the script exec thunks.
-----------------------------------------------------------------------------*/

AParticleFX::AParticleFX()
{
	ParticleList = NULL;
	
	CreateParticleList();

	Age	= 0.0f;
	ElapsedTime = 0.0f;
	bUpdate = true;
	LOD = 1.0f;
}

//----------------------------------------------------------------------------

void AParticleFX::InitExecution()
{
	Super::InitExecution();
	hp2_rs_engine_particle_fx_init_execution( this, &GHp2ParticleFxServices );
}

//----------------------------------------------------------------------------

void AParticleFX::PostEditChange()
{
}

//----------------------------------------------------------------------------

UPrimitive* AParticleFX::GetPrimitive() const
{
	return Super::GetPrimitive();
}

//----------------------------------------------------------------------------

FCoords AParticleFX::GetRenderBoundingBox( UBOOL Exact )
{
	guard(AParticleFX::GetRenderBoundingBox);
	FCoords Out(0.0f);
	hp2_rs_engine_particle_fx_get_render_bounding_box( this, &Out, &GHp2ParticleFxServices );
	return Out;
	unguard;
}

//----------------------------------------------------------------------------

void AParticleFX::CreateParticleList()
{
	guard(AParticleFX::CreateParticleList);
	check(!ParticleList);
	check(DrawType == DT_Particles);

	if (bDeleteMe)
		return;
	ParticleList = UParticleList::Create(this);
	check(ParticleList);
	unguard;
}

//----------------------------------------------------------------------------

void AParticleFX::Destroy()
{
	guard(AParticleFX::Destroy);

	// Kill the particle list.
	delete ParticleList;
	ParticleList = 0;
	Super::Destroy();
	unguard;
}

//----------------------------------------------------------------------------

void AParticleFX::UpdateParticles(FLOAT Tick)
{
	hp2_rs_engine_particle_fx_update_particles( this, Tick, &GHp2ParticleFxServices );
}

//----------------------------------------------------------------------------

int AParticleFX::EmitParticles(FLOAT Tick)
{
	guard(AParticleFX::EmitParticles);
	FAppRandTraceNativeObjectScope RandTrace( this, "Render/ParticleFX", INDEX_NONE, 1 );
	Clock(GStat.ParticleEmitTime);
	return hp2_rs_engine_particle_fx_emit_particles( this, Tick, &GHp2ParticleFxServices );
	unguard;
}

//----------------------------------------------------------------------------

FLOAT AParticleFX::MaxLifetime()
{
	return hp2_rs_engine_particle_fx_max_lifetime( this );
}

//----------------------------------------------------------------------------

UBOOL AParticleFX::Tick( FLOAT DeltaSeconds, ELevelTick TickType )
{
	hp2_rs_engine_particle_fx_tick( this, DeltaSeconds, TickType, &GHp2ParticleFxServices );
	return Super::Tick( DeltaSeconds, TickType );
}

//----------------------------------------------------------------------------

bool AParticleFX::Update(FLOAT Step)
{
	guard(AParticleFX::Update);
	FAppRandTraceNativeObjectScope RandTrace( this, "Render/ParticleFX", INDEX_NONE, 0 );
	return hp2_rs_engine_particle_fx_update( this, Step, &GHp2ParticleFxServices ) != 0;
	unguard;
}

//----------------------------------------------------------------------------

const AParticleFX* AParticleFX::GetSysParams(char* Buffer) const
{
	return (const AParticleFX*)hp2_rs_engine_particle_fx_get_sys_params( const_cast<AParticleFX*>(this), Buffer, &GHp2ParticleFxServices );
}

//----------------------------------------------------------------------------

void AParticleFX::GetParams(FParams& Params) const
{
	hp2_rs_engine_particle_fx_get_params( this, &Params, &GHp2ParticleFxServices );
}

//----------------------------------------------------------------------------

bool AParticleFX::AddParticle( int Id, FVector& Position, FParams* Params )
{
	return hp2_rs_engine_particle_fx_add_particle( this, Id, &Position, Params, &GHp2ParticleFxServices ) != 0;
}

//----------------------------------------------------------------------------

int AParticleFX::RecomputeDeltas(INT Id)
{
	return hp2_rs_engine_particle_fx_recompute_deltas( this, Id, &GHp2ParticleFxServices );
}

//----------------------------------------------------------------------------

FLOAT AParticleFX::Lod( float ScreenFrac )
{
	return hp2_rs_engine_particle_fx_lod( this, ScreenFrac, &GHp2ParticleFxServices );
}

//----------------------------------------------------------------------------

FLOAT AParticleFX::LodParticles( float ScreenFrac )
{
	return hp2_rs_engine_particle_fx_lod_particles( this, ScreenFrac, &GHp2ParticleFxServices );
}

//----------------------------------------------------------------------------

FLOAT AParticleFX::LodParticleDensity( FSceneNode* Frame, float ScreenFrac )
{
	return hp2_rs_engine_particle_fx_lod_particle_density( this, Frame, ScreenFrac, &GHp2ParticleFxServices );
}

//----------------------------------------------------------------------------

FLOAT AParticleFX::ParticleLOD = 1.0f;

//----------------------------------------------------------------------------

void AParticleFX::execNumParticles( FFrame& Stack, RESULT_DECL )
{
	guard(AParticleFX::execNumParticles);
	P_FINISH;

	*(INT*)Result = hp2_rs_engine_particle_fx_num_particles( this, &GHp2ParticleFxServices );
	unguardexec;
}

//----------------------------------------------------------------------------

void AParticleFX::execAddParticle( FFrame& Stack, RESULT_DECL )
{
	guard(AParticleFX::execAddParticle);
	P_GET_INT(Id);
	P_GET_VECTOR(Loc);
	P_FINISH;

	hp2_rs_engine_particle_fx_exec_add_particle( this, Id, &Loc, &GHp2ParticleFxServices );
	unguardexec;
}

//----------------------------------------------------------------------------

void AParticleFX::execGetParticleParams( FFrame& Stack, RESULT_DECL )
{
	guard(AParticleFX::execGetParticleParams);
	P_GET_INT(Id);
	P_GET_STRUCT_REF(FParticleParams,Params)
	P_FINISH;

	*(bool*)Result = hp2_rs_engine_particle_fx_get_particle_params( this, Id, Params, &GHp2ParticleFxServices ) != 0;
	unguardexec;
}

//----------------------------------------------------------------------------

void AParticleFX::execSetParticleParams( FFrame& Stack, RESULT_DECL )
{
	guard(AParticleFX::execSetParticleParams);
	P_GET_INT(Id);
	P_GET_STRUCT_REF(FParticleParams,Params)
	P_FINISH;

	*(bool*)Result = hp2_rs_engine_particle_fx_set_particle_params( this, Id, Params, &GHp2ParticleFxServices ) != 0;
	unguardexec;
}

//----------------------------------------------------------------------------

void AParticleFX::execRecomputeDeltas( FFrame& Stack, RESULT_DECL )
{
	P_GET_INT(Id);
	P_FINISH;

	*(bool*)Result = hp2_rs_engine_particle_fx_recompute_deltas( this, Id, &GHp2ParticleFxServices ) != 0;
}



IMPLEMENT_CLASS(AParticleFX);


/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

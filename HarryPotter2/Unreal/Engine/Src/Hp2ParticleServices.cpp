/*=============================================================================
	Hp2ParticleServices.cpp: Thunks the Rust particle port calls back into.
	Each thunk performs one of the original engine operations unchanged.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2ParticleServices.h"
#include "hp2_rs.h"

static float ParticleAppFrand()
{
	return appFrand();
}

static float ParticleSinTab( int I )
{
	return GMath.SinTab( I );
}

static float ParticleCosTab( int I )
{
	return GMath.CosTab( I );
}

static void ParticleTotalWind( void* Level, const FVector* Loc, FVector* Out )
{
	*Out = AWind::GetTotalWind( (ULevel*)Level, *Loc );
}

static int ParticleFastLineCheck( void* Level, const FVector* End, const FVector* Start )
{
	return ((ULevel*)Level)->Model->FastLineCheck( *End, *Start ) ? 1 : 0;
}

static int ParticleSystemTraceLevelHit( void* System, const FVector* End, const FVector* Start, FVector* Normal, FVector* Location )
{
	AParticleFX* Sys = (AParticleFX*)System;
	FCheckResult Hit(1.f);
	Sys->GetLevel()->SingleLineCheck( Hit, NULL, *End, *Start, TRACE_VisBlocking );

	if( Hit.Actor != NULL && Hit.Actor->IsA(ALevelInfo::StaticClass()) )
	{
		*Normal = Hit.Normal;
		*Location = Hit.Location;
		return 1;
	}
	return 0;
}

// Verbatim from UParticle::Update (UnParticle.cpp:153-162). A null Palette leaves Out untouched.
static void ParticlePalettePlane( void* ColorPalette, float Age, float Lifetime, float ColorDelay, FPlane* Out )
{
	UTexture* Tex = (UTexture*)ColorPalette;
	if( Tex->Palette )
	{
		if( ( Lifetime > 0.0f ) && ( Age > ColorDelay ) )
			*Out = Tex->Palette->Colors( (INT)((Age / Lifetime)*255.0f) ).Plane();
		else
			*Out = Tex->Palette->Colors(0).Plane();
	}
}

const Hp2ParticleServices GHp2ParticleServices =
{
	ParticleAppFrand,
	ParticleSinTab,
	ParticleCosTab,
	ParticleTotalWind,
	ParticleFastLineCheck,
	ParticleSystemTraceLevelHit,
	ParticlePalettePlane,
};

/*=============================================================================
	UnCamMgr.cpp: Unreal viewport manager, generic implementation.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "hp2_rs.h"
#include "Hp2CamMgrServices.h"

/*-----------------------------------------------------------------------------
	UClient implementation.
-----------------------------------------------------------------------------*/

//
// Constructor.
//
UClient::UClient()
{
	guard(UClient::UClient);

	// Hook in.
	UBitmap::__Client = this;//!!why?

	unguard;
}
void UClient::PostEditChange()
{
	guard(UClient::PostEditChange);
	Super::PostEditChange();
	hp2_rs_engine_cam_mgr_post_edit_change( &Brightness, &MipFactor, &ParticleDensity );
	unguard;
}
FLOAT UClient::GetEffectiveFovAngle( FLOAT AuthoredHFov, INT ViewportX, INT ViewportY, UBOOL bMaintainVerticalFOV )
{
	return hp2_rs_engine_cam_mgr_get_effective_fov_angle( AuthoredHFov, ViewportX, ViewportY, bMaintainVerticalFOV );
}
void UClient::StaticConstructor()
{
	guard(UClient::StaticConstructor);

	UEnum* Details=new(GetClass(),TEXT("Details"))UEnum( NULL );
	new(Details->Names)FName( TEXT("High")     );
	new(Details->Names)FName( TEXT("Medium")   );
	new(Details->Names)FName( TEXT("Low")      );
	new(Details->Names)FName( TEXT("Ultra Low"));

	new(GetClass(),TEXT("WindowedViewportX"),	RF_Public)UIntProperty  (CPP_PROPERTY(WindowedViewportX     ), TEXT("Client"),  CPF_Config );
	new(GetClass(),TEXT("WindowedViewportY"),	RF_Public)UIntProperty  (CPP_PROPERTY(WindowedViewportY     ), TEXT("Client"),  CPF_Config );
	new(GetClass(),TEXT("WindowedColorBits"),	RF_Public)UIntProperty  (CPP_PROPERTY(WindowedColorBits     ), TEXT("Client"),  CPF_Config );
	new(GetClass(),TEXT("FullscreenViewportX"),	RF_Public)UIntProperty  (CPP_PROPERTY(FullscreenViewportX   ), TEXT("Client"),  CPF_Config );
	new(GetClass(),TEXT("FullscreenViewportY"),	RF_Public)UIntProperty  (CPP_PROPERTY(FullscreenViewportY	), TEXT("Client"),  CPF_Config );
	new(GetClass(),TEXT("FullscreenColorBits"),	RF_Public)UIntProperty  (CPP_PROPERTY(FullscreenColorBits	), TEXT("Client"),  CPF_Config );
	new(GetClass(),TEXT("MipFactor"),			RF_Public)UFloatProperty(CPP_PROPERTY(MipFactor				), TEXT("Client"),  CPF_Config );
	new(GetClass(),TEXT("Brightness"),			RF_Public)UFloatProperty(CPP_PROPERTY(Brightness			), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("CaptureMouse"),		RF_Public)UBoolProperty (CPP_PROPERTY(CaptureMouse			), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("CurvedSurfaces"),		RF_Public)UBoolProperty (CPP_PROPERTY(CurvedSurfaces		), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("TextureDetail"),		RF_Public)UByteProperty (CPP_PROPERTY(TextureLODSet[1]		), TEXT("Display"), CPF_Config, Details );
	new(GetClass(),TEXT("SkinDetail"),			RF_Public)UByteProperty (CPP_PROPERTY(TextureLODSet[2]		), TEXT("Display"), CPF_Config, Details );
	new(GetClass(),TEXT("ScreenFlashes"),		RF_Public)UBoolProperty (CPP_PROPERTY(ScreenFlashes			), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("MaintainVerticalFOV"),	RF_Public)UBoolProperty (CPP_PROPERTY(MaintainVerticalFOV	), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("NativeText"),			RF_Public)UBoolProperty (CPP_PROPERTY(NativeText				), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("NoLighting"),			RF_Public)UBoolProperty (CPP_PROPERTY(NoLighting			), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("MinDesiredFrameRate"),	RF_Public)UFloatProperty(CPP_PROPERTY(MinDesiredFrameRate	), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("Decals"),				RF_Public)UBoolProperty (CPP_PROPERTY(Decals				), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("NoDynamicLights"),		RF_Public)UBoolProperty (CPP_PROPERTY(NoDynamicLights		), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("NoFractalAnim"),		RF_Public)UBoolProperty (CPP_PROPERTY(NoFractalAnim 		), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("ShowFPS"),				RF_Public)UBoolProperty (CPP_PROPERTY(ShowFPS				), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("ParticleDensity"),		RF_Public)UIntProperty  (CPP_PROPERTY(ParticleDensity		), TEXT("Display"), CPF_Config );
	MaintainVerticalFOV = 1;
	// Native text ships on by default. This is a product decision, not a
	// statement that the gate has reached default-enabled: the gate is still
	// experimental because the wasm provider has not been reviewed by eye, and
	// Build/feature-gates.json records that per provider. The 2002 bitmap
	// fallback remains the compatibility path and is still reachable with
	// [Display] NativeText=False in Game.ini.
	NativeText = 1;
	ShowFPS = 0;

	unguard;
}
void UClient::Destroy()
{
	guard(UClient::Destroy);
	UBitmap::__Client = NULL;
	Super::Destroy();
	unguard;
}

//
// Command line.
//
UBOOL UClient::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UClient::Exec);
	return hp2_rs_engine_cam_mgr_exec( &GHp2CamMgrServices, this, &Brightness, Cmd );
	unguard;
}

//
// Init.
//
void UClient::Init( UEngine* InEngine )
{
	guard(UClient::Init);
	Engine = InEngine;
	unguard;
}

//
// Flush.
//
void UClient::Flush( UBOOL AllowPrecache )
{
	guard(UClient::Flush);

	hp2_rs_engine_cam_mgr_flush( &GHp2CamMgrServices, this, AllowPrecache );
	unguard;
}

//
// Serializer.
//
void UClient::Serialize( FArchive& Ar )
{
	guard(UClient::Serialize);
	Super::Serialize( Ar );

	// Only serialize objects, since this can't be loaded or saved.
	Ar << Viewports;

	unguard;
}

IMPLEMENT_CLASS(UClient);

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

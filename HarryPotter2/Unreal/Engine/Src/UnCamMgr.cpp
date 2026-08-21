/*=============================================================================
	UnCamMgr.cpp: Unreal viewport manager, generic implementation.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"

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
	Brightness = Clamp(Brightness,0.f,1.f);
	MipFactor = Clamp(MipFactor,-3.f,3.f);
	ParticleDensity = Max(ParticleDensity, 0);
	unguard;
}
FLOAT UClient::GetEffectiveFovAngle( FLOAT AuthoredHFov, INT ViewportX, INT ViewportY, UBOOL bMaintainVerticalFOV )
{
	if( !bMaintainVerticalFOV || ViewportX<=0 || ViewportY<=0 || 3.0*ViewportX<=4.0*ViewportY )
		return AuthoredHFov;

	const DOUBLE AspectScale = (3.0*ViewportX) / (4.0*ViewportY);
	const FLOAT EffectiveHFov = (FLOAT)(2.0 * appAtan(appTan(AuthoredHFov * PI/360.0) * AspectScale) * 180.0/PI);
	return Min( EffectiveHFov, 169.999f );
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
	new(GetClass(),TEXT("NoLighting"),			RF_Public)UBoolProperty (CPP_PROPERTY(NoLighting			), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("MinDesiredFrameRate"),	RF_Public)UFloatProperty(CPP_PROPERTY(MinDesiredFrameRate	), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("Decals"),				RF_Public)UBoolProperty (CPP_PROPERTY(Decals				), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("NoDynamicLights"),		RF_Public)UBoolProperty (CPP_PROPERTY(NoDynamicLights		), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("NoFractalAnim"),		RF_Public)UBoolProperty (CPP_PROPERTY(NoFractalAnim 		), TEXT("Display"), CPF_Config );
	new(GetClass(),TEXT("ParticleDensity"),		RF_Public)UIntProperty  (CPP_PROPERTY(ParticleDensity		), TEXT("Display"), CPF_Config );
	MaintainVerticalFOV = 1;

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
	if( ParseCommand(&Cmd,TEXT("BRIGHTNESS")) )
	{
		if( (Brightness+=0.1f) >= 1.f )
			Brightness = 0;
		Engine->Flush(1);
		SaveConfig();
		if( Viewports.Num() && Viewports(0)->Actor )//!!ugly
			Viewports(0)->Actor->eventClientMessage( *FString::Printf(TEXT("Brightness level %i/10"), appRound(Brightness*10+1)), NAME_None, 0 );
		return 1;
	}
	else return 0;
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

	for( INT i=0; i<Viewports.Num(); i++ )
		if( Viewports(i)->RenDev )
			Viewports(i)->RenDev->Flush(AllowPrecache);

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

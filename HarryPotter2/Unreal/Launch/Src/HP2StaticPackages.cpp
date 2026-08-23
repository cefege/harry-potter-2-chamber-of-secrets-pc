/*=============================================================================
	HP2StaticPackages.cpp: Ordered static package bootstrap for HP2.
=============================================================================*/

#include "Engine.h"
#include "UnMesh.h"
#include "UnNet.h"
#include "UnCon.h"
#include "UnParticleList.h"
#include "UnSkeletalMesh.h"
#include "UnEngineNative.h"
#include "Render.h"
#include "UnRenderNative.h"
#include "UnFractal.h"
#if defined(HP2_WITH_CLIENT_PACKAGES) && HP2_WITH_CLIENT_PACKAGES
#include "SDLDrv.h"
#include "XOpenGLDrv.h"
#include "ALAudio.h"
#endif
#include "HP2StaticPackages.h"

void InstallHP2NativeLookups()
{
	static const NativeLookup Lookups[] =
	{
		&FindCoreUObjectNative,
		&FindCoreUCommandletNative,
		&FindEngineAActorNative,
		&FindEngineAPawnNative,
		&FindEngineAPlayerPawnNative,
		&FindEngineADecalNative,
		&FindEngineAStatLogNative,
		&FindEngineAStatLogFileNative,
		&FindEngineAZoneInfoNative,
		&FindEngineAWarpZoneInfoNative,
		&FindEngineALevelInfoNative,
		&FindEngineAGameInfoNative,
		&FindEngineANavigationPointNative,
		&FindEngineUCanvasNative,
		&FindEngineUConsoleNative,
		&FindEngineUScriptedTextureNative
	};
	enum { ExpectedLookupCount = 2 + 14 };
	static_assert(ARRAY_COUNT(Lookups) == ExpectedLookupCount, "HP2 native lookup count changed");
	static_assert(ARRAY_COUNT(Lookups) < ARRAY_COUNT(GNativeLookupFuncs), "HP2 native lookups must remain zero-terminated");

	for (INT Index = 0; Index < ARRAY_COUNT(GNativeLookupFuncs); ++Index)
		GNativeLookupFuncs[Index] = NULL;

	INT Lookup = 0;
	for (INT Index = 0; Index < ARRAY_COUNT(Lookups); ++Index)
	{
		if (Lookup >= ARRAY_COUNT(GNativeLookupFuncs))
			appErrorf(TEXT("HP2 native lookup table overflow"));
		GNativeLookupFuncs[Lookup++] = Lookups[Index];
	}

	if (Lookup != ExpectedLookupCount)
		appErrorf(TEXT("HP2 native lookup count mismatch: %i"), Lookup);
	if (Lookup >= ARRAY_COUNT(GNativeLookupFuncs) || GNativeLookupFuncs[Lookup] != NULL)
		appErrorf(TEXT("HP2 native lookup table is not zero-terminated"));
}

void RegisterHP2RuntimeClasses()
{
	AUTO_INITIALIZE_REGISTRANTS_ENGINE;
	AUTO_INITIALIZE_REGISTRANTS_RENDER;
	AUTO_INITIALIZE_REGISTRANTS_FIRE;
	RegisterEditorTransactionClasses();
}

#if defined(HP2_WITH_CLIENT_PACKAGES) && HP2_WITH_CLIENT_PACKAGES
#if defined(HP2_ENABLE_VULKAN_DRIVER) && HP2_ENABLE_VULKAN_DRIVER
extern "C" void autoInitializeRegistrantsVulkanDrv(void);
#endif

	void RegisterHP2ClientClasses()
	{
		AUTO_INITIALIZE_REGISTRANTS_SDLDRV;
		AUTO_INITIALIZE_REGISTRANTS_XOPENGLDRV;
		AUTO_INITIALIZE_REGISTRANTS_ALAUDIO;
#if defined(HP2_ENABLE_VULKAN_DRIVER) && HP2_ENABLE_VULKAN_DRIVER
		autoInitializeRegistrantsVulkanDrv();
#endif
	}
#endif

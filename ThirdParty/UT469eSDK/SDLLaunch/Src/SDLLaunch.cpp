/*=============================================================================
	Launch.cpp: Game launcher.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Daniel Vogel (based on XLaunch).

=============================================================================*/

// System includes
#include "SDLLaunchPrivate.h"

#ifndef FORCE_XOPENGLDRV
#define FORCE_XOPENGLDRV 1
#endif

/*-----------------------------------------------------------------------------
	Global variables.
-----------------------------------------------------------------------------*/

TCHAR GPackageInternal[64]=TEXT("SDLLaunch");
extern "C" {const TCHAR* GPackage=GPackageInternal;}

// Log file.
#include "FOutputDeviceFile.h"
FOutputDeviceFile Log;

// Error handler.
#include "FOutputDeviceSDLError.h"
FOutputDeviceSDLError Error;

// Feedback.
#include "FFeedbackContextSDL.h"
FFeedbackContextSDL Warn;

// File manager.
#include "FFileManagerLinux.h"
FFileManagerLinux FileManager;

// Config.
#undef _INC_EDITOR
#include "FConfigCacheIni.h"

#if __STATIC_LINK

// Extra stuff for static links for Engine.
#include "UnEngineNative.h"
#include "UnCon.h"
#include "UnRender.h"
#include "UnNet.h"

// Fire static stuff.
#include "UnFractal.h"

// IpDrv static stuff.
#include "UnIpDrv.h"
#include "UnTcpNetDriver.h"
#include "UnIpDrvCommandlets.h"
#include "UnIpDrvNative.h"
#include "HTTPDownload.h"

// UWeb static stuff.
#include "UWeb.h"
#include "UWebNative.h"

// SDLDrv static stuff.
#include "SDLDrv.h"

// Render static stuff.
#include "Render.h"
#include "UnRenderNative.h"

#include "ALAudio.h"
#include "Cluster.h"

#include "udemoprivate.h"

#if defined(__EMSCRIPTEN__)
    #if FORCE_XOPENGLDRV
        #include "XOpenGLDrv.h"
    #else
        #include "../../UTGLROpenGLDrv/Inc/OpenGLDrv.h"
        #include "../../UTGLROpenGLDrv/Inc/UTGLROpenGL.h"
    #endif
#else
    #include "XOpenGLDrv.h"
    #include "../../UTGLROpenGLDrv/Inc/OpenGLDrv.h"
    #include "../../UTGLROpenGLDrv/Inc/UTGLROpenGL.h"
#endif

#if MACOSX
# include "FruCoRe.h"
#endif

#include "EditorPrivate.h"
#include "UnEditorNative.h"

//#include "NullRender.h"

#endif

// SDL
#include <SDL2/SDL.h>

#ifdef __EMSCRIPTEN__
#include <emscripten.h>
#endif

#ifdef MACOSX
#include <mach-o/dyld.h>
#endif

//SplashScreen
SDL_Renderer*	SplashRenderer = NULL;
SDL_Surface*    SplashImage    = NULL;
SDL_Texture*    SplashTexture  = NULL;
SDL_Window*		SplashWindow   = NULL;

static void InitSplash(const TCHAR *fname)
{
	guard(InitSplash);

	SplashImage = SDL_LoadBMP(appToAnsi(fname));
	if (SplashImage == NULL)
	{
		SDL_ShowSimpleMessageBox(0, "SplashImage init error", SDL_GetError(), SplashWindow);
		return;
	}


	TCHAR WindowName[256];
	// Set viewport window's name to show resolution.
	appSnprintf( WindowName, ARRAY_COUNT(WindowName), TEXT("Unreal Tournament is starting...") );
	SplashWindow = SDL_CreateWindow(appToAnsi(WindowName),
                          SDL_WINDOWPOS_CENTERED,
                          SDL_WINDOWPOS_CENTERED,
                          SplashImage->w, SplashImage->h,
                          SDL_WINDOW_BORDERLESS);

	if (SplashWindow == NULL)
	{
        SDL_ShowSimpleMessageBox(0, "SplashWindow init error", SDL_GetError(), SplashWindow);
		return;
	}

	SplashRenderer = SDL_CreateRenderer(SplashWindow, -1, 0);
	if (SplashRenderer == NULL)
	{
		SDL_ShowSimpleMessageBox(0, "SplashRenderer init error", SDL_GetError(), SplashWindow);
		return;
	}

	SplashTexture = SDL_CreateTextureFromSurface(SplashRenderer, SplashImage);
	if (SplashTexture == NULL)
	{
		SDL_ShowSimpleMessageBox(0, "SplashTexture init error", SDL_GetError(), SplashWindow);
		return;
	}
    SDL_RenderClear(SplashRenderer);
    SDL_RenderCopy(SplashRenderer, SplashTexture, NULL, NULL);
    SDL_RenderPresent(SplashRenderer);

    unguard;
}

static void ExitSplash()
{
	guard(ExitSplash);

	// stijn: destroying the texture or renderer causes other SDL windows to render only black pixels
	// idk why...

//	if (SplashTexture)
//		SDL_DestroyTexture(SplashTexture);
	if (SplashImage)
        SDL_FreeSurface(SplashImage);
//	if (SplashRenderer)
//        SDL_DestroyRenderer(SplashRenderer);
	if (SplashWindow)
		SDL_DestroyWindow(SplashWindow);

    unguard;
}

static inline void FixIni()
{
	const TCHAR* IniVideoRendererKeys[] =
	{
		TEXT("RenderDevice"),
		TEXT("GameRenderDevice"),
		TEXT("WindowRenderDevice")
	};
	
	const TCHAR* LegacyOrWindowsVideoRenderers[] =
	{
        TEXT("D3DDrv.D3DRenderDevice"),
		TEXT("D3D8Drv.D3D8RenderDevice"),
		TEXT("D3D9Drv.D3D9RenderDevice"),
		TEXT("D3D10Drv.D3D10RenderDevice"),
		TEXT("D3D11Drv.D3D11RenderDevice"),
		TEXT("GlideDrv.GlideRenderDevice"),
		TEXT("SDLGLDrv.SDLGLRenderDevice"),
		TEXT("SoftDrv.SoftwareRenderDevice"),
		TEXT("SDLSoftDrv.SDLSoftwareRenderDevice")
	};

	const TCHAR* LegacyOrWindowsAudioRenderers[] =
	{
		TEXT("Audio.GenericAudioSubsystem"),
		TEXT("Galaxy.GalaxyAudioSubsystem"),
	};
	
    // A default config? Force it from WinDrv to SDLDrv... --ryan.
    //  Also clean up legacy Loki interfaces...
	if( !ParseParam(appCmdLine(),TEXT("NoForceSDLDrv")) )
    {
#ifdef __EMSCRIPTEN__
# if FORCE_XOPENGLDRV
		GConfig->SetString(TEXT("Engine.Engine"), TEXT("GameRenderDevice"), TEXT("XOpenGLDrv.XOpenGLRenderDevice"));
		GConfig->SetString(TEXT("Engine.Engine"), TEXT("WindowRenderDevice"), TEXT("XOpenGLDrv.XOpenGLRenderDevice"));
		GConfig->SetString(TEXT("Engine.Engine"), TEXT("RenderDevice"), TEXT("XOpenGLDrv.XOpenGLRenderDevice"));
# else
		GConfig->SetString(TEXT("Engine.Engine"), TEXT("GameRenderDevice"), TEXT("OpenGLDrv.OpenGLRenderDevice"));
		GConfig->SetString(TEXT("Engine.Engine"), TEXT("WindowRenderDevice"), TEXT("OpenGLDrv.OpenGLRenderDevice"));
		GConfig->SetString(TEXT("Engine.Engine"), TEXT("RenderDevice"), TEXT("OpenGLDrv.OpenGLRenderDevice"));
# endif
#else

	    if( appStricmp(GConfig->GetStr(TEXT("Engine.Engine"),TEXT("ViewportManager"),TEXT("System")),TEXT("WinDrv.WindowsClient"))==0 )
        {
            debugf(TEXT("Your ini had WinDrv...Forcing use of SDLDrv instead."));
			GConfig->SetString(TEXT("Engine.Engine"), TEXT("ViewportManager"), TEXT("SDLDrv.SDLClient"));
        }

		for (INT j = 0; j < ARRAY_COUNT(IniVideoRendererKeys); ++j)
		{
			for (INT i = 0; i < ARRAY_COUNT(LegacyOrWindowsVideoRenderers); ++i)
			{
				if (appStricmp(GConfig->GetStr(TEXT("Engine.Engine"), IniVideoRendererKeys[j], TEXT("System")), LegacyOrWindowsVideoRenderers[i]) == 0)
				{
					debugf(TEXT("Engine.Engine.%s was %s... Forcing use of OpenGLDrv instead."),
						   IniVideoRendererKeys[j],
						   GConfig->GetStr(TEXT("Engine.Engine"), IniVideoRendererKeys[j], TEXT("System")));
					GConfig->SetString(TEXT("Engine.Engine"), IniVideoRendererKeys[j], TEXT("OpenGLDrv.OpenGLRenderDevice"));
					break;
				}
			}
		}
#endif
    }

	if( !ParseParam(appCmdLine(),TEXT("NoForceALAudio")) )
    {
		for (INT i = 0; i < ARRAY_COUNT(LegacyOrWindowsAudioRenderers); ++i)
		{
			if (appStricmp(GConfig->GetStr(TEXT("Engine.Engine"), TEXT("AudioDevice"), TEXT("System")), LegacyOrWindowsAudioRenderers[i]) == 0)
			{
				debugf(TEXT("Engine.Engine.AudioDevice was %s... Forcing use of ALAudio instead."),
					   GConfig->GetStr(TEXT("Engine.Engine"), TEXT("AudioDevice"), TEXT("System")));
				GConfig->SetString(TEXT("Engine.Engine"), TEXT("AudioDevice"), TEXT("ALAudio.ALAudioSubsystem"));
				break;
			}
		}
    }
}


//
// Creates a UEngine object.
//
static UEngine* InitEngine()
{
	guard(InitEngine);
	DOUBLE LoadTime = appSecondsNew();

	// Set exec hook.
	GExec = NULL;

    FixIni();  // check for legacy and Windows-specific INI entries...

	// Create the global engine object.
	UClass* EngineClass;
	EngineClass = UObject::StaticLoadClass(
		UGameEngine::StaticClass(), NULL, 
		TEXT("ini:Engine.Engine.GameEngine"), 
		NULL, LOAD_NoFail, NULL 
	);
	UEngine* Engine = ConstructObject<UEngine>( EngineClass );
	Engine->Init();

	debugf( TEXT("Startup time: %f seconds."), appSecondsNew()-LoadTime );

	return Engine;
	unguard;
}

/*-----------------------------------------------------------------------------
	Main Loop
-----------------------------------------------------------------------------*/

//
// Exit wound.
// 
int CleanUpOnExit(int ErrorLevel)
{
	// Clean up our mess.
	GIsRunning = 0;
	/*
	if( GDisplay )
		XCloseDisplay(GDisplay);
	*/
	GFileManager->Delete(TEXT("Running.ini"),0,0);
	debugf( NAME_Title, LocalizeGeneral(TEXT("Exit")) );

	appPreExit();
	GIsGuarded = 0;

	// Shutdown.
	appExit();
	GIsStarted = 0;

	// Better safe than sorry.
	SDL_Quit( );

	#ifdef __EMSCRIPTEN__
	emscripten_cancel_main_loop();  // this should "kill" the app.
	emscripten_force_exit(ErrorLevel);  // this should "kill" the app.
    #endif

	_exit(ErrorLevel);
}

// just in case.  :)  --ryan.
static void sdl_atexit_handler(void)
{
    static bool already_called = false;

    if (!already_called)
    {
        already_called = true;
        SDL_Quit();
    }
}

struct MainLoopArgs
{
    DOUBLE OldTime;
    DOUBLE SecondStartTime;
    INT TickCount;
    UEngine *Engine;
};

static bool MainLoopIteration(MainLoopArgs *args)
{
	guard(MainLoopIteration);

	if ( !GIsRunning || GIsRequestingExit )
	{
		GIsRunning = 0;
		return false;
	}

    try 
	{
		DOUBLE &OldTime = args->OldTime;
		DOUBLE &SecondStartTime = args->SecondStartTime;
		INT &TickCount = args->TickCount;
		UEngine *Engine = args->Engine;

		// Update the world.
		guard(UpdateWorld);
		DOUBLE NewTime = appSecondsNew();
		FLOAT  DeltaTime = NewTime - OldTime;
		Engine->Tick( DeltaTime );
		if( GWindowManager )
			GWindowManager->Tick( DeltaTime );
		OldTime = NewTime;
		TickCount++;
		if( OldTime > SecondStartTime + 1 )
		{
			Engine->CurrentTickRate = (FLOAT)TickCount / (OldTime - SecondStartTime);
			SecondStartTime = OldTime;
			TickCount = 0;
		}
		unguard;
	}
	catch (...)
	{
		raise(SIGUSR1);
	}

    return true;
	unguard;
}

#ifdef __EMSCRIPTEN__
static void EmscriptenMainLoopIteration(void *args)
{
	if (!MainLoopIteration((MainLoopArgs *) args))
	{
		CleanUpOnExit(0);
		emscripten_cancel_main_loop();  // this should "kill" the app.
	}
}
#endif

//
// game message loop.
//
static void MainLoop( UEngine* Engine )
{
	guard(MainLoop);
	check(Engine);

	// Loop while running.
	GIsRunning = 1;
    MainLoopArgs *args = new MainLoopArgs;
    args->OldTime = appSecondsNew();
    args->SecondStartTime = args->OldTime;
    args->TickCount = 0;
    args->Engine = Engine;

	#ifdef __EMSCRIPTEN__
	emscripten_set_main_loop_arg(EmscriptenMainLoopIteration, args, 0, 1);
	#else
	while (MainLoopIteration(args))
	{
		// Enforce optional maximum tick rate.
		guard(EnforceTickRate);
		const FLOAT MaxTickRate = args->Engine->GetMaxTickRate();
		if( MaxTickRate>0.0 )
		{
			DOUBLE Delta = (1.0/MaxTickRate) - (appSecondsNew()-args->OldTime);
			appSleepLong( Max(0.0,Delta) );
		}
		unguard;
	}
	delete args;
	CleanUpOnExit(0);
	#endif

	unguard;
}

/*-----------------------------------------------------------------------------
	Main.
-----------------------------------------------------------------------------*/

//
// Simple copy.
// 

void SimpleCopy(TCHAR* fromfile, TCHAR* tofile)
{
	INT c;
	FILE* from;
	FILE* to;
	from = fopen(TCHAR_TO_ANSI(fromfile), "r");
	if (from == NULL)
		return;
	to = fopen(TCHAR_TO_ANSI(tofile), "w");
	if (to == NULL)
	{
		printf("Can't open or create %s", TCHAR_TO_ANSI(tofile));
		return;
	}
	while ((c = getc(from)) != EOF)
		putc(c, to);
	fclose(from);
	fclose(to);
}

//
// Entry point.
//
int main( int argc, char* argv[] )
{
	if (!appChdirSystem())
		wprintf(TEXT("WARNING: Could not chdir into the game's System folder\n"));
	
#if __STATIC_LINK
	// Clean lookups.
	for (INT k=0; k<ARRAY_COUNT(GNativeLookupFuncs); k++)
	{
		GNativeLookupFuncs[k] = NULL;
	}

	// Core lookups.
	INT Lookup = 0;
	GNativeLookupFuncs[Lookup++] = &FindCoreUObjectNative;
	GNativeLookupFuncs[Lookup++] = &FindCoreUCommandletNative;
	GNativeLookupFuncs[Lookup++] = &FindCoreURegistryNative;

	// Engine lookups.
	GNativeLookupFuncs[Lookup++] = &FindEngineAActorNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineAPawnNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineAPlayerPawnNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineADecalNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineAStatLogNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineAStatLogFileNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineAZoneInfoNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineAWarpZoneInfoNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineALevelInfoNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineAGameInfoNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineANavigationPointNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineUCanvasNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineUConsoleNative;
	GNativeLookupFuncs[Lookup++] = &FindEngineUScriptedTextureNative;

	GNativeLookupFuncs[Lookup++] = &FindIpDrvAInternetLinkNative;
	GNativeLookupFuncs[Lookup++] = &FindIpDrvAUdpLinkNative;
	GNativeLookupFuncs[Lookup++] = &FindIpDrvATcpLinkNative;

	// UWeb lookups.
	GNativeLookupFuncs[Lookup++] = &FindUWebUWebResponseNative;
	GNativeLookupFuncs[Lookup++] = &FindUWebUWebRequestNative;

	// UDemo lookups.
	AUTO_INITIALIZE_REGISTRANTS_UDEMO;

	// Editor lookups.
	GNativeLookupFuncs[Lookup++] = &FindEditorUBrushBuilderNative;
#endif

	UEngine* Engine = NULL;

	guard(main); 
	try 
	{ 
		GIsStarted		= 1;

		// Set module name.
		// vogel: FIXME: strip directories from argv[0]
		appStrncpy( GModule, TEXT("UnrealTournament"), ARRAY_COUNT(GModule));

		// Set the package name.
		appStrncpy( const_cast<TCHAR*>(GPackage), appPackage(), 64);	

		// Get the command line.
		FString CmdLine;
		for( INT i=1; i<argc; i++ )
		{
			if( i>1 )
				CmdLine += TEXT(" ");
			CmdLine += ANSI_TO_TCHAR(argv[i]); 
		}

		// Init core.
		GIsClient		= 1; 
		GIsGuarded		= 1;
		appInit( TEXT("UnrealTournament"), *CmdLine, nullptr, &Log, &Error, &Warn, &FileManager, FConfigCacheIni::Factory, 1 );

		if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_JOYSTICK) == -1)
		{
			const TCHAR *err = appFromAnsi(SDL_GetError());
			appErrorf(TEXT("Couldn't initialize SDL: %s\n"), err);
			appExit();
		}
			
		atexit(sdl_atexit_handler);

		// Init static classes.
#if __STATIC_LINK
		AUTO_INITIALIZE_REGISTRANTS_ENGINE;
		AUTO_INITIALIZE_REGISTRANTS_EDITOR;
		AUTO_INITIALIZE_REGISTRANTS_SDLDRV;
		AUTO_INITIALIZE_REGISTRANTS_ALAUDIO;
		UClusterAudioSubsystem::StaticClass();
#if defined(__EMSCRIPTEN__)
#if FORCE_XOPENGLDRV
		AUTO_INITIALIZE_REGISTRANTS_XOPENGLDRV;
#else
		AUTO_INITIALIZE_REGISTRANTS_OPENGLDRV;
#endif
#else
		AUTO_INITIALIZE_REGISTRANTS_OPENGLDRV;
		AUTO_INITIALIZE_REGISTRANTS_XOPENGLDRV;
#endif
#if MACOSX
		AUTO_INITIALIZE_REGISTRANTS_FRUCORE;
#endif
		//AUTO_INITIALIZE_REGISTRANTS_NULLRENDER;
		AUTO_INITIALIZE_REGISTRANTS_FIRE;
		AUTO_INITIALIZE_REGISTRANTS_IPDRV;
		AUTO_INITIALIZE_REGISTRANTS_UWEB;
		AUTO_INITIALIZE_REGISTRANTS_RENDER;

//!!! FIXME
		char dummyref = 0;
		extern BYTE GLoadedSDLDrv; snprintf(&dummyref, 1, "%d", (int) GLoadedSDLDrv);
#if defined(__EMSCRIPTEN__)
#if FORCE_XOPENGLDRV
		extern BYTE GLoadedXOpenGLDrv; snprintf(&dummyref, 1, "%d", (int) GLoadedXOpenGLDrv);
#else
		extern BYTE GLoadedOpenGLDrv; snprintf(&dummyref, 1, "%d", (int) GLoadedOpenGLDrv);
#endif
#else
		extern BYTE GLoadedOpenGLDrv; snprintf(&dummyref, 1, "%d", (int) GLoadedOpenGLDrv);
		extern BYTE GLoadedXOpenGLDrv; snprintf(&dummyref, 1, "%d", (int) GLoadedXOpenGLDrv);
#endif
//extern BYTE GLoadedNullRender; snprintf(&dummyref, 1, "%d", (int) GLoadedNullRender);
		extern BYTE GLoadedALAudio; snprintf(&dummyref, 1, "%d", (int) GLoadedALAudio);
#endif

		// Init mode.
		GIsServer		= 1;
		GIsClient		= !ParseParam(appCmdLine(), TEXT("SERVER"));
		GIsEditor		= 0;
		GIsScriptable	= 1;
		GLazyLoad		= !GIsClient || ParseParam(appCmdLine(), TEXT("LAZY"));

		// Init windowing.
		// InitWindowing();

		FString Filename = FString::Printf(TEXT("../Help/Splash%i.bmp"),((int)time(NULL)) % 5);
		if( GFileManager->FileSize(*Filename)<0 )
			Filename = FString(TEXT("../Help")) * TEXT("Logo.bmp");
		if( GFileManager->FileSize(*Filename)<0 )
			Filename = TEXT("../Help/Logo.bmp");

		// Init splash screen.
		if ( GFileManager->FileSize(*Filename) > 0)
			InitSplash(*Filename );
	
		// Init console log.
		if (ParseParam(*CmdLine, TEXT("LOG")))
		{
			Warn.AuxOut	= GLog;
			GLog		= &Warn;
		}

		// Init engine.
		Engine = InitEngine();
		if( Engine )
		{
			debugf( NAME_Title, LocalizeGeneral(TEXT("Run")) );

			// Remove splash screen.
			ExitSplash();

			// Optionally Exec and exec file.
			FString Temp;
			if( Parse(*CmdLine, TEXT("EXEC="), Temp) )
			{
				Temp = FString(TEXT("exec ")) + Temp;
				if( Engine->Client && Engine->Client->Viewports.Num() && Engine->Client->Viewports(0) )
					Engine->Client->Viewports(0)->Exec( *Temp, *GLog );
			}
		}
	}
	catch (...)
	{
		// Chained abort.  Do cleanup.
		Error.HandleError();
	}

	if (Engine)
	{
		// Start main engine loop.
		debugf( TEXT("Entering main loop.") );
		if ( !GIsRequestingExit )
			MainLoop( Engine );
	}

    unguard;

    return(0);
}


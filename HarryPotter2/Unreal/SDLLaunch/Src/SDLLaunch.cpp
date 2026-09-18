/*=============================================================================
	Launch.cpp: Game launcher.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Daniel Vogel (based on XLaunch).

=============================================================================*/

// Keep the portable C++ launcher model outside the legacy engine's reflected
// pack(4) regions so every translation unit agrees on its standard-library
// member layout.
#include "HP2LaunchPolicy.h"
#include "HP2LauncherStore.h"
#include "HP2MacLauncher.h"

// Engine/platform includes.
#include "SDLLaunchPrivate.h"
#include "HP2Paths.h"


/*-----------------------------------------------------------------------------
	Global variables.
-----------------------------------------------------------------------------*/

extern "C" { TCHAR GPackage[64] = TEXT("Game"); }

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
#include "FFileManagerUnix.h"
FFileManagerUnix FileManager;

// Memory allocator.
#include "FMallocAnsi.h"
FMallocAnsi Malloc;

// Config.
#undef _INC_EDITOR
#include "FConfigCacheIni.h"

#if __STATIC_LINK
#include "HP2StaticPackages.h"
#endif

// SDL
#include <SDL2/SDL.h>

#include <cstdio>
#include <cstring>
#include <limits.h>
#include <string>
#ifdef __EMSCRIPTEN__
#include <emscripten.h>
#endif


// Splash screen.
SDL_Renderer*	SplashRenderer = NULL;
SDL_Surface*    SplashImage    = NULL;
SDL_Texture*    SplashTexture  = NULL;
SDL_Window*		SplashWindow   = NULL;

static void InitSplash( const TCHAR* Filename )
{
	guard(InitSplash);

	TArray<BYTE> SplashData;
	if( !appLoadFileToArray(SplashData, Filename) || SplashData.Num()==0 )
	{
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Splash screen error", "Could not read splash screen data.", NULL);
		return;
	}

	SDL_RWops* SplashStream = SDL_RWFromConstMem(SplashData.GetData(), SplashData.Num());
	SplashImage = SplashStream ? SDL_LoadBMP_RW(SplashStream, 1) : NULL;
	if( !SplashImage )
	{
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Splash screen error", SDL_GetError(), NULL);
		return;
	}

	SplashWindow = SDL_CreateWindow(
		"Harry Potter 2 is starting...",
		SDL_WINDOWPOS_CENTERED,
		SDL_WINDOWPOS_CENTERED,
		SplashImage->w,
		SplashImage->h,
		SDL_WINDOW_BORDERLESS
	);
	if( !SplashWindow )
	{
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Splash screen error", SDL_GetError(), NULL);
		return;
	}

	SplashRenderer = SDL_CreateRenderer(SplashWindow, -1, 0);
	if( !SplashRenderer )
	{
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Splash screen error", SDL_GetError(), SplashWindow);
		return;
	}

	SplashTexture = SDL_CreateTextureFromSurface(SplashRenderer, SplashImage);
	if( !SplashTexture )
	{
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Splash screen error", SDL_GetError(), SplashWindow);
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

static void ApplyPortableConfig()
{
	guard(ApplyPortableConfig);

	const TCHAR* VideoKeys[] =
	{
		TEXT("GameRenderDevice"),
		TEXT("WindowedRenderDevice"),
		TEXT("RenderDevice")
	};
	const TCHAR* LegacyVideoDevices[] =
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
	const TCHAR* LegacyAudioDevices[] =
	{
		TEXT("Audio.GenericAudioSubsystem"),
		TEXT("Galaxy.GalaxyAudioSubsystem")
	};

	if( !GConfig || !GSys || !GFileManager )
		appErrorf(TEXT("Portable user configuration is unavailable"));
	const TCHAR* UserDir = appUserDir();
	if( !UserDir || !*UserDir )
		appErrorf(TEXT("Portable user directory is unavailable"));

	// Remove only networking actors whose packages are not part of this
	// runtime. Keep unrelated and locally configured ServerActors intact.
	TMultiMap<FString,FString>* GameEngineSection
		= GConfig->GetSectionPrivate(TEXT("Engine.GameEngine"), 0, 0);
	if( GameEngineSection )
	{
		TArray<FString> ServerActors;
		GameEngineSection->MultiFind(TEXT("ServerActors"), ServerActors);
		for( INT ActorIndex = 0; ActorIndex < ServerActors.Num(); ++ActorIndex )
		{
			const TCHAR* ActorSpec = *ServerActors(ActorIndex);
			TCHAR ActorClass[256];
			if
			(	ParseToken(ActorSpec, ActorClass, ARRAY_COUNT(ActorClass), 1)
			&&	(	appStrnicmp(ActorClass, TEXT("IpDrv."), appStrlen(TEXT("IpDrv."))) == 0
				||	appStrnicmp(ActorClass, TEXT("IpServer."), appStrlen(TEXT("IpServer."))) == 0 ) )
			{
				debugf(TEXT("Removing unavailable Engine.GameEngine.ServerActors=%s"), *ServerActors(ActorIndex));
				GameEngineSection->RemovePair(TEXT("ServerActors"), *ServerActors(ActorIndex));
			}
		}
	}

	// appInit creates the writable user configuration from the stock defaults
	// on first run. Migrate only unavailable platform devices so valid custom
	// selections remain intact.
	if( !ParseParam(appCmdLine(), TEXT("NoForceSDLDrv")) )
	{
		const TCHAR* ViewportManager = GConfig->GetStr(TEXT("Engine.Engine"), TEXT("ViewportManager"));
		if( appStricmp(ViewportManager, TEXT("WinDrv.WindowsClient")) == 0 )
			GConfig->SetString(TEXT("Engine.Engine"), TEXT("ViewportManager"), TEXT("SDLDrv.SDLClient"));

		for( INT KeyIndex = 0; KeyIndex < ARRAY_COUNT(VideoKeys); ++KeyIndex )
		{
			const TCHAR* ConfiguredDevice = GConfig->GetStr(TEXT("Engine.Engine"), VideoKeys[KeyIndex]);
			for( INT DeviceIndex = 0; DeviceIndex < ARRAY_COUNT(LegacyVideoDevices); ++DeviceIndex )
			{
				if( appStricmp(ConfiguredDevice, LegacyVideoDevices[DeviceIndex]) == 0 )
				{
					debugf(TEXT("Engine.Engine.%s was %s; selecting XOpenGLDrv."), VideoKeys[KeyIndex], ConfiguredDevice);
					GConfig->SetString(TEXT("Engine.Engine"), VideoKeys[KeyIndex], TEXT("XOpenGLDrv.XOpenGLRenderDevice"));
					break;
				}
			}
		}
	}

	if( !ParseParam(appCmdLine(), TEXT("NoForceALAudio")) )
	{
		const TCHAR* ConfiguredDevice = GConfig->GetStr(TEXT("Engine.Engine"), TEXT("AudioDevice"));
		for( INT DeviceIndex = 0; DeviceIndex < ARRAY_COUNT(LegacyAudioDevices); ++DeviceIndex )
		{
			if( appStricmp(ConfiguredDevice, LegacyAudioDevices[DeviceIndex]) == 0 )
			{
				debugf(TEXT("Engine.Engine.AudioDevice was %s; selecting ALAudio."), ConfiguredDevice);
				GConfig->SetString(TEXT("Engine.Engine"), TEXT("AudioDevice"), TEXT("ALAudio.ALAudioSubsystem"));
				break;
			}
		}
	}

	// appUserDir is canonical and trailing-slashed. Keep the writable roots
	// trailing-slashed as well because callers append filenames directly.
	const FString SavePath      = FString(UserDir) * TEXT("Save") * TEXT("");
	const FString SaveCachePath = SavePath * TEXT("cache") * TEXT("");
	const FString CachePath     = FString(UserDir) * TEXT("Cache") * TEXT("");
	if
	(	!GFileManager->MakeDirectory(*SavePath, 0)
	||	!GFileManager->MakeDirectory(*SaveCachePath, 0)
	||	!GFileManager->MakeDirectory(*CachePath, 0) )
		appErrorf(TEXT("Unable to establish portable paths under %s"), UserDir);

	GSys->SavePath     = SavePath;
	GSys->SaveSlotPath = SavePath;
	GSys->CachePath    = CachePath;

	// SaveSlotPath is the runtime slot root derived from persisted SavePath;
	// USystem exposes SavePath and CachePath (not SaveSlotPath) as config keys.
	GConfig->SetString(TEXT("Core.System"), TEXT("SavePath"), *GSys->SavePath);
	GConfig->SetString(TEXT("Core.System"), TEXT("CachePath"), *GSys->CachePath);

	INT RunCount = 0;
	GConfig->GetInt(TEXT("Engine.Engine"), TEXT("RunCount"), RunCount);
	GConfig->SetInt(TEXT("Engine.Engine"), TEXT("RunCount"), RunCount + 1);

	// Commit migration and path changes before StaticLoadClass constructs the
	// engine from this writable user configuration.
	GConfig->Flush(0);

	unguard;
}


//
// Creates a UEngine object.
//
static UEngine* InitEngine()
{
	guard(InitEngine);
	FTime LoadTime = appSeconds();

	// Set exec hook.
	GExec = NULL;

	ApplyPortableConfig();

	// Create the global engine object.
	UClass* EngineClass = UObject::StaticLoadClass(
		UGameEngine::StaticClass(),
		NULL,
		TEXT("ini:Engine.Engine.GameEngine"),
		NULL,
		LOAD_NoFail,
		NULL
	);
	UEngine* Engine = ConstructObject<UEngine>(EngineClass);
	Engine->Init();

	debugf(TEXT("Startup time: %f seconds."), appSeconds() - LoadTime);

	return Engine;
	unguard;
}

/*-----------------------------------------------------------------------------
	Main Loop
-----------------------------------------------------------------------------*/

//
// Exit wound.
// 
static void CleanUpOnExit( UEngine* Engine )
{
	guard(CleanUpOnExit);

	GIsRunning = 0;
	if( Engine && Engine->Audio )
		Engine->Audio->SetViewport(NULL);

	const FString RunningIni = FString(appUserDir()) * TEXT("Running.ini");
	GFileManager->Delete(*RunningIni, 0, 0);
	debugf(NAME_Title, LocalizeGeneral(TEXT("Exit")));

	appPreExit();
	GIsGuarded = 0;

	unguard;
}

// just in case.  :)  --ryan.
static void sdl_atexit_handler()
{
	static UBOOL AlreadyCalled = 0;
	if( !AlreadyCalled )
	{
		AlreadyCalled = 1;
		SDL_Quit();
	}
}

struct MainLoopArgs
{
	FTime OldTime;
	FTime SecondStartTime;
	INT TickCount;
	UEngine* Engine;
	INT RemainingTestTicks;
};

static bool MainLoopIteration( MainLoopArgs* Args )
{
	guard(MainLoopIteration);

	if( !GIsRunning || GIsRequestingExit )
	{
		GIsRunning = 0;
		return false;
	}

	FTime NewTime = appSeconds();
	FLOAT DeltaTime = Max(0.0f, NewTime - Args->OldTime);
	Args->Engine->Tick(DeltaTime);
	if( GWindowManager )
		GWindowManager->Tick(DeltaTime);
	Args->OldTime = NewTime;
	if( Args->RemainingTestTicks > 0 && --Args->RemainingTestTicks == 0 )
		appRequestExit(0);

	++Args->TickCount;
	const FLOAT RatePeriod = Args->OldTime - Args->SecondStartTime;
	if( RatePeriod > 1.0f )
	{
		Args->Engine->CurrentTickRate = Args->TickCount / RatePeriod;
		Args->SecondStartTime = Args->OldTime;
		Args->TickCount = 0;
	}

	return true;
	unguard;
}

#ifdef __EMSCRIPTEN__
static void EmscriptenMainLoopIteration( void* OpaqueArgs )
{
	MainLoopArgs* Args = static_cast<MainLoopArgs*>(OpaqueArgs);
	try
	{
		if( MainLoopIteration(Args) )
			return;

		UEngine* Engine = Args->Engine;
		CleanUpOnExit(Engine);
		delete Args;
		Args = NULL;
		appExit();
		GIsStarted = 0;
		SDL_Quit();
	}
	catch( ... )
	{
		delete Args;
		Error.HandleError();
		appExit();
		GIsStarted = 0;
		SDL_Quit();
	}
	emscripten_cancel_main_loop();
}
#endif


//
// game message loop.
//
static void MainLoop( UEngine* Engine, INT TestTicks )
{
	guard(MainLoop);
	check(Engine);

	GIsRunning = 1;

#ifdef __EMSCRIPTEN__
	MainLoopArgs* Args = new MainLoopArgs;
	Args->OldTime = appSeconds();
	Args->SecondStartTime = Args->OldTime;
	Args->TickCount = 0;
	Args->Engine = Engine;
	Args->RemainingTestTicks = TestTicks;
	emscripten_set_main_loop_arg(EmscriptenMainLoopIteration, Args, 0, 1);
#else
	MainLoopArgs Args;
	Args.OldTime = appSeconds();
	Args.SecondStartTime = Args.OldTime;
	Args.TickCount = 0;
	Args.Engine = Engine;
	Args.RemainingTestTicks = TestTicks;

	while( MainLoopIteration(&Args) )
	{
		guard(EnforceTickRate);

		FLOAT TargetTickRate = Engine->GetMaxTickRate();
		if( SDL_GetKeyboardFocus() == NULL )
			TargetTickRate = TargetTickRate > 0.0f ? Min(TargetTickRate, 10.0f) : 10.0f;

		if( TargetTickRate > 0.0f )
		{
			const FLOAT Remaining = (1.0f / TargetTickRate) - (appSeconds() - Args.OldTime);
			if( Remaining > 0.0f )
				appSleep(Remaining);
		}

		unguard;
	}
#endif

	unguard;
}

/*-----------------------------------------------------------------------------
	Main.
-----------------------------------------------------------------------------*/

//
// Simple copy.
// 

static bool BuildCommandLine( int ArgC, char* ArgV[], TCHAR* Out, INT OutCapacity )
{
	if( !Out || OutCapacity <= 0 )
		return false;

	INT OutLength = 0;
	Out[0] = 0;

	for( int ArgIndex = 1; ArgIndex < ArgC; ++ArgIndex )
	{
		if( IsHP2DataDirectoryArgument(ArgV[ArgIndex]) )
			continue;
		TCHAR Argument[4096];
		if( !appFromUtf8InPlace(Argument, ArgV[ArgIndex], ARRAY_COUNT(Argument)) )
			return false;

		const INT ArgumentLength = appStrlen(Argument);
		INT EqualsIndex = INDEX_NONE;
		UBOOL HasWhitespace = 0;
		for( INT CharIndex = 0; CharIndex < ArgumentLength; ++CharIndex )
		{
			if( Argument[CharIndex] == TEXT('=') && EqualsIndex == INDEX_NONE )
				EqualsIndex = CharIndex;
			if( Argument[CharIndex] == TEXT(' ') || Argument[CharIndex] == TEXT('\t') )
				HasWhitespace = 1;
			if( Argument[CharIndex] == TEXT('"') )
				return false;
		}

		const INT ExtraCharacters = (OutLength ? 1 : 0) + ArgumentLength + (HasWhitespace ? 2 : 0);
		if( OutLength + ExtraCharacters >= OutCapacity )
			return false;

		if( OutLength )
			Out[OutLength++] = TEXT(' ');

		if( HasWhitespace && EqualsIndex != INDEX_NONE )
		{
			for( INT CharIndex = 0; CharIndex <= EqualsIndex; ++CharIndex )
				Out[OutLength++] = Argument[CharIndex];
			Out[OutLength++] = TEXT('"');
			for( INT CharIndex = EqualsIndex + 1; CharIndex < ArgumentLength; ++CharIndex )
				Out[OutLength++] = Argument[CharIndex];
			Out[OutLength++] = TEXT('"');
		}
		else
		{
			if( HasWhitespace )
				Out[OutLength++] = TEXT('"');
			for( INT CharIndex = 0; CharIndex < ArgumentLength; ++CharIndex )
				Out[OutLength++] = Argument[CharIndex];
			if( HasWhitespace )
				Out[OutLength++] = TEXT('"');
		}
		Out[OutLength] = 0;
	}

	return true;
}

static bool BuildLauncherPaths( HP2Launcher::LauncherPaths& Paths, std::string& ErrorMessage )
{
	ANSICHAR UserRoot[PATH_MAX];
	ANSICHAR SystemRoot[PATH_MAX];
	if( !appToUtf8InPlace(UserRoot, appUserDir(), ARRAY_COUNT(UserRoot))
		|| !appToUtf8InPlace(SystemRoot, appBaseDir(), ARRAY_COUNT(SystemRoot)) )
	{
		ErrorMessage = "The launcher paths are not valid UTF-8 or exceed the system path limit.";
		return false;
	}
	Paths.userRoot = UserRoot;
	Paths.systemRoot = SystemRoot;
	return true;
}
static UBOOL EqualLauncherOptionName( const ANSICHAR* Text, INT Length, const ANSICHAR* Wanted )
{
	INT Index = 0;
	for( ; Index < Length && Wanted[Index]; ++Index )
	{
		ANSICHAR A = Text[Index];
		ANSICHAR B = Wanted[Index];
		if( A >= 'a' && A <= 'z' )
			A = static_cast<ANSICHAR>(A - 'a' + 'A');
		if( B >= 'a' && B <= 'z' )
			B = static_cast<ANSICHAR>(B - 'a' + 'A');
		if( A != B )
			return 0;
	}
	return Index == Length && Wanted[Index] == 0;
}

static const ANSICHAR* LauncherOptionValue(
	INT ArgC,
	char* ArgV[],
	const ANSICHAR* Wanted )
{
	for( INT ArgIndex = 1; ArgIndex < ArgC; ++ArgIndex )
	{
		const ANSICHAR* Option = ArgV[ArgIndex];
		while( *Option == '-' )
			++Option;
		const ANSICHAR* Equals = Option;
		while( *Equals && *Equals != '=' )
			++Equals;
		if( *Equals == '=' && EqualLauncherOptionName(Option, static_cast<INT>(Equals - Option), Wanted) )
			return Equals + 1;
	}
	return NULL;
}

static std::string LauncherLogPath(
	const HP2Launcher::LauncherPaths& Paths,
	INT ArgC,
	char* ArgV[] )
{
	const ANSICHAR* Relative = LauncherOptionValue(ArgC, ArgV, "LOG");
	if( Relative )
	{
		std::string Path = Paths.userRoot;
		if( !Path.empty() && Path.back() != '/' )
			Path += '/';
		Path += Relative;
		return Path;
	}
	const ANSICHAR* Absolute = LauncherOptionValue(ArgC, ArgV, "ABSLOG");
	if( Absolute )
		return Absolute;

	std::string Path = Paths.userRoot;
	if( !Path.empty() && Path.back() != '/' )
		Path += '/';
	Path += "HarryPotter2.log";
	return Path;
}


static bool PrependCommandLine( TCHAR* CommandLine, INT Capacity, const TCHAR* Prefix )
{
	if( !CommandLine || !Prefix || Capacity <= 0 )
		return false;

	const INT ExistingLength = appStrlen(CommandLine);
	const INT PrefixLength = appStrlen(Prefix);
	const INT SeparatorLength = ExistingLength > 0 ? 1 : 0;
	if( PrefixLength >= Capacity
		|| ExistingLength >= Capacity - PrefixLength - SeparatorLength )
		return false;

	const INT ExistingOffset = PrefixLength + SeparatorLength;
	for( INT Index = ExistingLength; Index >= 0; --Index )
		CommandLine[ExistingOffset + Index] = CommandLine[Index];
	for( INT Index = 0; Index < PrefixLength; ++Index )
		CommandLine[Index] = Prefix[Index];
	if( SeparatorLength )
		CommandLine[PrefixLength] = TEXT(' ');
	return true;
}

//
// Entry point.
//
int main( int argc, char* argv[] )
{
	if( !PrepareHP2Paths(argc, argv) )
		return 1;

	TCHAR CmdLine[1024];
	if( !BuildCommandLine(argc, argv, CmdLine, ARRAY_COUNT(CmdLine)) )
	{
		fprintf(stderr, "The command line is too long, contains an unsupported quote, or is not valid UTF-8.\n");
		return 1;
	}

	// The launcher owns standard-library objects before appInit. This process
	// overrides global new/delete with GMalloc, so install the launcher's ANSI
	// allocator first; FMallocAnsi::Init is intentionally idempotent when
	// appInit performs the normal engine initialization below.
	GMalloc = &Malloc;
	GMalloc->Init();

	if( HP2Launcher::ShouldRunNativeLauncher(argc, argv) )
	{
		HP2Launcher::LauncherPaths Paths;
		HP2Launcher::LauncherState State;
		std::string LauncherError;
		if( !BuildLauncherPaths(Paths, LauncherError)
			|| !HP2Launcher::LoadLauncherState(Paths, State, LauncherError) )
		{
			fprintf(stderr, "hp2: unable to prepare the native launcher: %s\n", LauncherError.c_str());
			return 1;
		}

		HP2Launcher::LauncherRequest Request;
		Request.settings = State.settings;
		Request.saves = State.saves;
		Request.rendererDisplayName = "XOpenGL";
		Request.userRoot = Paths.userRoot;
		Request.logPath = LauncherLogPath(Paths, argc, argv);

		for( ;; )
		{
			HP2Launcher::LauncherResult Result;
			const HP2Launcher::LaunchAction Action
				= HP2Launcher::RunHP2MacLauncher(Request, Result, LauncherError);
			if( Action == HP2Launcher::LaunchAction::Quit )
				return 0;
			if( Action == HP2Launcher::LaunchAction::Error )
			{
				fprintf(stderr, "hp2: native launcher failed: %s\n", LauncherError.c_str());
				return 1;
			}
			Result.selection.action = Action;

			std::string SelectedPrefix;
			TCHAR Selection[128];
			TCHAR PendingCmdLine[ARRAY_COUNT(CmdLine)];
			appStrcpy(PendingCmdLine, CmdLine);
			if( !HP2Launcher::BuildSelectedCommand(Result.selection, SelectedPrefix, LauncherError)
				|| !appFromUtf8InPlace(Selection, SelectedPrefix.c_str(), ARRAY_COUNT(Selection))
				|| !PrependCommandLine(PendingCmdLine, ARRAY_COUNT(PendingCmdLine), Selection) )
			{
				fprintf(stderr, "hp2: the selected launch command is invalid or does not fit: %s\n",
					LauncherError.c_str());
				return 1;
			}
			if( !HP2Launcher::ValidateLauncherSettings(Result.settings, LauncherError) )
			{
				Request.settings = Result.settings;
				Request.errorMessage = LauncherError;
				continue;
			}
			if( !HP2Launcher::CommitLauncherSettings(Paths, Result.settings, LauncherError) )
			{
				Request.settings = Result.settings;
				Request.errorMessage = std::string("The settings were not saved: ") + LauncherError;
				continue;
			}

			appStrcpy(CmdLine, PendingCmdLine);
			break;
		}
	}

#if __STATIC_LINK
	InstallHP2NativeLookups();
#endif

	INT ErrorLevel = 0;
	UEngine* Engine = NULL;

	guard(main);
	try
	{
		GIsStarted = 1;

		// GModule is an ANSI process-module identifier in HP2's Unix platform.
		strncpy(GModule, "HarryPotter2", sizeof(GModule) - 1);
		GModule[sizeof(GModule) - 1] = 0;

		GIsClient = 1;
		GIsGuarded = 1;
		appInit(TEXT("Game"), CmdLine, &Malloc, &Log, &Error, &Warn, &FileManager, FConfigCacheIni::Factory, 1);

#if __STATIC_LINK
		// UObject::StaticInit, including Core classes, is owned by appInit.
		RegisterHP2RuntimeClasses();
		RegisterHP2ClientClasses();
#endif

		if( SDL_Init(SDL_INIT_VIDEO | SDL_INIT_JOYSTICK) != 0 )
		{
			TCHAR SDLError[1024];
			if( !appFromUtf8InPlace(SDLError, SDL_GetError(), ARRAY_COUNT(SDLError)) )
				appStrcpy(SDLError, TEXT("Unknown SDL initialization error"));
			appErrorf(TEXT("Couldn't initialize SDL: %s"), SDLError);
		}
		atexit(sdl_atexit_handler);

		GIsServer = 1;
		GIsClient = !ParseParam(appCmdLine(), TEXT("SERVER"));
		GIsEditor = 0;
		GIsScriptable = 1;
		GLazyLoad = !GIsClient || ParseParam(appCmdLine(), TEXT("LAZY"));

		FString Filename = FString::Printf(TEXT("../Help/Splash%s.bmp"), UObject::GetLanguage());
		if( GFileManager->FileSize(*Filename) < 0 )
			Filename = FString(TEXT("../Help")) * TEXT("Logo.bmp");
		if( GFileManager->FileSize(*Filename) < 0 )
			Filename = TEXT("../Help/Logo.bmp");

		if( !ParseParam(CmdLine, TEXT("NOFRONTEND")) && GFileManager->FileSize(*Filename) > 0 )
			InitSplash(*Filename);

		if( ParseParam(CmdLine, TEXT("LOG")) )
		{
			Warn.AuxOut = GLog;
			GLog = &Warn;
		}

		Engine = InitEngine();
		if( Engine )
		{
			debugf(NAME_Title, LocalizeGeneral(TEXT("Run")));
			ExitSplash();

			FString Temp;
			if( Parse(CmdLine, TEXT("EXEC="), Temp) )
			{
				Temp = FString(TEXT("exec ")) + Temp;
				if( Engine->Client && Engine->Client->Viewports.Num() && Engine->Client->Viewports(0) )
					Engine->Client->Viewports(0)->Exec(*Temp, *GLog);
			}

			INT TestTicks = 0;
			Parse(CmdLine, TEXT("TESTTICKS="), TestTicks);
			debugf(TEXT("Entering main loop."));
			if( !GIsRequestingExit )
				MainLoop(Engine, Max(0, TestTicks));
		}

		CleanUpOnExit(Engine);
	}
	catch( ... )
	{
		ErrorLevel = 1;
		Error.HandleError();
	}

	appExit();
	GIsStarted = 0;
	SDL_Quit();

	return ErrorLevel;
	unguard;
}


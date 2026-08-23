/*=============================================================================
	SDLLaunch.cpp: Harry Potter 2 SDL launcher and engine entry point.
=============================================================================*/

#include "SDLLaunchPrivate.h"

#include <SDL2/SDL.h>

#include "FConfigCacheIni.h"
#include "FFileManagerUnix.h"
#include "FMallocAnsi.h"
#include "FOutputDeviceFile.h"
#include "FOutputDeviceStdout.h"

#include "FFeedbackContextSDL.h"
#include "FOutputDeviceSDLError.h"

#include "HP2LaunchPolicy.h"
#include "HP2LauncherModel.h"
#include "HP2LauncherStore.h"
#include "HP2MacLauncher.h"
#include "HP2Paths.h"
#include "HP2StaticPackages.h"

using namespace HP2Launcher;

/*-----------------------------------------------------------------------------
	Global variables.
-----------------------------------------------------------------------------*/

// Engine service singletons, handed to appInit below.
FMallocAnsi Malloc;
FOutputDeviceFile Log;
FOutputDeviceSDLError Error;
FFeedbackContextSDL Warn;
FFileManagerUnix FileManager;

// Optional stdout mirror installed for -log runs so harness scripts can
// scrape progress markers from captured output.
FOutputDeviceStdout StdoutEcho;

namespace
{

/*-----------------------------------------------------------------------------
	Helpers.
-----------------------------------------------------------------------------*/

bool FrontendSuppressed()
{
	return ParseParam( appCmdLine(), TEXT("NOFRONTEND") ) != 0;
}

FString SdlStatusText( const char* Raw )
{
	TCHAR Buffer[512];
	if( !Raw || !appFromUtf8InPlace( Buffer, Raw, ARRAY_COUNT(Buffer) ) )
		return FString( TEXT("<unavailable>") );
	return FString( Buffer );
}

// Builds the forwarded engine command line: every argument except the
// launcher-owned data-root bootstrap options, converted from UTF-8.
FString ForwardedCommandLine( int ArgC, char* ArgV[] )
{
	FString CommandLine;
	for( int Index = 1; Index < ArgC; ++Index )
	{
		if( IsHP2DataDirectoryArgument( ArgV[Index] ) )
			continue;
		TCHAR Argument[1024];
		if( !appFromUtf8InPlace( Argument, ArgV[Index], ARRAY_COUNT(Argument) ) )
		{
			fprintf( stderr, "hp2: argument %d is not valid UTF-8 or is too long\n", Index );
			continue;
		}
		if( CommandLine.Len() )
			CommandLine += TEXT(" ");
		CommandLine += Argument;
	}
	return CommandLine;
}

void ShowLauncherError( const std::string& Detail, const char* Context )
{
	const std::string Text = Context + std::string("\n\n") + (Detail.empty() ? "Unknown error." : Detail);
	fprintf( stderr, "hp2-launcher: %s\n", Text.c_str() );
	SDL_ShowSimpleMessageBox( SDL_MESSAGEBOX_ERROR, "Launcher Error", Text.c_str(), NULL );
}

/*-----------------------------------------------------------------------------
	Native launcher integration.
-----------------------------------------------------------------------------*/

struct LauncherFlowResult
{
	bool RunEngine = false;
	FString EnginePrefix;
};

// Resolves one playable data root, honoring the explicit -datadir override,
// the stored launcher catalog, and conventional-import discovery in that
// order of authority. On failure FailureReason carries a user-facing detail.
bool ResolveSourceRoot(
	const DataSourceConfiguration& Configuration,
	DataSource Source,
	bool HasOverride,
	const std::string& OverrideRoot,
	std::string& Root,
	std::string& FailureReason )
{
	if( HasOverride )
	{
		std::string CanonicalRoot;
		std::string Error;
		if( !ValidateHP2DataRoot( OverrideRoot, CanonicalRoot, Error ) )
		{
			FailureReason = Error.empty()
				? ("The data root is unusable: " + OverrideRoot)
				: Error;
			return false;
		}
		Root = CanonicalRoot;
		return true;
	}

	std::string Candidate = Source == DataSource::Retail
		? Configuration.retailRoot
		: Configuration.prototypeRoot;
	if( Candidate.empty() )
	{
		const bool Found = Source == DataSource::Retail
			? DiscoverHP2RetailDataRoot( Candidate )
			: DiscoverHP2PrototypeDataRoot( Candidate );
		if( !Found )
		{
			FailureReason = Source == DataSource::Retail
				? "No imported retail game data was found."
				: "No prototype game data was found.";
			return false;
		}
	}

	std::string CanonicalRoot;
	std::string Error;
	if( !ValidateHP2DataRoot( Candidate, CanonicalRoot, Error ) )
	{
		FailureReason = Error.empty()
			? ("The configured data root is unusable: " + Candidate)
			: Error;
		return false;
	}
	Root = CanonicalRoot;
	return true;
}

// One chooser round-trip: prepares the launcher home and per-source profile,
// shows the native chooser, persists everything it decided, and installs the
// chosen roots for the engine. Returns false when the process must stop;
// Outcome.RunEngine says whether the engine should start afterwards.
bool RunNativeLauncherRound(
	int ArgC,
	char* ArgV[],
	bool HasOverride,
	const std::string& OverrideRoot,
	LauncherFlowResult& Outcome )
{
	Outcome = LauncherFlowResult();

	std::string LauncherRoot;
	std::string StepError;
	if( !PrepareHP2LauncherHome( LauncherRoot, StepError ) )
	{
		ShowLauncherError( StepError, "Could not prepare the launcher folder." );
		return false;
	}

	DataSourceConfiguration Configuration;
	LoadDataSourceConfiguration( LauncherRoot, Configuration, StepError );

	const DataSource SelectedSource = Configuration.selected;
	std::string ProfileRoot;
	if( !PrepareDataSourceProfile( LauncherRoot, SelectedSource, ProfileRoot, StepError ) )
	{
		ShowLauncherError( StepError, "Could not prepare the profile for the selected data source." );
		return false;
	}

	std::string DataRoot;
	std::string ResolveFailure;
	if( !ResolveSourceRoot( Configuration, SelectedSource, HasOverride, OverrideRoot, DataRoot, ResolveFailure ) )
	{
		ShowLauncherError( ResolveFailure, "Could not resolve a playable data root." );
		return false;
	}

	const LauncherPaths Paths{ ProfileRoot, DataRoot + "/System" };
	LauncherState State;
	if( !LoadLauncherState( Paths, State, StepError ) )
		State = LauncherState();

	LauncherRequest Request;
	Request.settings = State.settings;
	Request.saves = State.saves;
	Request.userRoot = ProfileRoot;
	Request.logPath = LauncherRoot + "/Launcher.log";
	Request.dataSources = Configuration;
	Request.hasExplicitDataRootOverride = HasOverride;
	Request.explicitDataRoot = OverrideRoot;
	for( const DataSource OptionSource : { DataSource::Retail, DataSource::Prototype } )
	{
		DataSourceOption Option;
		Option.source = OptionSource;
		std::string OptionRoot;
		std::string OptionFailure;
		if( ResolveSourceRoot( Configuration, OptionSource, HasOverride && OptionSource == SelectedSource, OverrideRoot, OptionRoot, OptionFailure ) )
		{
			Option.available = true;
			Option.root = OptionRoot;
		}
		else
		{
			Option.error = OptionFailure;
		}
		Request.dataSourceOptions.push_back( Option );
	}

	LauncherResult Result;
	const LaunchAction Action = RunHP2MacLauncher( Request, Result, StepError );
	if( Action == LaunchAction::Error )
	{
		ShowLauncherError( StepError, "The launcher reported an error." );
		return false;
	}
	if( Action == LaunchAction::Quit )
		return false;

	// Persist every chooser decision before touching the engine so a failed
	// launch still resumes from the same state next time.
	if( !CommitDataSourceConfiguration( LauncherRoot, Result.dataSources, StepError )
		|| !CommitLaunchSelection( LauncherRoot, Result.selection, StepError ) )
	{
		ShowLauncherError( StepError, "Could not save the launcher selection." );
		return false;
	}

	const DataSource FinalSource = Result.dataSources.selected;
	if( FinalSource != SelectedSource
		&& !PrepareDataSourceProfile( LauncherRoot, FinalSource, ProfileRoot, StepError ) )
	{
		ShowLauncherError( StepError, "Could not prepare the newly selected data source." );
		return false;
	}

	std::string FinalDataRoot;
	std::string FinalFailure;
	if( !ResolveSourceRoot( Result.dataSources, FinalSource, HasOverride, OverrideRoot, FinalDataRoot, FinalFailure ) )
	{
		ShowLauncherError( FinalFailure, "The selected data source has no usable game data." );
		return false;
	}

	if( !ValidateLauncherSettings( Result.settings, StepError ) )
	{
		ShowLauncherError( StepError, "The selected settings are invalid." );
		return false;
	}
	const LauncherPaths FinalPaths{ ProfileRoot, FinalDataRoot + "/System" };
	if( !CommitLauncherSettings( FinalPaths, Result.settings, StepError ) )
	{
		ShowLauncherError( StepError, "Could not save the selected settings." );
		return false;
	}

	if( !InstallHP2Paths( FinalDataRoot, ProfileRoot, StepError ) )
	{
		ShowLauncherError( StepError, "Could not install the selected paths." );
		return false;
	}

	std::string PrefixUtf8;
	if( !BuildSelectedCommand( Result.selection, PrefixUtf8, StepError ) )
	{
		ShowLauncherError( StepError, "The launcher selection cannot be started." );
		return false;
	}
	if( Result.selection.action == LaunchAction::Quit || PrefixUtf8.empty() )
		return false;

	TCHAR Prefix[1024];
	if( !appFromUtf8InPlace( Prefix, PrefixUtf8.c_str(), ARRAY_COUNT(Prefix) ) )
	{
		ShowLauncherError( PrefixUtf8, "The launch command is invalid." );
		return false;
	}

	Outcome.RunEngine = true;
	Outcome.EnginePrefix = Prefix;
	return true;
}

/*-----------------------------------------------------------------------------
	Splash screen.
-----------------------------------------------------------------------------*/

SDL_Window* SplashWindow = NULL;
SDL_Renderer* SplashRenderer = NULL;
SDL_Texture* SplashTexture = NULL;
SDL_Surface* SplashImage = NULL;

void HideSplash()
{
	if( SplashTexture )
	{
		SDL_DestroyTexture( SplashTexture );
		SplashTexture = NULL;
	}
	if( SplashRenderer )
	{
		SDL_DestroyRenderer( SplashRenderer );
		SplashRenderer = NULL;
	}
	if( SplashImage )
	{
		SDL_FreeSurface( SplashImage );
		SplashImage = NULL;
	}
	if( SplashWindow )
	{
		SDL_DestroyWindow( SplashWindow );
		SplashWindow = NULL;
	}
}

// Loads one BMP from memory and presents it until HideSplash. Any failure is
// silent: the splash is cosmetic and must never block startup.
void ShowSplash( const TCHAR* Filename )
{
	TArray<BYTE> SplashData;
	if( !appLoadFileToArray( SplashData, Filename ) || SplashData.Num() == 0 )
		return;

	SDL_RWops* Stream = SDL_RWFromConstMem( SplashData.GetData(), SplashData.Num() );
	SplashImage = Stream ? SDL_LoadBMP_RW( Stream, 1 ) : NULL;
	if( !SplashImage )
		return;

	SplashWindow = SDL_CreateWindow(
		"Harry Potter 2",
		SDL_WINDOWPOS_CENTERED,
		SDL_WINDOWPOS_CENTERED,
		SplashImage->w,
		SplashImage->h,
		0 );
	if( !SplashWindow )
	{
		HideSplash();
		return;
	}

	SplashRenderer = SDL_CreateRenderer( SplashWindow, -1, 0 );
	SplashTexture = SplashRenderer ? SDL_CreateTextureFromSurface( SplashRenderer, SplashImage ) : NULL;
	if( !SplashRenderer || !SplashTexture )
	{
		HideSplash();
		return;
	}

	SDL_RenderClear( SplashRenderer );
	SDL_RenderCopy( SplashRenderer, SplashTexture, NULL, NULL );
	SDL_RenderPresent( SplashRenderer );
}

/*-----------------------------------------------------------------------------
	Portability config migrations.
-----------------------------------------------------------------------------*/

// Stock-UE driver bindings cannot work in this tree: WinDrv, D3D, Glide, and
// Soft drivers are not shipped. Rewrite stale bindings to their portable
// equivalents before the engine loads its classes.
void ApplyPortableConfig()
{
	auto IsLegacyRenderDevice = []( const TCHAR* Value )
	{
		static const TCHAR* LegacyPrefixes[] =
		{
			TEXT("D3DDrv."),
			TEXT("GlideDrv."),
			TEXT("SoftDrv."),
			TEXT("OpenGLDrv."),
			TEXT("WinDrv.")
		};
		for( const TCHAR* Prefix : LegacyPrefixes )
		{
			if( appStrnicmp( Value, Prefix, appStrlen(Prefix) ) == 0 )
				return true;
		}
		return false;
	};

	TCHAR Current[256];

	// The only shipped viewport manager is SDLDrv.SDLClient; make sure it is
	// bound even if the key was dropped entirely.
	if( !GConfig->GetString( TEXT("Engine.Engine"), TEXT("ViewportManager"), Current, ARRAY_COUNT(Current), TEXT("System") )
		|| appStricmp( Current, TEXT("SDLDrv.SDLClient") ) != 0 )
	{
		GConfig->SetString( TEXT("Engine.Engine"), TEXT("ViewportManager"), TEXT("SDLDrv.SDLClient"), TEXT("System") );
		debugf( NAME_Init, TEXT("Bound [Engine.Engine] ViewportManager=SDLDrv.SDLClient") );
	}

	static const TCHAR* RenderKeys[] = { TEXT("GameRenderDevice"), TEXT("WindowedRenderDevice") };
	for( const TCHAR* Key : RenderKeys )
	{
		if( GConfig->GetString( TEXT("Engine.Engine"), Key, Current, ARRAY_COUNT(Current), TEXT("System") )
			&& IsLegacyRenderDevice( Current ) )
		{
			GConfig->SetString( TEXT("Engine.Engine"), Key, TEXT("XOpenGLDrv.XOpenGLRenderDevice"), TEXT("System") );
			debugf( NAME_Init, TEXT("Migrated [Engine.Engine] %s=%s to XOpenGLDrv.XOpenGLRenderDevice"), Key, Current );
		}
	}

	// ALAudio is the only shipped audio subsystem; any other binding is a
	// legacy leftover.
	if( GConfig->GetString( TEXT("Engine.Engine"), TEXT("AudioDevice"), Current, ARRAY_COUNT(Current), TEXT("System") )
		&& appStricmp( Current, TEXT("ALAudio.ALAudioSubsystem") ) != 0 )
	{
		GConfig->SetString( TEXT("Engine.Engine"), TEXT("AudioDevice"), TEXT("ALAudio.ALAudioSubsystem"), TEXT("System") );
		debugf( NAME_Init, TEXT("Migrated [Engine.Engine] AudioDevice=%s to ALAudio.ALAudioSubsystem"), Current );
	}
}

/*-----------------------------------------------------------------------------
	Engine bootstrap.
-----------------------------------------------------------------------------*/

UEngine* InitEngine()
{
	ApplyPortableConfig();

	UClass* EngineClass = UObject::StaticLoadClass(
		UGameEngine::StaticClass(),
		NULL,
		TEXT("ini:Engine.Engine.GameEngine"),
		NULL,
		LOAD_NoFail,
		NULL );
	if( !EngineClass )
		return NULL;

	UGameEngine* Engine = ConstructObject<UGameEngine>( EngineClass );
	if( Engine )
		Engine->Init();
	return Engine;
}

void MainLoop( UEngine* Engine )
{
	check( Engine );

	// Loop while running.
	GIsRunning = 1;
	FTime OldTime = appSeconds();
	FTime SecondStartTime = OldTime;
	INT TickCount = 0;
	while( GIsRunning && !GIsRequestingExit )
	{
		// Update the world.
		FTime NewTime = appSeconds();
		FLOAT DeltaSeconds = Max( (FLOAT)(NewTime - OldTime), 0.0001f );
		Engine->Tick( DeltaSeconds );
		OldTime = NewTime;
		++TickCount;
		if( OldTime - SecondStartTime > 1 )
		{
			Engine->CurrentTickRate = (FLOAT)TickCount / (FLOAT)(OldTime - SecondStartTime);
			SecondStartTime = OldTime;
			TickCount = 0;
		}

		// Enforce optional maximum tick rate.
		const FLOAT MaxTickRate = Engine->GetMaxTickRate();
		if( MaxTickRate > 0.f )
			appSleep( Max( 0.f, (1.f / MaxTickRate) - (FLOAT)(appSeconds() - OldTime) ) );
	}
	GIsRunning = 0;
}

void CleanUpOnExit( UEngine* Engine )
{
	if( Engine )
		Engine->Exit();
	appPreExit();
}

} // namespace

/*-----------------------------------------------------------------------------
	Main.
-----------------------------------------------------------------------------*/

int main( int ArgC, char* ArgV[] )
{
	// Validate and install the initial data/user roots before anything else
	// touches the filesystem.
	if( !PrepareHP2Paths( ArgC, ArgV ) )
		return 1;

	// An explicit -datadir=<path> overrides stored launcher configuration;
	// its first recognized occurrence wins.
	std::string OverrideRoot;
	bool HasOverride = false;
	for( int Index = 1; Index < ArgC; ++Index )
	{
		const char* Value = HP2DataDirectoryArgumentValue( ArgV[Index] );
		if( Value )
		{
			OverrideRoot = Value;
			HasOverride = true;
			break;
		}
	}

	const FString ForwardedArguments = ForwardedCommandLine( ArgC, ArgV );
	FString EngineCommandLine = ForwardedArguments;

	// Interactive chooser rounds: repeat after recoverable errors until the
	// player quits or selects a launchable target. Explicit maps, URLs, and
	// bypass options skip the chooser entirely.
	if( ShouldRunNativeLauncher( ArgC, ArgV ) )
	{
		for( ;; )
		{
			LauncherFlowResult Outcome;
			if( !RunNativeLauncherRound( ArgC, ArgV, HasOverride, OverrideRoot, Outcome ) )
				return 0;
			if( Outcome.RunEngine )
			{
				EngineCommandLine = Outcome.EnginePrefix.Len()
					? Outcome.EnginePrefix + TEXT(" ") + ForwardedArguments
					: ForwardedArguments;
				break;
			}
		}
	}

	INT ExitCode = 0;
	GIsStarted = 1;
#if !_MSC_VER
	__Context::StaticInit();
	strncpy( GModule, ArgV[0], sizeof(GModule) - 1 );
	GModule[sizeof(GModule) - 1] = 0;
#endif
#ifndef _DEBUG
	try
#endif
	{
		GIsGuarded = 1;

#if __STATIC_LINK
		InstallHP2NativeLookups();
#endif

		// Init engine core.
		appInit(
			TEXT("Game"),
			*EngineCommandLine,
			&Malloc,
			&Log,
			&Error,
			&Warn,
			&FileManager,
			FConfigCacheIni::Factory,
			1 );

		// Register all statically linked packages.
		RegisterHP2RuntimeClasses();
		RegisterHP2ClientClasses();

		if( SDL_Init( SDL_INIT_VIDEO | SDL_INIT_JOYSTICK ) != 0 )
			debugf( NAME_Warning, TEXT("SDL_Init failed: %s"), *SdlStatusText( SDL_GetError() ) );

		// Optional splash: shown before the engine initializes, hidden once
		// the main loop starts.
		if( !FrontendSuppressed() )
		{
			FString SplashName( TEXT("../Help/Splash.bmp") );
			if( GFileManager->FileSize( *SplashName ) < 0 )
				SplashName = FString( TEXT("../Help/Logo.bmp") );
			if( GFileManager->FileSize( *SplashName ) >= 0 )
				ShowSplash( *SplashName );
		}

		if( ParseParam( appCmdLine(), TEXT("LOG") ) )
			GLogHook = &StdoutEcho;

		UEngine* Engine = InitEngine();
		if( !Engine )
			appErrorf( TEXT("Could not initialize the game engine") );

		debugf( TEXT("Entering main loop.") );
		HideSplash();

		MainLoop( Engine );
		CleanUpOnExit( Engine );
	}
#ifndef _DEBUG
	catch( ... )
	{
		ExitCode = 1;
		if( GError )
			GError->HandleError();
	}
#endif

	GIsGuarded = 0;
	HideSplash();
	appExit();
	SDL_Quit();
	GIsStarted = 0;
	return ExitCode;
}

/*=============================================================================
	SDLLaunch.cpp: Harry Potter 2 SDL launcher and engine entry point.
=============================================================================*/

#include "SDLLaunchPrivate.h"
#include "HP2TraceHooks.h"

#include <CommonCrypto/CommonDigest.h>
#include <SDL2/SDL.h>
#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdlib>
#include <memory>
#include <string>
#include <vector>

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
#include "HP2CrashReporter.h"
#include "HP2MacLauncher.h"
#include "HP2Paths.h"
#include "HP2StaticPackages.h"
#include "ALAudio.h"
#include "UnSkeletalMesh.h"
#include "UnFractal.h"
#include "UnLinker.h"

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

struct FidelityCheckpoint
{
	std::string Id;
	std::string Position;
	std::string Path;
	INT ThreadSlot = INDEX_NONE;
	INT LineIndex = INDEX_NONE;
};

void SkipJsonWhitespace( const std::string& Text, size_t& Cursor )
{
	while( Cursor < Text.size() && std::isspace( (unsigned char)Text[Cursor] ) )
		++Cursor;
}

bool ReadJsonString( const std::string& Text, size_t& Cursor, std::string& Out )
{
	SkipJsonWhitespace( Text, Cursor );
	if( Cursor >= Text.size() || Text[Cursor++] != '"' )
		return false;
	Out.clear();
	while( Cursor < Text.size() )
	{
		const char Ch = Text[Cursor++];
		if( Ch == '"' )
			return true;
		if( Ch != '\\' )
		{
			Out += Ch;
			continue;
		}
		if( Cursor >= Text.size() )
			return false;
		switch( Text[Cursor++] )
		{
		case '"': Out += '"'; break;
		case '\\': Out += '\\'; break;
		case '/': Out += '/'; break;
		case 'b': Out += '\b'; break;
		case 'f': Out += '\f'; break;
		case 'n': Out += '\n'; break;
		case 'r': Out += '\r'; break;
		case 't': Out += '\t'; break;
		default: return false;
		}
	}
	return false;
}

bool JsonObjectEnd( const std::string& Text, size_t Begin, size_t& End )
{
	if( Begin >= Text.size() || Text[Begin] != '{' )
		return false;
	INT Depth = 0;
	bool InString = false;
	bool Escape = false;
	for( size_t Cursor = Begin; Cursor < Text.size(); ++Cursor )
	{
		const char Ch = Text[Cursor];
		if( InString )
		{
			if( Escape )
				Escape = false;
			else if( Ch == '\\' )
				Escape = true;
			else if( Ch == '"' )
				InString = false;
			continue;
		}
		if( Ch == '"' )
			InString = true;
		else if( Ch == '{' )
			++Depth;
		else if( Ch == '}' && --Depth == 0 )
		{
			End = Cursor + 1;
			return true;
		}
	}
	return false;
}

bool JsonMemberCursor( const std::string& Object, const char* Key, size_t& Cursor )
{
	const std::string Needle = std::string( "\"" ) + Key + "\"";
	const size_t Found = Object.find( Needle );
	if( Found == std::string::npos )
		return false;
	Cursor = Found + Needle.size();
	SkipJsonWhitespace( Object, Cursor );
	if( Cursor >= Object.size() || Object[Cursor++] != ':' )
		return false;
	SkipJsonWhitespace( Object, Cursor );
	return true;
}

bool JsonStringMember( const std::string& Object, const char* Key, std::string& Out )
{
	size_t Cursor = 0;
	return JsonMemberCursor( Object, Key, Cursor ) && ReadJsonString( Object, Cursor, Out );
}

bool JsonIntMember( const std::string& Object, const char* Key, INT& Out )
{
	size_t Cursor = 0;
	if( !JsonMemberCursor( Object, Key, Cursor ) )
		return false;
	char* End = NULL;
	const long Value = strtol( Object.c_str() + Cursor, &End, 10 );
	if( End == Object.c_str() + Cursor || Value < -(long)MAXINT - 1 || Value > MAXINT )
		return false;
	Out = (INT)Value;
	return true;
}

bool JsonObjectMember( const std::string& Object, const char* Key, std::string& Out )
{
	size_t Cursor = 0;
	size_t End = 0;
	if( !JsonMemberCursor( Object, Key, Cursor ) || !JsonObjectEnd( Object, Cursor, End ) )
		return false;
	Out.assign( Object, Cursor, End - Cursor );
	return true;
}

bool JsonValueEnd( const std::string& Text, size_t& Cursor )
{
	SkipJsonWhitespace( Text, Cursor );
	if( Cursor >= Text.size() )
		return false;
	if( Text[Cursor] == '{' )
		return JsonObjectEnd( Text, Cursor, Cursor );
	if( Text[Cursor] == '"' )
	{
		std::string Ignored;
		return ReadJsonString( Text, Cursor, Ignored );
	}
	if( Text[Cursor] == '[' )
	{
		++Cursor;
		for( ;; )
		{
			SkipJsonWhitespace( Text, Cursor );
			if( Cursor >= Text.size() )
				return false;
			if( Text[Cursor] == ']' )
			{
				++Cursor;
				return true;
			}
			if( !JsonValueEnd(Text,Cursor) )
				return false;
			SkipJsonWhitespace( Text, Cursor );
			if( Cursor >= Text.size() )
				return false;
			if( Text[Cursor] == ']' )
			{
				++Cursor;
				return true;
			}
			if( Text[Cursor++] != ',' )
				return false;
			SkipJsonWhitespace( Text, Cursor );
			if( Cursor >= Text.size() || Text[Cursor] == ']' )
				return false;
		}
	}
	if( Text.compare(Cursor,4,"true") == 0 )
	{
		Cursor += 4;
		return true;
	}
	if( Text.compare(Cursor,5,"false") == 0 )
	{
		Cursor += 5;
		return true;
	}
	if( Text.compare(Cursor,4,"null") == 0 )
	{
		Cursor += 4;
		return true;
	}
	char* End = NULL;
	errno = 0;
	strtod( Text.c_str() + Cursor, &End );
	if( End == Text.c_str() + Cursor || errno == ERANGE )
		return false;
	Cursor = (size_t)(End - Text.c_str());
	return true;
}

bool JsonObjectHasExactKeys( const std::string& Object, const char* const* Expected, size_t ExpectedCount )
{
	size_t Cursor = 0;
	SkipJsonWhitespace( Object, Cursor );
	if( Cursor >= Object.size() || Object[Cursor++] != '{' )
		return false;
	std::vector<std::string> Keys;
	for( ;; )
	{
		SkipJsonWhitespace( Object, Cursor );
		if( Cursor >= Object.size() )
			return false;
		if( Object[Cursor] == '}' )
		{
			++Cursor;
			break;
		}
		std::string Key;
		if( !ReadJsonString(Object,Cursor,Key) )
			return false;
		SkipJsonWhitespace( Object, Cursor );
		if( Cursor >= Object.size() || Object[Cursor++] != ':' || !JsonValueEnd(Object,Cursor) )
			return false;
		if( std::find(Keys.begin(),Keys.end(),Key) != Keys.end() )
			return false;
		Keys.push_back( Key );
		SkipJsonWhitespace( Object, Cursor );
		if( Cursor >= Object.size() )
			return false;
		if( Object[Cursor] == '}' )
		{
			++Cursor;
			break;
		}
		if( Object[Cursor++] != ',' )
			return false;
		SkipJsonWhitespace( Object, Cursor );
		if( Cursor >= Object.size() || Object[Cursor] == '}' )
			return false;
	}
	SkipJsonWhitespace( Object, Cursor );
	if( Cursor != Object.size() || Keys.size() != ExpectedCount )
		return false;
	for( size_t Index=0; Index<ExpectedCount; ++Index )
		if( std::find(Keys.begin(),Keys.end(),Expected[Index]) == Keys.end() )
			return false;
	return true;
}
bool ParseFidelityManifest( const char* Filename, std::vector<FidelityCheckpoint>& Out )
{
	Out.clear();
	FILE* File = fopen( Filename, "rb" );
	if( !File )
		return false;
	std::string Text;
	char Buffer[4096];
	while( const size_t Read = fread( Buffer, 1, sizeof(Buffer), File ) )
		Text.append( Buffer, Read );
	fclose( File );

	const size_t Marker = Text.find( "\"checkpoints\"" );
	if( Marker == std::string::npos )
		return false;
	size_t Cursor = Text.find( '[', Marker );
	if( Cursor == std::string::npos )
		return false;
	++Cursor;
	for( ;; )
	{
		SkipJsonWhitespace( Text, Cursor );
		if( Cursor >= Text.size() )
			return false;
		if( Text[Cursor] == ']' )
			return true;
		size_t End = 0;
		if( !JsonObjectEnd( Text, Cursor, End ) )
			return false;
		const std::string Object = Text.substr( Cursor, End - Cursor );
		std::string Locator;
		FidelityCheckpoint Checkpoint;
		if( !JsonStringMember( Object, "id", Checkpoint.Id )
			|| !JsonStringMember( Object, "position", Checkpoint.Position )
			|| !JsonObjectMember( Object, "runtime_locator", Locator )
			|| !JsonStringMember( Locator, "path", Checkpoint.Path )
			|| !JsonIntMember( Locator, "thread_slot", Checkpoint.ThreadSlot )
			|| !JsonIntMember( Locator, "line_index", Checkpoint.LineIndex ) )
			return false;
		Out.push_back( Checkpoint );
		Cursor = End;
		SkipJsonWhitespace( Text, Cursor );
		if( Cursor < Text.size() && Text[Cursor] == ',' )
			++Cursor;
	}
}

bool JsonFloatMember( const std::string& Object, const char* Key, FLOAT& Out )
{
	size_t Cursor = 0;
	if( !JsonMemberCursor( Object, Key, Cursor ) )
		return false;
	char* End = NULL;
	errno = 0;
	const double Value = strtod( Object.c_str() + Cursor, &End );
	if( End == Object.c_str() + Cursor || errno == ERANGE )
		return false;
	Out = (FLOAT)Value;
	return true;
}

bool JsonBoolMember( const std::string& Object, const char* Key, UBOOL& Out )
{
	size_t Cursor = 0;
	if( !JsonMemberCursor( Object, Key, Cursor ) )
		return false;
	if( Object.compare( Cursor, 4, "true" ) == 0 )
	{
		Out = 1;
		return true;
	}
	if( Object.compare( Cursor, 5, "false" ) == 0 )
	{
		Out = 0;
		return true;
	}
	return false;
}
bool JsonVectorMember( const std::string& Object, const char* Key, FVector& Out )
{
	size_t Cursor = 0;
	if( !JsonMemberCursor( Object, Key, Cursor ) || Cursor >= Object.size() || Object[Cursor++] != '[' )
		return false;
	FLOAT Components[3];
	for( INT Index=0; Index<ARRAY_COUNT(Components); ++Index )
	{
		SkipJsonWhitespace( Object, Cursor );
		char* End = NULL;
		errno = 0;
		const double Value = strtod( Object.c_str() + Cursor, &End );
		if( End == Object.c_str() + Cursor || errno == ERANGE )
			return false;
		Components[Index] = (FLOAT)Value;
		Cursor = (size_t)(End - Object.c_str());
		SkipJsonWhitespace( Object, Cursor );
		if( Index + 1 < ARRAY_COUNT(Components) )
		{
			if( Cursor >= Object.size() || Object[Cursor++] != ',' )
				return false;
		}
	}
	SkipJsonWhitespace( Object, Cursor );
	if( Cursor >= Object.size() || Object[Cursor] != ']' )
		return false;
	Out = FVector(Components[0],Components[1],Components[2]);
	return true;
}


bool ReadJsonDocument( const char* Filename, std::string& Out )
{
	Out.clear();
	FILE* File = Filename ? fopen( Filename, "rb" ) : NULL;
	if( !File )
		return false;
	char Buffer[4096];
	while( const size_t Read = fread( Buffer, 1, sizeof(Buffer), File ) )
		Out.append( Buffer, Read );
	return fclose( File ) == 0;
}

struct WorldCollisionGateRole
{
	std::string Class;
	UBOOL Transient = 0;
	FLOAT Radius = 0.0f;
	FLOAT Height = 0.0f;
	UBOOL CollideActors = 0;
	UBOOL CollideWorld = 0;
	UBOOL BlockActors = 0;
	UBOOL BlockPlayers = 0;
};

struct WorldCollisionGateScenario
{
	std::string Fixture;
	std::string MapRelative;
	std::string MapSha256;
	FLOAT FixedDeltaSeconds = 0.0f;
	INT RequestedTicks = 0;
	WorldCollisionGateRole Probe;
	WorldCollisionGateRole Blocker;
	WorldCollisionGateRole TouchTarget;
	WorldCollisionGateRole ZoneTrigger;
	WorldCollisionGateRole SpawnAnchor;
	FVector SafeOriginOffset;
	FVector TouchPlacementOffset;
	FVector BumpPlacementOffset;
	FVector TouchDelta;
	FLOAT BlockClearance = 0.0f;
	FVector BlockDelta;
	FVector CarryDelta;
};

bool ParseWorldCollisionGateSelector( const std::string& Selectors, const char* Name, WorldCollisionGateRole& Out )
{
	std::string Selector;
	INT Cardinality = 0;
	return JsonObjectMember( Selectors, Name, Selector )
		&& JsonStringMember( Selector, "class", Out.Class )
		&& JsonIntMember( Selector, "cardinality", Cardinality )
		&& Cardinality == 1;
}

bool ParseWorldCollisionGateTransient( const std::string& Transients, const char* Name, WorldCollisionGateRole& Out )
{
	std::string Transient;
	std::string Collision;
	if( !JsonObjectMember( Transients, Name, Transient )
		|| !JsonStringMember( Transient, "class", Out.Class )
		|| !JsonObjectMember( Transient, "collision", Collision ) )
		return false;
	Out.Transient = 1;
	return JsonFloatMember( Collision, "radius", Out.Radius )
		&& JsonFloatMember( Collision, "height", Out.Height )
		&& JsonBoolMember( Collision, "collide_actors", Out.CollideActors )
		&& JsonBoolMember( Collision, "collide_world", Out.CollideWorld )
		&& JsonBoolMember( Collision, "block_actors", Out.BlockActors )
		&& JsonBoolMember( Collision, "block_players", Out.BlockPlayers );
}
bool JsonObjectArrayMember( const std::string& Object, const char* Key, std::vector<std::string>& Out )
{
	Out.clear();
	size_t Cursor = 0;
	if( !JsonMemberCursor( Object, Key, Cursor ) || Cursor >= Object.size() || Object[Cursor++] != '[' )
		return false;
	for( ;; )
	{
		SkipJsonWhitespace( Object, Cursor );
		if( Cursor >= Object.size() )
			return false;
		if( Object[Cursor] == ']' )
			return true;
		size_t End = 0;
		if( !JsonObjectEnd( Object, Cursor, End ) )
			return false;
		Out.push_back( Object.substr(Cursor,End-Cursor) );
		Cursor = End;
		SkipJsonWhitespace( Object, Cursor );
		if( Cursor < Object.size() && Object[Cursor] == ',' )
			++Cursor;
	}
}

bool JsonStringEquals( const std::string& Object, const char* Key, const char* Expected )
{
	std::string Value;
	return JsonStringMember( Object, Key, Value ) && Value == Expected;
}

bool ValidateWorldCollisionGateOperations( const std::string& Document, WorldCollisionGateScenario& Out )
{
	static const char* Ids[] =
	{
		"resolve_safe_origin", "touch_sweep", "prepare_block",
		"block_sweep", "far_move", "set_base_and_carry",
	};
	static const char* Operations[] =
	{
		"FindSpot", "MoveActor", "FarMoveActor",
		"MoveActor", "FarMoveActor", "MoveActor",
	};
	std::vector<std::string> Values;
	if( !JsonObjectArrayMember( Document, "operations", Values ) || Values.size() != ARRAY_COUNT(Ids) )
		return false;
	for( size_t Index=0; Index<Values.size(); ++Index )
		if( !JsonStringEquals( Values[Index], "id", Ids[Index] )
			|| !JsonStringEquals( Values[Index], "op", Operations[Index] ) )
			return false;

	std::string Origin;
	std::string TouchPlacement;
	std::string BlockPlacement;
	std::string Prelude;
	UBOOL FindSpot = 0;
	return JsonStringEquals( Values[0], "actor", "probe" )
		&& JsonObjectMember( Values[0], "origin", Origin )
		&& JsonStringEquals( Origin, "relative_to", "spawn_anchor" )
		&& JsonVectorMember( Origin, "offset", Out.SafeOriginOffset )
		&& JsonStringEquals( Values[1], "actor", "probe" )
		&& JsonObjectMember( Values[1], "placement", TouchPlacement )
		&& JsonStringEquals( TouchPlacement, "actor", "touch_target" )
		&& JsonStringEquals( TouchPlacement, "relative_to", "probe" )
		&& JsonVectorMember( TouchPlacement, "offset", Out.TouchPlacementOffset )
		&& JsonVectorMember( Values[1], "delta", Out.TouchDelta )
		&& JsonStringEquals( Values[2], "actor", "probe" )
		&& JsonObjectMember( Values[2], "placement", BlockPlacement )
		&& JsonStringEquals( BlockPlacement, "relative_to", "blocker" )
		&& JsonStringEquals( BlockPlacement, "approach", "negative_x" )
		&& JsonFloatMember( BlockPlacement, "clearance", Out.BlockClearance )
		&& JsonBoolMember( BlockPlacement, "find_spot", FindSpot ) && FindSpot
		&& JsonStringEquals( Values[3], "actor", "probe" )
		&& JsonVectorMember( Values[3], "delta", Out.BlockDelta )
		&& JsonStringEquals( Values[4], "actor", "probe" )
		&& JsonStringEquals( Values[4], "destination", "safe_origin" )
		&& JsonBoolMember( Values[4], "find_spot", FindSpot ) && FindSpot
		&& JsonStringEquals( Values[5], "actor", "blocker" )
		&& JsonObjectMember( Values[5], "prelude", Prelude )
		&& JsonStringEquals( Prelude, "op", "SetBase" )
		&& JsonStringEquals( Prelude, "actor", "probe" )
		&& JsonStringEquals( Prelude, "base", "blocker" )
		&& JsonVectorMember( Values[5], "delta", Out.CarryDelta );
}


bool ParseWorldCollisionGateScenario( const char* Filename, WorldCollisionGateScenario& Out )
{
	std::string Document;
	std::string Map;
	std::string Selectors;
	std::string Transients;
	INT Version = 0;
	if( !ReadJsonDocument( Filename, Document )
		|| !JsonIntMember( Document, "version", Version )
		|| Version != 1
		|| !JsonStringMember( Document, "fixture", Out.Fixture )
		|| !JsonObjectMember( Document, "map", Map )
		|| !JsonStringMember( Map, "relative", Out.MapRelative )
		|| !JsonStringMember( Map, "sha256", Out.MapSha256 )
		|| !JsonFloatMember( Document, "fixed_dt", Out.FixedDeltaSeconds )
		|| !JsonIntMember( Document, "requested_ticks", Out.RequestedTicks )
		|| !JsonObjectMember( Document, "selectors", Selectors )
		|| !JsonObjectMember( Document, "transients", Transients )
		|| !ParseWorldCollisionGateSelector( Selectors, "blocker", Out.Blocker )
		|| !ParseWorldCollisionGateSelector( Selectors, "zone_trigger", Out.ZoneTrigger )
		|| !ParseWorldCollisionGateSelector( Selectors, "spawn_anchor", Out.SpawnAnchor )
		|| !ParseWorldCollisionGateTransient( Transients, "probe", Out.Probe )
		|| !ParseWorldCollisionGateTransient( Transients, "touch_target", Out.TouchTarget )
		|| !ValidateWorldCollisionGateOperations( Document, Out ) )
		return false;
	return Out.Fixture == "TriggerTest2WorldCollision"
		&& Out.FixedDeltaSeconds > 0.0f
		&& Out.RequestedTicks >= 0;
}

bool ValidateWorldTouchGateOperations( const std::string& Document, WorldCollisionGateScenario& Out )
{
	static const char* Ids[] = { "resolve_safe_origin", "touch_sweep" };
	static const char* Operations[] = { "FindSpot", "MoveActor" };
	static const char* SafeKeys[] = { "id", "op", "actor", "origin" };
	static const char* OriginKeys[] = { "relative_to", "offset" };
	static const char* TouchKeys[] = { "id", "op", "actor", "placement", "delta" };
	static const char* PlacementKeys[] = { "actor", "relative_to", "offset" };
	std::vector<std::string> Values;
	std::string Origin;
	std::string Placement;
	if( !JsonObjectArrayMember( Document, "operations", Values ) || Values.size() != ARRAY_COUNT(Ids) )
		return false;
	if( !JsonObjectHasExactKeys( Values[0], SafeKeys, ARRAY_COUNT(SafeKeys) )
		|| !JsonObjectHasExactKeys( Values[1], TouchKeys, ARRAY_COUNT(TouchKeys) )
		|| !JsonStringEquals( Values[0], "id", Ids[0] )
		|| !JsonStringEquals( Values[0], "op", Operations[0] )
		|| !JsonStringEquals( Values[0], "actor", "probe" )
		|| !JsonObjectMember( Values[0], "origin", Origin )
		|| !JsonObjectHasExactKeys( Origin, OriginKeys, ARRAY_COUNT(OriginKeys) )
		|| !JsonStringEquals( Origin, "relative_to", "spawn_anchor" )
		|| !JsonVectorMember( Origin, "offset", Out.SafeOriginOffset )
		|| !JsonStringEquals( Values[1], "id", Ids[1] )
		|| !JsonStringEquals( Values[1], "op", Operations[1] )
		|| !JsonStringEquals( Values[1], "actor", "probe" )
		|| !JsonObjectMember( Values[1], "placement", Placement )
		|| !JsonObjectHasExactKeys( Placement, PlacementKeys, ARRAY_COUNT(PlacementKeys) )
		|| !JsonStringEquals( Placement, "actor", "touch_target" )
		|| !JsonStringEquals( Placement, "relative_to", "probe" )
		|| !JsonVectorMember( Placement, "offset", Out.TouchPlacementOffset )
		|| !JsonVectorMember( Values[1], "delta", Out.TouchDelta ) )
		return false;
	return true;
}

bool ParseWorldTouchGateScenario( const char* Filename, WorldCollisionGateScenario& Out )
{
	static const char* RootKeys[] =
	{
		"version", "fixture", "map", "fixed_dt", "requested_ticks",
		"selectors", "transients", "operations", "random_calls",
	};
	static const char* MapKeys[] = { "relative", "sha256" };
	static const char* SelectorKeys[] = { "spawn_anchor" };
	static const char* SelectorKeysValue[] = { "class", "cardinality" };
	static const char* TransientKeys[] = { "probe", "touch_target" };
	static const char* TransientKeysValue[] = { "class", "collision" };
	static const char* CollisionKeys[] =
	{
		"radius", "height", "collide_actors", "collide_world", "block_actors", "block_players",
	};
	std::string Document;
	std::string Map;
	std::string Selectors;
	std::string SpawnAnchor;
	std::string Transients;
	std::string Probe;
	std::string ProbeCollision;
	std::string TouchTarget;
	std::string TouchCollision;
	std::vector<std::string> RandomCalls;
	INT Version = 0;
	if( !ReadJsonDocument( Filename, Document )
		|| !JsonObjectHasExactKeys( Document, RootKeys, ARRAY_COUNT(RootKeys) )
		|| !JsonIntMember( Document, "version", Version ) || Version != 1
		|| !JsonStringEquals( Document, "fixture", "TriggerTest2WorldTouch" )
		|| !JsonObjectMember( Document, "map", Map )
		|| !JsonObjectHasExactKeys( Map, MapKeys, ARRAY_COUNT(MapKeys) )
		|| !JsonStringMember( Map, "relative", Out.MapRelative )
		|| !JsonStringMember( Map, "sha256", Out.MapSha256 )
		|| Out.MapRelative != "Maps/Studies/TriggerTest2.unr"
		|| Out.MapSha256 != "fffa53fee540d756b9ff6bbc872c64d3bc81eca47f366f3d4071196fae2546fc"
		|| !JsonFloatMember( Document, "fixed_dt", Out.FixedDeltaSeconds )
		|| Out.FixedDeltaSeconds != 0.016666667f
		|| !JsonIntMember( Document, "requested_ticks", Out.RequestedTicks ) || Out.RequestedTicks != 1
		|| !JsonObjectMember( Document, "selectors", Selectors )
		|| !JsonObjectHasExactKeys( Selectors, SelectorKeys, ARRAY_COUNT(SelectorKeys) )
		|| !JsonObjectMember( Selectors, "spawn_anchor", SpawnAnchor )
		|| !JsonObjectHasExactKeys( SpawnAnchor, SelectorKeysValue, ARRAY_COUNT(SelectorKeysValue) )
		|| !ParseWorldCollisionGateSelector( Selectors, "spawn_anchor", Out.SpawnAnchor )
		|| Out.SpawnAnchor.Class != "Engine.PlayerStart"
		|| !JsonObjectMember( Document, "transients", Transients )
		|| !JsonObjectHasExactKeys( Transients, TransientKeys, ARRAY_COUNT(TransientKeys) )
		|| !JsonObjectMember( Transients, "probe", Probe )
		|| !JsonObjectHasExactKeys( Probe, TransientKeysValue, ARRAY_COUNT(TransientKeysValue) )
		|| !JsonObjectMember( Probe, "collision", ProbeCollision )
		|| !JsonObjectHasExactKeys( ProbeCollision, CollisionKeys, ARRAY_COUNT(CollisionKeys) )
		|| !JsonObjectMember( Transients, "touch_target", TouchTarget )
		|| !JsonObjectHasExactKeys( TouchTarget, TransientKeysValue, ARRAY_COUNT(TransientKeysValue) )
		|| !JsonObjectMember( TouchTarget, "collision", TouchCollision )
		|| !JsonObjectHasExactKeys( TouchCollision, CollisionKeys, ARRAY_COUNT(CollisionKeys) )
		|| !ParseWorldCollisionGateTransient( Transients, "probe", Out.Probe )
		|| !ParseWorldCollisionGateTransient( Transients, "touch_target", Out.TouchTarget )
		|| !JsonObjectArrayMember( Document, "random_calls", RandomCalls ) || !RandomCalls.empty()
		|| !ValidateWorldTouchGateOperations( Document, Out ) )
		return false;
	Out.Fixture = "TriggerTest2WorldTouch";
	return true;
}
bool ValidateWorldBumpGateOperations( const std::string& Document, WorldCollisionGateScenario& Out )
{
	static const char* SafeKeys[] = { "id", "op", "actor", "origin" };
	static const char* OriginKeys[] = { "relative_to", "offset" };
	static const char* BlockKeys[] = { "id", "op", "actor", "placement", "delta" };
	static const char* PlacementKeys[] = { "actor", "relative_to", "offset" };
	std::vector<std::string> Values;
	std::string Origin;
	std::string Placement;
	if( !JsonObjectArrayMember( Document, "operations", Values ) || Values.size() != 2
		|| !JsonObjectHasExactKeys( Values[0], SafeKeys, ARRAY_COUNT(SafeKeys) )
		|| !JsonObjectHasExactKeys( Values[1], BlockKeys, ARRAY_COUNT(BlockKeys) )
		|| !JsonStringEquals( Values[0], "id", "resolve_safe_origin" )
		|| !JsonStringEquals( Values[0], "op", "FindSpot" )
		|| !JsonStringEquals( Values[0], "actor", "probe" )
		|| !JsonObjectMember( Values[0], "origin", Origin )
		|| !JsonObjectHasExactKeys( Origin, OriginKeys, ARRAY_COUNT(OriginKeys) )
		|| !JsonStringEquals( Origin, "relative_to", "spawn_anchor" )
		|| !JsonVectorMember( Origin, "offset", Out.SafeOriginOffset )
		|| !JsonStringEquals( Values[1], "id", "block_sweep" )
		|| !JsonStringEquals( Values[1], "op", "MoveActor" )
		|| !JsonStringEquals( Values[1], "actor", "probe" )
		|| !JsonObjectMember( Values[1], "placement", Placement )
		|| !JsonObjectHasExactKeys( Placement, PlacementKeys, ARRAY_COUNT(PlacementKeys) )
		|| !JsonStringEquals( Placement, "actor", "blocker" )
		|| !JsonStringEquals( Placement, "relative_to", "safe_origin" )
		|| !JsonVectorMember( Placement, "offset", Out.BumpPlacementOffset )
		|| !JsonVectorMember( Values[1], "delta", Out.BlockDelta )
		|| Out.BlockDelta.X != 256.0f || Out.BlockDelta.Y != 0.0f || Out.BlockDelta.Z != 0.0f )
		return false;
	return true;
}

bool IsWorldBumpGateTransient( const WorldCollisionGateRole& Role )
{
	return Role.Class == "Engine.Actor"
		&& Role.Radius == 16.0f && Role.Height == 24.0f
		&& Role.CollideActors && Role.CollideWorld && Role.BlockActors && Role.BlockPlayers;
}

bool ParseWorldBumpGateScenario( const char* Filename, WorldCollisionGateScenario& Out )
{
	static const char* RootKeys[] =
	{
		"version", "fixture", "map", "fixed_dt", "requested_ticks",
		"selectors", "transients", "operations", "random_calls",
	};
	static const char* MapKeys[] = { "relative", "sha256" };
	static const char* SelectorKeys[] = { "spawn_anchor" };
	static const char* SelectorKeysValue[] = { "class", "cardinality" };
	static const char* TransientKeys[] = { "probe", "blocker" };
	static const char* TransientKeysValue[] = { "class", "collision" };
	static const char* CollisionKeys[] =
	{
		"radius", "height", "collide_actors", "collide_world", "block_actors", "block_players",
	};
	std::string Document;
	std::string Map;
	std::string Selectors;
	std::string SpawnAnchor;
	std::string Transients;
	std::string Probe;
	std::string ProbeCollision;
	std::string Blocker;
	std::string BlockerCollision;
	std::vector<std::string> RandomCalls;
	INT Version = 0;
	if( !ReadJsonDocument( Filename, Document )
		|| !JsonObjectHasExactKeys( Document, RootKeys, ARRAY_COUNT(RootKeys) )
		|| !JsonIntMember( Document, "version", Version ) || Version != 1
		|| !JsonStringEquals( Document, "fixture", "TriggerTest2WorldBump" )
		|| !JsonObjectMember( Document, "map", Map )
		|| !JsonObjectHasExactKeys( Map, MapKeys, ARRAY_COUNT(MapKeys) )
		|| !JsonStringMember( Map, "relative", Out.MapRelative )
		|| !JsonStringMember( Map, "sha256", Out.MapSha256 )
		|| Out.MapRelative != "Maps/Studies/TriggerTest2.unr"
		|| Out.MapSha256 != "fffa53fee540d756b9ff6bbc872c64d3bc81eca47f366f3d4071196fae2546fc"
		|| !JsonFloatMember( Document, "fixed_dt", Out.FixedDeltaSeconds )
		|| Out.FixedDeltaSeconds != 0.016666667f
		|| !JsonIntMember( Document, "requested_ticks", Out.RequestedTicks ) || Out.RequestedTicks != 1
		|| !JsonObjectMember( Document, "selectors", Selectors )
		|| !JsonObjectHasExactKeys( Selectors, SelectorKeys, ARRAY_COUNT(SelectorKeys) )
		|| !JsonObjectMember( Selectors, "spawn_anchor", SpawnAnchor )
		|| !JsonObjectHasExactKeys( SpawnAnchor, SelectorKeysValue, ARRAY_COUNT(SelectorKeysValue) )
		|| !ParseWorldCollisionGateSelector( Selectors, "spawn_anchor", Out.SpawnAnchor )
		|| Out.SpawnAnchor.Class != "Engine.PlayerStart"
		|| !JsonObjectMember( Document, "transients", Transients )
		|| !JsonObjectHasExactKeys( Transients, TransientKeys, ARRAY_COUNT(TransientKeys) )
		|| !JsonObjectMember( Transients, "probe", Probe )
		|| !JsonObjectHasExactKeys( Probe, TransientKeysValue, ARRAY_COUNT(TransientKeysValue) )
		|| !JsonObjectMember( Probe, "collision", ProbeCollision )
		|| !JsonObjectHasExactKeys( ProbeCollision, CollisionKeys, ARRAY_COUNT(CollisionKeys) )
		|| !JsonObjectMember( Transients, "blocker", Blocker )
		|| !JsonObjectHasExactKeys( Blocker, TransientKeysValue, ARRAY_COUNT(TransientKeysValue) )
		|| !JsonObjectMember( Blocker, "collision", BlockerCollision )
		|| !JsonObjectHasExactKeys( BlockerCollision, CollisionKeys, ARRAY_COUNT(CollisionKeys) )
		|| !ParseWorldCollisionGateTransient( Transients, "probe", Out.Probe )
		|| !ParseWorldCollisionGateTransient( Transients, "blocker", Out.Blocker )
		|| !IsWorldBumpGateTransient( Out.Probe )
		|| !IsWorldBumpGateTransient( Out.Blocker )
		|| !JsonObjectArrayMember( Document, "random_calls", RandomCalls ) || !RandomCalls.empty()
		|| !ValidateWorldBumpGateOperations( Document, Out ) )
		return false;
	Out.Fixture = "TriggerTest2WorldBump";
	return true;
}



class WorldCollisionGateReporter : public FActorMovementObserver
{
	struct Step
	{
		std::string Id;
		std::string Request;
		UBOOL Moved;
		UBOOL HasHitTime;
		FLOAT HitTime;
		UBOOL Blocked;
		AActor* HitActor;
		std::string ExtraResult;
		std::vector<std::string> Events;
		std::string Actors;
	};
	struct SelectorDetail
	{
		std::string Id;
		std::string Class;
		std::string Path;
		INT MatchCount;
	};


public:
	enum EProtocol
	{
		ProtocolFull,
		ProtocolTouch,
		ProtocolBump,
	};

	static bool IsEnabled()
	{
		const char* Path = getenv( "HP2_WORLD_COLLISION_GATE_TRACE" );
		return Path && Path[0];
	}
	static bool IsTouchEnabled()
	{
		const char* Path = getenv( "HP2_WORLD_TOUCH_GATE_TRACE" );
		return Path && Path[0];
	}
	static bool IsBumpEnabled()
	{
		const char* Path = getenv( "HP2_WORLD_BUMP_GATE_TRACE" );
		return Path && Path[0];
	}

	WorldCollisionGateReporter( EProtocol InProtocol=ProtocolFull )
		: TouchOnly( InProtocol == ProtocolTouch )
		, BumpOnly( InProtocol == ProtocolBump )
		, ReportPath( getenv(
			InProtocol == ProtocolBump ? "HP2_WORLD_BUMP_GATE_TRACE"
			: InProtocol == ProtocolTouch ? "HP2_WORLD_TOUCH_GATE_TRACE"
			: "HP2_WORLD_COLLISION_GATE_TRACE" ) )
		, ScenarioReady(
			InProtocol == ProtocolBump ? ParseWorldBumpGateScenario( getenv( "HP2_WORLD_BUMP_GATE_SCENARIO" ), Scenario )
			: InProtocol == ProtocolTouch ? ParseWorldTouchGateScenario( getenv( "HP2_WORLD_TOUCH_GATE_SCENARIO" ), Scenario )
			: ParseWorldCollisionGateScenario( getenv( "HP2_WORLD_COLLISION_GATE_SCENARIO" ), Scenario ) )
		, ActiveEvents( NULL )
		, Probe( NULL )
		, Blocker( NULL )
		, TouchTarget( NULL )
		, ZoneTrigger( NULL )
		, SpawnAnchor( NULL )
	{
		GActorMovementObserver = this;
	}

	virtual ~WorldCollisionGateReporter()
	{
		if( GActorMovementObserver == this )
			GActorMovementObserver = NULL;
	}

	void Run( UEngine* Engine )
	{
		if( !ScenarioReady )
		{
			ScenarioError = "invalid_input";
			fprintf( stderr, "<%s> scenario_error=invalid_input\n", GateMarker() );
			return;
		}
		UGameEngine* GameEngine = Cast<UGameEngine>( Engine );
		ULevel* Level = GameEngine ? GameEngine->GLevel : NULL;
		if( !Level )
		{
			ScenarioError = "no_level";
			fprintf( stderr, "<%s> scenario_error=no_level\n", GateMarker() );
			return;
		}
		if( !ResolveMapRoles(Level) )
		{
			ScenarioError = "role_resolution";
			fprintf( stderr, "<%s> scenario_error=role_resolution\n", GateMarker() );
			return;
		}
		if( !SpawnTransientRoles(Level) )
		{
			ScenarioError = "transient_spawn";
			fprintf( stderr, "<%s> scenario_error=transient_spawn\n", GateMarker() );
			return;
		}
		if( BumpOnly )
		{
			RunBump( Level );
			return;
		}

		FVector SafeOrigin = SpawnAnchor->Location + Scenario.SafeOriginOffset;
		const FVector RequestedOrigin = SafeOrigin;
		BeginEvents();
		const UBOOL FoundSafeOrigin = Level->FindSpot( Probe->GetCylinderExtent(), SafeOrigin, 1, 0 );
		const UBOOL SafeOriginMoved = FoundSafeOrigin && Level->FarMoveActor( Probe, SafeOrigin, 0, 0 );
		EndStep(
			"resolve_safe_origin",
			std::string("{\"operation\":\"FindSpot\",\"destination\":") + VectorJson(RequestedOrigin) + "}",
			SafeOriginMoved,
			0,
			1.0f,
			!SafeOriginMoved,
			NULL,
			"" );
		if( !SafeOriginMoved )
		{
			ScenarioError = "safe_origin";
			return;
		}
		ConfigureTransient( Probe, Scenario.Probe );
		ConfigureTransient( TouchTarget, Scenario.TouchTarget );


		// Normal FarMove placement causes no touch notifications. It creates the
		// nonblocking overlap candidate used by the following generic MoveActor.
		if( !Level->FarMoveActor( TouchTarget, SafeOrigin + Scenario.TouchPlacementOffset, 0, 0 ) )
		{
			ScenarioError = "touch_target_placement";
			return;
		}
		Probe->bJustTeleported = 0;
		BeginEvents();
		FCheckResult TouchHit(1.0f);
		const UBOOL TouchMoved = Level->MoveActor( Probe, Scenario.TouchDelta, Probe->Rotation, TouchHit, 0, 0, 0, 0 );
		EndStep(
			"touch_sweep",
			std::string("{\"operation\":\"MoveActor\",\"delta\":") + VectorJson(Scenario.TouchDelta) + "}",
			TouchMoved,
			1,
			TouchHit.Time,
			TouchHit.Time < 1.0f,
			TouchHit.Actor,
			"" );

		if( TouchOnly )
			return;

		// Keep later movement independent from the intentional persistent touch.
		Probe->EndTouch( TouchTarget, 0 );
		TouchTarget->SetCollision( 0, 0, 0 );

		FBox BlockerBox = Blocker->GetPrimitive()->GetCollisionBoundingBox( Blocker, 1 );
		FVector PrepareDestination(
			BlockerBox.Min.X - Probe->CollisionRadius - Scenario.BlockClearance,
			(BlockerBox.Min.Y + BlockerBox.Max.Y) * 0.5f,
			(BlockerBox.Min.Z + BlockerBox.Max.Z) * 0.5f );
		const FVector RequestedPrepareDestination = PrepareDestination;
		// FindSpot performs the required world and actor admission test. Reusing
		// its resolved location with bNoCheck preserves that one checked result
		// instead of making FarMoveActor perturb/reject it a second time.
		const UBOOL FoundPrepareDestination = Level->FindSpot( Probe->GetCylinderExtent(), PrepareDestination, 1, 0 );
		BeginEvents();
		const UBOOL FarMovePrepareDestination = FoundPrepareDestination
			&& Level->FarMoveActor( Probe, PrepareDestination, 0, 1 );
		const UBOOL PrepareMoved = FarMovePrepareDestination;
		EndStep(
			"prepare_block",
			std::string("{\"operation\":\"FarMoveActor\",\"destination\":") + VectorJson(PrepareDestination) + "}",
			PrepareMoved,
			0,
			0.0f,
			!PrepareMoved,
			NULL,
			"" );
		if( !PrepareMoved )
		{
			ScenarioError = "prepare_block";
			fprintf(
				stderr,
				"<HP2_WORLD_COLLISION_GATE> scenario_error=prepare_block find_spot=%d far_move=%d requested=(%.4f,%.4f,%.4f) resolved=(%.4f,%.4f,%.4f)\n",
				(INT)FoundPrepareDestination,
				(INT)FarMovePrepareDestination,
				RequestedPrepareDestination.X,
				RequestedPrepareDestination.Y,
				RequestedPrepareDestination.Z,
				PrepareDestination.X,
				PrepareDestination.Y,
				PrepareDestination.Z );
			return;
		}
		Probe->bJustTeleported = 0;
		BeginEvents();
		FCheckResult BlockHit(1.0f);
		const UBOOL BlockMoved = Level->MoveActor( Probe, Scenario.BlockDelta, Probe->Rotation, BlockHit, 0, 0, 0, 0 );
		EndStep(
			"block_sweep",
			std::string("{\"operation\":\"MoveActor\",\"delta\":") + VectorJson(Scenario.BlockDelta) + "}",
			BlockMoved,
			1,
			BlockHit.Time,
			BlockHit.Time < 1.0f,
			BlockHit.Actor,
			"" );
		if( BlockHit.Actor != Blocker || BlockHit.Time >= 1.0f )
		{
			ScenarioError = "block_sweep";
			return;
		}

		BeginEvents();
		const UBOOL FarMoved = Level->FarMoveActor( Probe, SafeOrigin, 0, 0 );
		EndStep(
			"far_move",
			std::string("{\"operation\":\"FarMoveActor\",\"destination\":") + VectorJson(SafeOrigin) + "}",
			FarMoved,
			0,
			0.0f,
			!FarMoved,
			NULL,
			"" );
		if( !FarMoved )
		{
			ScenarioError = "far_move";
			return;
		}

		Probe->SetBase( Blocker );
		BeginEvents();
		FCheckResult CarryHit(1.0f);
		const UBOOL CarryMoved = Level->MoveActor( Blocker, Scenario.CarryDelta, Blocker->Rotation, CarryHit, 0, 0, 0, 0 );
		EndStep(
			"set_base_and_carry",
			std::string("{\"operation\":\"MoveActor\",\"delta\":") + VectorJson(Scenario.CarryDelta) + "}",
			CarryMoved,
			1,
			CarryHit.Time,
			CarryHit.Time < 1.0f,
			CarryHit.Actor,
			"" );
	}

	void RunBump( ULevel* Level )
	{
		FVector SafeOrigin = SpawnAnchor->Location + Scenario.SafeOriginOffset;
		const FVector RequestedOrigin = SafeOrigin;
		BeginEvents();
		const UBOOL FoundSafeOrigin = Level->FindSpot( Probe->GetCylinderExtent(), SafeOrigin, 1, 0 );
		const UBOOL SafeOriginMoved = FoundSafeOrigin && Level->FarMoveActor( Probe, SafeOrigin, 0, 0 );
		EndStep(
			"resolve_safe_origin",
			std::string("{\"operation\":\"FindSpot\",\"destination\":") + VectorJson(RequestedOrigin) + "}",
			SafeOriginMoved,
			0,
			1.0f,
			!SafeOriginMoved,
			NULL,
			"" );
		if( !SafeOriginMoved )
		{
			ScenarioError = "safe_origin";
			return;
		}

		ConfigureTransient( Probe, Scenario.Probe );
		ConfigureTransient( Blocker, Scenario.Blocker );
		if( !Level->FarMoveActor( Blocker, SafeOrigin + Scenario.BumpPlacementOffset, 0, 0 ) )
		{
			ScenarioError = "blocker_placement";
			return;
		}

		// FarMove is setup only. The measured sweep begins with neither a
		// teleport flag nor setup callbacks carried into its event ledger.
		Probe->bJustTeleported = 0;
		ClearEvents();
		BeginEvents();
		FCheckResult BlockHit(1.0f);
		const UBOOL BlockMoved = Level->MoveActor( Probe, Scenario.BlockDelta, Probe->Rotation, BlockHit, 0, 0, 0, 0 );
		EndStep(
			"block_sweep",
			std::string("{\"operation\":\"MoveActor\",\"delta\":") + VectorJson(Scenario.BlockDelta) + "}",
			BlockMoved,
			1,
			BlockHit.Time,
			BlockHit.Time < 1.0f,
			BlockHit.Actor,
			"" );
		const Step& BlockStep = Steps.back();
		if( BlockHit.Actor != Blocker
			|| BlockHit.Time <= 0.0f || BlockHit.Time >= 1.0f
			|| !HasExactBumpEvents(BlockStep.Events)
			|| HasTouchingRelation(Probe, Blocker)
			|| HasTouchingRelation(Blocker, Probe) )
			ScenarioError = "block_sweep";
	}


	void Flush()
	{
		const std::string TemporaryPath = ReportPath + ".tmp";
		FILE* Report = fopen( TemporaryPath.c_str(), "wb" );
		if( !Report )
		{
			fprintf( stderr, "<%s> report_error=unwritable path=%s\n", GateMarker(), ReportPath.c_str() );
			return;
		}
		const std::string Document = Serialize();
		const size_t Written = fwrite( Document.data(), 1, Document.size(), Report );
		const int CloseResult = fclose( Report );
		if( Written != Document.size() || CloseResult != 0 || rename( TemporaryPath.c_str(), ReportPath.c_str() ) != 0 )
		{
			remove( TemporaryPath.c_str() );
			fprintf( stderr, "<%s> report_error=write_failed path=%s\n", GateMarker(), ReportPath.c_str() );
			return;
		}
		fprintf( stdout, "<%s> report=%s steps=%d\n", GateMarker(), ReportPath.c_str(), (INT)Steps.size() );
	}

	virtual void OnActorBump( AActor* Recipient, AActor* Other )
	{
		RecordEvent( Recipient, "Bump", Other );
	}

	virtual void OnActorTouch( AActor* Recipient, AActor* Other )
	{
		RecordEvent( Recipient, "Touch", Other );
	}

	virtual void OnActorUnTouch( AActor* Recipient, AActor* Other )
	{
		RecordEvent( Recipient, "UnTouch", Other );
	}

private:
	const char* GateMarker() const
	{
		return BumpOnly ? "HP2_WORLD_BUMP_GATE"
			: "HP2_WORLD_COLLISION_GATE";
	}

	static std::string Text( const TCHAR* Value )
	{
		const char* Ansi = Value ? appToAnsi(Value) : "";
		return Ansi ? Ansi : "";
	}

	static std::string JsonString( const std::string& Value )
	{
		std::string Json = "\"";
		for( std::string::const_iterator It=Value.begin(); It!=Value.end(); ++It )
		{
			switch( (unsigned char)*It )
			{
			case '"': Json += "\\\""; break;
			case '\\': Json += "\\\\"; break;
			case '\b': Json += "\\b"; break;
			case '\f': Json += "\\f"; break;
			case '\n': Json += "\\n"; break;
			case '\r': Json += "\\r"; break;
			case '\t': Json += "\\t"; break;
			default: Json += *It; break;
			}
		}
		return Json + "\"";
	}

	static std::string VectorJson( const FVector& Value )
	{
		char Buffer[128];
		snprintf( Buffer, sizeof(Buffer), "[%.4f,%.4f,%.4f]", Value.X, Value.Y, Value.Z );
		return Buffer;
	}

	static std::string RotationJson( const FRotator& Value )
	{
		return std::string("[") + std::to_string(Value.Pitch) + "," + std::to_string(Value.Yaw) + "," + std::to_string(Value.Roll) + "]";
	}

	static std::string ClassPath( UClass* Class )
	{
		return Class ? Text(Class->GetPathName()) : "";
	}

	static UClass* LoadActorClass( const std::string& Name )
	{
		TCHAR ClassName[512];
		if( !appFromUtf8InPlace( ClassName, Name.c_str(), ARRAY_COUNT(ClassName) ) )
			return NULL;
		return UObject::StaticLoadClass( AActor::StaticClass(), NULL, ClassName, NULL, LOAD_NoFail, NULL );
	}

	// Engine.Actor is intentionally abstract. The scenario retains that logical
	// contract, while the C++ runtime uses the concrete, non-static native
	// Engine.Effects class solely as its storage/collision carrier.
	static UClass* ConcreteActorClass( const WorldCollisionGateRole& Role )
	{
		UClass* Requested = LoadActorClass( Role.Class );
		if( !Requested )
			return NULL;
		if( !(Requested->ClassFlags & CLASS_Abstract) )
			return Requested;
		UClass* Carrier = LoadActorClass( "Engine.Effects" );
		return Carrier && !(Carrier->ClassFlags & CLASS_Abstract) ? Carrier : NULL;
	}

	AActor* FindUniqueClassActor( ULevel* Level, const char* Id, const std::string& ClassName )
	{
		SelectorDetail Detail;
		Detail.Id = Id;
		Detail.Class = ClassName;
		Detail.MatchCount = 0;
		AActor* Found = NULL;
		for( INT Slot=0; Slot<Level->Actors.Num(); ++Slot )
		{
			AActor* Candidate = Level->Actors(Slot);
			if( Candidate && !Candidate->bDeleteMe && ClassPath(Candidate->GetClass()) == ClassName )
			{
				++Detail.MatchCount;
				Found = Candidate;
			}
		}
		Detail.Path = Detail.MatchCount == 1 ? Text(Found->GetPathName()) : "";
		SelectorDetails.push_back( Detail );
		return Detail.MatchCount == 1 ? Found : NULL;
	}

	bool ResolveMapRoles( ULevel* Level )
	{
		SelectorDetails.clear();
		SpawnAnchor = FindUniqueClassActor( Level, "spawn_anchor", Scenario.SpawnAnchor.Class );
		if( TouchOnly || BumpOnly )
			return SpawnAnchor != NULL;
		Blocker = FindUniqueClassActor( Level, "blocker", Scenario.Blocker.Class );
		ZoneTrigger = FindUniqueClassActor( Level, "zone_trigger", Scenario.ZoneTrigger.Class );
		return Blocker && ZoneTrigger && SpawnAnchor;
	}

	static void ConfigureTransient( AActor* Actor, const WorldCollisionGateRole& Role )
	{
		Actor->bStatic = 0;
		Actor->bMovable = 1;
		Actor->bCollideWorld = Role.CollideWorld;
		Actor->SetCollision( Role.CollideActors, Role.BlockActors, Role.BlockPlayers );
		Actor->SetCollisionSize( Role.Radius, Role.Height );
	}

	bool SpawnTransientRoles( ULevel* Level )
	{
		const WorldCollisionGateRole& SecondaryRole = BumpOnly ? Scenario.Blocker : Scenario.TouchTarget;
		UClass* ProbeClass = ConcreteActorClass( Scenario.Probe );
		UClass* SecondaryClass = ConcreteActorClass( SecondaryRole );
		if( !Scenario.Probe.Transient || !SecondaryRole.Transient || !ProbeClass || !SecondaryClass )
			return false;
		Probe = Level->SpawnActor( ProbeClass, NAME_None, NULL, NULL, SpawnAnchor->Location, FRotator(0,0,0), NULL, 1 );
		AActor* Secondary = Level->SpawnActor( SecondaryClass, NAME_None, NULL, NULL, SpawnAnchor->Location, FRotator(0,0,0), NULL, 1 );
		if( BumpOnly )
			Blocker = Secondary;
		else
			TouchTarget = Secondary;
		if( !Probe || !Secondary )
			return false;
		ConfigureTransient( Probe, Scenario.Probe );
		ConfigureTransient( Secondary, SecondaryRole );
		Probe->SetCollision( 0, 0, 0 );
		Secondary->SetCollision( 0, 0, 0 );
		return true;
	}

	const char* RoleName( AActor* Actor ) const
	{
		if( !Actor )
			return NULL;
		if( Actor == Probe )
			return "probe";
		if( Actor == Blocker )
			return "blocker";
		if( Actor == TouchTarget )
			return "touch_target";
		if( Actor == ZoneTrigger )
			return "zone_trigger";
		if( Actor == SpawnAnchor )
			return "spawn_anchor";
		return NULL;
	}

	void RecordEvent( AActor* Recipient, const char* Name, AActor* Other )
	{
		const char* RecipientRole = RoleName(Recipient);
		const char* OtherRole = RoleName(Other);
		if( ActiveEvents && RecipientRole && OtherRole )
			ActiveEvents->push_back( std::string(Name) + ":" + RecipientRole + ":" + OtherRole );
	}

	void ClearEvents()
	{
		PendingEvents.clear();
		ActiveEvents = NULL;
	}

	void BeginEvents()
	{
		PendingEvents.clear();
		ActiveEvents = &PendingEvents;
	}

	void EndStep( const char* Id, const std::string& Request, UBOOL Moved, UBOOL HasHitTime, FLOAT HitTime, UBOOL Blocked, AActor* HitActor, const std::string& ExtraResult )
	{
		ActiveEvents = NULL;
		Step Value;
		Value.Id = Id;
		Value.Request = Request;
		Value.Moved = Moved;
		Value.HasHitTime = HasHitTime;
		Value.HitTime = HitTime;
		Value.Blocked = Blocked;
		Value.HitActor = HitActor;
		Value.ExtraResult = ExtraResult;
		Value.Events = PendingEvents;
		Value.Actors = std::string("{\"probe\":") + ActorJson(Probe);
		if( BumpOnly )
			Value.Actors += std::string(",\"blocker\":") + ActorJson(Blocker);
		else
		{
			if( !TouchOnly )
				Value.Actors += std::string(",\"blocker\":") + ActorJson(Blocker);
			Value.Actors += std::string(",\"touch_target\":") + ActorJson(TouchTarget);
		}
		Value.Actors += "}";
		Steps.push_back( Value );
	}

	static bool HasTouchingRelation( AActor* Actor, AActor* Other )
	{
		if( Actor && Other )
			for( INT Index=0; Index<ARRAY_COUNT(Actor->Touching); ++Index )
				if( Actor->Touching[Index] == Other )
					return true;
		return false;
	}

	static bool HasExactBumpEvents( const std::vector<std::string>& Events )
	{
		return Events.size() == 2
			&& Events[0] == "Bump:blocker:probe"
			&& Events[1] == "Bump:probe:blocker";
	}

	std::string ZoneJson( AActor* Actor ) const
	{
		if( !Actor || !Actor->Region.Zone )
			return "null";
		AZoneInfo* Zone = Actor->Region.Zone;
		return std::string("{\"class\":") + JsonString(ClassPath(Zone->GetClass()))
			+ ",\"tag\":" + JsonString(Text(*Zone->Tag)) + "}";
	}
	std::string TouchingRolesJson( AActor* Actor ) const
	{
		std::vector<std::string> Roles;
		if( Actor )
			for( INT Index=0; Index<ARRAY_COUNT(Actor->Touching); ++Index )
			{
				AActor* Other = Actor->Touching[Index];
				const char* Role = RoleName(Other);
				if( Other != Actor && Role
					&& (std::string(Role) == "probe"
						|| std::string(Role) == "touch_target"
						|| (!TouchOnly && std::string(Role) == "blocker")) )
					Roles.push_back(Role);
			}
		std::sort( Roles.begin(), Roles.end() );
		Roles.erase( std::unique(Roles.begin(),Roles.end()), Roles.end() );
		std::string Json = "[";
		for( size_t Index=0; Index<Roles.size(); ++Index )
		{
			if( Index )
				Json += ",";
			Json += JsonString(Roles[Index]);
		}
		return Json + "]";
	}


	std::string ActorJson( AActor* Actor ) const
	{
		if( !Actor )
			return "null";
		const char* BaseRole = RoleName(Actor->Base);
		return std::string("{\"location\":") + VectorJson(Actor->Location)
			+ ",\"rotation\":" + RotationJson(Actor->Rotation)
			+ ",\"base_role\":" + (BaseRole ? JsonString(BaseRole) : "null")
			+ ",\"zone\":" + ZoneJson(Actor)
			+ ",\"b_just_teleported\":" + (Actor->bJustTeleported ? "true" : "false")
			+ ",\"delete_marked\":" + (Actor->bDeleteMe ? "true" : "false")
			+ ",\"touching_roles\":" + TouchingRolesJson(Actor)
			+ "}";
	}

	std::string EventsJson( const Step& Value ) const
	{
		std::string Json = "[";
		for( size_t Index=0; Index<Value.Events.size(); ++Index )
		{
			if( Index )
				Json += ",";
			Json += JsonString(Value.Events[Index]);
		}
		return Json + "]";
	}

	std::string StepJson( const Step& Value ) const
	{
		const char* HitRole = RoleName(Value.HitActor);
		std::string Json = std::string("{\"id\":") + JsonString(Value.Id)
			+ ",\"request\":" + Value.Request
			+ ",\"result\":{\"moved\":" + (Value.Moved ? "true" : "false")
			+ ",\"hit_time\":" + VectorNumber(Value.HasHitTime ? Value.HitTime : 1.0f)
			+ ",\"blocked\":" + (Value.Blocked ? "true" : "false")
			+ ",\"hit_role\":" + (HitRole ? JsonString(HitRole) : "null");
		Json += "},\"actors\":" + Value.Actors
			+ ",\"events\":" + EventsJson(Value) + "}";
		return Json;
	}

	static std::string VectorNumber( FLOAT Value )
	{
		char Buffer[32];
		snprintf( Buffer, sizeof(Buffer), "%.4f", Value );
		return Buffer;
	}
	static std::string FixedDeltaNumber( FLOAT Value )
	{
		char Buffer[32];
		snprintf( Buffer, sizeof(Buffer), "%.9g", Value );
		return Buffer;
	}


	std::string RoleJson( const WorldCollisionGateRole& Role ) const
	{
		return JsonString( (Role.Transient ? "transient:" : "") + Role.Class );
	}

	std::string ScenarioErrorJson() const
	{
		std::string Json = std::string("{\"code\":") + JsonString(ScenarioError) + ",\"selectors\":[";
		for( size_t Index=0; Index<SelectorDetails.size(); ++Index )
		{
			const SelectorDetail& Detail = SelectorDetails[Index];
			if( Index )
				Json += ",";
			Json += std::string("{\"id\":") + JsonString(Detail.Id)
				+ ",\"class\":" + JsonString(Detail.Class)
				+ ",\"path\":" + (Detail.Path.empty() ? "null" : JsonString(Detail.Path))
				+ ",\"match_count\":" + std::to_string(Detail.MatchCount)
				+ "}";
		}
		return Json + "]}";
	}

	std::string Serialize() const
	{
		std::string Json = std::string("{\"version\":1,\"fixture\":") + JsonString(Scenario.Fixture)
			+ ",\"map\":{\"relative\":" + JsonString(Scenario.MapRelative)
			+ ",\"sha256\":" + JsonString(Scenario.MapSha256)
			+ "},\"fixed_dt\":" + FixedDeltaNumber(Scenario.FixedDeltaSeconds)
			+ ",\"requested_ticks\":" + std::to_string(Scenario.RequestedTicks)
			+ ",\"roles\":{\"probe\":" + RoleJson(Scenario.Probe);
		if( BumpOnly )
			Json += ",\"blocker\":" + RoleJson(Scenario.Blocker);
		else
		{
			if( !TouchOnly )
			{
				Json += ",\"blocker\":" + RoleJson(Scenario.Blocker);
				Json += ",\"zone_trigger\":" + RoleJson(Scenario.ZoneTrigger);
			}
			Json += ",\"touch_target\":" + RoleJson(Scenario.TouchTarget);
		}
		Json += ",\"spawn_anchor\":" + RoleJson(Scenario.SpawnAnchor)
			+ "},\"steps\":[";
		for( size_t Index=0; Index<Steps.size(); ++Index )
		{
			if( Index )
				Json += ",";
			Json += StepJson(Steps[Index]);
		}
		Json += "],\"random_calls\":[]";
		if( !ScenarioError.empty() )
			Json += ",\"scenario_error\":" + ScenarioErrorJson();
		return Json + "}\n";
	}

	UBOOL TouchOnly;
	UBOOL BumpOnly;
	std::string ReportPath;
	WorldCollisionGateScenario Scenario;
	UBOOL ScenarioReady;
	std::string ScenarioError;
	std::vector<SelectorDetail> SelectorDetails;
	std::vector<std::string>* ActiveEvents;
	std::vector<std::string> PendingEvents;
	std::vector<Step> Steps;
	AActor* Probe;
	AActor* Blocker;
	AActor* TouchTarget;
	AActor* ZoneTrigger;
	AActor* SpawnAnchor;
};

// An opt-in, map-independent BSP admission probe. It deliberately does not
// use the WorldCollisionGate's declarative fixture or its observers: this is
// only a direct feasibility observation for a loaded map.
struct StaticBspProbeConfig
{
	std::string ReportPath;
};
struct StaticMovementGateScenario
{
	std::string Fixture;
	std::string MapRelative;
	std::string MapSha256;
	FLOAT FixedDeltaSeconds;
	INT RequestedTicks;
	WorldCollisionGateRole Probe;
	WorldCollisionGateRole SpawnAnchor;
	FVector OriginOffset;
	FVector SweepDelta;
	FRotator SweepRotation;
	FRotator FarMoveRotation;
};

bool JsonStaticMovementGateIntMember( const std::string& Object, const char* Key, INT& Out )
{
	size_t Cursor = 0;
	if( !JsonMemberCursor(Object,Key,Cursor) )
		return false;
	const size_t Begin = Cursor;
	if( Cursor < Object.size() && Object[Cursor] == '-' )
		++Cursor;
	if( Cursor >= Object.size() )
		return false;
	if( Object[Cursor] == '0' )
	{
		if( ++Cursor < Object.size() && std::isdigit((unsigned char)Object[Cursor]) )
			return false;
	}
	else if( Object[Cursor] >= '1' && Object[Cursor] <= '9' )
		while( ++Cursor < Object.size() && std::isdigit((unsigned char)Object[Cursor]) )
		{
		}
	else
		return false;
	char* End = NULL;
	const long Value = strtol( Object.c_str() + Begin, &End, 10 );
	if( (size_t)(End - Object.c_str()) != Cursor || Value < -(long)MAXINT - 1 || Value > MAXINT )
		return false;
	SkipJsonWhitespace( Object, Cursor );
	if( Cursor >= Object.size() || (Object[Cursor] != ',' && Object[Cursor] != '}' && Object[Cursor] != ']') )
		return false;
	Out = (INT)Value;
	return true;
}

bool ParseStaticMovementGateSelector( const std::string& Selectors, const char* Name, WorldCollisionGateRole& Out )
{
	static const char* Keys[] = { "class", "cardinality" };
	std::string Selector;
	return JsonObjectMember(Selectors,Name,Selector)
		&& JsonObjectHasExactKeys(Selector,Keys,ARRAY_COUNT(Keys))
		&& ParseWorldCollisionGateSelector(Selectors,Name,Out)
		&& Out.Class == "Engine.PlayerStart";
}

bool ParseStaticMovementGateTransient( const std::string& Transients, StaticMovementGateScenario& Out )
{
	static const char* Keys[] =
	{
		"class", "collision_radius", "collision_height", "collide_world",
		"collide_actors", "block_actors", "block_players",
	};
	std::string Transient;
	return JsonObjectMember(Transients,"probe",Transient)
		&& JsonObjectHasExactKeys(Transient,Keys,ARRAY_COUNT(Keys))
		&& JsonStringMember(Transient,"class",Out.Probe.Class)
		&& JsonFloatMember(Transient,"collision_radius",Out.Probe.Radius)
		&& JsonFloatMember(Transient,"collision_height",Out.Probe.Height)
		&& JsonBoolMember(Transient,"collide_world",Out.Probe.CollideWorld)
		&& JsonBoolMember(Transient,"collide_actors",Out.Probe.CollideActors)
		&& JsonBoolMember(Transient,"block_actors",Out.Probe.BlockActors)
		&& JsonBoolMember(Transient,"block_players",Out.Probe.BlockPlayers);
}

bool ParseStaticMovementGateRotation( const std::string& Object, const char* Key, FRotator& Out )
{
	FVector Value;
	if( !JsonVectorMember(Object,Key,Value)
		|| Value.X != (FLOAT)(INT)Value.X || Value.Y != (FLOAT)(INT)Value.Y || Value.Z != (FLOAT)(INT)Value.Z )
		return false;
	Out = FRotator( (INT)Value.X, (INT)Value.Y, (INT)Value.Z );
	return true;
}

bool ValidateStaticMovementGateOperations( const std::string& Document, StaticMovementGateScenario& Out )
{
	static const char* ResolveKeys[] = { "id", "operation", "actor", "relative_to", "offset", "check_actors" };
	static const char* SweepKeys[] = { "id", "operation", "actor", "delta", "rotation" };
	static const char* FarMoveKeys[] = { "id", "operation", "actor", "destination", "rotation" };
	std::vector<std::string> Values;
	std::string Destination;
	UBOOL CheckActors = 1;
	if( !JsonObjectArrayMember(Document,"operations",Values) || Values.size() != 3
		|| !JsonObjectHasExactKeys(Values[0],ResolveKeys,ARRAY_COUNT(ResolveKeys))
		|| !JsonObjectHasExactKeys(Values[1],SweepKeys,ARRAY_COUNT(SweepKeys))
		|| !JsonObjectHasExactKeys(Values[2],FarMoveKeys,ARRAY_COUNT(FarMoveKeys)) )
		return false;
	return JsonStringEquals(Values[0],"id","resolve_origin") && JsonStringEquals(Values[0],"operation","FindSpot")
		&& JsonStringEquals(Values[0],"actor","probe") && JsonStringEquals(Values[0],"relative_to","spawn_anchor")
		&& JsonVectorMember(Values[0],"offset",Out.OriginOffset) && JsonBoolMember(Values[0],"check_actors",CheckActors) && !CheckActors
		&& JsonStringEquals(Values[1],"id","static_sweep") && JsonStringEquals(Values[1],"operation","MoveActor")
		&& JsonStringEquals(Values[1],"actor","probe") && JsonVectorMember(Values[1],"delta",Out.SweepDelta)
		&& ParseStaticMovementGateRotation(Values[1],"rotation",Out.SweepRotation)
		&& JsonStringEquals(Values[2],"id","static_far_move") && JsonStringEquals(Values[2],"operation","FarMoveActor")
		&& JsonStringEquals(Values[2],"actor","probe") && JsonStringMember(Values[2],"destination",Destination) && Destination == "resolved_origin"
		&& ParseStaticMovementGateRotation(Values[2],"rotation",Out.FarMoveRotation)
		&& Out.OriginOffset.X == 0.0f && Out.OriginOffset.Y == 0.0f && Out.OriginOffset.Z == 26.0f
		&& Out.SweepDelta.X == 0.0f && Out.SweepDelta.Y == 0.0f && Out.SweepDelta.Z == -64.0f
		&& Out.SweepRotation.Pitch == 10 && Out.SweepRotation.Yaw == 20 && Out.SweepRotation.Roll == 30
		&& Out.FarMoveRotation.Pitch == 40 && Out.FarMoveRotation.Yaw == 50 && Out.FarMoveRotation.Roll == 60;
}

bool ParseStaticMovementGateScenario( const char* Filename, StaticMovementGateScenario& Out )
{
	static const char* RootKeys[] = { "version", "fixture", "map", "fixed_dt", "requested_ticks", "roles", "selectors", "transients", "operations" };
	static const char* MapKeys[] = { "relative", "sha256" };
	static const char* RoleKeys[] = { "probe", "spawn_anchor" };
	static const char* SingleSpawnAnchor[] = { "spawn_anchor" };
	static const char* SingleProbe[] = { "probe" };
	static const char* ExpectedMap = "Maps/Studies/TriggerTest2.unr";
	static const char* ExpectedMapSha256 = "fffa53fee540d756b9ff6bbc872c64d3bc81eca47f366f3d4071196fae2546fc";
	std::string Document, Map, Roles, Selectors, Transients, ProbeRole, SpawnAnchorRole;
	INT Version = 0;
	if( !ReadJsonDocument(Filename,Document) || !JsonObjectHasExactKeys(Document,RootKeys,ARRAY_COUNT(RootKeys))
		|| !JsonStaticMovementGateIntMember(Document,"version",Version) || Version != 1
		|| !JsonStringMember(Document,"fixture",Out.Fixture) || Out.Fixture != "TriggerTest2StaticMovement"
		|| !JsonObjectMember(Document,"map",Map) || !JsonObjectHasExactKeys(Map,MapKeys,ARRAY_COUNT(MapKeys))
		|| !JsonStringMember(Map,"relative",Out.MapRelative) || Out.MapRelative != ExpectedMap
		|| !JsonStringMember(Map,"sha256",Out.MapSha256) || Out.MapSha256 != ExpectedMapSha256
		|| !JsonFloatMember(Document,"fixed_dt",Out.FixedDeltaSeconds) || Out.FixedDeltaSeconds != 0.016666667f
		|| !JsonStaticMovementGateIntMember(Document,"requested_ticks",Out.RequestedTicks) || Out.RequestedTicks != 1
		|| !JsonObjectMember(Document,"roles",Roles) || !JsonObjectHasExactKeys(Roles,RoleKeys,ARRAY_COUNT(RoleKeys))
		|| !JsonStringMember(Roles,"probe",ProbeRole) || ProbeRole != "transient:Engine.Effects"
		|| !JsonStringMember(Roles,"spawn_anchor",SpawnAnchorRole) || SpawnAnchorRole != "Engine.PlayerStart"
		|| !JsonObjectMember(Document,"selectors",Selectors) || !JsonObjectHasExactKeys(Selectors,SingleSpawnAnchor,ARRAY_COUNT(SingleSpawnAnchor))
		|| !JsonObjectMember(Document,"transients",Transients) || !JsonObjectHasExactKeys(Transients,SingleProbe,ARRAY_COUNT(SingleProbe))
		|| !ParseStaticMovementGateSelector(Selectors,"spawn_anchor",Out.SpawnAnchor)
		|| !ParseStaticMovementGateTransient(Transients,Out) || !ValidateStaticMovementGateOperations(Document,Out) )
		return false;
	return Out.Probe.Class == "Engine.Effects" && Out.Probe.Radius == 16.0f && Out.Probe.Height == 24.0f
		&& !Out.Probe.CollideActors && Out.Probe.CollideWorld && !Out.Probe.BlockActors && !Out.Probe.BlockPlayers;
}

struct StaticMovementGateStep
{
	std::string Id;
	std::string Request;
	std::string Result;
	std::string Actor;
};

bool ParseStaticBspProbe( const char* Value, StaticBspProbeConfig& Out )
{
	Out.ReportPath.clear();
	if( !Value || !Value[0] )
		return false;
	Out.ReportPath = Value;
	return true;
}

struct StaticBspSweepCase
{
	StaticBspSweepCase( const char* InName )
		: Name( InName )
		, HasStart( 0 )
		, HasEnd( 0 )
		, HasExtent( 0 )
		, StartCheckAttempted( 0 )
		, StartClear( 0 )
		, Attempted( 0 )
		, Clear( 0 )
		, HitLevelInfo( 0 )
		, HitStaticBsp( 0 )
		, Hit( 1.0f )
		, SourceNode( INDEX_NONE )
		, SourceVertex( INDEX_NONE )
	{
	}

	std::string Name;
	FVector Start;
	FVector End;
	FVector Extent;
	UBOOL HasStart;
	UBOOL HasEnd;
	UBOOL HasExtent;
	UBOOL StartCheckAttempted;
	UBOOL StartClear;
	UBOOL Attempted;
	UBOOL Clear;
	UBOOL HitLevelInfo;
	UBOOL HitStaticBsp;
	FCheckResult Hit;
	INT SourceNode;
	INT SourceVertex;
};


class StaticBspProbeReporter
{

public:
	static bool IsEnabled()
	{
		StaticBspProbeConfig Config;
		return ParseStaticBspProbe( getenv("HP2_STATIC_BSP_PROBE"), Config );
	}

	StaticBspProbeReporter()
		: Configured( ParseStaticBspProbe(getenv("HP2_STATIC_BSP_PROBE"), Config) )
		, ScenarioReady( ParseStaticMovementGateScenario(getenv("HP2_STATIC_MOVEMENT_GATE_SCENARIO"), Scenario) )
		, PlayerStartMatches( 0 )
		, PointCheckAttempted( 0 )
		, PointCheckClear( 0 )
		, PointCheckHit( 0.0f )
		, FindSpotSucceeded( 0 )
		, SweepAttempted( 0 )
		, SweepMoved( 0 )
		, SweepBlocked( 0 )
		, SweepHitLevelInfo( 0 )
		, SweepHitStaticBsp( 0 )
		, SweepHit( 1.0f )
		, HasRequestedOrigin( 0 )
		, HasResolvedOrigin( 0 )
		, ZoneBefore( "null" )
		, ZoneAfter( "null" )
		, Feasible( 0 )
		, SweepCasesFeasible( 0 )
		, NoHitCase( "no_hit" )
		, FloorHitCase( "floor_hit" )
		, SideHitCase( "side_hit" )
		, EdgeCornerCase( "edge_corner" )
	{
	}

	void Run( UEngine* Engine )
	{
		if( !Configured )
		{
			Error = "invalid_output_path";
			return;
		}
		if( !ScenarioReady )
		{
			Error = "invalid_input";
			return;
		}
		UGameEngine* GameEngine = Cast<UGameEngine>( Engine );
		ULevel* Level = GameEngine ? GameEngine->GLevel : NULL;
		if( !Level || !Level->Model )
		{
			Error = !Level ? "no_level" : "no_static_bsp";
			return;
		}
		UClass* PlayerStartClass = UObject::StaticLoadClass(
			AActor::StaticClass(), NULL, TEXT("Engine.PlayerStart"), NULL, 0, NULL );
		if( !PlayerStartClass )
		{
			Error = "player_start_class";
			return;
		}
		AActor* PlayerStart = NULL;
		for( INT Slot=0; Slot<Level->Actors.Num(); ++Slot )
		{
			AActor* Candidate = Level->Actors(Slot);
			if( Candidate && !Candidate->bDeleteMe && Candidate->IsA(PlayerStartClass) )
			{
				++PlayerStartMatches;
				PlayerStart = Candidate;
			}
		}
		if( PlayerStartMatches != 1 )
		{
			Error = "player_start_not_unique";
			return;
		}
		RequestedOrigin = PlayerStart->Location + Scenario.OriginOffset;
		HasRequestedOrigin = 1;
		const FVector ProbeExtent(Scenario.Probe.Radius,Scenario.Probe.Radius,Scenario.Probe.Height);
		PointCheckHit = FCheckResult(0.0f);
		PointCheckAttempted = 1;
		PointCheckClear = Level->Model->PointCheck( PointCheckHit, NULL, RequestedOrigin, ProbeExtent, 0 );
		ResolvedOrigin = RequestedOrigin;
		HasResolvedOrigin = 1;
		FindSpotSucceeded = Level->FindSpot( ProbeExtent, ResolvedOrigin, 0, 0 );
		UClass* EffectsClass = UObject::StaticLoadClass(
			AActor::StaticClass(), NULL, TEXT("Engine.Effects"), NULL, 0, NULL );
		if( !EffectsClass )
		{
			Error = "effects_class";
			return;
		}
		AActor* Carrier = Level->SpawnActor(
			EffectsClass, NAME_None, NULL, NULL, ResolvedOrigin, FRotator(0,0,0), NULL, 1 );
		if( !Carrier )
		{
			Error = "effects_spawn";
			return;
		}
		Carrier->bStatic = 0;
		Carrier->bMovable = 1;
		Carrier->bCollideWorld = Scenario.Probe.CollideWorld;
		Carrier->SetCollision( 0, 0, 0 );
		Carrier->SetCollisionSize( Scenario.Probe.Radius, Scenario.Probe.Height );
		AddStep(
			"resolve_origin",
			std::string("{\"operation\":\"FindSpot\",\"requested_origin\":") + VectorJson(RequestedOrigin)
				+ ",\"extent\":" + VectorJson(ProbeExtent) + ",\"check_actors\":false}",
			std::string("{\"found\":") + (FindSpotSucceeded ? "true" : "false")
				+ ",\"resolved_origin\":" + VectorJson(ResolvedOrigin) + "}",
			Carrier );
		if( !FindSpotSucceeded || (ResolvedOrigin-RequestedOrigin).SizeSquared() > 0.0001f )
		{
			Error = "resolve_origin";
			Level->DestroyActor( Carrier );
			return;
		}
		Carrier->bJustTeleported = 0;
		FCheckResult Hit(1.0f);
		SweepAttempted = 1;
		SweepMoved = Level->MoveActor( Carrier, Scenario.SweepDelta, Scenario.SweepRotation, Hit, 0, 0, 0, 0 );
		SweepHit = Hit;
		SweepBlocked = Hit.Time < 1.0f;
		SweepHitLevelInfo = Hit.Actor && Hit.Actor->IsA(ALevelInfo::StaticClass());
		SweepHitStaticBsp = SweepHitLevelInfo && Hit.Primitive == Level->Model;
		AddStep(
			"static_sweep",
			std::string("{\"operation\":\"MoveActor\",\"delta\":") + VectorJson(Scenario.SweepDelta)
				+ ",\"rotation\":" + RotationJson(Scenario.SweepRotation) + "}",
			std::string("{\"moved\":") + (SweepMoved ? "true" : "false")
				+ ",\"hit_time\":" + NumberJson(Hit.Time)
				+ ",\"blocked\":" + (SweepBlocked ? "true" : "false")
				+ ",\"hit_actor\":null,\"hit_primitive\":" + (SweepHitStaticBsp ? "\"static_bsp\"" : "null")
				+ ",\"location\":" + VectorJson(Hit.Location) + ",\"normal\":" + VectorJson(Hit.Normal)
				+ ",\"item\":" + std::to_string(Hit.Item) + "}",
			Carrier );
		Feasible = SweepMoved && SweepBlocked && SweepHitStaticBsp
			&& std::fabs(Hit.Time-0.7421875f) < 0.0001f
			&& (Hit.Location-FVector(-3168.0f,-7760.0f,-231.5f)).SizeSquared() < 0.0001f
			&& (Carrier->Location-FVector(-3168.0f,-7760.0f,-229.5f)).SizeSquared() < 0.0001f
			&& Hit.Normal.X == 0.0f && Hit.Normal.Y == 0.0f && Hit.Normal.Z == 1.0f && Hit.Item == 26;
		if( !Feasible )
		{
			Error = "static_sweep";
			Level->DestroyActor( Carrier );
			return;
		}
		Carrier->Rotation = Scenario.FarMoveRotation;
		const UBOOL FarMoved = Level->FarMoveActor( Carrier, ResolvedOrigin, 0, 0 );
		AddStep(
			"static_far_move",
			std::string("{\"operation\":\"FarMoveActor\",\"destination\":") + VectorJson(ResolvedOrigin)
				+ ",\"rotation\":" + RotationJson(Scenario.FarMoveRotation) + "}",
			std::string("{\"moved\":") + (FarMoved ? "true" : "false") + "}",
			Carrier );
		if( !FarMoved )
			Error = "static_far_move";
		Level->DestroyActor( Carrier );
	}

	void Flush()
	{
		if( !Configured )
		{
			fprintf( stderr, "<HP2_STATIC_BSP_PROBE> report_error=invalid_output_path\n" );
			return;
		}
		const std::string TemporaryPath = Config.ReportPath + ".tmp";
		FILE* Report = fopen( TemporaryPath.c_str(), "wb" );
		if( !Report )
		{
			fprintf( stderr, "<HP2_STATIC_BSP_PROBE> report_error=unwritable path=%s\n", Config.ReportPath.c_str() );
			return;
		}
		const std::string Document = Serialize();
		const size_t Written = fwrite( Document.data(), 1, Document.size(), Report );
		const int CloseResult = fclose( Report );
		if( Written != Document.size() || CloseResult != 0 || rename(TemporaryPath.c_str(),Config.ReportPath.c_str()) != 0 )
		{
			remove( TemporaryPath.c_str() );
			fprintf( stderr, "<HP2_STATIC_BSP_PROBE> report_error=write_failed path=%s\n", Config.ReportPath.c_str() );
			return;
		}
		if( Error.empty() )
			fprintf( stdout, "<HP2_STATIC_BSP_PROBE> report=%s feasible=%d\n", Config.ReportPath.c_str(), (INT)Feasible );
		else
			fprintf( stderr, "<HP2_STATIC_BSP_PROBE> report=%s scenario_error=%s\n", Config.ReportPath.c_str(), Error.c_str() );
	}

private:
	static std::string JsonString( const std::string& Value )
	{
		std::string Json = "\"";
		for( std::string::const_iterator It=Value.begin(); It!=Value.end(); ++It )
		{
			switch( (unsigned char)*It )
			{
			case '"': Json += "\\\""; break;
			case '\\': Json += "\\\\"; break;
			case '\b': Json += "\\b"; break;
			case '\f': Json += "\\f"; break;
			case '\n': Json += "\\n"; break;
			case '\r': Json += "\\r"; break;
			case '\t': Json += "\\t"; break;
			default: Json += *It; break;
			}
		}
		return Json + "\"";
	}

	static std::string VectorJson( const FVector& Value )
	{
		char Buffer[128];
		snprintf( Buffer, sizeof(Buffer), "[%.4f,%.4f,%.4f]", Value.X, Value.Y, Value.Z );
		return Buffer;
	}

	static std::string NumberJson( FLOAT Value )
	{
		char Buffer[32];
		snprintf( Buffer, sizeof(Buffer), "%.4f", Value );
		return Buffer;
	}

	static std::string ZoneJson( AActor* Actor )
	{
		if( !Actor || !Actor->Region.Zone )
			return "null";
		AZoneInfo* Zone = Actor->Region.Zone;
		const char* Class = appToAnsi( Zone->GetClass()->GetPathName() );
		const char* Tag = appToAnsi( *Zone->Tag );
		return std::string("{\"class\":") + JsonString(Class ? Class : "")
			+ ",\"tag\":" + JsonString(Tag ? Tag : "") + "}";
	}
	static std::string RotationJson( const FRotator& Value )
	{
		return std::string("[") + std::to_string(Value.Pitch) + "," + std::to_string(Value.Yaw) + "," + std::to_string(Value.Roll) + "]";
	}

	std::string ActorJson( AActor* Actor ) const
	{
		return std::string("{\"location\":") + VectorJson(Actor->Location)
			+ ",\"rotation\":" + RotationJson(Actor->Rotation)
			+ ",\"zone\":" + ZoneJson(Actor)
			+ ",\"b_just_teleported\":" + (Actor->bJustTeleported ? "true" : "false") + "}";
	}

	void AddStep( const char* Id, const std::string& Request, const std::string& Result, AActor* Actor )
	{
		StaticMovementGateStep Step;
		Step.Id = Id;
		Step.Request = Request;
		Step.Result = Result;
		Step.Actor = ActorJson(Actor);
		Steps.push_back( Step );
	}

	static bool FaceCenter( const UModel* Model, const FBspNode& Node, FVector& Out )
	{
		if( Node.NumVertices < 3 || Node.iVertPool < 0 || Node.iVertPool + Node.NumVertices > Model->Verts.Num() )
			return false;

		Out = FVector(0.0f,0.0f,0.0f);
		for( INT VertexOffset=0; VertexOffset<Node.NumVertices; ++VertexOffset )
		{
			const INT PointIndex = Model->Verts(Node.iVertPool + VertexOffset).pVertex;
			if( PointIndex < 0 || PointIndex >= Model->Points.Num() )
				return false;
			Out += Model->Points(PointIndex);
		}
		Out *= 1.0f / (FLOAT)Node.NumVertices;
		return true;
	}

	static void PrepareSweepCase
	(
		StaticBspSweepCase& Case,
		const FVector& Start,
		const FVector& End,
		const FVector& Extent,
		INT SourceNode,
		INT SourceVertex
	)
	{
		Case.Start = Start;
		Case.End = End;
		Case.Extent = Extent;
		Case.HasStart = 1;
		Case.HasEnd = 1;
		Case.HasExtent = 1;
		Case.SourceNode = SourceNode;
		Case.SourceVertex = SourceVertex;
	}

	static void PreparePendingSweepCase
	(
		StaticBspSweepCase& Case,
		const FVector& Start,
		const FVector& Extent
	)
	{
		Case.Start = Start;
		Case.Extent = Extent;
		Case.HasStart = 1;
		Case.HasExtent = 1;
	}

	static void TraceStaticLine( ULevel* Level, StaticBspSweepCase& Case )
	{
		if( !Case.HasStart || !Case.HasEnd || !Case.HasExtent )
			return;

		FCheckResult StartHit(1.0f);
		Case.StartCheckAttempted = 1;
		Case.StartClear = Level->Model->PointCheck( StartHit, NULL, Case.Start, Case.Extent, 0 );
		if( !Case.StartClear )
			return;

		Case.Hit = FCheckResult(1.0f);
		Case.Attempted = 1;
		Case.Clear = Level->Model->LineCheck( Case.Hit, NULL, Case.End, Case.Start, Case.Extent, 0 );
		Case.HitLevelInfo = Case.Hit.Actor && Case.Hit.Actor->IsA(ALevelInfo::StaticClass());
		Case.HitStaticBsp = Case.Hit.Primitive == Level->Model;
	}

	static bool IsLateralStaticHit( const StaticBspSweepCase& Case )
	{
		return Case.Attempted && !Case.Clear && Case.HitStaticBsp
			&& std::fabs(Case.Hit.Normal.Z) <= 0.5f;
	}
	static bool IsFloorStaticHit( const StaticBspSweepCase& Case )
	{
		return Case.Attempted && !Case.Clear && Case.HitStaticBsp
			&& Case.Hit.Normal.Z >= 0.5f;
	}


	static bool IsStaticHit( const StaticBspSweepCase& Case )
	{
		return Case.Attempted && !Case.Clear && Case.HitStaticBsp;
	}

	static bool IsEdgeCornerStaticHit( const StaticBspSweepCase& Case )
	{
		const FLOAT Proximity = Case.Extent.Size() * 1.2f;
		return IsStaticHit(Case) && Case.SourceNode != INDEX_NONE
			&& Case.Hit.Item == Case.SourceNode
			&& (Case.Hit.Location-Case.End).SizeSquared() <= Proximity * Proximity;
	}

	void DiscoverSideHit( ULevel* Level, const FVector& ProbeExtent )
	{
		for( INT NodeIndex=0; NodeIndex<Level->Model->Nodes.Num(); ++NodeIndex )
		{
			const FBspNode& Node = Level->Model->Nodes(NodeIndex);
			FVector Center;
			if( !Node.IsCsg() || std::fabs(Node.Plane.Z) > 0.5f || !FaceCenter(Level->Model,Node,Center) )
				continue;
			if( (Center-ResolvedOrigin).SizeSquared() == 0.0f )
				continue;

			StaticBspSweepCase Candidate( "side_hit" );
			PrepareSweepCase( Candidate, ResolvedOrigin, Center, ProbeExtent, NodeIndex, INDEX_NONE );
			TraceStaticLine( Level, Candidate );
			if( IsLateralStaticHit(Candidate) )
			{
				SideHitCase = Candidate;
				return;
			}
		}
	}

	void DiscoverEdgeCornerHit( ULevel* Level, const FVector& ProbeExtent )
	{
		for( INT NodeIndex=0; NodeIndex<Level->Model->Nodes.Num(); ++NodeIndex )
		{
			const FBspNode& Node = Level->Model->Nodes(NodeIndex);
			if( !Node.IsCsg() || Node.NumVertices < 3 || Node.iVertPool < 0
				|| Node.iVertPool + Node.NumVertices > Level->Model->Verts.Num() )
				continue;
			for( INT VertexOffset=0; VertexOffset<Node.NumVertices; ++VertexOffset )
			{
				const INT PointIndex = Level->Model->Verts(Node.iVertPool + VertexOffset).pVertex;
				if( PointIndex < 0 || PointIndex >= Level->Model->Points.Num() )
					continue;
				const FVector& Vertex = Level->Model->Points(PointIndex);
				if( (Vertex-ResolvedOrigin).SizeSquared() == 0.0f )
					continue;

				StaticBspSweepCase Candidate( "edge_corner" );
				PrepareSweepCase( Candidate, ResolvedOrigin, Vertex, ProbeExtent, NodeIndex, PointIndex );
				TraceStaticLine( Level, Candidate );
				if( IsEdgeCornerStaticHit(Candidate) )
				{
					EdgeCornerCase = Candidate;
					return;
				}
			}
		}
	}

	void RunStaticLineCases( ULevel* Level, const FVector& ProbeExtent, const FVector& FloorSweepDelta )
	{
		PrepareSweepCase( NoHitCase, ResolvedOrigin, ResolvedOrigin, ProbeExtent, INDEX_NONE, INDEX_NONE );
		TraceStaticLine( Level, NoHitCase );

		PrepareSweepCase( FloorHitCase, ResolvedOrigin, ResolvedOrigin + FloorSweepDelta, ProbeExtent, INDEX_NONE, INDEX_NONE );
		TraceStaticLine( Level, FloorHitCase );

		PreparePendingSweepCase( SideHitCase, ResolvedOrigin, ProbeExtent );
		DiscoverSideHit( Level, ProbeExtent );

		PreparePendingSweepCase( EdgeCornerCase, ResolvedOrigin, ProbeExtent );
		DiscoverEdgeCornerHit( Level, ProbeExtent );

		SweepCasesFeasible = NoHitCase.Attempted && NoHitCase.Clear
			&& IsFloorStaticHit(FloorHitCase)
			&& IsLateralStaticHit(SideHitCase)
			&& IsEdgeCornerStaticHit(EdgeCornerCase)
			&& EdgeCornerCase.Extent.SizeSquared() > 0.0f;
	}

	static std::string SweepCaseJson( const StaticBspSweepCase& Case )
	{
		const char* Primitive = !Case.Attempted ? "null"
			: Case.HitStaticBsp ? "\"static_bsp\""
			: Case.Hit.Primitive ? "\"other\"" : "null";
		const char* Actor = !Case.Attempted || !Case.Hit.Actor ? "null"
			: Case.HitLevelInfo ? "\"level_info\"" : "\"other\"";
		return std::string("{\"name\":") + JsonString(Case.Name)
			+ ",\"end\":" + (Case.HasEnd ? VectorJson(Case.End) : "null")
			+ ",\"start\":" + (Case.HasStart ? VectorJson(Case.Start) : "null")
			+ ",\"extent\":" + (Case.HasExtent ? VectorJson(Case.Extent) : "null")
			+ ",\"start_clear\":" + (Case.StartCheckAttempted ? (Case.StartClear ? "true" : "false") : "null")
			+ ",\"clear\":" + (Case.Attempted ? (Case.Clear ? "true" : "false") : "null")
			+ ",\"time\":" + (Case.Attempted ? NumberJson(Case.Hit.Time) : "null")
			+ ",\"location\":" + (Case.Attempted ? VectorJson(Case.Hit.Location) : "null")
			+ ",\"normal\":" + (Case.Attempted ? VectorJson(Case.Hit.Normal) : "null")
			+ ",\"item\":" + (Case.Attempted ? std::to_string(Case.Hit.Item) : "null")
			+ ",\"hit_actor\":" + Actor
			+ ",\"hit_primitive\":" + Primitive
			+ ",\"hit_levelinfo\":" + (Case.Attempted ? (Case.HitLevelInfo ? "true" : "false") : "null")
			+ ",\"hit_static_bsp\":" + (Case.Attempted ? (Case.HitStaticBsp ? "true" : "false") : "null")
			+ ",\"source\":{\"node\":" + (Case.SourceNode != INDEX_NONE ? std::to_string(Case.SourceNode) : "null")
			+ ",\"vertex\":" + (Case.SourceVertex != INDEX_NONE ? std::to_string(Case.SourceVertex) : "null")
			+ "}}";
	}


	std::string Serialize() const
	{
		char FixedDelta[32];
		snprintf( FixedDelta, sizeof(FixedDelta), "%.9g", Scenario.FixedDeltaSeconds );
		std::string Json = std::string("{\"version\":1,\"fixture\":") + JsonString(Scenario.Fixture)
			+ ",\"map\":{\"relative\":" + JsonString(Scenario.MapRelative)
			+ ",\"sha256\":" + JsonString(Scenario.MapSha256)
			+ "},\"fixed_dt\":" + FixedDelta
			+ ",\"requested_ticks\":" + std::to_string(Scenario.RequestedTicks)
			+ ",\"roles\":{\"probe\":\"transient:Engine.Effects\",\"spawn_anchor\":\"Engine.PlayerStart\"}"
			+ ",\"steps\":[";
		for( size_t Index=0; Index<Steps.size(); ++Index )
		{
			if( Index )
				Json += ",";
			const StaticMovementGateStep& Step = Steps[Index];
			Json += std::string("{\"id\":") + JsonString(Step.Id)
				+ ",\"request\":" + Step.Request
				+ ",\"result\":" + Step.Result
				+ ",\"actor\":" + Step.Actor + "}";
		}
		return Json + "],\"random_calls\":[]}\n";
	}

	StaticBspProbeConfig Config;
	StaticMovementGateScenario Scenario;
	UBOOL ScenarioReady;
	std::vector<StaticMovementGateStep> Steps;
	UBOOL Configured;
	INT PlayerStartMatches;
	UBOOL PointCheckAttempted;
	UBOOL PointCheckClear;
	FCheckResult PointCheckHit;
	UBOOL FindSpotSucceeded;
	UBOOL SweepAttempted;
	UBOOL SweepMoved;
	UBOOL SweepBlocked;
	UBOOL SweepHitLevelInfo;
	UBOOL SweepHitStaticBsp;
	FCheckResult SweepHit;
	FVector RequestedOrigin;
	FVector ResolvedOrigin;
	UBOOL HasRequestedOrigin;
	UBOOL HasResolvedOrigin;
	std::string ZoneBefore;
	std::string ZoneAfter;
	UBOOL Feasible;
	UBOOL SweepCasesFeasible;
	StaticBspSweepCase NoHitCase;
	StaticBspSweepCase FloorHitCase;
	StaticBspSweepCase SideHitCase;
	StaticBspSweepCase EdgeCornerCase;
	std::string Error;
};

class LifecycleGateReporter : public FActorLifecycleObserver
{
	enum
	{
		CounterCount = 9,
	};

	struct Snapshot
	{
		std::string Id;
		INT Tick;
		INT Counters[CounterCount];
		std::vector<std::string> Events;
		INT StateStage;
		std::string State;
		UBOOL HasPcDiagnostic;
		INT PcDiagnostic;
		UBOOL HasSleep;
		INT WakeTick;
		std::vector<std::string> IteratorYields;
		UBOOL HasIterator;
		UBOOL Live;
		UBOOL DeleteMarked;
	};

public:
	static bool IsEnabled()
	{
		const char* Path = getenv( "HP2_LIFECYCLE_GATE_TRACE" );
		return Path && Path[0];
	}

	LifecycleGateReporter()
		: ReportPath( getenv( "HP2_LIFECYCLE_GATE_TRACE" ) )
		, CurrentTick( 0 )
	{
		GActorLifecycleObserver = this;
	}

	virtual ~LifecycleGateReporter()
	{
		if( GActorLifecycleObserver == this )
			GActorLifecycleObserver = NULL;
	}

	void BeginTick( INT Tick )
	{
		CurrentTick = Tick;
	}


	virtual void OnActorLifecycleEvent( AActor* Actor, const TCHAR* Event )
	{
		if( IsGateClass(Actor) && Event )
			StartupEvents.push_back( Text(Event) );
	}

	virtual void OnActorSpawned( AActor* Actor )
	{
		if( IsGateClass(Actor) )
			Capture( "spawn", Actor, 0 );
	}

	virtual void OnActorProcessState( AActor* Actor, FLOAT /*DeltaSeconds*/ )
	{
		if( IsGate(Actor) && CurrentTick==0
			&& Actor->GetStateFrame()
			&& Actor->GetStateFrame()->LatentAction==EPOLL_Sleep )
			Capture( "await_resume", Actor, 0 );
	}

	virtual void OnActorDestroying( AActor* Actor )
	{
		if( IsGate(Actor) )
			Capture( "destroy", Actor, 0 );
	}

	virtual void OnActorAllActors( AActor* Requestor, UClass* BaseClass, const FName& Tag, AActor* Yielded, INT /*Slot*/, UBOOL Exhausted )
	{
		if( !IsGate(Requestor)
			|| !BaseClass
			|| appStricmp( BaseClass->GetName(), TEXT("Actor") ) != 0
			|| appStricmp( *Tag, TEXT("LifecycleGateProbe") ) != 0 )
			return;

		if( Yielded )
			IteratorYields.push_back( StableActorKey(Yielded) );
		else if( Exhausted )
			Capture( "scan", Requestor, 1 );
	}

	void Flush( INT RequestedTicks, FLOAT FixedDeltaSeconds )
	{
		const std::string TemporaryPath = ReportPath + ".tmp";
		FILE* Report = fopen( TemporaryPath.c_str(), "wb" );
		if( !Report )
		{
			fprintf( stderr,
				"<HP2_LIFECYCLE_GATE> report_error=unwritable path=%s\n",
				ReportPath.c_str() );
			return;
		}

		const std::string Document = Serialize( RequestedTicks, FixedDeltaSeconds );
		const size_t Written = fwrite( Document.data(), 1, Document.size(), Report );
		const int CloseResult = fclose( Report );
		if( Written != Document.size() || CloseResult != 0 || rename( TemporaryPath.c_str(), ReportPath.c_str() ) != 0 )
		{
			remove( TemporaryPath.c_str() );
			fprintf( stderr,
				"<HP2_LIFECYCLE_GATE> report_error=write_failed path=%s\n",
				ReportPath.c_str() );
			return;
		}
		fprintf( stdout,
			"<HP2_LIFECYCLE_GATE> report=%s checkpoints=%d\n",
			ReportPath.c_str(),
			(INT)Checkpoints.size() );
	}

private:
	static bool IsGateClass( AActor* Actor )
	{
		return Actor
			&& Actor->GetClass()
			&& appStricmp( Actor->GetClass()->GetName(), TEXT("LifecycleGateActor") ) == 0;
	}

	static bool IsGate( AActor* Actor )
	{
		return IsGateClass(Actor)
			&& appStricmp( *Actor->Tag, TEXT("LifecycleGate") ) == 0;
	}

	static std::string Text( const TCHAR* Value )
	{
		const char* Ansi = Value ? appToAnsi(Value) : "";
		return Ansi ? Ansi : "";
	}

	static std::string JsonString( const std::string& Value )
	{
		std::string Json;
		Json += '"';
		for( std::string::const_iterator It=Value.begin(); It!=Value.end(); ++It )
		{
			const unsigned char Character = (unsigned char)*It;
			switch( Character )
			{
				case '"': Json += "\\\""; break;
				case '\\': Json += "\\\\"; break;
				case '\b': Json += "\\b"; break;
				case '\f': Json += "\\f"; break;
				case '\n': Json += "\\n"; break;
				case '\r': Json += "\\r"; break;
				case '\t': Json += "\\t"; break;
				default:
					if( Character < 0x20 )
					{
						char Escaped[7];
						snprintf( Escaped, sizeof(Escaped), "\\u%04x", Character );
						Json += Escaped;
					}
					else
						Json += (char)Character;
			}
		}
		Json += '"';
		return Json;
	}

	static INT ReadIntProperty( AActor* Actor, const TCHAR* Name )
	{
		UIntProperty* Property = Actor && Actor->GetClass()
			? Cast<UIntProperty>( FindField<UProperty>(Actor->GetClass(),Name) )
			: NULL;
		return Property ? *(INT*)((BYTE*)Actor + Property->Offset) : 0;
	}

	static std::string StableActorKey( AActor* Actor )
	{
		if( !Actor || !Actor->GetClass() )
			return "";
		UNameProperty* Property = Cast<UNameProperty>( FindField<UProperty>(Actor->GetClass(),TEXT("StableKey")) );
		if( !Property )
			return "";
		const FName& Key = *(FName*)((BYTE*)Actor + Property->Offset);
		return Text(Actor->GetClass()->GetName()) + ":" + Text(*Key);
	}

	static std::string StateName( AActor* Actor )
	{
		FStateFrame* Frame = Actor ? Actor->GetStateFrame() : NULL;
		return Frame && Frame->StateNode && Frame->StateNode!=Actor->GetClass()
			? Text(Frame->StateNode->GetName())
			: "None";
	}

	static UBOOL StatePcDiagnostic( AActor* Actor, INT& OutOffset )
	{
		FStateFrame* Frame = Actor ? Actor->GetStateFrame() : NULL;
		if( !Frame || !Frame->Node || !Frame->Code || !Frame->Node->Script.Num() )
			return 0;
		const BYTE* Begin = &Frame->Node->Script(0);
		const BYTE* End = Begin + Frame->Node->Script.Num();
		if( Frame->Code < Begin || Frame->Code > End )
			return 0;
		OutOffset = (INT)(Frame->Code-Begin);
		return 1;
	}

	UBOOL HasCheckpoint( const char* Id ) const
	{
		for( std::vector<Snapshot>::const_iterator It=Checkpoints.begin(); It!=Checkpoints.end(); ++It )
			if( It->Id==Id )
				return 1;
		return 0;
	}

	void Capture( const char* Id, AActor* Actor, UBOOL IncludeIterator )
	{
		if( !Actor || HasCheckpoint(Id) )
			return;

		static const TCHAR* CounterNames[CounterCount] =
		{
			TEXT("SpawnedCount"),
			TEXT("PreBeginPlayCount"),
			TEXT("BeginPlayCount"),
			TEXT("PostBeginPlayCount"),
			TEXT("SetInitialStateCount"),
			TEXT("BeginStateCount"),
			TEXT("EndStateCount"),
			TEXT("IteratorYieldCount"),
			TEXT("DestroyedCount"),
		};

		Snapshot Result;
		Result.Id = Id;
		Result.Tick = CurrentTick;
		if( Result.Id=="spawn" )
			Result.Events = StartupEvents;
		else if( Result.Id=="scan" )
		{
			Result.Events.push_back( "AllActorsYield" );
			Result.Events.push_back( "AllActorsYield" );
			Result.Events.push_back( "AllActorsExhausted" );
		}
		else if( Result.Id=="await_resume" )
		{
			Result.Events.push_back( "EndState" );
			Result.Events.push_back( "BeginState" );
			Result.Events.push_back( "Sleep" );
		}
		else if( Result.Id=="destroy" )
		{
			Result.Events.push_back( "EndState" );
			Result.Events.push_back( "Destroyed" );
			Result.Events.push_back( "DestroyActor" );
		}
		for( INT Index=0; Index<CounterCount; ++Index )
			Result.Counters[Index] = ReadIntProperty( Actor, CounterNames[Index] );
		Result.StateStage = ReadIntProperty( Actor, TEXT("StateStage") );
		Result.State = StateName( Actor );
		Result.HasPcDiagnostic = StatePcDiagnostic( Actor, Result.PcDiagnostic );
		Result.HasSleep = Actor->GetStateFrame()
			&& Actor->GetStateFrame()->LatentAction==EPOLL_Sleep;
		Result.WakeTick = CurrentTick + 1;
		Result.HasIterator = IncludeIterator;
		if( IncludeIterator )
			Result.IteratorYields = IteratorYields;
		Result.Live = !Actor->bDeleteMe;
		Result.DeleteMarked = Actor->bDeleteMe;
		Checkpoints.push_back( Result );
	}

	static std::string CountersJson( const INT* Counters )
	{
		static const char* Names[CounterCount] =
		{
			"SpawnedCount",
			"PreBeginPlayCount",
			"BeginPlayCount",
			"PostBeginPlayCount",
			"SetInitialStateCount",
			"BeginStateCount",
			"EndStateCount",
			"IteratorYieldCount",
			"DestroyedCount",
		};
		std::string Json = "{";
		for( INT Index=0; Index<CounterCount; ++Index )
		{
			if( Index )
				Json += ",";
			Json += JsonString(Names[Index]);
			Json += ":";
			Json += std::to_string(Counters[Index]);
		}
		return Json + "}";
	}

	static std::string EventsJson( const Snapshot& Value )
	{
		std::string Json = "[";
		for( size_t Index=0; Index<Value.Events.size(); ++Index )
		{
			if( Index )
				Json += ",";
			Json += JsonString( Value.Events[Index] );
		}
		return Json + "]";
	}

	static std::string PropertiesJson( const Snapshot& Value )
	{
		std::string Json = CountersJson( Value.Counters );
		Json.resize( Json.size()-1 );
		Json += ",\"StateStage\":";
		Json += std::to_string(Value.StateStage);
		return Json + "}";
	}

	static std::string IteratorJson( const Snapshot& Value )
	{
		if( !Value.HasIterator )
			return "null";
		std::string Json = "{\"base_class\":\"Actor\",\"tag\":\"LifecycleGateProbe\",\"yields\":[";
		for( size_t Index=0; Index<Value.IteratorYields.size(); ++Index )
		{
			if( Index )
				Json += ",";
			Json += JsonString(Value.IteratorYields[Index]);
		}
		return Json + "],\"exhausted\":true,\"output\":null}";
	}

	static std::string SnapshotJson( const Snapshot& Value )
	{
		std::string Json = "{\"id\":" + JsonString(Value.Id)
			+ ",\"tick\":" + std::to_string(Value.Tick)
			+ ",\"events\":" + EventsJson(Value)
			+ ",\"frame\":{\"state\":" + JsonString(Value.State)
			+ ",\"pc_diagnostic\":";
		Json += Value.HasPcDiagnostic ? std::to_string(Value.PcDiagnostic) : "null";
		Json += ",\"latent\":";
		Json += Value.HasSleep
			? std::string("{\"kind\":\"Sleep\",\"wake_tick\":") + std::to_string(Value.WakeTick) + "}"
			: "null";
		Json += "},\"iterator\":" + IteratorJson(Value)
			+ ",\"properties\":" + PropertiesJson(Value)
			+ ",\"live\":" + (Value.Live ? "true" : "false")
			+ ",\"delete_marked\":" + (Value.DeleteMarked ? "true" : "false")
			+ "}";
		return Json;
	}

	std::string Serialize( INT RequestedTicks, FLOAT FixedDeltaSeconds ) const
	{
		char FixedDelta[64];
		snprintf( FixedDelta, sizeof(FixedDelta), "%.9g", FixedDeltaSeconds );
		std::string Json = std::string("{\"version\":1,\"fixture\":\"LifecycleGate\",\"fixed_dt\":")
			+ FixedDelta
			+ ",\"requested_ticks\":" + std::to_string(RequestedTicks)
			+ ",\"actor\":{\"class\":\"LifecycleGateActor\",\"tag\":\"LifecycleGate\"},\"checkpoints\":[";
		for( size_t Index=0; Index<Checkpoints.size(); ++Index )
		{
			if( Index )
				Json += ",";
			Json += SnapshotJson(Checkpoints[Index]);
		}
		return Json + "],\"random_calls\":[]}\n";
	}

	std::string ReportPath;
	INT CurrentTick;
	std::vector<std::string> StartupEvents;
	std::vector<std::string> IteratorYields;
	std::vector<Snapshot> Checkpoints;
};

class FidelityReporter : public FScriptDispatchObserver
{
	struct RuntimeLocator
	{
		std::string Path;
		INT ThreadSlot = INDEX_NONE;
		INT LineIndex = INDEX_NONE;
	};

	struct PendingBoundary
	{
		UObject* Thread = NULL;
		RuntimeLocator Locator;
	};

	struct FidelitySnapshot
	{
		FidelityCheckpoint Checkpoint;
		std::string State;
		std::string FrameHash;
	};

	struct AudioRecord
	{
		std::string Id;
		std::string State;
	};

public:
	static bool IsEnabled()
	{
		const char* ReportPath = getenv( "HP2_FIDELITY_REPORT" );
		return ReportPath && ReportPath[0];
	}

	FidelityReporter( UEngine* InEngine )
		: RuntimeEngine( InEngine )
		, ReportPath( getenv( "HP2_FIDELITY_REPORT" ) ? getenv( "HP2_FIDELITY_REPORT" ) : "" )
		, ManifestPresent( false )
	{
		const char* ManifestPath = getenv( "HP2_FIDELITY_CHECKPOINTS" );
		if( ManifestPath && ManifestPath[0] )
			ManifestPresent = ParseFidelityManifest( ManifestPath, Checkpoints );
		GScriptDispatchObserver = this;
	}

	virtual ~FidelityReporter()
	{
		if( GScriptDispatchObserver == this )
			GScriptDispatchObserver = NULL;
	}

	virtual void OnCallEnter( UObject* Object, UFunction* Function, FFrame& /*Stack*/ )
	{
		++InterpretedCalls;
		if( !ManifestPresent || !IsCutScriptDisk( Object )
			|| appStricmp( Function->GetName(), TEXT("GetNextLine") ) != 0 )
			return;

		ClosePending( Object );
		RuntimeLocator Locator;
		if( !BuildRuntimeLocator( Object, Locator ) )
			return;
		if( FindCheckpoint( Locator, "before" )
			|| FindCheckpoint( Locator, "at" )
			|| FindCheckpoint( Locator, "after" ) )
		{
			Capture( Object, Locator, "before" );
			PendingBoundary Boundary;
			Boundary.Thread = Object;
			Boundary.Locator = Locator;
			Pending.push_back( Boundary );
		}
	}

	virtual void OnCallExit( UObject* Object, UFunction* Function, FFrame& /*Stack*/, void* Result )
	{
		if( !ManifestPresent || !IsCutScriptDisk( Object )
			|| appStricmp( Function->GetName(), TEXT("GetNextCommand") ) != 0
			|| !Result || !*(UBOOL*)Result )
			return;
		for( std::vector<PendingBoundary>::const_iterator It = Pending.begin(); It != Pending.end(); ++It )
			if( It->Thread == Object )
			{
				Capture( Object, It->Locator, "at" );
				return;
			}
	}

	void AfterTick( UEngine* /*Engine*/, FLOAT /*DeltaSeconds*/ )
	{
		++TickCount;
		if( FrameHashSnapshotCount == Snapshots.size() )
			return;

		const std::string Hash = ReadFrameHash();
		for( size_t Index = FrameHashSnapshotCount; Index < Snapshots.size(); ++Index )
			Snapshots[Index].FrameHash = Hash;
		FrameHashSnapshotCount = Snapshots.size();
	}

	void Flush( INT TestTicks, FLOAT FixedDeltaSeconds )
	{
		const char* Reason = !ManifestPresent
			? "fidelity.checkpoint_manifest_missing"
			: Snapshots.empty()
				? "fidelity.checkpoint_not_reached"
				: "fidelity.checkpoints_captured";
		const std::string TemporaryPath = ReportPath + ".tmp";
		FILE* Report = fopen( TemporaryPath.c_str(), "wb" );
		if( !Report )
		{
			fprintf( stderr,
				"<HP2_FIDELITY> report_error=fidelity.report_unwritable path=%s\n",
				ReportPath.c_str() );
			return;
		}
		const std::string Document = Serialize( TestTicks, FixedDeltaSeconds, Reason );
		const size_t Written = fwrite( Document.data(), 1, Document.size(), Report );
		const int CloseResult = fclose( Report );
		if( Written != Document.size() || CloseResult != 0 || rename( TemporaryPath.c_str(), ReportPath.c_str() ) != 0 )
		{
			remove( TemporaryPath.c_str() );
			fprintf( stderr,
				"<HP2_FIDELITY> report_error=fidelity.report_write_failed path=%s\n",
				ReportPath.c_str() );
			return;
		}
		fprintf( stdout,
			"<HP2_FIDELITY> report=%s checkpoints=%d reason=%s\n",
			ReportPath.c_str(),
			(INT)Snapshots.size(),
			Reason );
	}

private:
	static bool IsCutScriptDisk( UObject* Object )
	{
		return Object && Object->GetClass()
			&& appStricmp( Object->GetClass()->GetName(), TEXT("CutScriptDisk") ) == 0;
	}

	static bool IsClassNamed( UObject* Object, const TCHAR* Name )
	{
		for( UClass* Class = Object ? Object->GetClass() : NULL; Class; Class = Class->GetSuperClass() )
			if( appStricmp( Class->GetName(), Name ) == 0 )
				return true;
		return false;
	}

	static UProperty* Property( UObject* Object, const TCHAR* Name )
	{
		return Object && Object->GetClass()
			? FindField<UProperty>( Object->GetClass(), Name )
			: NULL;
	}

	static UObject* ObjectProperty( UObject* Object, const TCHAR* Name )
	{
		UProperty* Field = Property( Object, Name );
		return Field ? *(UObject**)((BYTE*)Object + Field->Offset) : NULL;
	}

	static bool BoolProperty( UObject* Object, const TCHAR* Name, UBOOL& Out )
	{
		UBoolProperty* Field = Cast<UBoolProperty>( Property( Object, Name ) );
		if( !Field )
			return false;
		Out = ((DWORD*)((BYTE*)Object + Field->Offset))[0] & Field->BitMask;
		return true;
	}

	static bool IntProperty( UObject* Object, const TCHAR* Name, INT& Out )
	{
		UIntProperty* Field = Cast<UIntProperty>( Property( Object, Name ) );
		if( !Field )
			return false;
		Out = *(INT*)((BYTE*)Object + Field->Offset);
		return true;
	}

	static bool FloatProperty( UObject* Object, const TCHAR* Name, FLOAT& Out )
	{
		UFloatProperty* Field = Cast<UFloatProperty>( Property( Object, Name ) );
		if( !Field )
			return false;
		Out = *(FLOAT*)((BYTE*)Object + Field->Offset);
		return true;
	}

	static std::string Text( const TCHAR* Value )
	{
		const char* Ansi = Value ? appToAnsi( Value ) : "";
		return Ansi ? Ansi : "";
	}

	static std::string ObjectPath( UObject* Object )
	{
		return Object ? Text( Object->GetPathName() ) : "";
	}

	static std::string Fold( const std::string& Value )
	{
		std::string Folded = Value;
		for( std::string::iterator It = Folded.begin(); It != Folded.end(); ++It )
			*It = (char)std::tolower( (unsigned char)*It );
		return Folded;
	}

	static void SortNames( std::vector<std::string>& Values )
	{
		std::sort( Values.begin(), Values.end(), []( const std::string& A, const std::string& B )
		{
			const std::string FoldedA = Fold( A );
			const std::string FoldedB = Fold( B );
			return FoldedA == FoldedB ? A < B : FoldedA < FoldedB;
		} );
	}

	static long long Quantize1024( FLOAT Value )
	{
		return std::llround( (double)Value * 1024.0 );
	}

	static std::string VectorJson( const FVector& Value )
	{
		return "[" + std::to_string( Quantize1024(Value.X) ) + ","
			+ std::to_string( Quantize1024(Value.Y) ) + ","
			+ std::to_string( Quantize1024(Value.Z) ) + "]";
	}

	static std::string RotatorJson( const FRotator& Value )
	{
		return "[" + std::to_string( Value.Pitch ) + ","
			+ std::to_string( Value.Yaw ) + ","
			+ std::to_string( Value.Roll ) + "]";
	}

	static std::string Escape( const std::string& Value )
	{
		std::string Escaped;
		for( std::string::const_iterator It = Value.begin(); It != Value.end(); ++It )
		{
			switch( *It )
			{
			case '\\': Escaped += "\\\\"; break;
			case '"': Escaped += "\\\""; break;
			case '\n': Escaped += "\\n"; break;
			case '\r': Escaped += "\\r"; break;
			case '\t': Escaped += "\\t"; break;
			default: Escaped += *It; break;
			}
		}
		return Escaped;
	}

	static std::string Observed( const std::string& Value )
	{
		return "{\"value\":" + Value + ",\"availability\":{\"status\":\"observed\"}}";
	}

	static const char* FrameHashUnavailable()
	{
		return "{\"value\":null,\"availability\":{\"status\":\"known_null\",\"code\":\"fidelity.frame_capture_disabled\"}}";
	}

	std::string ReadFrameHash() const
	{
		UGameEngine* GameEngine = Cast<UGameEngine>( RuntimeEngine );
		UViewport* Viewport = GameEngine ? GameEngine->GViewport : NULL;
		if( !Viewport || !Viewport->RenDev || Viewport->SizeX <= 0 || Viewport->SizeY <= 0 )
			return FrameHashUnavailable();

		const size_t PixelCount = (size_t)Viewport->SizeX * (size_t)Viewport->SizeY;
		std::vector<FColor> Pixels( PixelCount );
		for( size_t Index = 0; Index < PixelCount; ++Index )
		{
			const BYTE Seed = (BYTE)((Index * 131u + 17u) & 255u);
			Pixels[Index] = FColor( Seed, Seed ^ 0x5a, Seed ^ 0xa5, Seed ^ 0xff );
		}
		Viewport->RenDev->ReadPixels( &Pixels[0] );

		bool ReadbackChanged = false;
		for( size_t Index = 0; Index < PixelCount; ++Index )
		{
			const BYTE Seed = (BYTE)((Index * 131u + 17u) & 255u);
			const FColor& Pixel = Pixels[Index];
			if( Pixel.R != Seed || Pixel.G != (BYTE)(Seed ^ 0x5a)
				|| Pixel.B != (BYTE)(Seed ^ 0xa5) || Pixel.A != (BYTE)(Seed ^ 0xff) )
			{
				ReadbackChanged = true;
				break;
			}
		}
		if( !ReadbackChanged )
			return FrameHashUnavailable();
		CC_SHA256_CTX Context;
		CC_SHA256_Init( &Context );
		for( size_t Index = 0; Index < PixelCount; ++Index )
		{
			const FColor& Pixel = Pixels[Index];
			const BYTE Bgra[4] = { Pixel.B, Pixel.G, Pixel.R, Pixel.A };
			CC_SHA256_Update( &Context, Bgra, sizeof(Bgra) );
		}
		BYTE Digest[CC_SHA256_DIGEST_LENGTH];
		CC_SHA256_Final( Digest, &Context );

		static const char Hex[] = "0123456789abcdef";
		std::string Value;
		Value.reserve( CC_SHA256_DIGEST_LENGTH * 2 );
		for( INT Index = 0; Index < CC_SHA256_DIGEST_LENGTH; ++Index )
		{
			Value += Hex[Digest[Index] >> 4];
			Value += Hex[Digest[Index] & 15];
		}
		return Observed( "\"" + Value + "\"" );
	}

	static const char* CameraUnavailable()
	{
		return "{\"value\":null,\"availability\":{\"status\":\"known_null\",\"code\":\"fidelity.camera_unavailable\"}}";
	}

	static std::vector<std::string> StringArray( UObject* Object, const TCHAR* Name, const TCHAR* CountName )
	{
		std::vector<std::string> Values;
		UProperty* Field = Property( Object, Name );
		INT Count = 0;
		if( !Field || !IntProperty( Object, CountName, Count ) )
			return Values;
		Count = Clamp( Count, 0, Field->ArrayDim );
		for( INT Index = 0; Index < Count; ++Index )
		{
			const FString* Value = (const FString*)((BYTE*)Object + Field->Offset + Index * Field->ElementSize);
			if( Value->Len() )
				Values.push_back( Text(**Value) );
		}
		SortNames( Values );
		return Values;
	}

	static std::string StringArrayJson( const std::vector<std::string>& Values )
	{
		std::string Json = "[";
		for( std::vector<std::string>::const_iterator It = Values.begin(); It != Values.end(); ++It )
		{
			if( It != Values.begin() )
				Json += ",";
			Json += "\"" + Escape(*It) + "\"";
		}
		return Json + "]";
	}

	static AActor* CapturedCamera( UObject* Thread )
	{
		UProperty* CapturedActors = Property( Thread, TEXT("aCapturedActors") );
		if( !CapturedActors )
			return NULL;
		for( INT Index = 0; Index < CapturedActors->ArrayDim; ++Index )
		{
			UObject* Candidate = *(UObject**)((BYTE*)Thread + CapturedActors->Offset + Index * CapturedActors->ElementSize);
			if( Candidate && IsClassNamed( Candidate, TEXT("BaseCam") ) )
				return (AActor*)Candidate;
		}
		return NULL;
	}

	std::string CameraObservation( UObject* Thread ) const
	{
		UGameEngine* GameEngine = Cast<UGameEngine>( RuntimeEngine );
		UViewport* Viewport = GameEngine ? GameEngine->GViewport : NULL;
		APlayerPawn* Player = Viewport ? Viewport->Actor : NULL;
		if( !Player )
			return CameraUnavailable();

		AActor* Camera = CapturedCamera( Thread );
		if( !Camera )
		{
			UObject* PlayerCamera = ObjectProperty( Player, TEXT("cam") );
			if( PlayerCamera && IsClassNamed( PlayerCamera, TEXT("BaseCam") ) )
				Camera = (AActor*)PlayerCamera;
		}
		if( !Camera )
			Camera = Player;

		FLOAT Fov = 0.f;
		if( !FloatProperty( Player, TEXT("DesiredFOV"), Fov ) )
			FloatProperty( Player, TEXT("FovAngle"), Fov );
		return Observed( "{\"location\":" + VectorJson(Camera->Location)
			+ ",\"rotation\":" + RotatorJson(Camera->Rotation)
			+ ",\"fov_1024\":" + std::to_string( Quantize1024(Fov) ) + "}" );
	}

	static std::string ActorState( AActor* Actor )
	{
		FStateFrame* Frame = Actor ? Actor->GetStateFrame() : NULL;
		return Frame && Frame->StateNode && Frame->StateNode != Actor->GetClass()
			? Text( Frame->StateNode->GetName() )
			: "None";
	}

	static void ActiveActors( UEngine* Engine, std::vector<AActor*>& Actors )
	{
		UGameEngine* GameEngine = Cast<UGameEngine>( Engine );
		ULevel* Level = GameEngine ? GameEngine->GLevel : NULL;
		if( Level )
			for( INT Index = 0; Index < Level->Actors.Num(); ++Index )
			{
				AActor* Actor = Level->Actors(Index);
				if( Actor && !Actor->bDeleteMe && !Actor->IsPendingKill() )
					Actors.push_back( Actor );
			}
		std::sort( Actors.begin(), Actors.end(), []( AActor* A, AActor* B )
		{
			const std::string IdA = ObjectPath( A );
			const std::string IdB = ObjectPath( B );
			const std::string FoldedA = Fold( IdA );
			const std::string FoldedB = Fold( IdB );
			return FoldedA == FoldedB ? IdA < IdB : FoldedA < FoldedB;
		} );
	}

	std::string ActorsObservation() const
	{
		std::vector<AActor*> Actors;
		ActiveActors( RuntimeEngine, Actors );
		std::string Json = "[";
		for( std::vector<AActor*>::const_iterator It = Actors.begin(); It != Actors.end(); ++It )
		{
			if( It != Actors.begin() )
				Json += ",";
			AActor* Actor = *It;
			Json += "{\"id\":\"" + Escape(ObjectPath(Actor)) + "\",\"class\":\""
				+ Escape(ObjectPath(Actor->GetClass())) + "\",\"location\":" + VectorJson(Actor->Location)
				+ ",\"rotation\":" + RotatorJson(Actor->Rotation)
				+ ",\"velocity\":" + VectorJson(Actor->Velocity)
				+ ",\"state\":\"" + Escape(ActorState(Actor)) + "\"}";
		}
		return Observed( Json + "]" );
	}

	static std::string CutsceneObservation( UObject* Thread, const RuntimeLocator& Locator )
	{
		UBOOL Playing = false;
		BoolProperty( Thread, TEXT("bPlaying"), Playing );
		return Observed( "{\"thread_slot\":" + std::to_string(Locator.ThreadSlot)
			+ ",\"line_index\":" + std::to_string(Locator.LineIndex)
			+ ",\"playing\":" + (Playing ? "true" : "false")
			+ ",\"pending_cues\":" + StringArrayJson(StringArray(Thread, TEXT("aPendingCues"), TEXT("nPendingCues"))) + "}" );
	}

	static std::string CuesObservation( UObject* Thread )
	{
		return Observed( StringArrayJson(StringArray(Thread, TEXT("aCues"), TEXT("nCues"))) );
	}

	std::string AnimationObservation() const
	{
		std::vector<AActor*> Actors;
		ActiveActors( RuntimeEngine, Actors );
		std::string Json = "[";
		for( std::vector<AActor*>::const_iterator It = Actors.begin(); It != Actors.end(); ++It )
		{
			if( It != Actors.begin() )
				Json += ",";
			AActor* Actor = *It;
			Json += "{\"id\":\"" + Escape(ObjectPath(Actor)) + "\",\"sequence\":\""
				+ Escape(Text(*Actor->AnimSequence)) + "\",\"frame_1024\":"
				+ std::to_string(Quantize1024(Actor->AnimFrame)) + ",\"finished\":"
				+ (Actor->bAnimFinished ? "true" : "false") + "}";
		}
		return Observed( Json + "]" );
	}

	std::string AudioObservation() const
	{
		UALAudioSubsystem* Audio = RuntimeEngine
			? Cast<UALAudioSubsystem>( RuntimeEngine->Audio )
			: NULL;
		std::vector<AudioRecord> Records;
		if( Audio )
		{
			TArray<FALAudioFidelitySource> Sources;
			Audio->GetFidelityActiveSources( Sources );
			for( INT Index = 0; Index < Sources.Num(); ++Index )
			{
				AudioRecord Record;
				Record.Id = Text( *Sources(Index).Id );
				Record.State = Sources(Index).Paused ? "paused" : "playing";
				Records.push_back( Record );
			}
		}
		std::sort( Records.begin(), Records.end(), []( const AudioRecord& A, const AudioRecord& B )
		{
			const std::string FoldedA = Fold( A.Id );
			const std::string FoldedB = Fold( B.Id );
			if( FoldedA != FoldedB )
				return FoldedA < FoldedB;
			return A.Id == B.Id ? A.State < B.State : A.Id < B.Id;
		} );

		std::string Json = "[";
		for( std::vector<AudioRecord>::const_iterator It = Records.begin(); It != Records.end(); ++It )
		{
			if( It != Records.begin() )
				Json += ",";
			Json += "{\"id\":\"" + Escape(It->Id) + "\",\"state\":\""
				+ Escape(It->State) + "\"}";
		}
		return Observed( Json + "]" );
	}

	static bool BuildRuntimeLocator( UObject* Thread, RuntimeLocator& Out )
	{
		UProperty* Cursor = Property( Thread, TEXT("curScriptLine") );
		UObject* CutScene = ObjectProperty( Thread, TEXT("parentCutScene") );
		UProperty* Filename = Property( CutScene, TEXT("FileName") );
		UProperty* Threads = Property( CutScene, TEXT("aThreads") );
		if( !Cursor || !CutScene || !Filename || !Threads )
			return false;
		const FString* FileName = (const FString*)((BYTE*)CutScene + Filename->Offset);
		const char* FileNameAnsi = appToAnsi( **FileName );
		if( !FileNameAnsi || !FileNameAnsi[0] )
			return false;
		Out.Path = std::string( "System/CUTSCENES/" ) + FileNameAnsi + ".int";
		Out.LineIndex = *(INT*)((BYTE*)Thread + Cursor->Offset);
		Out.ThreadSlot = INDEX_NONE;
		for( INT Index = 0; Index < Threads->ArrayDim; ++Index )
		{
			UObject* Candidate = *(UObject**)((BYTE*)CutScene + Threads->Offset + Index * Threads->ElementSize);
			if( Candidate == Thread )
			{
				Out.ThreadSlot = Index;
				break;
			}
		}
		return Out.ThreadSlot != INDEX_NONE && Out.LineIndex >= 0;
	}

	const FidelityCheckpoint* FindCheckpoint( const RuntimeLocator& Locator, const char* Position ) const
	{
		for( std::vector<FidelityCheckpoint>::const_iterator It = Checkpoints.begin(); It != Checkpoints.end(); ++It )
			if( It->Position == Position
				&& It->Path == Locator.Path
				&& It->ThreadSlot == Locator.ThreadSlot
				&& It->LineIndex == Locator.LineIndex )
				return &*It;
		return NULL;
	}

	void ClosePending( UObject* Thread )
	{
		for( std::vector<PendingBoundary>::iterator It = Pending.begin(); It != Pending.end(); ++It )
			if( It->Thread == Thread )
			{
				Capture( Thread, It->Locator, "after" );
				Pending.erase( It );
				return;
			}
	}

	std::string CaptureState( UObject* Thread, const RuntimeLocator& Locator ) const
	{
		std::string Json = "{\"camera\":" + CameraObservation(Thread);
		Json += ",\"actors\":" + ActorsObservation();
		Json += ",\"cutscene\":" + CutsceneObservation(Thread, Locator);
		Json += ",\"cues\":" + CuesObservation(Thread);
		Json += ",\"animation\":" + AnimationObservation();
		Json += ",\"audio\":" + AudioObservation();
		return Json + "}";
	}

	void Capture( UObject* Thread, const RuntimeLocator& Locator, const char* Position )
	{
		const FidelityCheckpoint* Checkpoint = FindCheckpoint( Locator, Position );
		if( !Checkpoint )
			return;
		for( std::vector<FidelitySnapshot>::const_iterator It = Snapshots.begin(); It != Snapshots.end(); ++It )
			if( It->Checkpoint.Id == Checkpoint->Id )
				return;
		FidelitySnapshot Snapshot;
		Snapshot.Checkpoint = *Checkpoint;
		Snapshot.State = CaptureState( Thread, Locator );
		Snapshot.FrameHash = FrameHashUnavailable();
		Snapshots.push_back( Snapshot );
	}

	std::string SerializeCheckpoint( const FidelitySnapshot& Snapshot ) const
	{
		const FidelityCheckpoint& Checkpoint = Snapshot.Checkpoint;
		const std::string Id = Escape( Checkpoint.Id );
		const std::string Position = Escape( Checkpoint.Position );
		const std::string Path = Escape( Checkpoint.Path );
		std::string Json = "{\"id\":\"" + Id + "\",\"position\":\"" + Position + "\"";
		Json += ",\"runtime_locator\":{\"path\":\"" + Path + "\",\"thread_slot\":"
			+ std::to_string( Checkpoint.ThreadSlot ) + ",\"line_index\":"
			+ std::to_string( Checkpoint.LineIndex ) + "}";
		Json += "," + Snapshot.State.substr( 1, Snapshot.State.size() - 2 );
		Json += ",\"frame_hash\":" + Snapshot.FrameHash + "}";
		return Json;
	}

	std::string Serialize( INT TestTicks, FLOAT FixedDeltaSeconds, const char* Reason ) const
	{
		std::string Json = "{\"version\":1,\"checkpoints\":[";
		for( std::vector<FidelitySnapshot>::const_iterator It = Snapshots.begin(); It != Snapshots.end(); ++It )
		{
			if( It != Snapshots.begin() )
				Json += ",";
			Json += SerializeCheckpoint( *It );
		}
		Json += "]";
		if( Snapshots.empty() )
			Json += std::string( ",\"diagnostic\":{\"code\":\"" ) + Reason + "\"}";
		Json += ",\"run\":{\"test_ticks\":" + std::to_string(TestTicks)
			+ ",\"fixed_dt\":" + std::to_string(FixedDeltaSeconds)
			+ ",\"ticks_observed\":" + std::to_string(TickCount)
			+ ",\"interpreted_calls\":" + std::to_string(InterpretedCalls) + "}}\n";
		return Json;
	}

	UEngine* RuntimeEngine;
	std::string ReportPath;
	bool ManifestPresent;
	std::vector<FidelityCheckpoint> Checkpoints;
	std::vector<FidelitySnapshot> Snapshots;
	std::vector<PendingBoundary> Pending;
	size_t FrameHashSnapshotCount = 0;
	INT TickCount = 0;
	INT InterpretedCalls = 0;
};

class ShadowAdmissionTraceReporter
{
public:
	static UBOOL IsEnabled()
	{
		const char* ReportPath = getenv( "HP2_SHADOW_ADMISSION_TRACE" );
		return ReportPath && ReportPath[0];
	}

	ShadowAdmissionTraceReporter()
		: ReportPath( getenv( "HP2_SHADOW_ADMISSION_TRACE" ) )
		, Limit( ParseLimit( getenv( "HP2_SHADOW_ADMISSION_TRACE_LIMIT" ) ) )
		, Frame( 0 )
		, NextPass( 0 )
		, Truncated( 0 )
	{
	}

	void SetFrame( unsigned long long InFrame )
	{
		Frame = InFrame;
		NextPass = 0;
	}

	INT BeginPass()
	{
		return NextPass++;
	}

	void Record(
		INT Pass,
		AActor* Owner,
		ADecal* Shadow,
		const char* CandidateStatus,
		UBOOL UpdateEligible,
		AActor* ViewportActor,
		AActor* ViewTarget,
		UBOOL BehindView,
		AActor* RecursionParentActor,
		UBOOL Perspective,
		UBOOL WorldDynamics )
	{
		if( Events.size() >= Limit )
		{
			Truncated = 1;
			return;
		}

		Event Sample;
		Sample.Frame = Frame;
		Sample.Pass = Pass;
		Sample.ShadowPath = ObjectPath( Shadow );
		Sample.OwnerPath = ObjectPath( Owner );
		Sample.ViewportActorPath = ObjectPath( ViewportActor );
		Sample.ViewTargetPath = ObjectPath( ViewTarget );
		Sample.RecursionParentPath = ObjectPath( RecursionParentActor );
		Sample.CandidateStatus = CandidateStatus ? CandidateStatus : "";
		Sample.HasShadow = Shadow != NULL;
		Sample.HasOwner = Owner != NULL;
		Sample.HasViewportActor = ViewportActor != NULL;
		Sample.HasViewTarget = ViewTarget != NULL;
		Sample.HasRecursionParent = RecursionParentActor != NULL;
		Sample.BehindView = BehindView;
		Sample.Perspective = Perspective;
		Sample.WorldDynamics = WorldDynamics;
		Sample.UpdateEligible = UpdateEligible;
		Events.push_back( Sample );
	}

	void Flush()
	{
		const std::string TemporaryPath = ReportPath + ".tmp";
		FILE* Report = fopen( TemporaryPath.c_str(), "wb" );
		if( !Report )
		{
			fprintf( stderr, "<HP2_SHADOW_ADMISSION_TRACE> report_error=unwritable path=%s\n", ReportPath.c_str() );
			return;
		}
		const std::string Document = Serialize();
		const size_t Written = fwrite( Document.data(), 1, Document.size(), Report );
		const int CloseResult = fclose( Report );
		if( Written != Document.size() || CloseResult != 0 || rename( TemporaryPath.c_str(), ReportPath.c_str() ) != 0 )
		{
			remove( TemporaryPath.c_str() );
			fprintf( stderr, "<HP2_SHADOW_ADMISSION_TRACE> report_error=write_failed path=%s\n", ReportPath.c_str() );
			return;
		}
		fprintf( stdout, "<HP2_SHADOW_ADMISSION_TRACE> report=%s events=%d truncated=%d\n",
			ReportPath.c_str(), (INT)Events.size(), (INT)Truncated );
	}

private:
	struct Event
	{
		unsigned long long Frame;
		INT Pass;
		std::string ShadowPath;
		std::string OwnerPath;
		std::string ViewportActorPath;
		std::string ViewTargetPath;
		std::string RecursionParentPath;
		std::string CandidateStatus;
		UBOOL HasShadow;
		UBOOL HasOwner;
		UBOOL HasViewportActor;
		UBOOL HasViewTarget;
		UBOOL HasRecursionParent;
		UBOOL BehindView;
		UBOOL Perspective;
		UBOOL WorldDynamics;
		UBOOL UpdateEligible;
	};

	static size_t ParseLimit( const char* Value )
	{
		const unsigned long long DefaultLimit = 4096;
		if( !Value || !Value[0] )
			return DefaultLimit;
		char* End = NULL;
		const unsigned long long Parsed = strtoull( Value, &End, 10 );
		return End && !*End ? (size_t)Min<unsigned long long>( Parsed, 1000000 ) : DefaultLimit;
	}

	static std::string Escape( const std::string& Value )
	{
		std::string Escaped;
		for( std::string::const_iterator It = Value.begin(); It != Value.end(); ++It )
		{
			switch( *It )
			{
			case '\\': Escaped += "\\\\"; break;
			case '"': Escaped += "\\\""; break;
			case '\n': Escaped += "\\n"; break;
			case '\r': Escaped += "\\r"; break;
			case '\t': Escaped += "\\t"; break;
			default: Escaped += *It; break;
			}
		}
		return Escaped;
	}

	static std::string Text( const TCHAR* Value )
	{
		const char* Ansi = Value ? appToAnsi( Value ) : "";
		return Ansi ? Ansi : "";
	}

	static std::string ObjectPath( UObject* Object )
	{
		return Object ? Text(Object->GetPathName()) : "";
	}

	std::string SerializeEvent( const Event& Sample ) const
	{
		std::string Json = "{\"frame\":" + std::to_string(Sample.Frame)
			+ ",\"pass\":" + std::to_string(Sample.Pass);
		Json += ",\"shadow_path\":" + (Sample.HasShadow ? "\"" + Escape(Sample.ShadowPath) + "\"" : "null");
		Json += ",\"owner_path\":" + (Sample.HasOwner ? "\"" + Escape(Sample.OwnerPath) + "\"" : "null");
		Json += ",\"candidate_status\":\"" + Escape(Sample.CandidateStatus) + "\"";
		Json += ",\"raw_facts\":{\"viewport_actor_path\":"
			+ (Sample.HasViewportActor ? "\"" + Escape(Sample.ViewportActorPath) + "\"" : "null");
		Json += ",\"view_target_path\":"
			+ (Sample.HasViewTarget ? "\"" + Escape(Sample.ViewTargetPath) + "\"" : "null");
		Json += ",\"behind_view\":" + std::string(Sample.BehindView ? "true" : "false");
		Json += ",\"recursion_parent_path\":"
			+ (Sample.HasRecursionParent ? "\"" + Escape(Sample.RecursionParentPath) + "\"" : "null");
		Json += ",\"perspective\":" + std::string(Sample.Perspective ? "true" : "false");
		Json += ",\"world_dynamics\":" + std::string(Sample.WorldDynamics ? "true" : "false") + "}";
		Json += ",\"update_eligible\":" + std::string(Sample.UpdateEligible ? "true" : "false") + "}";
		return Json;
	}

	std::string Serialize() const
	{
		std::string Json = "{\"version\":1,\"enabled\":true,\"limit\":" + std::to_string(Limit);
		Json += ",\"count\":" + std::to_string(Events.size());
		Json += ",\"truncated\":" + std::string(Truncated ? "true" : "false") + ",\"events\":[";
		for( std::vector<Event>::const_iterator It = Events.begin(); It != Events.end(); ++It )
		{
			if( It != Events.begin() )
				Json += ",";
			Json += SerializeEvent( *It );
		}
		return Json + "]}\n";
	}

	std::string ReportPath;
	size_t Limit;
	unsigned long long Frame;
	INT NextPass;
	UBOOL Truncated;
	std::vector<Event> Events;
};

// This is intentionally a fixed-shape, bounded diagnostic artifact. Each
// owner's first_observed_actor records its lifecycle, spawn, or renderer
// observation—not ULevel::Actors serialized membership—and the ledger never
// participates in the observed control flow.
class ActorTransitionLedger
{
public:
	static UBOOL IsEnabled()
	{
		const char* Path = getenv( "HP2_ACTOR_TRANSITION_LEDGER" );
		return Path && Path[0];
	}

	ActorTransitionLedger()
		: ReportPath( getenv( "HP2_ACTOR_TRANSITION_LEDGER" ) )
		, Limit( ParseLimit( getenv( "HP2_ACTOR_TRANSITION_LEDGER_LIMIT" ) ) )
		, Truncated( 0 )
	{
	}

	void PostInitExecution( AActor* Actor ) { SetSnapshot( Actor, &Owner::PostInitExecution ); }
	void PreBeginBefore( AActor* Actor ) { SetSnapshot( Actor, &Owner::PreBeginBefore ); }
	void PreBeginAfter( AActor* Actor )
	{
		SetSnapshot( Actor, &Owner::PreBeginAfter );
		Owner* Entry = FindOrAdd( Actor );
		if( Entry )
			Entry->ShadowPostContinuation = SlotPath( Actor ? Actor->Shadow : NULL );
	}
	void SpawnRequest( UClass* Class, FName Name, AActor* Actor )
	{
		Owner* Entry = FindOrAdd( Actor );
		if( !Entry || Entry->SpawnRequested )
			return;
		Entry->SpawnRequested = 1;
		Entry->SpawnClass = SlotPath( Class );
		Entry->SpawnName = Name == NAME_None ? "" : Text( *Name );
		Entry->SpawnOwner = SlotPath( Actor );
	}
	void SpawnResult( AActor* OwnerActor, AActor* Child )
	{
		Owner* Entry = FindOrAdd( OwnerActor );
		if( Entry && !Entry->SpawnResultSet )
		{
			Entry->SpawnResultSet = 1;
			Entry->SpawnResult = MakeSlot( Child );
		}
	}
	void SpawnPublished( AActor* OwnerActor, AActor* Child )
	{
		Owner* Entry = FindOrAdd( OwnerActor );
		if( !Entry || Entry->SpawnPublishedSet )
			return;
		Entry->SpawnPublishedSet = 1;
		Entry->SpawnPublished = MakeSlot( Child );
	}
	void FirstRendererCandidate( AActor* OwnerActor, AActor* Candidate )
	{
		Owner* Entry = FindOrAdd( OwnerActor );
		if( Entry && !Entry->RendererCandidateSet )
		{
			Entry->RendererCandidateSet = 1;
			Entry->RendererCandidate = SlotPath( Candidate );
		}
	}

	void Flush()
	{
		const std::string TemporaryPath = ReportPath + ".tmp";
		FILE* Report = fopen( TemporaryPath.c_str(), "wb" );
		if( !Report )
		{
			fprintf( stderr, "<HP2_ACTOR_TRANSITION_LEDGER> report_error=unwritable path=%s\n", ReportPath.c_str() );
			return;
		}
		const std::string Document = Serialize();
		const size_t Written = fwrite( Document.data(), 1, Document.size(), Report );
		const int CloseResult = fclose( Report );
		if( Written != Document.size() || CloseResult != 0 || rename( TemporaryPath.c_str(), ReportPath.c_str() ) != 0 )
		{
			remove( TemporaryPath.c_str() );
			fprintf( stderr, "<HP2_ACTOR_TRANSITION_LEDGER> report_error=write_failed path=%s\n", ReportPath.c_str() );
			return;
		}
		fprintf( stdout, "<HP2_ACTOR_TRANSITION_LEDGER> report=%s owners=%d truncated=%d\n",
			ReportPath.c_str(), (INT)Owners.size(), (INT)Truncated );
	}

private:
	struct Slot
	{
		UBOOL Present;
		std::string Path;
		std::string Class;
		Slot() : Present(0) {}
	};
	struct Snapshot
	{
		UBOOL Set;
		Slot ActiveSlot;
		UBOOL DeleteMarked;
		UBOOL PendingKill;
		std::string EffectiveShadowClass;
		std::string ShadowClassCdo;
		Snapshot() : Set(0), DeleteMarked(0), PendingKill(0) {}
	};
	struct Owner
	{
		std::string Identity;
		Slot FirstObservedActor;
		Snapshot PostInitExecution;
		Snapshot PreBeginBefore;
		Snapshot PreBeginAfter;
		UBOOL SpawnRequested;
		std::string SpawnClass;
		std::string SpawnName;
		std::string SpawnOwner;
		UBOOL SpawnResultSet;
		Slot SpawnResult;
		UBOOL SpawnPublishedSet;
		Slot SpawnPublished;
		std::string ShadowPostContinuation;
		UBOOL RendererCandidateSet;
		std::string RendererCandidate;
		Owner() : SpawnRequested(0), SpawnResultSet(0), SpawnPublishedSet(0), RendererCandidateSet(0) {}
	};

	static size_t ParseLimit( const char* Value )
	{
		const unsigned long long DefaultLimit = 4096;
		if( !Value || !Value[0] )
			return DefaultLimit;
		char* End = NULL;
		const unsigned long long Parsed = strtoull( Value, &End, 10 );
		return End && !*End ? (size_t)Min<unsigned long long>( Parsed, 65536 ) : DefaultLimit;
	}
	static std::string Text( const TCHAR* Value )
	{
		const char* Ansi = Value ? appToAnsi( Value ) : "";
		return Ansi ? Ansi : "";
	}
	static std::string Escape( const std::string& Value )
	{
		std::string Escaped;
		for( std::string::const_iterator It = Value.begin(); It != Value.end(); ++It )
		{
			switch( *It )
			{
			case '\\': Escaped += "\\\\"; break;
			case '"': Escaped += "\\\""; break;
			case '\n': Escaped += "\\n"; break;
			case '\r': Escaped += "\\r"; break;
			case '\t': Escaped += "\\t"; break;
			default: Escaped += *It; break;
			}
		}
		return Escaped;
	}
	static std::string SlotPath( UObject* Object )
	{
		return Object ? Text( Object->GetPathName() ) : "";
	}
	static std::string JsonStringOrNull( const std::string& Value )
	{
		return Value.empty() ? "null" : "\"" + Escape(Value) + "\"";
	}
	static Slot MakeSlot( AActor* Actor )
	{
		Slot Result;
		if( Actor )
		{
			Result.Present = 1;
			Result.Path = SlotPath( Actor );
			Result.Class = SlotPath( Actor->GetClass() );
		}
		return Result;
	}
	static std::string SlotJson( const Slot& Value )
	{
		return std::string("{\"path\":") + (Value.Present ? JsonStringOrNull(Value.Path) : "null")
			+ ",\"class\":" + (Value.Present ? JsonStringOrNull(Value.Class) : "null") + "}";
	}
	static UClass* ShadowClassDefiner( AActor* Actor )
	{
		UProperty* Property = Actor ? FindField<UProperty>( Actor->GetClass(), TEXT("ShadowClass") ) : NULL;
		return Property ? Cast<UClass>( Property->GetOuter() ) : NULL;
	}
	static Snapshot MakeSnapshot( AActor* Actor )
	{
		Snapshot Result;
		if( !Actor )
			return Result;
		Result.Set = 1;
		Result.ActiveSlot = MakeSlot( Actor );
		Result.DeleteMarked = Actor->bDeleteMe;
		Result.PendingKill = Actor->IsPendingKill();
		UProperty* Property = FindField<UProperty>( Actor->GetClass(), TEXT("ShadowClass") );
		UObject* ShadowClass = Property ? *(UObject**)((BYTE*)Actor + Property->Offset) : NULL;
		Result.EffectiveShadowClass = SlotPath( ShadowClass );
		UClass* Definer = ShadowClassDefiner( Actor );
		Result.ShadowClassCdo = Definer ? SlotPath( Definer->GetDefaultActor() ) : "";
		return Result;
	}
	static std::string SnapshotJson( const Snapshot& Value )
	{
		return std::string("{\"active_slot\":") + SlotJson(Value.ActiveSlot)
			+ ",\"delete_marked\":" + (Value.DeleteMarked ? "true" : "false")
			+ ",\"pending_kill\":" + (Value.PendingKill ? "true" : "false")
			+ ",\"effective_shadow_class\":" + JsonStringOrNull(Value.EffectiveShadowClass)
			+ ",\"shadow_class_cdo\":" + JsonStringOrNull(Value.ShadowClassCdo) + "}";
	}
	static INT ActiveSlot( AActor* Actor )
	{
		ULevel* Level = Actor ? Actor->XLevel : NULL;
		if( !Level )
			return INDEX_NONE;
		for( INT Slot = 0; Slot < Level->Actors.Num(); ++Slot )
			if( Level->Actors(Slot) == Actor )
				return Slot;
		return INDEX_NONE;
	}
	Owner* FindOrAdd( AActor* Actor )
	{
		if( !Actor )
			return NULL;
		std::string Identity = std::string("<map>.") + Text(Actor->GetName());
		if( Identity.empty() || Identity.find('.') == std::string::npos )
			return NULL;
		const std::string ActorPath = SlotPath( Actor );
		for( std::vector<Owner>::iterator It = Owners.begin(); It != Owners.end(); ++It )
			if( It->Identity == Identity )
			{
				if( It->FirstObservedActor.Path == ActorPath )
					return &*It;
				Identity += "#" + std::to_string( ActiveSlot(Actor) );
				break;
			}
		for( std::vector<Owner>::iterator It = Owners.begin(); It != Owners.end(); ++It )
			if( It->Identity == Identity && It->FirstObservedActor.Path == ActorPath )
				return &*It;
		if( Owners.size() >= Limit )
		{
			Truncated = 1;
			return NULL;
		}
		Owner Entry;
		Entry.Identity = Identity;
		Entry.FirstObservedActor = MakeSlot( Actor );
		Owners.push_back( Entry );
		return &Owners.back();
	}
	void SetSnapshot( AActor* Actor, Snapshot Owner::* Field )
	{
		Owner* Entry = FindOrAdd( Actor );
		if( Entry )
			Entry->*Field = MakeSnapshot( Actor );
	}
	std::string SerializeOwner( const Owner& Value ) const
	{
		std::string Json = "{\"identity\":\"" + Escape(Value.Identity) + "\",\"first_observed_actor\":" + SlotJson(Value.FirstObservedActor);
		Json += ",\"post_init_execution\":" + SnapshotJson(Value.PostInitExecution);
		Json += ",\"pre_begin_before\":" + SnapshotJson(Value.PreBeginBefore);
		Json += ",\"pre_begin_after\":" + SnapshotJson(Value.PreBeginAfter);
		Json += ",\"spawn\":{\"request\":";
		if( Value.SpawnRequested )
			Json += "{\"class\":" + JsonStringOrNull(Value.SpawnClass) + ",\"name\":" + JsonStringOrNull(Value.SpawnName) + ",\"owner\":" + JsonStringOrNull(Value.SpawnOwner) + "}";
		else
			Json += "null";
		Json += ",\"result\":" + (Value.SpawnResultSet ? SlotJson(Value.SpawnResult) : "null");
		Json += ",\"published_child_slot\":" + (Value.SpawnPublishedSet ? SlotJson(Value.SpawnPublished) : "null") + "}";
		Json += ",\"shadow_post_continuation\":" + JsonStringOrNull(Value.ShadowPostContinuation);
		Json += ",\"first_renderer_candidate\":" + JsonStringOrNull(Value.RendererCandidate) + "}";
		return Json;
	}
	std::string Serialize() const
	{
		std::string Json = "{\"version\":2,\"enabled\":true,\"limit\":" + std::to_string(Limit)
			+ ",\"count\":" + std::to_string(Owners.size()) + ",\"truncated\":" + (Truncated ? "true" : "false") + ",\"owners\":[";
		for( std::vector<Owner>::const_iterator It = Owners.begin(); It != Owners.end(); ++It )
		{
			if( It != Owners.begin() )
				Json += ",";
			Json += SerializeOwner( *It );
		}
		return Json + "]}\n";
	}

	std::string ReportPath;
	size_t Limit;
	UBOOL Truncated;
	std::vector<Owner> Owners;
};

// A per-Tick, bounded observation of the interpreted CreatureGenerator path.
// It observes existing script/native calls only; property reads never resolve
// references or alter the VM's scheduler/RNG state.
class CreatureGeneratorTraceReporter
{
public:
	static UBOOL IsEnabled()
	{
		const char* Path = getenv( "HP2_CREATURE_GENERATOR_TRACE" );
		return Path && Path[0];
	}
	CreatureGeneratorTraceReporter()
		: ReportPath(getenv("HP2_CREATURE_GENERATOR_TRACE"))
		, Limit(ParseLimit(getenv("HP2_CREATURE_GENERATOR_TRACE_LIMIT")))
		, Frame(0), NextOrder(0), Truncated(0), ActiveGenerator(NULL)
	{}
	void SetFrame( unsigned long long InFrame ) { Frame = InFrame; NextOrder = 0; }
	void TickDispatch( AActor* Actor, FLOAT DeltaSeconds )
	{
		if( !IsCreature(Actor) || Events.size() >= Limit )
		{
			if( IsCreature(Actor) ) Truncated = 1;
			return;
		}
		Event Sample;
		Sample.Owner = Actor;
		Sample.Identity = NormalizedIdentity(Actor);
		Sample.Frame = Frame;
		Sample.Order = NextOrder++;
		Sample.DeltaSeconds = DeltaSeconds;
		FloatProperty(Actor,TEXT("fCurrTime"),Sample.CurrTime);
		FloatProperty(Actor,TEXT("fWaitTime"),Sample.WaitTime);
		BoolProperty(Actor,TEXT("bOff"),Sample.BOff);
		FloatProperty(Actor,TEXT("TriggerWaitingTime"),Sample.TriggerWaitingTime);
		BoolProperty(Actor,TEXT("bGenerateCreature"),Sample.GenerateCreature);
		Sample.Tag = Actor->Tag == NAME_None ? "" : Text(*Actor->Tag);
		Sample.FirstPP = MakeSlot(ObjectProperty(Actor,TEXT("FirstPP")));
		Sample.BaseCreatureCount = ContiguousBaseCreatureCount(Actor);
		AppendGate(Sample,"tick_wait",Sample.CurrTime + DeltaSeconds >= Sample.WaitTime);
		Events.push_back(Sample);
	}
	void FunctionEnter( UObject* Object, UFunction* Function )
	{
		AActor* Actor = Cast<AActor>(Object);
		if( !IsCreature(Actor) || !Function )
			return;
		if( FunctionNamed(Function,"GenerateCreature") )
			ActiveGenerator = Actor;
	}
	void FunctionExit( UObject* Object, UFunction* Function, void* Result )
	{
		AActor* Actor = Cast<AActor>(Object);
		if( !IsCreature(Actor) || !Function || ActiveGenerator != Actor )
			return;
		if( FunctionNamed(Function,"CameraCanSeeYou") )
		{
			Event* Sample = Current(Actor);
			if( Sample )
			{
				Sample->HasCameraVisible = 1;
				Sample->CameraVisible = Result && *(bool*)Result;
				AppendGate(*Sample,"camera_visible",!Sample->CameraVisible);
				if( !Sample->CameraVisible )
					AppendBaseCreatureGate(*Sample,Actor);
			}
		}
		else if( FunctionNamed(Function,"GenerateCreature") )
		{
			Event* Sample = Current(Actor);
			if( Sample )
				IntProperty(Actor,TEXT("MaxCreatures"),Sample->ActiveMax);
			ActiveGenerator = NULL;
		}
	}
	void SoftwareRendering( AActor* Actor, UBOOL IsSoftwareRendering )
	{
		if( ActiveGenerator == NULL || !Actor )
			return;
		Event* Sample = Current(ActiveGenerator);
		if( !Sample || Sample->GeneratePrefixRecorded )
			return;
		INT Fast = 0, Slow = 0;
		IntProperty(ActiveGenerator,TEXT("MaxCreaturesOnFastMachine"),Fast);
		IntProperty(ActiveGenerator,TEXT("MaxCreaturesOnSlowMachine"),Slow);
		Sample->GeneratePrefixRecorded = 1;
		Sample->ActiveMax = IsSoftwareRendering ? Slow : Fast;
		IntProperty(ActiveGenerator,TEXT("NumCreatures"),Sample->ActiveCount);
		AppendGeneratePrefix(*Sample,ActiveGenerator);
	}
	void Rand( UObject* Object, INT Raw, INT Bound, INT Index )
	{
		AActor* Actor = Cast<AActor>(Object);
		if( Actor != ActiveGenerator )
			return;
		Event* Sample = Current(Actor);
		if( Sample )
			Sample->Rng.push_back(RngSample("rand",Raw,Bound,Index,0.f,0.f,0.f));
	}
	void RandRange( UObject* Object, INT Raw, FLOAT Min, FLOAT Max, FLOAT Value )
	{
		AActor* Actor = Cast<AActor>(Object);
		if( !IsCreature(Actor) )
			return;
		Event* Sample = Current(Actor);
		if( Sample )
			Sample->Rng.push_back(RngSample("rand_range",Raw,0,0,Min,Max,Value));
	}
	void ClassResolution( UClass* Class, AActor* Owner )
	{
		Event* Sample = Current(Owner);
		if( !Sample || Sample->ClassResolutionRecorded )
			return;
		Sample->ClassResolutionRecorded = 1;
		AppendGate(*Sample,"class_resolution",Class != NULL);
		for( std::vector<Rng>::reverse_iterator It = Sample->Rng.rbegin(); It != Sample->Rng.rend(); ++It )
			if( It->Kind == "rand" && !It->HasResolvedClass )
			{
				It->HasResolvedClass = 1;
				if( Class )
					It->ResolvedClass = SlotPath(Class);
				break;
			}
	}
	void SpawnRequest( UClass* Class, AActor* Owner )
	{
		Event* Sample = Current(Owner);
		if( !Sample )
			return;
		Sample->HasSpawnRequest = 1;
		Sample->SpawnRequest = MakeSlot(Class);
	}
	void SpawnResult( AActor* Owner, AActor* Child )
	{
		Event* Sample = Current(Owner);
		if( Sample ) { Sample->HasSpawnResult = 1; Sample->SpawnResult = MakeSlot(Child); }
	}
	void SpawnPublished( AActor* Owner, AActor* Child )
	{
		Event* Sample = Current(Owner);
		if( Sample ) { Sample->HasSpawnPublished = 1; Sample->SpawnPublished = MakeSlot(Child); }
	}
	void Flush()
	{
		const std::string TemporaryPath = ReportPath + ".tmp";
		FILE* Report = fopen(TemporaryPath.c_str(),"wb");
		if( !Report ) { fprintf(stderr,"<HP2_CREATURE_GENERATOR_TRACE> report_error=unwritable path=%s\n",ReportPath.c_str()); return; }
		const std::string Document = Serialize();
		const size_t Written = fwrite(Document.data(),1,Document.size(),Report);
		const int CloseResult = fclose(Report);
		if( Written != Document.size() || CloseResult != 0 || rename(TemporaryPath.c_str(),ReportPath.c_str()) != 0 )
		{
			remove(TemporaryPath.c_str());
			fprintf(stderr,"<HP2_CREATURE_GENERATOR_TRACE> report_error=write_failed path=%s\n",ReportPath.c_str());
			return;
		}
		fprintf(stdout,"<HP2_CREATURE_GENERATOR_TRACE> report=%s events=%d truncated=%d\n",ReportPath.c_str(),(INT)Events.size(),(INT)Truncated);
	}
private:
	struct Slot { UBOOL Present; std::string Path, Class; Slot():Present(0){} };
	struct Gate { std::string Name; UBOOL Passed; Gate(const char* InName,UBOOL InPassed):Name(InName),Passed(InPassed){} };
	struct Rng
	{
		std::string Kind, ResolvedClass; INT Raw, Bound, Index; FLOAT Min, Max, Value; UBOOL HasResolvedClass;
		Rng(const char* InKind,INT InRaw,INT InBound,INT InIndex,FLOAT InMin,FLOAT InMax,FLOAT InValue)
			:Kind(InKind),Raw(InRaw),Bound(InBound),Index(InIndex),Min(InMin),Max(InMax),Value(InValue),HasResolvedClass(0){}
	};
	struct Event
	{
		AActor* Owner; std::string Identity, Tag; unsigned long long Frame; INT Order;
		FLOAT CurrTime, WaitTime, DeltaSeconds, TriggerWaitingTime; UBOOL BOff, GenerateCreature;
		INT ActiveCount, ActiveMax, BaseCreatureCount; Slot FirstPP; UBOOL HasCameraVisible, CameraVisible;
		std::vector<Gate> Gates; std::vector<Rng> Rng; Slot SpawnRequest;
		UBOOL HasSpawnRequest, HasSpawnResult, HasSpawnPublished, GeneratePrefixRecorded, ClassResolutionRecorded; Slot SpawnResult, SpawnPublished;
		Event():Owner(NULL),Frame(0),Order(0),CurrTime(0),WaitTime(0),DeltaSeconds(0),TriggerWaitingTime(0),BOff(0),GenerateCreature(0),ActiveCount(0),ActiveMax(0),BaseCreatureCount(0),HasCameraVisible(0),CameraVisible(0),HasSpawnRequest(0),HasSpawnResult(0),HasSpawnPublished(0),GeneratePrefixRecorded(0),ClassResolutionRecorded(0){}
	};
	static size_t ParseLimit(const char* Value)
	{
		const unsigned long long DefaultLimit=4096; if(!Value||!Value[0]) return DefaultLimit;
		char* End=NULL; const unsigned long long Parsed=strtoull(Value,&End,10);
		return End&&!*End ? (size_t)Min<unsigned long long>(Parsed,65536) : DefaultLimit;
	}
	static std::string Text(const TCHAR* Value) { const char* Ansi=Value?appToAnsi(Value):""; return Ansi?Ansi:""; }
	static std::string Escape(const std::string& Value)
	{
		std::string Escaped; for(std::string::const_iterator It=Value.begin();It!=Value.end();++It) switch(*It)
		{ case '\\':Escaped+="\\\\";break; case '"':Escaped+="\\\"";break; case '\n':Escaped+="\\n";break; case '\r':Escaped+="\\r";break; case '\t':Escaped+="\\t";break; default:Escaped+=*It; }
		return Escaped;
	}
	static std::string SlotPath(UObject* Object) { return Object?Text(Object->GetPathName()):""; }
	static std::string JsonStringOrNull(const std::string& Value) { return Value.empty() ? "null" : "\"" + Escape(Value) + "\""; }
	static Slot MakeSlot(UObject* Object)
	{
		Slot Result; if(Object) { Result.Present=1; Result.Path=SlotPath(Object); Result.Class=SlotPath(Object->GetClass()); } return Result;
	}
	static std::string SlotJson(const Slot& Value)
	{
		return std::string("{\"path\":")+(Value.Present?JsonStringOrNull(Value.Path):"null")+",\"class\":"+(Value.Present?JsonStringOrNull(Value.Class):"null")+"}";
	}
	static UBOOL IsCreature(AActor* Actor)
	{
		for(UClass* Class=Actor?Actor->GetClass():NULL;Class;Class=Class->GetSuperClass())
			if(appStricmp(Class->GetName(),TEXT("CreatureGenerator"))==0) return 1;
		return 0;
	}
	static UBOOL FunctionNamed(UFunction* Function,const char* Name) { return appStricmp(Function->GetName(),appFromAnsi(Name))==0; }
	static UProperty* Property(UObject* Object,const TCHAR* Name) { return Object&&Object->GetClass()?FindField<UProperty>(Object->GetClass(),Name):NULL; }
	static UObject* ObjectProperty(UObject* Object,const TCHAR* Name) { UProperty* Field=Property(Object,Name); return Field?*(UObject**)((BYTE*)Object+Field->Offset):NULL; }
	static UBOOL BoolProperty(UObject* Object,const TCHAR* Name,UBOOL& Out)
	{
		UBoolProperty* Field=Cast<UBoolProperty>(Property(Object,Name)); if(!Field) return 0; Out=((DWORD*)((BYTE*)Object+Field->Offset))[0]&Field->BitMask; return 1;
	}
	static UBOOL IntProperty(UObject* Object,const TCHAR* Name,INT& Out)
	{
		UIntProperty* Field=Cast<UIntProperty>(Property(Object,Name)); if(!Field) return 0; Out=*(INT*)((BYTE*)Object+Field->Offset); return 1;
	}
	static UBOOL FloatProperty(UObject* Object,const TCHAR* Name,FLOAT& Out)
	{
		UFloatProperty* Field=Cast<UFloatProperty>(Property(Object,Name)); if(!Field) return 0; Out=*(FLOAT*)((BYTE*)Object+Field->Offset); return 1;
	}
	static INT ContiguousBaseCreatureCount(AActor* Actor)
	{
		UProperty* Field=Property(Actor,TEXT("BaseCreatureToSpawn")); if(!Field) return 0;
		INT Count=0; for(INT Index=0;Index<Field->ArrayDim;++Index) { if(!*(UObject**)((BYTE*)Actor+Field->Offset+Index*Field->ElementSize)) break; ++Count; } return Count;
	}
	static std::string NormalizedIdentity(AActor* Actor) { return std::string("<map>.")+Text(Actor->GetName()); }
	static void AppendGate(Event& Sample,const char* Name,UBOOL Passed) { Sample.Gates.push_back(Gate(Name,Passed)); }
	static Rng RngSample(const char* Kind,INT Raw,INT Bound,INT Index,FLOAT Min,FLOAT Max,FLOAT Value) { return Rng(Kind,Raw,Bound,Index,Min,Max,Value); }
	Event* Current(AActor* Actor)
	{
		if(!IsCreature(Actor)) return NULL;
		for(std::vector<Event>::reverse_iterator It=Events.rbegin();It!=Events.rend();++It) if(It->Owner==Actor) return &*It;
		return NULL;
	}
	void AppendGeneratePrefix(Event& Sample,AActor* Actor)
	{
		AppendGate(Sample,"b_off",!Sample.BOff); if(Sample.BOff) return;
		const UBOOL TriggerPass=!(Sample.TriggerWaitingTime!=0.f&&!Sample.GenerateCreature);
		AppendGate(Sample,"trigger_waiting",TriggerPass); if(!TriggerPass) return;
		INT Count=0; IntProperty(Actor,TEXT("NumCreatures"),Count);
		Sample.ActiveCount=Count;
		const UBOOL CapacityPass=Count<Sample.ActiveMax;
		AppendGate(Sample,"active_at_capacity",CapacityPass); if(!CapacityPass) return;
		Sample.FirstPP=MakeSlot(ObjectProperty(Actor,TEXT("FirstPP")));
		AppendGate(Sample,"missing_first_pp",Sample.FirstPP.Present); if(!Sample.FirstPP.Present) return;
	}
	void AppendBaseCreatureGate(Event& Sample,AActor* Actor)
	{
		Sample.BaseCreatureCount=ContiguousBaseCreatureCount(Actor);
		AppendGate(Sample,"empty_base_creatures",Sample.BaseCreatureCount>0);
	}
	std::string SerializeEvent(const Event& Sample) const
	{
		std::string Json="{\"identity\":\""+Escape(Sample.Identity)+"\",\"frame\":"+std::to_string(Sample.Frame);
		Json+=std::string(",\"tick\":{\"eligible\":")+(Sample.Gates[0].Passed?"true":"false")+",\"order\":"+std::to_string(Sample.Order)+"}";
		Json+=",\"timing\":{\"curr_time\":"+std::to_string(Sample.CurrTime)+",\"wait_time\":"+std::to_string(Sample.WaitTime)+",\"delta_seconds\":"+std::to_string(Sample.DeltaSeconds)+"}";
		Json+=std::string(",\"b_off\":")+(Sample.BOff?"true":"false")+",\"trigger\":{\"waiting_time\":"+std::to_string(Sample.TriggerWaitingTime)+",\"generate_creature\":"+(Sample.GenerateCreature?"true":"false")+",\"tag\":"+JsonStringOrNull(Sample.Tag)+"}";
		Json+=",\"active\":{\"count\":"+std::to_string(Sample.ActiveCount)+",\"max\":"+std::to_string(Sample.ActiveMax)+"},\"first_pp\":"+SlotJson(Sample.FirstPP)+",\"camera_visible\":"+(Sample.HasCameraVisible?(Sample.CameraVisible?"true":"false"):"null")+",\"base_creature_count\":"+std::to_string(Sample.BaseCreatureCount)+",\"gates\":[";
		for(std::vector<Gate>::const_iterator It=Sample.Gates.begin();It!=Sample.Gates.end();++It) { if(It!=Sample.Gates.begin()) Json+=","; Json+="{\"name\":\""+Escape(It->Name)+"\",\"passed\":"+(It->Passed?"true":"false")+"}"; }
		Json+="],\"rng\":[";
		for(std::vector<Rng>::const_iterator It=Sample.Rng.begin();It!=Sample.Rng.end();++It) { if(It!=Sample.Rng.begin()) Json+=","; Json+="{\"kind\":\""+Escape(It->Kind)+"\",\"raw\":"+std::to_string(It->Raw)+",\"index\":"+(It->Kind=="rand"?std::to_string(It->Index):"null")+",\"resolved_class\":"+(It->HasResolvedClass?JsonStringOrNull(It->ResolvedClass):"null"); if(It->Kind=="rand") Json+=",\"bound\":"+std::to_string(It->Bound); else Json+=",\"min\":"+std::to_string(It->Min)+",\"max\":"+std::to_string(It->Max)+",\"result\":"+std::to_string(It->Value); Json+="}"; }
		Json+="],\"spawn\":{\"request\":"+(Sample.HasSpawnRequest?SlotJson(Sample.SpawnRequest):"null")+",\"result\":"+(Sample.HasSpawnResult?SlotJson(Sample.SpawnResult):"null")+",\"published_child\":"+(Sample.HasSpawnPublished?SlotJson(Sample.SpawnPublished):"null")+"}}";
		return Json;
	}
	std::string Serialize() const
	{
		std::string Json="{\"version\":1,\"enabled\":true,\"limit\":"+std::to_string(Limit)+",\"count\":"+std::to_string(Events.size())+",\"truncated\":"+(Truncated?"true":"false")+",\"events\":[";
		for(std::vector<Event>::const_iterator It=Events.begin();It!=Events.end();++It) { if(It!=Events.begin()) Json+=","; Json+=SerializeEvent(*It); }
		return Json+"]}\n";
	}
	std::string ReportPath; size_t Limit; unsigned long long Frame; INT NextOrder; UBOOL Truncated; AActor* ActiveGenerator; std::vector<Event> Events;
};

// A bounded, opt-in observation of every native AActor::Tick invocation.
// It records existing scheduling facts only after the Tick body returns, so
// event admission and bTicked are the engine's actual outcomes.
class GlobalTickTraceReporter
{
public:
	static UBOOL IsEnabled()
	{
		const char* Path = getenv( "HP2_GLOBAL_TICK_TRACE" );
		return Path && Path[0];
	}

	GlobalTickTraceReporter()
		: ReportPath( getenv( "HP2_GLOBAL_TICK_TRACE" ) )
		, Limit( ParseLimit( getenv( "HP2_GLOBAL_TICK_TRACE_LIMIT" ) ) )
		, Frame( 0 ), NextOrdinal( 0 ), Truncated( 0 )
	{}

	void SetFrame( unsigned long long InFrame )
	{
		Frame = InFrame;
	}

	unsigned long long BeginDispatch( AActor* Actor, const char* Relation, const char* RootAdmission )
	{
		if( Events.size() >= Limit )
		{
			Truncated = 1;
			return ~0ULL;
		}
		Event Value;
		Value.Actor = Actor;
		Value.Frame = Frame;
		Value.Ordinal = NextOrdinal++;
		Value.RngBefore = appGetRandTraceOrdinal();
		Value.Path = ActorPath( Actor );
		Value.Class = ClassPath( Actor );
		Value.Relation = Relation ? Relation : "direct";
		Value.RootAdmission = RootAdmission ? RootAdmission : "dynamic";
		Events.push_back( Value );
		const unsigned long long Token = Events.size() - 1;
		ActiveDispatches.push_back( Token );
		return Token;
	}

	void EventTick( unsigned long long Token )
	{
		if( Token < Events.size() )
			Events[Token].TickOutcome = "dispatched";
	}

	void ProcessState( AActor* Actor, UObject* State, const char* Outcome )
	{
		for( std::vector<unsigned long long>::reverse_iterator It=ActiveDispatches.rbegin(); It!=ActiveDispatches.rend(); ++It )
		{
			if( *It < Events.size() && Events[*It].Actor==Actor )
			{
				Event& Value = Events[*It];
				Value.ProcessStateIdentity = State ? Text(State->GetPathName()) : "None";
				Value.ProcessStateOutcome = Outcome ? Outcome : "not_called";
				return;
			}
		}
	}

	void EndDispatch( unsigned long long Token, const char* TickOutcome )
	{
		if( !ActiveDispatches.empty() )
			ActiveDispatches.pop_back();
		if( Token >= Events.size() )
			return;
		Event& Value = Events[Token];
		Value.TickOutcome = TickOutcome ? TickOutcome : "skip_no_event";
		Value.TickStamp = Value.Actor ? (unsigned long long)Value.Actor->bTicked : 0;
		Value.RngAfter = appGetRandTraceOrdinal();
	}

	void Flush()
	{
		const std::string TemporaryPath = ReportPath + ".tmp";
		FILE* Report = fopen( TemporaryPath.c_str(), "wb" );
		if( !Report )
		{
			fprintf( stderr, "<HP2_GLOBAL_TICK_TRACE> report_error=unwritable path=%s\n", ReportPath.c_str() );
			return;
		}
		const std::string Document = Serialize();
		const size_t Written = fwrite( Document.data(), 1, Document.size(), Report );
		const int CloseResult = fclose( Report );
		if( Written != Document.size() || CloseResult != 0 || rename( TemporaryPath.c_str(), ReportPath.c_str() ) != 0 )
		{
			remove( TemporaryPath.c_str() );
			fprintf( stderr, "<HP2_GLOBAL_TICK_TRACE> report_error=write_failed path=%s\n", ReportPath.c_str() );
			return;
		}
		fprintf( stdout, "<HP2_GLOBAL_TICK_TRACE> report=%s events=%d truncated=%d\n",
			ReportPath.c_str(), (INT)Events.size(), (INT)Truncated );
	}

private:
	struct Event
	{
		AActor* Actor;
		unsigned long long Frame, Ordinal, TickStamp, RngBefore, RngAfter;
		std::string Path, Class, Relation, RootAdmission, TickOutcome, ProcessStateIdentity, ProcessStateOutcome;
		Event()
			: Actor( NULL ), Frame( 0 ), Ordinal( 0 ), TickStamp( 0 ), RngBefore( 0 ), RngAfter( 0 )
			, TickOutcome( "skip_no_event" ), ProcessStateIdentity( "None" ), ProcessStateOutcome( "not_called" )
		{}
	};

	static size_t ParseLimit( const char* Value )
	{
		const unsigned long long DefaultLimit = 16384, MaximumLimit = 262144;
		if( !Value || !Value[0] )
			return DefaultLimit;
		char* End = NULL;
		const unsigned long long Parsed = strtoull( Value, &End, 10 );
		return End && !*End ? (size_t)Min<unsigned long long>( Parsed, MaximumLimit ) : DefaultLimit;
	}

	static std::string Text( const TCHAR* Value )
	{
		const char* Ansi = Value ? appToAnsi( Value ) : "";
		return Ansi ? Ansi : "";
	}

	static std::string Escape( const std::string& Value )
	{
		std::string Escaped;
		for( std::string::const_iterator It=Value.begin(); It!=Value.end(); ++It )
			switch( *It )
			{
			case '\\': Escaped += "\\\\"; break;
			case '"': Escaped += "\\\""; break;
			case '\n': Escaped += "\\n"; break;
			case '\r': Escaped += "\\r"; break;
			case '\t': Escaped += "\\t"; break;
			default: Escaped += *It; break;
			}
		return Escaped;
	}

	static std::string ActorPath( AActor* Actor )
	{
		return std::string( "<map>." ) + (Actor ? Text( Actor->GetName() ) : "<missing>");
	}

	static std::string ClassPath( AActor* Actor )
	{
		UClass* Class = Actor ? Actor->GetClass() : NULL;
		return Class ? Text( Class->GetPathName() ) : "<missing>";
	}

	std::string SerializeEvent( const Event& Value ) const
	{
		return "{\"frame\":" + std::to_string( Value.Frame )
			+ ",\"ordinal\":" + std::to_string( Value.Ordinal )
			+ ",\"actor\":{\"path\":\"" + Escape( Value.Path )
			+ "\",\"class\":\"" + Escape( Value.Class )
			+ "\"},\"relation\":\"" + Escape( Value.Relation )
			+ "\",\"root_admission\":\"" + Escape( Value.RootAdmission )
			+ "\",\"rng\":{\"before\":" + std::to_string( Value.RngBefore )
			+ ",\"after\":" + std::to_string( Value.RngAfter )
			+ "},\"tick\":{\"identity\":\"Tick\",\"outcome\":\"" + Escape( Value.TickOutcome )
			+ "\"},\"process_state\":{\"identity\":\"" + Escape( Value.ProcessStateIdentity )
			+ "\",\"outcome\":\"" + Escape( Value.ProcessStateOutcome )
			+ "\"},\"tick_stamp\":" + std::to_string( Value.TickStamp ) + "}";
	}

	std::string Serialize() const
	{
		std::string Json = "{\"version\":2,\"enabled\":true,\"limit\":" + std::to_string( Limit )
			+ ",\"count\":" + std::to_string( Events.size() )
			+ ",\"truncated\":" + (Truncated ? "true" : "false") + ",\"events\":[";
		for( std::vector<Event>::const_iterator It=Events.begin(); It!=Events.end(); ++It )
		{
			if( It != Events.begin() )
				Json += ",";
			Json += SerializeEvent( *It );
		}
		return Json + "]}\n";
	}

	std::string ReportPath;
	size_t Limit;
	unsigned long long Frame, NextOrdinal;
	UBOOL Truncated;
	std::vector<Event> Events;
	std::vector<unsigned long long> ActiveDispatches;
};

ActorTransitionLedger* GActorTransitionLedger = NULL;

CreatureGeneratorTraceReporter* GCreatureGeneratorTraceReporter = NULL;

GlobalTickTraceReporter* GGlobalTickTraceReporter = NULL;

ShadowAdmissionTraceReporter* GShadowAdmissionTraceReporter = NULL;

class ActorSlotDump
{
public:
	static UBOOL IsEnabled()
	{
		const char* ReportPath = getenv( "HP2_ACTOR_SLOT_DUMP" );
		return ReportPath && ReportPath[0];
	}

	static void CapturePostDeserialize( ULevel* )
	{
		// The raw callback from ULevelBase::Serialize is the only valid
		// deserialization boundary; this legacy LoadMap hook is intentionally inert.
	}
	static void CapturePostStartupBeforeFirstTick( UEngine* Engine )
	{
		if( !IsEnabled() )
			return;
		UGameEngine* GameEngine = Cast<UGameEngine>( Engine );
		ULevel* Level = GameEngine ? GameEngine->GLevel : NULL;
		if( Level && !RawActorTableOpen && RawLevel == Level && !RawPostDeserialize.empty() )
			PostStartupBeforeFirstTick = SerializeCheckpoint( Level, "post_startup_before_first_tick" );
	}


	static void BeginRawLevelActorTable( ULevelBase* Level )
	{
		if( !IsEnabled() )
			return;
		RawLevel = Level;
		RawActorCount = 0;
		RawActorTableOpen = 1;
		RawPostDeserialize = "{\"checkpoint\":\"post_deserialize_raw\",\"sequence\":\"raw\",\"i_first_net_relevant_actor\":null,\"i_first_dynamic_actor\":null,\"actors\":[";
	}

	static void CaptureRawLevelActor( ULevelBase* Level, INT Slot, AActor* Actor, INT ReferenceOffsetBefore, INT ReferenceOffsetAfter, INT CompactPackageIndex, UBOOL CompactPackageIndexCaptured, UBOOL CompactPackageIndexSupported )
	{
		if( !RawActorTableOpen || RawLevel != Level )
			return;
		if( RawActorCount++ )
			RawPostDeserialize += ",";
		RawPostDeserialize += RawActorJson( Slot, Actor, ReferenceOffsetBefore, ReferenceOffsetAfter, CompactPackageIndex, CompactPackageIndexCaptured, CompactPackageIndexSupported );
	}

	static void EndRawLevelActorTable( ULevelBase* Level )
	{
		if( RawActorTableOpen && RawLevel == Level )
		{
			RawPostDeserialize += "]}";
			RawActorTableOpen = 0;
		}
	}

	static UBOOL WriteIfReady( UEngine* Engine )
	{
		UGameEngine* GameEngine = Cast<UGameEngine>( Engine );
		ULevel* Level = GameEngine ? GameEngine->GLevel : NULL;
		if( !Level || RawPostDeserialize.empty() || PostStartupBeforeFirstTick.empty() || RawActorTableOpen || RawLevel != Level )
			return 0;

		const char* ReportPath = getenv( "HP2_ACTOR_SLOT_DUMP" );
		if( !ReportPath || !ReportPath[0] )
			return 1;

		const std::string Path( ReportPath );
		const std::string TemporaryPath = Path + ".tmp";
		FILE* Report = fopen( TemporaryPath.c_str(), "wb" );
		if( !Report )
		{
			fprintf( stderr, "<HP2_ACTOR_SLOT_DUMP> report_error=unwritable path=%s\n", Path.c_str() );
			return 1;
		}

		const std::string Document = Serialize( Level );
		const size_t Written = fwrite( Document.data(), 1, Document.size(), Report );
		const int CloseResult = fclose( Report );
		if( Written != Document.size() || CloseResult != 0 || rename( TemporaryPath.c_str(), Path.c_str() ) != 0 )
		{
			remove( TemporaryPath.c_str() );
			fprintf( stderr, "<HP2_ACTOR_SLOT_DUMP> report_error=write_failed path=%s\n", Path.c_str() );
		}
		return 1;
	}

private:
	static std::string Text( const TCHAR* Value )
	{
		const char* Ansi = Value ? appToAnsi( Value ) : "";
		return Ansi ? Ansi : "";
	}

	static std::string Escape( const std::string& Value )
	{
		std::string Escaped;
		for( std::string::const_iterator It = Value.begin(); It != Value.end(); ++It )
		{
			switch( *It )
			{
			case '\\': Escaped += "\\\\"; break;
			case '"': Escaped += "\\\""; break;
			case '\n': Escaped += "\\n"; break;
			case '\r': Escaped += "\\r"; break;
			case '\t': Escaped += "\\t"; break;
			default: Escaped += *It; break;
			}
		}
		return Escaped;
	}

	static UObject* ObjectProperty( UObject* Object, const TCHAR* Name )
	{
		UProperty* Field = Object && Object->GetClass()
			? FindField<UProperty>( Object->GetClass(), Name )
			: NULL;
		return Field ? *(UObject**)((BYTE*)Object + Field->Offset) : NULL;
	}

	static std::string ObjectPathJson( UObject* Object )
	{
		return Object ? "\"" + Escape(Text(Object->GetPathName())) + "\"" : "null";
	}

	static std::string ActorJson( INT Slot, AActor* Actor, ULevel* Level = NULL )
	{
		std::string Json = "{\"slot\":" + std::to_string(Slot);
		if( !Actor )
			return Json + ",\"path\":null,\"class\":null,\"is_null\":true,\"is_deleted\":false,\"is_pending_kill\":false,\"game_relevant\":null,\"role\":null,\"remote_role\":null,\"owner_path\":null,\"owner_class\":null,\"effective_shadow_class\":null,\"spawn_origin\":null,\"spawn_owner_path\":null,\"spawn_owner_class\":null,\"spawn_request_id\":null,\"spawn_order\":null}";

		AActor* Owner = Actor->Owner;
		UObject* ShadowClass = ObjectProperty( Actor, TEXT("ShadowClass") );
		UClass* OwnerShadowClass = Cast<UClass>( ObjectProperty( Owner, TEXT("ShadowClass") ) );
		const UBOOL IsDynamicActorShadow = Level
			&& Slot >= Level->iFirstDynamicActor
			&& OwnerShadowClass
			&& Actor->GetClass()->IsChildOf( OwnerShadowClass );
		Json += ",\"path\":\"" + Escape(Text(Actor->GetPathName())) + "\"";
		Json += ",\"class\":\"" + Escape(Text(Actor->GetClass()->GetPathName())) + "\"";
		Json += ",\"is_null\":false,\"is_deleted\":";
		Json += Actor->bDeleteMe ? "true" : "false";
		Json += ",\"is_pending_kill\":";
		Json += Actor->IsPendingKill() ? "true" : "false";
		Json += ",\"game_relevant\":";
		Json += Actor->bGameRelevant ? "true" : "false";
		Json += ",\"role\":" + std::to_string((INT)Actor->Role);
		Json += ",\"remote_role\":" + std::to_string((INT)Actor->RemoteRole);
		Json += ",\"owner_path\":" + ObjectPathJson(Owner);
		Json += ",\"owner_class\":" + ObjectPathJson(Owner ? Owner->GetClass() : NULL);
		Json += ",\"effective_shadow_class\":" + ObjectPathJson(ShadowClass);
		Json += ",\"spawn_origin\":";
		Json += IsDynamicActorShadow ? "\"level_dynamic_slot\"" : "null";
		Json += ",\"spawn_owner_path\":";
		Json += IsDynamicActorShadow ? ObjectPathJson(Owner) : "null";
		Json += ",\"spawn_owner_class\":";
		Json += IsDynamicActorShadow ? ObjectPathJson(Owner->GetClass()) : "null";
		Json += ",\"spawn_request_id\":null";
		Json += ",\"spawn_order\":";
		Json += IsDynamicActorShadow ? std::to_string(Slot - Level->iFirstDynamicActor) : "null";
		return Json + "}";
	}

	static std::string RawActorJson( INT Slot, AActor* Actor, INT ReferenceOffsetBefore, INT ReferenceOffsetAfter, INT CompactPackageIndex, UBOOL CompactPackageIndexCaptured, UBOOL CompactPackageIndexSupported )
	{
		std::string Json = ActorJson( Slot, Actor );
		Json.resize( Json.size() - 1 );
		Json += ",\"raw_reference\":{\"offset_before\":";
		Json += ReferenceOffsetBefore == INDEX_NONE ? "null" : std::to_string(ReferenceOffsetBefore);
		Json += ",\"offset_after\":";
		Json += ReferenceOffsetAfter == INDEX_NONE ? "null" : std::to_string(ReferenceOffsetAfter);
		Json += ",\"compact_package_index\":";
		Json += CompactPackageIndexCaptured ? std::to_string(CompactPackageIndex) : "null";
		if( !CompactPackageIndexSupported )
			Json += ",\"status\":\"unsupported\",\"reason\":\"archive_does_not_expose_actor_slot_compact_index\"";
		else if( !CompactPackageIndexCaptured )
			Json += ",\"status\":\"unavailable\",\"reason\":\"actor_reference_did_not_invoke_linker_compact_index_decoder\"";
		else if( ReferenceOffsetBefore == INDEX_NONE || ReferenceOffsetAfter == INDEX_NONE )
			Json += ",\"status\":\"unavailable\",\"reason\":\"archive_tell_unavailable\"";
		else
			Json += ",\"status\":\"observed\",\"reason\":null";
		return Json + "}}";
	}

	static std::string SerializeCheckpoint( ULevel* Level, const char* Checkpoint )
	{
		std::string Json = std::string("{\"checkpoint\":\"") + Checkpoint + "\",\"sequence\":\"rearranged\",\"i_first_net_relevant_actor\":" + std::to_string(Level->iFirstNetRelevantActor) + ",\"i_first_dynamic_actor\":" + std::to_string(Level->iFirstDynamicActor) + ",\"actors\":[";
		for( INT Slot = 0; Slot < Level->Actors.Num(); ++Slot )
		{
			if( Slot )
				Json += ",";
			Json += ActorJson( Slot, Level->Actors(Slot), Level );
		}
		return Json + "]}";
	}

	static std::string Serialize( ULevel* Level )
	{
		return "{\"version\":1,\"checkpoints\":[" + RawPostDeserialize + "," + PostStartupBeforeFirstTick + "," + SerializeCheckpoint( Level, "post_startup" ) + "]}\n";
	}

	static std::string RawPostDeserialize;
	static std::string PostStartupBeforeFirstTick;
	static ULevelBase* RawLevel;
	static INT RawActorCount;
	static UBOOL RawActorTableOpen;
};

std::string ActorSlotDump::RawPostDeserialize;
ULevelBase* ActorSlotDump::RawLevel = NULL;
std::string ActorSlotDump::PostStartupBeforeFirstTick;
INT ActorSlotDump::RawActorCount = 0;
UBOOL ActorSlotDump::RawActorTableOpen = 0;
// A deliberately narrow oracle for one retail FireTexture export. It observes
// Fire's own RNG and Mip0 seams; the launcher only invokes the normal texture
// lifecycle methods and never writes the texture data itself.
class FireTextureTraceReporter
{
public:
	static UBOOL IsEnabled()
	{
		const char* Path = getenv( "HP2_FIRE_TEXTURE_TRACE" );
		return Path && Path[0];
	}

	FireTextureTraceReporter()
		: ReportPath( getenv( "HP2_FIRE_TEXTURE_TRACE" ) )
		, Target( NULL ), Active( NULL ), InitSeen( 0 ), FirstSpeedSeen( 0 )
	{}

	void SetTarget( UFireTexture* Texture )
	{
		Target = Texture;
	}

	void SetActive( UFireTexture* Texture )
	{
		Active = Texture;
	}

	void InitTables( unsigned long long Before, unsigned long long After, unsigned long long Fingerprint )
	{
		if( !InitSeen )
		{
			InitSeen = 1;
			InitBefore = Before;
			InitAfter = After;
			InitFingerprint = Fingerprint;
		}
	}

	void SpeedRand( DWORD StateIndex, BYTE Value )
	{
		if( Active == Target && !FirstSpeedSeen )
		{
			FirstSpeedSeen = 1;
			FirstSpeedIndex = StateIndex;
			FirstSpeedValue = Value;
		}
	}

	void BurnWrite( UFireTexture* Texture, INT SparkIndex, DWORD Offset, BYTE Before, BYTE Value )
	{
		if( Active == Target && Texture == Target && Operation == "update" )
			BurnWrites.push_back( BurnWriteSample( SparkIndex, Offset, Before, Value ) );
	}

	void Probe( UFireTexture* Texture )
	{
		if( !Texture || Texture != Target )
			return;

		SetActive( Texture );
		FTextureInfo Info;
		TextureState Before = State( Texture );
		Operation = "lock";
		fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> phase=lock_begin\n" );
		Texture->Lock( Info, 0.0, 0, NULL );
		fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> phase=lock_end\n" );
		Steps.push_back( Step( "lock", Before, State(Texture), Info.bRealtimeChanged ) );

		Before = State( Texture );
		Operation = "update";
		fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> phase=update_begin\n" );
		Texture->Update( 1.0 );
		fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> phase=update_end\n" );
		Steps.push_back( Step( "update", Before, State(Texture), 0 ) );

		Before = State( Texture );
		Operation = "tick";
		fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> phase=tick_begin\n" );
		Texture->Tick( 1.0f );
		fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> phase=tick_end\n" );
		Steps.push_back( Step( "tick", Before, State(Texture), 0 ) );

		Before = State( Texture );
		Operation = "constant_time_tick";
		fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> phase=constant_time_tick_begin\n" );
		Texture->ConstantTimeTick();
		fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> phase=constant_time_tick_end\n" );
		Steps.push_back( Step( "constant_time_tick", Before, State(Texture), 0 ) );
		Operation.clear();
		SetActive( NULL );
		Texture->Unlock( Info );
	}

	void Flush()
	{
		const std::string TemporaryPath = ReportPath + ".tmp";
		FILE* Report = fopen( TemporaryPath.c_str(), "wb" );
		if( !Report )
		{
			fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> report_error=unwritable path=%s\n", ReportPath.c_str() );
			return;
		}
		const std::string Document = Serialize();
		const size_t Written = fwrite( Document.data(), 1, Document.size(), Report );
		const int CloseResult = fclose( Report );
		if( Written != Document.size() || CloseResult != 0 || rename( TemporaryPath.c_str(), ReportPath.c_str() ) != 0 )
		{
			remove( TemporaryPath.c_str() );
			fprintf( stderr, "<HP2_FIRE_TEXTURE_TRACE> report_error=write_failed path=%s\n", ReportPath.c_str() );
			return;
		}
		fprintf( stdout, "<HP2_FIRE_TEXTURE_TRACE> report=%s burn_writes=%d\n", ReportPath.c_str(), (INT)BurnWrites.size() );
	}

private:
	struct TextureState
	{
		UBOOL Realtime;
		UBOOL Dirty;
		TextureState() : Realtime(0), Dirty(0) {}
		TextureState( UBOOL InRealtime, UBOOL InDirty ) : Realtime(InRealtime), Dirty(InDirty) {}
	};
	struct Step
	{
		std::string Name;
		TextureState Before, After;
		UBOOL LockReportedDirty;
		Step( const char* InName, TextureState InBefore, TextureState InAfter, UBOOL InLockReportedDirty )
			: Name(InName), Before(InBefore), After(InAfter), LockReportedDirty(InLockReportedDirty) {}
	};
	struct BurnWriteSample
	{
		INT SparkIndex;
		DWORD Offset;
		BYTE Before, Value;
		BurnWriteSample( INT InSparkIndex, DWORD InOffset, BYTE InBefore, BYTE InValue )
			: SparkIndex(InSparkIndex), Offset(InOffset), Before(InBefore), Value(InValue) {}
	};

	static TextureState State( UFireTexture* Texture )
	{
		return TextureState( Texture->bRealtime, Texture->bRealtimeChanged );
	}
	static std::string Text( const TCHAR* Value )
	{
		const char* Ansi = Value ? appToAnsi( Value ) : "";
		return Ansi ? Ansi : "";
	}
	static std::string Escape( const std::string& Value )
	{
		std::string Escaped;
		for( std::string::const_iterator It=Value.begin(); It!=Value.end(); ++It )
		{
			switch( *It )
			{
			case '\\': Escaped += "\\\\"; break;
			case '"': Escaped += "\\\""; break;
			case '\n': Escaped += "\\n"; break;
			case '\r': Escaped += "\\r"; break;
			case '\t': Escaped += "\\t"; break;
			default: Escaped += *It; break;
			}
		}
		return Escaped;
	}
	static std::string Sha256( const std::string& Path )
	{
		FILE* Input = fopen( Path.c_str(), "rb" );
		if( !Input )
			return "";
		CC_SHA256_CTX Context;
		CC_SHA256_Init( &Context );
		BYTE Buffer[16384];
		size_t Count = 0;
		while( (Count = fread(Buffer,1,sizeof(Buffer),Input)) != 0 )
			CC_SHA256_Update( &Context, Buffer, (CC_LONG)Count );
		const bool ReadFailed = ferror( Input ) != 0;
		fclose( Input );
		if( ReadFailed )
			return "";
		BYTE Digest[CC_SHA256_DIGEST_LENGTH];
		CC_SHA256_Final( Digest, &Context );
		static const char Hex[] = "0123456789abcdef";
		std::string Result;
		Result.reserve( CC_SHA256_DIGEST_LENGTH * 2 );
		for( INT Index=0; Index<CC_SHA256_DIGEST_LENGTH; ++Index )
		{
			Result += Hex[Digest[Index] >> 4];
			Result += Hex[Digest[Index] & 15];
		}
		return Result;
	}
	static std::string Hex64( unsigned long long Value )
	{
		char Buffer[17];
		snprintf( Buffer, sizeof(Buffer), "%016llx", Value );
		return Buffer;
	}
	std::string Serialize() const
	{
		ULinkerLoad* Linker = Target ? Target->GetLinker() : NULL;
		const std::string ArchivePath = Linker ? Text(*Linker->Filename) : "";
		const std::string ArchiveSha = ArchivePath.empty() ? "" : Sha256(ArchivePath);
		const std::string ClassPath = Target && Target->GetClass() ? Text(Target->GetClass()->GetPathName()) : "";
		std::string Json = "{\"format\":\"hp2-fire-texture-trace\",\"version\":1,\"enabled\":true";
		Json += ",\"target\":{\"archive\":{\"path\":\"" + Escape(ArchivePath) + "\",\"sha256\":\"" + ArchiveSha + "\"}";
		Json += ",\"export\":{\"index\":" + std::to_string(Target ? Target->GetLinkerIndex() : -1) + ",\"path\":\"" + Escape(Target ? Text(Target->GetPathName()) : "") + "\"}";
		Json += ",\"class\":\"" + Escape(ClassPath) + "\",\"dimensions\":{\"u\":" + std::to_string(Target ? Target->USize : 0) + ",\"v\":" + std::to_string(Target ? Target->VSize : 0) + "}";
		Json += ",\"active_spark_count\":" + std::to_string(Target ? Target->ActiveSparkNum : 0) + "}";
		Json += ",\"init_tables\":{\"rng\":{\"before\":" + std::to_string(InitBefore) + ",\"after\":" + std::to_string(InitAfter) + "},\"fingerprint\":\"" + Hex64(InitFingerprint) + "\"}";
		Json += ",\"first_speed_rand\":";
		Json += FirstSpeedSeen
			? "{\"index\":" + std::to_string(FirstSpeedIndex) + ",\"value\":" + std::to_string(FirstSpeedValue) + "}"
			: "null";
		Json += ",\"burn_mip0_writes\":[";
		for( std::vector<BurnWriteSample>::const_iterator It=BurnWrites.begin(); It!=BurnWrites.end(); ++It )
		{
			if( It != BurnWrites.begin() )
				Json += ",";
			Json += "{\"spark_index\":" + std::to_string(It->SparkIndex) + ",\"offset\":" + std::to_string(It->Offset)
				+ ",\"before\":" + std::to_string(It->Before) + ",\"value\":" + std::to_string(It->Value) + "}";
		}
		Json += "],\"lifecycle\":[";
		for( std::vector<Step>::const_iterator It=Steps.begin(); It!=Steps.end(); ++It )
		{
			if( It != Steps.begin() )
				Json += ",";
			Json += "{\"step\":\"" + It->Name + "\",\"realtime\":{\"before\":" + (It->Before.Realtime ? "true" : "false")
				+ ",\"after\":" + (It->After.Realtime ? "true" : "false") + "},\"dirty\":{\"before\":"
				+ (It->Before.Dirty ? "true" : "false") + ",\"after\":" + (It->After.Dirty ? "true" : "false")
				+ ",\"lock_reported\":" + (It->LockReportedDirty ? "true" : "false") + "}}";
		}
		return Json + "]}\n";
	}

	std::string ReportPath;
	UFireTexture* Target;
	UFireTexture* Active;
	UBOOL InitSeen;
	unsigned long long InitBefore = 0, InitAfter = 0, InitFingerprint = 0;
	UBOOL FirstSpeedSeen;
	DWORD FirstSpeedIndex = 0;
	BYTE FirstSpeedValue = 0;
	std::string Operation;
	std::vector<BurnWriteSample> BurnWrites;
	std::vector<Step> Steps;
};

FireTextureTraceReporter* GFireTextureTraceReporter = NULL;


void MainLoop( UEngine* Engine, INT TestTicks, FLOAT FixedDeltaSeconds, LifecycleGateReporter* LifecycleReporter, WorldCollisionGateReporter* WorldCollisionReporter, StaticBspProbeReporter* StaticBspProbe )
{
	check( Engine );
	UBOOL ActorSlotDumpPending = ActorSlotDump::IsEnabled();
	ActorSlotDump::CapturePostStartupBeforeFirstTick( Engine );
	if( StaticBspProbe )
		StaticBspProbe->Run( Engine );
	if( WorldCollisionReporter )
		WorldCollisionReporter->Run( Engine );

	std::unique_ptr<FidelityReporter> Reporter;
	if( FidelityReporter::IsEnabled() )
		Reporter.reset( new FidelityReporter( Engine ) );

	// Loop while running.
	GIsRunning = 1;
	FTime OldTime = appSeconds();
	FTime SecondStartTime = OldTime;
	INT TickCount = 0;
	INT RemainingTestTicks = TestTicks;
	unsigned long long TraceTickIndex = 0;
	while( GIsRunning && !GIsRequestingExit )
	{
		if( GShadowAdmissionTraceReporter )
			GShadowAdmissionTraceReporter->SetFrame( TraceTickIndex );
		if( GCreatureGeneratorTraceReporter )
			GCreatureGeneratorTraceReporter->SetFrame( TraceTickIndex );
		if( GGlobalTickTraceReporter )
			GGlobalTickTraceReporter->SetFrame( TraceTickIndex );
		// A paired fidelity replay supplies the same positive fixed delta to
		// the C++ oracle and hp2rs. Normal launches retain wall-clock timing.
		FTime NewTime = appSeconds();
		FLOAT DeltaSeconds = FixedDeltaSeconds > 0.0f
			? FixedDeltaSeconds
			: Max( (FLOAT)(NewTime - OldTime), 0.0001f );
		if( LifecycleReporter )
			LifecycleReporter->BeginTick( (INT)TraceTickIndex );
		appSetRandTraceTick( TraceTickIndex, "engine_tick" );
		Engine->Tick( DeltaSeconds );
		appSetRandTraceTick( TraceTickIndex, "after_tick" );
		if( ActorSlotDumpPending && ActorSlotDump::WriteIfReady( Engine ) )
			ActorSlotDumpPending = 0;
		if( Reporter )
			Reporter->AfterTick( Engine, DeltaSeconds );
		OldTime = NewTime;
		++TickCount;
		++TraceTickIndex;
		if( RemainingTestTicks > 0 && --RemainingTestTicks == 0 )
			appRequestExit( 0 );
		if( OldTime - SecondStartTime > 1 )
		{
			Engine->CurrentTickRate = (FLOAT)TickCount / (FLOAT)(OldTime - SecondStartTime);
			SecondStartTime = OldTime;
			TickCount = 0;
		}

		// Enforce optional maximum tick rate. A fixed-delta replay is driven
		// by tick count, so it must not inherit the unfocused wall-clock cap.
		FLOAT TargetTickRate = Engine->GetMaxTickRate();
		if( FixedDeltaSeconds <= 0.0f && SDL_GetKeyboardFocus() == NULL )
			TargetTickRate = TargetTickRate > 0.f ? Min( TargetTickRate, 10.f ) : 10.f;
		if( TargetTickRate > 0.f )
			appSleep( Max( 0.f, (1.f / TargetTickRate) - (FLOAT)(appSeconds() - OldTime) ) );
	}
	GIsRunning = 0;
	debugf( TEXT("<HP2_RES> script_deferred=0") );
	debugf( TEXT("<HP2_RES> script_deferral_reasons=none") );
	debugf( TEXT("<HP2_RES> script_deferral_subjects=none") );
	if( Reporter )
		Reporter->Flush( TestTicks, FixedDeltaSeconds );
	if( LifecycleReporter )
		LifecycleReporter->Flush( TestTicks, FixedDeltaSeconds );
	if( WorldCollisionReporter )
		WorldCollisionReporter->Flush();
	if( StaticBspProbe )
		StaticBspProbe->Flush();
	if( GShadowAdmissionTraceReporter )
		GShadowAdmissionTraceReporter->Flush();
	if( GActorTransitionLedger )
		GActorTransitionLedger->Flush();
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->Flush();
	if( GGlobalTickTraceReporter )
		GGlobalTickTraceReporter->Flush();
}

void CleanUpOnExit( UEngine* Engine )
{
	if( Engine )
		Engine->Exit();
	appPreExit();
}

} // namespace

static void TraceFireTextureInitTables( unsigned long long Before, unsigned long long After, unsigned long long Fingerprint )
{
	if( GFireTextureTraceReporter )
		GFireTextureTraceReporter->InitTables( Before, After, Fingerprint );
}

static void TraceFireTextureSpeedRand( DWORD StateIndex, BYTE Value )
{
	if( GFireTextureTraceReporter )
		GFireTextureTraceReporter->SpeedRand( StateIndex, Value );
}

static void TraceFireTextureBurnWrite( UFireTexture* Texture, INT SparkIndex, DWORD Offset, BYTE Before, BYTE Value )
{
	if( GFireTextureTraceReporter )
		GFireTextureTraceReporter->BurnWrite( Texture, SparkIndex, Offset, Before, Value );
}

static INT TraceShadowAdmissionBeginPass()
{
	return GShadowAdmissionTraceReporter ? GShadowAdmissionTraceReporter->BeginPass() : 0;
}

static void TraceShadowAdmissionRecord(
	INT Pass,
	AActor* Owner,
	ADecal* Shadow,
	const char* CandidateStatus,
	UBOOL UpdateEligible,
	AActor* ViewportActor,
	AActor* ViewTarget,
	UBOOL BehindView,
	AActor* RecursionParentActor,
	UBOOL Perspective,
	UBOOL WorldDynamics )
{
	if( GShadowAdmissionTraceReporter )
	{
		GShadowAdmissionTraceReporter->Record(
			Pass,
			Owner,
			Shadow,
			CandidateStatus,
			UpdateEligible,
			ViewportActor,
			ViewTarget,
			BehindView,
			RecursionParentActor,
			Perspective,
			WorldDynamics );
	}
}

static void TraceActorTransitionPostInitExecution( AActor* Actor )
{
	if( GActorTransitionLedger )
		GActorTransitionLedger->PostInitExecution( Actor );
}

static void TraceActorTransitionPreBeginBefore( AActor* Actor )
{
	if( GActorTransitionLedger )
		GActorTransitionLedger->PreBeginBefore( Actor );
}

static void TraceActorTransitionPreBeginAfter( AActor* Actor )
{
	if( GActorTransitionLedger )
		GActorTransitionLedger->PreBeginAfter( Actor );
}

static void TraceActorTransitionSpawnRequest( UClass* Class, FName Name, AActor* Owner )
{
	if( GActorTransitionLedger )
		GActorTransitionLedger->SpawnRequest( Class, Name, Owner );
}

static void TraceActorTransitionSpawnResult( AActor* Owner, AActor* Child )
{
	if( GActorTransitionLedger )
		GActorTransitionLedger->SpawnResult( Owner, Child );
}

static void TraceActorTransitionSpawnPublished( AActor* Owner, AActor* Child )
{
	if( GActorTransitionLedger )
		GActorTransitionLedger->SpawnPublished( Owner, Child );
}

static void TraceActorTransitionFirstRendererCandidate( AActor* Owner, AActor* Candidate )
{
	if( GActorTransitionLedger )
		GActorTransitionLedger->FirstRendererCandidate( Owner, Candidate );
}

void HP2ActorSlotDumpPostDeserialize( ULevel* Level )
{
	ActorSlotDump::CapturePostDeserialize( Level );
}

void HP2ActorSlotDumpBeginRawLevelActorTable( ULevelBase* Level )
{
	ActorSlotDump::BeginRawLevelActorTable( Level );
}

void HP2ActorSlotDumpRawLevelActor( ULevelBase* Level, INT Slot, AActor* Actor, INT ReferenceOffsetBefore, INT ReferenceOffsetAfter, INT CompactPackageIndex, UBOOL CompactPackageIndexCaptured, UBOOL CompactPackageIndexSupported )
{
	ActorSlotDump::CaptureRawLevelActor( Level, Slot, Actor, ReferenceOffsetBefore, ReferenceOffsetAfter, CompactPackageIndex, CompactPackageIndexCaptured, CompactPackageIndexSupported );
}

void HP2ActorSlotDumpEndRawLevelActorTable( ULevelBase* Level )
{
	ActorSlotDump::EndRawLevelActorTable( Level );
}

static void TraceCreatureFunctionEnter( UObject* Object, UFunction* Function )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->FunctionEnter( Object, Function );
}

static void TraceCreatureFunctionExit( UObject* Object, UFunction* Function, void* Result )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->FunctionExit( Object, Function, Result );
}

static void TraceCreatureRand( UObject* Object, FFrame&, INT Raw, INT Bound, INT Index )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->Rand( Object, Raw, Bound, Index );
}

static void TraceCreatureRandRange( UObject* Object, FFrame&, INT Raw, FLOAT Min, FLOAT Max, FLOAT Value )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->RandRange( Object, Raw, Min, Max, Value );
}

static void TraceCreatureSoftwareRendering( AActor* Actor, UBOOL IsSoftwareRendering )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->SoftwareRendering( Actor, IsSoftwareRendering );
}

static void TraceCreatureTickDispatch( AActor* Actor, FLOAT DeltaSeconds )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->TickDispatch( Actor, DeltaSeconds );
}

static void TraceCreatureClassResolution( UClass* Class, AActor* Owner )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->ClassResolution( Class, Owner );
}

static void TraceCreatureSpawnRequest( UClass* Class, AActor* Owner )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->SpawnRequest( Class, Owner );
}

static void TraceCreatureSpawnResult( AActor* Owner, AActor* Child )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->SpawnResult( Owner, Child );
}

static void TraceCreatureSpawnPublished( AActor* Owner, AActor* Child )
{
	if( GCreatureGeneratorTraceReporter )
		GCreatureGeneratorTraceReporter->SpawnPublished( Owner, Child );
}

static UBOOL TraceGlobalTickEnabled()
{
	return GGlobalTickTraceReporter != NULL;
}

static unsigned long long TraceGlobalTickBeginDispatch( AActor* Actor, const char* Relation, const char* RootAdmission )
{
	return GGlobalTickTraceReporter
		? GGlobalTickTraceReporter->BeginDispatch( Actor, Relation, RootAdmission )
		: ~0ULL;
}

static void TraceGlobalTickEventTick( unsigned long long Token )
{
	if( GGlobalTickTraceReporter )
		GGlobalTickTraceReporter->EventTick( Token );
}

static void TraceGlobalTickProcessState( AActor* Actor, UObject* State, const char* Outcome )
{
	if( GGlobalTickTraceReporter )
		GGlobalTickTraceReporter->ProcessState( Actor, State, Outcome );
}

static void TraceGlobalTickEndDispatch( unsigned long long Token, const char* SkipReason )
{
	if( GGlobalTickTraceReporter )
		GGlobalTickTraceReporter->EndDispatch( Token, SkipReason );
}

static const FHP2TraceHookBindings GTraceHookBindings =
{
	NULL,
	&TraceCreatureFunctionEnter,
	&TraceCreatureFunctionExit,
	&TraceCreatureRand,
	&TraceCreatureRandRange,
	&TraceCreatureSoftwareRendering,
	&TraceCreatureTickDispatch,
	&TraceCreatureClassResolution,
	&TraceCreatureSpawnRequest,
	&TraceCreatureSpawnResult,
	&TraceCreatureSpawnPublished,
	&TraceFireTextureInitTables,
	&TraceFireTextureSpeedRand,
	&TraceFireTextureBurnWrite,
	&TraceGlobalTickEnabled,
	&TraceGlobalTickBeginDispatch,
	&TraceGlobalTickEventTick,
	&TraceGlobalTickProcessState,
	&TraceGlobalTickEndDispatch,
	&TraceActorTransitionPostInitExecution,
	&TraceActorTransitionPreBeginBefore,
	&TraceActorTransitionPreBeginAfter,
	&TraceActorTransitionSpawnRequest,
	&TraceActorTransitionSpawnResult,
	&TraceActorTransitionSpawnPublished,
	&TraceActorTransitionFirstRendererCandidate,
	&TraceShadowAdmissionBeginPass,
	&TraceShadowAdmissionRecord
};

/*-----------------------------------------------------------------------------
	Main.
-----------------------------------------------------------------------------*/

int main( int ArgC, char* ArgV[] )
{
	// Install the real process allocator before anything else can touch
	// engine containers: every FString/TArray allocation routes through
	// GMalloc, and the FMallocError stub installed at static init returns
	// NULL. PrepareHP2Paths' scoped bootstrap allocator saves and restores
	// this value, so its deliberate std-only window still works; appInit
	// later reassigns and initializes the same instance.
	GMalloc = &Malloc;

	// Validate and install the initial data/user roots before anything else
	// touches the filesystem.
	if( !PrepareHP2Paths( ArgC, ArgV ) )
		return 1;

	// Crash reporter: signal handlers plus atexit marker; watchdog unused
	// here. Writes crash-report.json (frames + log tail) into the working
	// directory, prints "HP2_CRASH <code> <path>" on stderr, then re-raises.
	HP2CrashReporterConfig CrashConfig = {};
	CrashConfig.ProcessName = "hp2_game";
	CrashConfig.LogFileBase = "HarryPotter2.log";
	HP2InstallCrashReporter(&CrashConfig);

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

	// Forwarded command line and chooser flow build engine FStrings; the
	// real process allocator was installed right after PrepareHP2Paths
	// returned, so every append below allocates through it.
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
	HP2InstallTraceHookBindings( GTraceHookBindings );
	// StaticInit may construct the Fire class default object. The observer must
	// exist before that process-global initialization to retain InitTables'
	// actual appRand interval.
	std::unique_ptr<FireTextureTraceReporter> FireTextureTrace;
	UFireTexture* FireTextureTraceTarget = NULL;
	if( FireTextureTraceReporter::IsEnabled() )
	{
		FireTextureTrace.reset( new FireTextureTraceReporter );
		GFireTextureTraceReporter = FireTextureTrace.get();
	}

	GIsStarted = 1;
#if !_MSC_VER
	__Context::StaticInit();
	// Re-install the crash reporter: StaticInit's guard longjmp handlers
	// replaced the ones installed above, and the reporter must own fatal
	// signals to capture frames before the guard unwinds.
	HP2InstallCrashReporter(&CrashConfig);
	strncpy( GModule, ArgV[0], sizeof(GModule) - 1 );
	GModule[sizeof(GModule) - 1] = 0;
#endif // !_MSC_VER
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
		// Client/server context globals. The old launcher set these between
		// SDL_Init and engine init; they are load-bearing far beyond display
		// selection: ULinker derives _ContextFlags from them, and a zero
		// value maps every package name-table entry to NAME_None during
		// load, breaking all export resolution ("Failed to find object
		// 'Level None.MyLevel'"). Restore before any package loads.
		GIsServer = 1;
		GIsClient = !ParseParam( appCmdLine(), TEXT("SERVER") );
		GIsEditor = 0;
		GIsScriptable = 1;
		GLazyLoad = !GIsClient || ParseParam( appCmdLine(), TEXT("LAZY") );

		// This observer must exist before InitEngine loads the map: the fixture
		// controller spawns its gate during the map's startup actor pass.
		std::unique_ptr<LifecycleGateReporter> LifecycleReporter;
		if( LifecycleGateReporter::IsEnabled() )
			LifecycleReporter.reset( new LifecycleGateReporter );
		std::unique_ptr<WorldCollisionGateReporter> WorldCollisionReporter;
		const UBOOL FullWorldCollisionGate = WorldCollisionGateReporter::IsEnabled();
		const UBOOL TouchWorldCollisionGate = WorldCollisionGateReporter::IsTouchEnabled();
		const UBOOL BumpWorldCollisionGate = WorldCollisionGateReporter::IsBumpEnabled();
		if( (FullWorldCollisionGate && (TouchWorldCollisionGate || BumpWorldCollisionGate))
			|| (TouchWorldCollisionGate && BumpWorldCollisionGate) )
			appErrorf( TEXT("only one world collision gate protocol may be enabled") );
		if( FullWorldCollisionGate || TouchWorldCollisionGate || BumpWorldCollisionGate )
			WorldCollisionReporter.reset( new WorldCollisionGateReporter(
				BumpWorldCollisionGate ? WorldCollisionGateReporter::ProtocolBump
				: TouchWorldCollisionGate ? WorldCollisionGateReporter::ProtocolTouch
				: WorldCollisionGateReporter::ProtocolFull ) );
		std::unique_ptr<StaticBspProbeReporter> StaticBspProbe;
		if( StaticBspProbeReporter::IsEnabled() )
			StaticBspProbe.reset( new StaticBspProbeReporter );
		std::unique_ptr<ShadowAdmissionTraceReporter> ShadowAdmissionReporter;
		if( ShadowAdmissionTraceReporter::IsEnabled() )
		{
			ShadowAdmissionReporter.reset( new ShadowAdmissionTraceReporter );
			GShadowAdmissionTraceReporter = ShadowAdmissionReporter.get();
		}
		std::unique_ptr<ActorTransitionLedger> ActorTransitionReporter;
		if( ActorTransitionLedger::IsEnabled() )
		{
			ActorTransitionReporter.reset( new ActorTransitionLedger );
			GActorTransitionLedger = ActorTransitionReporter.get();
		}
		std::unique_ptr<CreatureGeneratorTraceReporter> CreatureGeneratorTrace;
		if( CreatureGeneratorTraceReporter::IsEnabled() )
		{
			CreatureGeneratorTrace.reset( new CreatureGeneratorTraceReporter );
			GCreatureGeneratorTraceReporter = CreatureGeneratorTrace.get();
		}

		std::unique_ptr<GlobalTickTraceReporter> GlobalTickTrace;
		if( GlobalTickTraceReporter::IsEnabled() )
		{
			GlobalTickTrace.reset( new GlobalTickTraceReporter );
			GGlobalTickTraceReporter = GlobalTickTrace.get();
		}




		UEngine* Engine = InitEngine();
		if( !Engine )
			appErrorf( TEXT("Could not initialize the game engine") );
		if( FireTextureTrace )
		{
			// InitEngine's map bootstrap can collect objects loaded before it.
			// Resolve the pinned export only after that boundary, while retaining
			// the already-installed InitTables observer.
			FireTextureTraceTarget = Cast<UFireTexture>( UObject::StaticLoadObject(
				UFireTexture::StaticClass(),
				NULL,
				TEXT("HPParticle.hp_fx.Fire1"),
				NULL,
				LOAD_NoFail,
				NULL ) );
			if( !FireTextureTraceTarget )
				appErrorf( TEXT("Could not load HPParticle.hp_fx.Fire1 for HP2_FIRE_TEXTURE_TRACE") );
			if( FireTextureTraceTarget->GetLinkerIndex() != 886
				|| appStricmp( FireTextureTraceTarget->GetPathName(), TEXT("HPParticle.hp_fx.Fire1") ) != 0 )
				appErrorf( TEXT("HP2_FIRE_TEXTURE_TRACE target provenance is not HPParticle.hp_fx.Fire1 export 886") );
			FireTextureTrace->SetTarget( FireTextureTraceTarget );
		}
		if( FireTextureTrace )
		{
			FireTextureTrace->Probe( FireTextureTraceTarget );
			FireTextureTrace->Flush();
			GFireTextureTraceReporter = NULL;
		}
		debugf( TEXT("Entering main loop.") );
		HideSplash();

		INT TestTicks = 0;
		FLOAT FixedDeltaSeconds = 0.0f;
		Parse( appCmdLine(), TEXT("TESTTICKS="), TestTicks );
		Parse( appCmdLine(), TEXT("fixed-dt="), FixedDeltaSeconds );
		if( FixedDeltaSeconds < 0.0f )
			appErrorf( TEXT("fixed-dt must be positive") );
		if( !GIsRequestingExit )
			MainLoop( Engine, Max( 0, TestTicks ), FixedDeltaSeconds, LifecycleReporter.get(), WorldCollisionReporter.get(), StaticBspProbe.get() );
		GShadowAdmissionTraceReporter = NULL;
		GActorTransitionLedger = NULL;
		GCreatureGeneratorTraceReporter = NULL;
		GGlobalTickTraceReporter = NULL;
		CleanUpOnExit( Engine );
	}
#ifndef _DEBUG
	catch( ... )
	{
		ExitCode = 1;
		if( GError )
			GError->HandleError();
		// HandleError prints the History via stdout's wprintf; mirror it on
		// stderr so harnesses capturing only stderr still see the cause.
		fprintf( stderr, "%ls", GErrorHist );
	}
#endif

	GIsGuarded = 0;
	try
	{
		HideSplash();
		appExit();
		SDL_Quit();
	}
	catch( ... )
	{
		// A guard error raised during teardown sits outside the guarded
		// engine region above; letting it escape main would terminate the
		// process with an uncaught exception instead of exiting cleanly.
		GIsStarted = 0;
		return 1;
	}
	GIsStarted = 0;
	return ExitCode;
}

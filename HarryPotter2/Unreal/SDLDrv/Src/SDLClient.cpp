/*=============================================================================
	SDLClient.cpp: SDL-based platform client.
=============================================================================*/

#include "SDLDrv.h"
#include "UnRenDev.h"
#include "hp2_rs.h"

#include <string.h>
#include <stddef.h>

/*-----------------------------------------------------------------------------
	Static registration.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(USDLClient);

/*-----------------------------------------------------------------------------
	Construction / reflection.
-----------------------------------------------------------------------------*/

//
// The SDL client.
//
USDLClient::USDLClient()
:	Joystick( NULL )
,	JoyButtons( 0 )
,	JoyHats( 0 )
,	JoyAxes( 0 )
,	JoyHatFoldsToButtons( 0 )
,	LastCurrent( NULL )
{
	guard(USDLClient::USDLClient);
	unguard;
}

void USDLClient::StaticConstructor()
{
	guard(USDLClient::StaticConstructor);

	// Configuration surface ([SDLDrv.SDLClient]).
	new(GetClass(),TEXT("UseJoystick"),			RF_Public)UBoolProperty (CPP_PROPERTY(UseJoystick          ), TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("StartupFullscreen"),	RF_Public)UBoolProperty (CPP_PROPERTY(StartupFullscreen     ), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("BorderlessWindow"),	RF_Public)UBoolProperty (CPP_PROPERTY(BorderlessWindow      ), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("UseDesktopResolution"),RF_Public)UBoolProperty (CPP_PROPERTY(UseDesktopResolution  ), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("IgnoreHat"),			RF_Public)UBoolProperty (CPP_PROPERTY(IgnoreHat             ), TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("IgnoreUngrabbedMouse"),RF_Public)UBoolProperty (CPP_PROPERTY(IgnoreUngrabbedMouse  ), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("AllowUnicodeKeys"),	RF_Public)UBoolProperty (CPP_PROPERTY(AllowUnicodeKeys      ), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("AllowCommandQKeys"),	RF_Public)UBoolProperty (CPP_PROPERTY(AllowCommandQKeys     ), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("MacKeepAllScreensOn"),	RF_Public)UBoolProperty (CPP_PROPERTY(MacKeepAllScreensOn   ), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("MacNativeTextToSpeech"),RF_Public)UBoolProperty(CPP_PROPERTY(MacNativeTextToSpeech ), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("JoystickNumber"),		RF_Public)UIntProperty  (CPP_PROPERTY(JoystickNumber        ), TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("JoystickHatNumber"),	RF_Public)UIntProperty  (CPP_PROPERTY(JoystickHatNumber     ), TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("ScaleJBX"),			RF_Public)UFloatProperty(CPP_PROPERTY(ScaleJBX              ), TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("ScaleJBY"),			RF_Public)UFloatProperty(CPP_PROPERTY(ScaleJBY              ), TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("JoystickDeadZone"),	RF_Public)UFloatProperty(CPP_PROPERTY(JoystickDeadZone      ), TEXT("Joystick"), CPF_Config );
	new(GetClass(),TEXT("TextToSpeechFile"),	RF_Public)UStrProperty (CPP_PROPERTY(TextToSpeechFile       ), TEXT("Display"),  CPF_Config );
	new(GetClass(),TEXT("UIScale"),				RF_Public)UFloatProperty(CPP_PROPERTY(UIScale               ), TEXT("Display"),  CPF_Config );

	// Pinned model defaults; configuration overrides them at load time.
	WindowedViewportX		= 1024;
	WindowedViewportY		= 768;
	WindowedColorBits		= 32;
	FullscreenViewportX	= 1024;
	FullscreenViewportY	= 768;
	FullscreenColorBits	= 32;
	Brightness				= 0.5f;
	CaptureMouse				= 1;
	ScreenFlashes				= 1;
	MaintainVerticalFOV	= 1;
	Decals						= 1;
	ShowFPS						= 0;
	MinDesiredFrameRate	= 30.0f;
	ParticleDensity			= 1;
	StartupFullscreen		= 0;
	UIScale						= 1.0f;
	JoystickDeadZone		= 0.2f;

	unguard;
}

/*-----------------------------------------------------------------------------
	UObject interface.
-----------------------------------------------------------------------------*/

void USDLClient::NotifyDestroy( void* Src )
{
	// Intentionally inert: nothing owned here tracks notify sources.
}

void USDLClient::Destroy()
{
	guard(USDLClient::Destroy);

	// Shut down every viewport's render device before tearing the viewports down.
	for( INT Index = 0; Index < Viewports.Num(); ++Index )
	{
		UViewport* Viewport = Viewports(Index);
		if( Viewport && Viewport->RenDev )
		{
			Viewport->RenDev->Exit();
			delete Viewport->RenDev;
			Viewport->RenDev = NULL;
		}
	}
	while( Viewports.Num() )
	{
		UViewport* Viewport = Viewports(0);
		Viewports.Remove( 0 );
		delete Viewport;
	}

	SDL_Quit();
	Super::Destroy();
	unguard;
}

void USDLClient::PostEditChange()
{
	guard(USDLClient::PostEditChange);
	Super::PostEditChange();
	// Guard against garbage (including NaN) arriving through configuration.
	if( !(UIScale > 0.f) )
		UIScale = 1.f;
	unguard;
}

void USDLClient::ShutdownAfterError()
{
	guard(USDLClient::ShutdownAfterError);
	SDL_Quit();
	Super::ShutdownAfterError();
	unguard;
}

/*-----------------------------------------------------------------------------
	UClient interface.
-----------------------------------------------------------------------------*/

void USDLClient::Init( UEngine* InEngine )
{
	guard(USDLClient::Init);
	Super::Init( InEngine );

	// The game owns the mouse whenever a viewport wants it; user config
	// cannot disable that contract.
	CaptureMouse = 1;
	PostEditChange();

	if( ParseParam(appCmdLine(),TEXT("defaultres")) )
	{
		WindowedViewportX = 640;
		WindowedViewportY = 480;
	}

	if( UseJoystick )
	{
		if( !OpenJoystick() )
			UseJoystick = 0;
	}
	unguard;
}

void USDLClient::ShowViewportWindows( DWORD ShowFlags, int DoShow )
{
	// SDL windows manage their own visibility.
}

void USDLClient::EnableViewportWindows( DWORD ShowFlags, int DoEnable )
{
	// SDL windows manage their own visibility.
}

void USDLClient::TeardownSR()
{
	// Nothing to tear down; kept for interface parity.
}

UBOOL USDLClient::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(USDLClient::Exec);
	return UClient::Exec( Cmd, Ar );
	unguard;
}

// Layout mirror of hp2-ffi's FfiRepaintCandidate (crates/hp2-ffi/src/input.rs).
static_assert(sizeof(FfiRepaintCandidate) == 16, "FfiRepaintCandidate drifted from hp2-ffi");
static_assert(offsetof(FfiRepaintCandidate, realtime) == 0, "FfiRepaintCandidate drifted from hp2-ffi");
static_assert(offsetof(FfiRepaintCandidate, size_x) == 4, "FfiRepaintCandidate drifted from hp2-ffi");
static_assert(offsetof(FfiRepaintCandidate, size_y) == 8, "FfiRepaintCandidate drifted from hp2-ffi");
static_assert(offsetof(FfiRepaintCandidate, best_minus_candidate) == 12, "FfiRepaintCandidate drifted from hp2-ffi");

void USDLClient::Tick()
{
	guard(USDLClient::Tick);
	for( INT Index = 0; Index < Viewports.Num(); ++Index )
		if( Viewports(Index) )
			Viewports(Index)->UpdateInput( 0 );

	// Present frames: repaint the most stale realtime viewport each tick.
	// UGameEngine::Tick delegates all drawing here, so a client that only
	// pumps input never presents anything (blank window; render-device
	// Unlock/present hooks never run).
	// The victim rule lives in Rust (hp2_rs_input_repaint_replaces_best,
	// crates/hp2-platform/src/input_pump.rs). FTime is 64-bit fixed-point, so
	// the comparison crosses the boundary as FTime::operator-, whose float sign is
	// exact: victim > candidate exactly when (victim - candidate) > 0. Fold one
	// viewport at a time so Tick stays allocation-free.
	UViewport* BestViewport = NULL;
	for( INT Index = 0; Index < Viewports.Num(); ++Index )
	{
		UViewport* Viewport = Viewports(Index);
		if( !Viewport )
			continue;
		const FLOAT BestMinusCandidate = BestViewport
			? BestViewport->LastUpdateTime - Viewport->LastUpdateTime
			: 0.f;
		const FfiRepaintCandidate Candidate =
		{
			Viewport->IsRealtime() != 0,
			Viewport->SizeX,
			Viewport->SizeY,
			BestMinusCandidate
		};
		if( hp2_rs_input_repaint_replaces_best( BestViewport != NULL, Candidate ) )
			BestViewport = Viewport;
	}
	if( BestViewport )
		BestViewport->Repaint( 1 );
	unguard;
}

void USDLClient::MakeCurrent( UViewport* NewViewport )
{
	LastCurrent = NewViewport;
}

UViewport* USDLClient::GetLastCurrent()
{
	return LastCurrent;
}

UViewport* USDLClient::NewViewport( const FName Name )
{
	guard(USDLClient::NewViewport);
	return new(this,Name)USDLViewport();
	unguard;
}

/*-----------------------------------------------------------------------------
	Joystick.
-----------------------------------------------------------------------------*/

UBOOL USDLClient::OpenJoystick()
{
	guard(USDLClient::OpenJoystick);
	CloseJoystick();
	INT NewButtons = 0, NewHats = 0, NewAxes = 0;
	bool bNewHatFoldsToButtons = false;
	Joystick = static_cast<SDL_Joystick*>( hp2_rs_platform_open_joystick(
		JoystickNumber, IgnoreHat != 0, &NewButtons, &NewHats, &NewAxes, &bNewHatFoldsToButtons ) );
	if( !Joystick )
		return 0;
	JoyButtons = NewButtons;
	JoyHats = NewHats;
	JoyAxes = NewAxes;
	JoyHatFoldsToButtons = bNewHatFoldsToButtons;
	return 1;
	unguard;
}

void USDLClient::CloseJoystick()
{
	guard(USDLClient::CloseJoystick);
	hp2_rs_platform_close_joystick( Joystick );
	Joystick = NULL;
	JoyButtons = 0;
	JoyHats = 0;
	JoyAxes = 0;
	JoyHatFoldsToButtons = 0;
	unguard;
}

/*-----------------------------------------------------------------------------
	System integration.

	GetClipboardText/SetClipboardText are replaced by the hp2_rs_platform_*
	FFI surface (crates/hp2-platform/src/clipboard.rs) -- hand-written
	extern "C" declarations against the pinned SDL2 headers, resolved at
	this binary's final link via Build/CMake/HP2Targets.cmake's
	SDL2::SDL2-static interface link on hp2_ffi, not a second vendored
	SDL2 copy. See that module's doc comment for the full design and the
	"Approved deviations" entry in local://rust-inplace-port-plan.md for
	why (an earlier `sdl2` crate design was reverted: it would have
	bundled a second copy of SDL2's object code into libhp2_ffi.a).
-----------------------------------------------------------------------------*/

namespace
{
void ClipboardSinkToFString( void* Ctx, const BYTE* Text, SIZE_T Len )
{
	TArray<ANSICHAR> Utf8;
	Utf8.AddZeroed( (INT)Len + 1 );
	if( Len )
		appMemcpy( &Utf8(0), Text, (INT)Len );
	TArray<TCHAR> Buffer;
	Buffer.AddZeroed( (INT)Len + 1 );
	appFromUtf8InPlace( &Buffer(0), &Utf8(0), Buffer.Num() );
	*static_cast<FString*>(Ctx) = FString( &Buffer(0) );
}
} // namespace

FString USDLClient::GetClipboardText() const
{
	guard(USDLClient::GetClipboardText);
	FString Result;
	hp2_rs_platform_get_clipboard_text( &ClipboardSinkToFString, &Result );
	return Result;
	unguard;
}

UBOOL USDLClient::SetClipboardText( const TCHAR* Text )
{
	guard(USDLClient::SetClipboardText);
	TArray<ANSICHAR> Utf8;
	Utf8.AddZeroed( appStrlen(Text) * 4 + 1 );
	appToUtf8InPlace( &Utf8(0), Text, Utf8.Num() );
	return hp2_rs_platform_set_clipboard_text( reinterpret_cast<const BYTE*>(&Utf8(0)), strlen(&Utf8(0)) ) ? 1 : 0;
	unguard;
}

void USDLClient::StartTextInput()
{
	// Text events arrive unconditionally on this platform.
}

void USDLClient::StopTextInput()
{
	// Text events arrive unconditionally on this platform.
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

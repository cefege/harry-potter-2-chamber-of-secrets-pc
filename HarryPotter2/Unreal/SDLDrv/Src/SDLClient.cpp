/*=============================================================================
	SDLClient.cpp: SDL-based platform client.
=============================================================================*/

#include "SDLDrv.h"
#include "UnRenDev.h"

#include <string.h>

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
	UViewport* BestViewport = NULL;
	for( INT Index = 0; Index < Viewports.Num(); ++Index )
	{
		UViewport* Viewport = Viewports(Index);
		if
		(	Viewport
		&&	Viewport->IsRealtime()
		&&	Viewport->SizeX
		&&	Viewport->SizeY
		&&	(!BestViewport || BestViewport->LastUpdateTime > Viewport->LastUpdateTime) )
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
	if( SDL_InitSubSystem( SDL_INIT_JOYSTICK ) < 0 )
		return 0;
	const INT DeviceCount = SDL_NumJoysticks();
	if( JoystickNumber < 0 || JoystickNumber >= DeviceCount )
		return 0;
	Joystick = SDL_JoystickOpen( JoystickNumber );
	if( !Joystick )
		return 0;
	SDL_JoystickEventState( SDL_ENABLE );

	// Buttons clamp to the engine's IK_Joy1..IK_Joy16 range.
	JoyButtons = Min( SDL_JoystickNumButtons( Joystick ), 16 );
	JoyHats    = Max( SDL_JoystickNumHats( Joystick ), 0 );
	JoyAxes    = Clamp( SDL_JoystickNumAxes( Joystick ), 0, (INT)MaxJoystickAxes );

	// Hats fold onto buttons starting at IK_Joy13, but only when there are
	// more than twelve real buttons to displace them and hats exist at all.
	const INT PhysicalButtons = Max( SDL_JoystickNumButtons( Joystick ), 0 );
	JoyHatFoldsToButtons = (PhysicalButtons > 12 && JoyHats > 0 && !IgnoreHat) ? 1 : 0;
	return 1;
	unguard;
}

void USDLClient::CloseJoystick()
{
	guard(USDLClient::CloseJoystick);
	if( Joystick )
	{
		SDL_JoystickClose( Joystick );
		Joystick = NULL;
	}
	JoyButtons = 0;
	JoyHats = 0;
	JoyAxes = 0;
	JoyHatFoldsToButtons = 0;
	unguard;
}

/*-----------------------------------------------------------------------------
	System integration.
-----------------------------------------------------------------------------*/

FString USDLClient::GetClipboardText() const
{
	guard(USDLClient::GetClipboardText);
	FString Result;
	char* Utf8 = SDL_GetClipboardText();
	if( Utf8 )
	{
		TArray<TCHAR> Buffer;
		Buffer.AddZeroed( (INT)strlen( Utf8 ) + 1 );
		appFromUtf8InPlace( &Buffer(0), Utf8, Buffer.Num() );
		Result = FString( &Buffer(0) );
		SDL_free( Utf8 );
	}
	return Result;
	unguard;
}

UBOOL USDLClient::SetClipboardText( const TCHAR* Text )
{
	guard(USDLClient::SetClipboardText);
	TArray<ANSICHAR> Utf8;
	Utf8.AddZeroed( appStrlen(Text) * 4 + 1 );
	appToUtf8InPlace( &Utf8(0), Text, Utf8.Num() );
	return SDL_SetClipboardText( &Utf8(0) ) == 0 ? 1 : 0;
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

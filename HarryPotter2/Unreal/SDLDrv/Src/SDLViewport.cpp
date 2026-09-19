/*=============================================================================
	SDLViewport.cpp: SDL-based platform viewport.
=============================================================================*/

#include "SDLDrv.h"
#include "HP2InputEvents.h"
#include "HP2MouseCapturePolicy.h"
#include "UnRenDev.h"
#include "FConfigCacheIni.h"
#include <errno.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

/*-----------------------------------------------------------------------------
	Static registration.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(USDLViewport);

/*-----------------------------------------------------------------------------
	Locals.
-----------------------------------------------------------------------------*/

// Joystick axes in the exact order the engine's input contracts expect.
static const EInputKey GJoyAxisKeys[USDLClient::MaxJoystickAxes] =
{
	IK_JoyX, IK_JoyY, IK_JoyZ, IK_JoyR, IK_JoyU, IK_JoyV, IK_UnknownEA, IK_UnknownEB
};

// Hats fold onto buttons starting at IK_Joy13 once twelve real buttons exist.
static const INT GHatButtonBase = 12;

// Window pixels per viewport pixel; identity until both sizes are known.
static void UpdateMouseScale( USDLViewport& Viewport )
{
	INT WindowX = 0, WindowY = 0;
	if( Viewport.SdlWindow )
		SDL_GetWindowSize( Viewport.SdlWindow, &WindowX, &WindowY );
	Viewport.MouseScaleX = (WindowX > 0 && Viewport.SizeX > 0) ? (FLOAT)Viewport.SizeX / (FLOAT)WindowX : 1.f;
	Viewport.MouseScaleY = (WindowY > 0 && Viewport.SizeY > 0) ? (FLOAT)Viewport.SizeY / (FLOAT)WindowY : 1.f;
}

static DWORD ButtonsFromSdlState( Uint32 State )
{
	DWORD Buttons = 0;
	if( State & SDL_BUTTON_LMASK ) Buttons |= MOUSE_Left;
	if( State & SDL_BUTTON_RMASK ) Buttons |= MOUSE_Right;
	if( State & SDL_BUTTON_MMASK ) Buttons |= MOUSE_Middle;
	return Buttons;
}

static void TextToUtf8( const TCHAR* Text, TArray<ANSICHAR>& Out )
{
	Out.AddZeroed( appStrlen(Text) * 4 + 1 );
	appToUtf8InPlace( &Out(0), Text, Out.Num() );
}

/*-----------------------------------------------------------------------------
	Construction / destruction.
-----------------------------------------------------------------------------*/

//
// The SDL viewport.
//
USDLViewport::USDLViewport()
:	SdlWindow( NULL )
,	BlitFlags( 0 )
,	LostGrab( 0 )
,	LostFullscreen( 0 )
,	MouseIsGrabbed( 0 )
,	LastJoyHat( IK_None )
,	MouseScaleX( 1.f )
,	MouseScaleY( 1.f )
,	SpeechPid( -1 )
{
	guard(USDLViewport::USDLViewport);

	// Freeze the keysym translation table used by the event pump; the
	// quirks (Ctrl collapse, GUI-as-F24, tilde aliases) live in
	// HP2MapKeysym and are contract-tested there.
	for( INT Index = 0; Index < KeysymMapSize; ++Index )
		KeysymMap[Index] = HP2MapKeysym( (SDL_Keycode)Index );

	// System cursors.
	SystemCursors[0] = SDL_CreateSystemCursor( SDL_SYSTEM_CURSOR_ARROW );
	SystemCursors[1] = SDL_CreateSystemCursor( SDL_SYSTEM_CURSOR_SIZEALL );
	SystemCursors[2] = SDL_CreateSystemCursor( SDL_SYSTEM_CURSOR_SIZENESW );
	SystemCursors[3] = SDL_CreateSystemCursor( SDL_SYSTEM_CURSOR_SIZENS );
	SystemCursors[4] = SDL_CreateSystemCursor( SDL_SYSTEM_CURSOR_SIZENWSE );
	SystemCursors[5] = SDL_CreateSystemCursor( SDL_SYSTEM_CURSOR_SIZEWE );
	SystemCursors[6] = SDL_CreateSystemCursor( SDL_SYSTEM_CURSOR_WAIT );

	// Adopt the desktop display mode's depth. Headless hosts report no
	// mode; the 32-bit default stands and everything degrades safely.
	ColorBytes = 4;
	SDL_DisplayMode DesktopMode;
	memset( &DesktopMode, 0, sizeof(DesktopMode) );
	if( SDL_GetDesktopDisplayMode( 0, &DesktopMode ) == 0 && DesktopMode.format != SDL_PIXELFORMAT_UNKNOWN )
	{
		const INT DesktopBits = SDL_BITSPERPIXEL( DesktopMode.format );
		if( DesktopBits > 0 && DesktopBits <= 16 )
		{
			ColorBytes = 2;
			Caps |= CC_RGB565;
		}
		else
		{
			ColorBytes = 4;
		}
	}

	SDL_version Linked;
	SDL_GetVersion( &Linked );
	debugf(
		NAME_Init,
		TEXT("SDLDrv: compiled against SDL %i.%i.%i, linked against SDL %i.%i.%i"),
		(INT)SDL_MAJOR_VERSION, (INT)SDL_MINOR_VERSION, (INT)SDL_PATCHLEVEL,
		(INT)Linked.major, (INT)Linked.minor, (INT)Linked.patch );

	unguard;
}

void USDLViewport::Destroy()
{
	guard(USDLViewport::Destroy);

	UpdateMouseGrabState( 0 );
	if( SdlWindow )
	{
		SDL_DestroyWindow( SdlWindow );
		SdlWindow = NULL;
	}
	for( INT Index = 0; Index < NumSystemCursors; ++Index )
	{
		if( SystemCursors[Index] )
		{
			SDL_FreeCursor( SystemCursors[Index] );
			SystemCursors[Index] = NULL;
		}
	}
	Super::Destroy();
	unguard;
}

void USDLViewport::ShutdownAfterError()
{
	guard(USDLViewport::ShutdownAfterError);
	SDL_Quit();
	Super::ShutdownAfterError();
	unguard;
}

/*-----------------------------------------------------------------------------
	UViewport interface.
-----------------------------------------------------------------------------*/

UBOOL USDLViewport::Lock( FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData, INT* HitSize )
{
	guard(USDLViewport::Lock);
	if( !RenDev || !SizeX || !SizeY )
		return 0;
	RenDev->Lock( FlashScale, FlashFog, ScreenClear, RenderLockFlags, HitData, HitSize );
	CurrentTime = appSeconds();
	FrameCount++;
	return 1;
	unguard;
}

UBOOL USDLViewport::ResizeViewport( DWORD BlitType, INT NewX, INT NewY, INT NewColorBytes )
{
	guard(USDLViewport::ResizeViewport);

	if( !(BlitType & BLIT_NoWindowChange) )
	{
		if( BlitType & BLIT_Fullscreen )
			BlitFlags |= BLIT_Fullscreen;
		else
			BlitFlags &= ~BLIT_Fullscreen;
	}
	if( NewX != INDEX_NONE )
		SizeX = NewX;
	if( NewY != INDEX_NONE )
		SizeY = NewY;
	if( NewColorBytes != INDEX_NONE )
		ColorBytes = Clamp( NewColorBytes, 2, 4 );

	if( SdlWindow )
	{
		SDL_SetWindowSize( SdlWindow, SizeX, SizeY );
		SDL_SetWindowFullscreen( SdlWindow, IsFullscreen() ? SDL_WINDOW_FULLSCREEN_DESKTOP : 0 );
		UpdateMouseScale( *this );
	}
	if( RenDev )
		RenDev->SetRes( SizeX, SizeY, ColorBytes, IsFullscreen() );
	return RenDev ? 1 : 0;
	unguard;
}

void USDLViewport::Unlock( UBOOL Blit )
{
	guard(USDLViewport::Unlock);
	// Mirror UViewport::Unlock: release the render-device lock taken in
	// Lock(). Skipping it left RenDev locked after the first presented
	// frame, failing every subsequent Lock assertion.
	RenDev->Unlock( Blit );
	if( Blit )
		LastUpdateTime = CurrentTime;
	unguard;
}

void USDLViewport::Repaint( UBOOL Blit )
{
	guard(USDLViewport::Repaint);
	UEngine* ClientEngine = GetOuterUSDLClient()->Engine;
	if( ClientEngine && SizeX && SizeY )
		ClientEngine->Draw( this, Blit );
	unguard;
}

void USDLViewport::SetModeCursor()
{
	guard(USDLViewport::SetModeCursor);
	const INT Index = Clamp( (INT)SelectedCursor, 0, (INT)NumSystemCursors - 1 );
	SDL_Cursor* Cursor = SystemCursors[Index];
	if( Cursor )
		SDL_SetCursor( Cursor );
	unguard;
}

void USDLViewport::UpdateWindowFrame()
{
	guard(USDLViewport::UpdateWindowFrame);
	if( SdlWindow )
		SDL_SetWindowBordered( SdlWindow, GetOuterUSDLClient()->BorderlessWindow ? SDL_FALSE : SDL_TRUE );
	unguard;
}

void USDLViewport::OpenWindow( DWORD ParentWindow, UBOOL Temporary, INT NewX, INT NewY, INT OpenX, INT OpenY )
{
	guard(USDLViewport::OpenWindow);
	USDLClient* C = GetOuterUSDLClient();

	// Legacy parent handles arrive as 32-bit values from old front ends;
	// widen them to host pointers without truncation surprises.
	void* ParentHandle = NULL;
	if( ParentWindow )
	{
		static_assert(sizeof(void*) >= sizeof(DWORD), "host pointers must widen legacy window handles");
		ParentHandle = (void*)(UPTRINT)ParentWindow; // consumed by legacy host integrations
	}

	(void)ParentHandle;
	const INT Width  = NewX != INDEX_NONE ? NewX : (OpenX != INDEX_NONE ? OpenX : C->WindowedViewportX);
	const INT Height = NewY != INDEX_NONE ? NewY : (OpenY != INDEX_NONE ? OpenY : C->WindowedViewportY);

	if( !SdlWindow )
	{
		// EGL (Wayland) binds the framebuffer config at window creation, unlike
		// GLX which can still honour attributes set before context creation.
		// XOpenGL's SetSDLAttributes runs only later, in SetRes, so request the
		// framebuffer-side attributes here or Wayland silently downgrades depth
		// (observed: 24 requested, 16 provided). Context version/profile stay in
		// SetSDLAttributes; they are only consumed at context creation.
		SDL_GL_SetAttribute( SDL_GL_DOUBLEBUFFER, 1 );
		SDL_GL_SetAttribute( SDL_GL_DEPTH_SIZE, 24 );
		SDL_GL_SetAttribute( SDL_GL_RED_SIZE, 8 );
		SDL_GL_SetAttribute( SDL_GL_GREEN_SIZE, 8 );
		SDL_GL_SetAttribute( SDL_GL_BLUE_SIZE, 8 );

		DWORD Flags = SDL_WINDOW_OPENGL | SDL_WINDOW_RESIZABLE | (Temporary ? SDL_WINDOW_HIDDEN : 0);
		if( C->BorderlessWindow )
			Flags |= SDL_WINDOW_BORDERLESS;
		SdlWindow = SDL_CreateWindow(
			"Harry Potter 2",
			SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
			Width, Height, Flags );
		if( !SdlWindow )
			appErrorf( TEXT("SDL_CreateWindow failed: %s"), ANSI_TO_TCHAR(SDL_GetError()) );
	}

	SizeX = Width;
	SizeY = Height;
	ColorBytes = C->WindowedColorBits <= 16 ? 2 : 4;
	BlitFlags &= ~BLIT_Fullscreen;
	if( C->StartupFullscreen || C->UseDesktopResolution )
	{
		BlitFlags |= BLIT_Fullscreen;
		SDL_SetWindowFullscreen( SdlWindow, SDL_WINDOW_FULLSCREEN_DESKTOP );
	}
	LostFullscreen = 0;
	UpdateMouseScale( *this );

	if( !ParseParam(appCmdLine(),TEXT("nohard")) )
	{
		TryRenderDevice( TEXT("ini:Engine.Engine.GameRenderDevice"), SizeX, SizeY, ColorBytes, IsFullscreen() );
		if( !RenDev )
			appErrorf( TEXT("No render device could be initialized (see the XOpenGL messages above for the failing reason)") );
	}
	unguard;
}

void USDLViewport::CloseWindow()
{
	// SDL windows need no explicit close choreography.
}

void USDLViewport::UpdateInput( UBOOL Reset )
{
	guard(USDLViewport::UpdateInput);
	USDLClient* C = GetOuterUSDLClient();

	if( Reset )
		ReleaseAllInput();

	// Relative mouse deltas are sampled once per pump while grabbed.
	if( MouseIsGrabbed && SdlWindow )
	{
		INT DeltaX = 0, DeltaY = 0;
		SDL_GetRelativeMouseState( &DeltaX, &DeltaY );
		if( DeltaX )
			CauseInputEvent( IK_MouseX, IST_Axis, (FLOAT)DeltaX * MouseScaleX );
		if( DeltaY )
			CauseInputEvent( IK_MouseY, IST_Axis, -(FLOAT)DeltaY * MouseScaleY );
	}

	SDL_Event Event;
	while( SDL_PollEvent(&Event) )
	{
		switch( Event.type )
		{
			case SDL_KEYDOWN:
			case SDL_KEYUP:
			{
				const FHP2InputEvent Normalized = HP2MakeKeyEvent( Event.key.keysym.sym, Event.type == SDL_KEYDOWN );
				if( Normalized.Key != IK_None )
					CauseInputEvent( Normalized.Key, Normalized.Action, 0.f );
				break;
			}
			case SDL_TEXTINPUT:
			{
				const FHP2InputEvent Normalized = HP2MakeTextEvent( Event.text.text );
				UEngine* ClientEngine = C->Engine;
				if( Normalized.Key != IK_None && ClientEngine && Actor )
					ClientEngine->Key( this, Normalized.Key );
				break;
			}
			case SDL_MOUSEMOTION:
			{
				if( !MouseIsGrabbed && C->Engine )
					C->Engine->MousePosition( this,
						ButtonsFromSdlState( Event.motion.state ),
						(FLOAT)Event.motion.x * MouseScaleX,
						(FLOAT)Event.motion.y * MouseScaleY );
				break;
			}
			case SDL_MOUSEBUTTONDOWN:
			case SDL_MOUSEBUTTONUP:
			{
				const FHP2InputEvent Normalized = HP2MakeMouseButtonEvent( Event.button.button, Event.type == SDL_MOUSEBUTTONDOWN );
				if( Normalized.Key != IK_None )
					CauseInputEvent( Normalized.Key, Normalized.Action, 0.f );
				break;
			}
			case SDL_MOUSEWHEEL:
			{
				FHP2InputEvent Wheel[3];
				const INT Count = HP2MakeWheelEvents( Event.wheel.y, Wheel );
				for( INT Index = 0; Index < Count; ++Index )
					CauseInputEvent( Wheel[Index].Key, Wheel[Index].Action, Wheel[Index].Delta );
				break;
			}
			case SDL_JOYAXISMOTION:
			{
				if( Event.jaxis.axis >= USDLClient::MaxJoystickAxes )
					break;
				FLOAT Value = (FLOAT)Event.jaxis.value / 32768.f;
				if( Value > -C->JoystickDeadZone && Value < C->JoystickDeadZone )
					Value = 0.f;
				const FLOAT Scale =
					Event.jaxis.axis == 0 ? C->ScaleJBX :
					Event.jaxis.axis == 1 ? C->ScaleJBY : 1.f;
				if( Scale != 0.f )
					Value *= Scale;
				CauseInputEvent( GJoyAxisKeys[Event.jaxis.axis], IST_Axis, Value );
				break;
			}
			case SDL_JOYBUTTONDOWN:
			case SDL_JOYBUTTONUP:
			{
				if( Event.jbutton.button >= 16 )
					break;
				CauseInputEvent( (EInputKey)(IK_Joy1 + Event.jbutton.button), Event.type == SDL_JOYBUTTONDOWN ? IST_Press : IST_Release, 0.f );
				break;
			}
			case SDL_JOYHATMOTION:
			{
				if( !C->JoyHatFoldsToButtons )
					break;
				EInputKey Folded = IK_None;
				const Uint8 Hat = Event.jhat.value;
				if( Hat & SDL_HAT_UP )         Folded = (EInputKey)(IK_Joy1 + GHatButtonBase + 0);
				else if( Hat & SDL_HAT_DOWN )  Folded = (EInputKey)(IK_Joy1 + GHatButtonBase + 1);
				else if( Hat & SDL_HAT_LEFT )  Folded = (EInputKey)(IK_Joy1 + GHatButtonBase + 2);
				else if( Hat & SDL_HAT_RIGHT ) Folded = (EInputKey)(IK_Joy1 + GHatButtonBase + 3);
				if( Folded != LastJoyHat )
				{
					if( LastJoyHat != IK_None )
						CauseInputEvent( LastJoyHat, IST_Release, 0.f );
					if( Folded != IK_None )
						CauseInputEvent( Folded, IST_Press, 0.f );
					LastJoyHat = Folded;
				}
				break;
			}
			case SDL_WINDOWEVENT:
			{
				const Uint8 Kind = Event.window.event;
				const UBOOL bLoss    = Kind == SDL_WINDOWEVENT_FOCUS_LOST || Kind == SDL_WINDOWEVENT_MINIMIZED || Kind == SDL_WINDOWEVENT_HIDDEN;
				const UBOOL bRestore = Kind == SDL_WINDOWEVENT_FOCUS_GAINED || Kind == SDL_WINDOWEVENT_RESTORED || Kind == SDL_WINDOWEVENT_SHOWN;
				const HP2Capture::FState State = { MouseIsGrabbed != 0, LostGrab != 0, C->CaptureMouse != 0 };
				if( bLoss )
				{
					const HP2Capture::FDecision Decision = HP2Capture::OnLoss( State );
					if( Decision.Action == HP2Capture::Action_Release )
					{
						UpdateMouseGrabState( 0 );
						LostGrab = 1;
					}
					// Held input dies on every loss event, grabbed or not.
					ReleaseAllInput();
				}
				else if( bRestore && SdlWindow )
				{
					const Uint32 WindowFlags = SDL_GetWindowFlags( SdlWindow );
					const HP2Capture::FWindowFlags Flags =
					{
						(WindowFlags & SDL_WINDOW_INPUT_FOCUS) != 0,
						(WindowFlags & (SDL_WINDOW_MINIMIZED | SDL_WINDOW_HIDDEN)) != 0
					};
					const HP2Capture::FDecision Decision = HP2Capture::OnRestore( State, Flags );
					if( Decision.Action == HP2Capture::Action_Restore )
					{
						UpdateMouseGrabState( C->CaptureMouse );
						LostGrab = 0;
					}
				}
				else if( Kind == SDL_WINDOWEVENT_RESIZED )
				{
					// Wayland compositors can resize the xdg-toplevel out from
					// under us (tiling layout, compositor-driven fullscreen)
					// without ever routing through ToggleFullscreen/EndFullscreen.
					// Left unhandled, SizeX/SizeY, MouseScaleX/Y, and the render
					// device's resolution go stale while SDL's own window size
					// (and therefore SDL_MOUSEMOTION coordinates) track the new
					// size, so the menu cursor drifts away from the pointer by
					// the gap between the old and new size.
					const INT NewWidth = Event.window.data1;
					const INT NewHeight = Event.window.data2;
					if( NewWidth > 0 && NewHeight > 0 && (NewWidth != SizeX || NewHeight != SizeY) )
						ResizeViewport( BLIT_NoWindowChange, NewWidth, NewHeight, INDEX_NONE );
				}
				break;
			}
			default:
				break;
		}
	}
	unguard;
}

void* USDLViewport::GetWindow()
{
	return (void*)SdlWindow;
}

void USDLViewport::SetMouseCapture( UBOOL Capture, UBOOL Clip, UBOOL FocusOnly )
{
	guard(USDLViewport::SetMouseCapture);
	if( !Capture )
	{
		// An explicit release cancels any grab deferred below while still
		// waiting for focus; otherwise a later FOCUS_GAINED could complete
		// a grab into a menu that opened before focus ever arrived.
		LostGrab = 0;
	}
	else if( FocusOnly && SdlWindow && !(SDL_GetWindowFlags(SdlWindow) & SDL_WINDOW_INPUT_FOCUS) )
	{
		// Window doesn't have input focus yet -- a real race right as
		// gameplay starts, before the compositor finishes focusing the
		// freshly created window. Defer instead of dropping the request:
		// mark it pending exactly like a grab lost to a real focus loss, so
		// the existing SDL_WINDOWEVENT_FOCUS_GAINED handler's tested
		// HP2Capture::OnRestore completes it once focus genuinely arrives.
		Capture = 0;
		LostGrab = 1;
	}
	UpdateMouseGrabState( Capture );
	unguard;
}

void USDLViewport::DrawString( DWORD Flags, UFont* Font, INT& DrawX, INT& DrawY, const TCHAR* Text, const FPlane& Color )
{
	// Native text rendering rides the render device's canvas path.
}

UFont* USDLViewport::CreateNativeFont( const TCHAR* FontName, int Height )
{
	guard(USDLViewport::CreateNativeFont);
	UFont* Font = ConstructObject<UFont>( UFont::StaticClass(), GetOuter() );
	if( Font )
	{
		Font->FontName   = FontName;
		Font->FontHeight = Height;
		Font->NativeFont = NULL;
	}
	return Font;
	unguard;
}

/*-----------------------------------------------------------------------------
	Input helpers.
-----------------------------------------------------------------------------*/

UBOOL USDLViewport::CauseInputEvent( INT iKey, EInputAction Action, FLOAT Delta )
{
	guard(USDLViewport::CauseInputEvent);
	if( !GetOuterUSDLClient()->Engine || !Actor )
		return 0;
	return GetOuterUSDLClient()->Engine->InputEvent( this, (EInputKey)iKey, Action, Delta );
	unguard;
}

DWORD USDLViewport::GetViewportButtonFlags()
{
	INT X = 0, Y = 0;
	return ButtonsFromSdlState( SDL_GetMouseState( &X, &Y ) );
}

void USDLViewport::GetMouseState( FLOAT& X, FLOAT& Y, DWORD& Buttons )
{
	INT PixelX = 0, PixelY = 0;
	Buttons = ButtonsFromSdlState( SDL_GetMouseState( &PixelX, &PixelY ) );
	X = (FLOAT)PixelX * MouseScaleX;
	Y = (FLOAT)PixelY * MouseScaleY;
}

void USDLViewport::UpdateMouseGrabState( UBOOL Capture )
{
	guard(USDLViewport::UpdateMouseGrabState);
	Capture = Capture ? 1 : 0;
	// UWindow's console only adopts an absolute pointer position when this is
	// set (WindowConsole.uc RenderUWindow); otherwise it accumulates IK_MouseX/Y
	// deltas, which this driver emits only while grabbed, so a released-for-menu
	// cursor would never move. Mirrors UWindowsViewport::SetMouseCapture. Assign
	// above the early return so cold start (MouseIsGrabbed(0) at construction)
	// still sets the flag on the first UpdateMouseGrabState(0) call.
	bWindowsMouseAvailable = !Capture;
	if( Capture == (INT)MouseIsGrabbed )
		return; // No transition; cursor visibility must stay untouched.

	MouseIsGrabbed = Capture;
	if( SdlWindow )
		SDL_SetWindowGrab( SdlWindow, Capture ? SDL_TRUE : SDL_FALSE );
	SDL_SetRelativeMouseMode( Capture ? SDL_TRUE : SDL_FALSE );
	// Releasing shows the cursor; grabbing hides it again.
	SDL_ShowCursor( Capture ? SDL_DISABLE : SDL_ENABLE );
	unguard;
}

void USDLViewport::ReleaseJoystickInput()
{
	guard(USDLViewport::ReleaseJoystickInput);
	for( INT Index = 0; Index < USDLClient::MaxJoystickAxes; ++Index )
		CauseInputEvent( GJoyAxisKeys[Index], IST_Axis, 0.f );
	LastJoyHat = IK_None;
	unguard;
}

void USDLViewport::ReleaseAllInput()
{
	guard(USDLViewport::ReleaseAllInput);

	// Held keys release first, ascending IK order so bindings observe a
	// deterministic sequence.
	if( Input )
	{
		for( INT Key = 0; Key < IK_MAX; ++Key )
			if( Input->KeyDown(Key) )
				CauseInputEvent( Key, IST_Release, 0.f );
	}

	// Joystick axes neutralize next...
	ReleaseJoystickInput();

	// ...then the mouse axes.
	CauseInputEvent( IK_MouseX, IST_Axis, 0.f );
	CauseInputEvent( IK_MouseY, IST_Axis, 0.f );
	CauseInputEvent( IK_MouseW, IST_Axis, 0.f );

	unguard;
}

const TCHAR* USDLViewport::GetLocalizedKeyName( EInputKey Key ) const
{
	// Key-name localization lives with the game data; nothing to offer yet.
	return TEXT("");
}

UBOOL USDLViewport::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(USDLViewport::Exec);
	if( ParseCommand(&Cmd,TEXT("GETRES")) || ParseCommand(&Cmd,TEXT("GETCURRENTRES")) )
	{
		Ar.Logf( TEXT("%i %i %i"), SizeX, SizeY, ColorBytes );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GETCURRENTCOLORDEPTH")) )
	{
		Ar.Logf( TEXT("%i"), ColorBytes );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GETCURRENTBITDEPTH")) )
	{
		Ar.Logf( TEXT("%i"), ColorBytes * 8 );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("TOGGLEFULLSCREEN")) )
	{
		ToggleFullscreen();
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("DUMPCAPTUREDMOUSE")) )
	{
		Ar.Logf( TEXT("grabbed=%i lostgrab=%i scale=%f,%f"),
			(INT)MouseIsGrabbed, (INT)LostGrab, MouseScaleX, MouseScaleY );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GETSYSTEMINI")) )
	{
		Ar.Logf( TEXT("%s"), GConfig ? *static_cast<FConfigCacheIni*>(GConfig)->SystemIni : TEXT("") );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GETUSERINI")) )
	{
		Ar.Logf( TEXT("%s"), GConfig ? *static_cast<FConfigCacheIni*>(GConfig)->UserIni : TEXT("") );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("RELAUNCHSUPPORT")) )
	{
		EndFullscreen();
		TryRenderDevice( TEXT("ini:Engine.Engine.GameRenderDevice"), INDEX_NONE, INDEX_NONE, INDEX_NONE, IsFullscreen() );
		return 1;
	}
	return Super::Exec( Cmd, Ar );
	unguard;
}

void USDLViewport::TryRenderDevice( const TCHAR* ClassName, INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen )
{
	guard(USDLViewport::TryRenderDevice);
	USDLClient* C = GetOuterUSDLClient();

	// Shut down any current render device first.
	if( RenDev )
	{
		RenDev->Exit();
		delete RenDev;
		RenDev = NULL;
	}

	// Resolve defaults left unspecified by the caller.
	if( NewX == INDEX_NONE )
		NewX = Fullscreen ? C->FullscreenViewportX : C->WindowedViewportX;
	if( NewY == INDEX_NONE )
		NewY = Fullscreen ? C->FullscreenViewportY : C->WindowedViewportY;
	if( NewColorBytes == INDEX_NONE )
		NewColorBytes = Fullscreen ? C->FullscreenColorBits / 8 : ColorBytes;

	// Primary request, then the ini fallback appropriate to the mode.
	const TCHAR* Candidates[2] = { ClassName, NULL };
	if( !Fullscreen && appStricmp( ClassName, TEXT("ini:Engine.Engine.WindowedRenderDevice") ) != 0 )
		Candidates[1] = TEXT("ini:Engine.Engine.WindowedRenderDevice");
	else if( Fullscreen && appStricmp( ClassName, TEXT("ini:Engine.Engine.GameRenderDevice") ) != 0 )
		Candidates[1] = TEXT("ini:Engine.Engine.GameRenderDevice");

	for( INT Attempt = 0; Attempt < 2 && !RenDev; ++Attempt )
	{
		if( !Candidates[Attempt] )
			break;
		UClass* RenderClass = UObject::StaticLoadClass( URenderDevice::StaticClass(), NULL, Candidates[Attempt], NULL, LOAD_NoFail, NULL );
		if( !RenderClass )
			continue;
		RenDev = ConstructObject<URenderDevice>( RenderClass, this );
		if( !RenDev )
			continue;
		if( RenDev->Init( this, NewX, NewY, NewColorBytes, Fullscreen ) )
		{
			if( GIsRunning && Actor )
				Actor->GetLevel()->DetailChange( RenDev->HighDetailActors );
		}
		else
		{
			debugf( NAME_Log, TEXT("Render device %s failed to initialize"), Candidates[Attempt] );
			delete RenDev;
			RenDev = NULL;
		}
	}

	GRenderDevice = RenDev;
	unguard;
}

/*-----------------------------------------------------------------------------
	Fullscreen and window chrome.
-----------------------------------------------------------------------------*/

UBOOL USDLViewport::IsFullscreen()
{
	return (BlitFlags & BLIT_Fullscreen) ? 1 : 0;
}

void USDLViewport::ToggleFullscreen()
{
	guard(USDLViewport::ToggleFullscreen);
	LostFullscreen = IsFullscreen() ? 1 : 0;
	ResizeViewport( IsFullscreen() ? 0 : BLIT_Fullscreen, INDEX_NONE, INDEX_NONE, INDEX_NONE );
	unguard;
}

void USDLViewport::EndFullscreen()
{
	guard(USDLViewport::EndFullscreen);
	LostFullscreen = IsFullscreen() ? 1 : 0;
	ResizeViewport( 0, INDEX_NONE, INDEX_NONE, INDEX_NONE );
	unguard;
}

void USDLViewport::SetTopness()
{
	// SDL owns window stacking; nothing to do.
}

void USDLViewport::SetTitleBar( const TCHAR* Title )
{
	guard(USDLViewport::SetTitleBar);
	if( SdlWindow )
	{
		TArray<ANSICHAR> Utf8;
		TextToUtf8( Title, Utf8 );
		SDL_SetWindowTitle( SdlWindow, &Utf8(0) );
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Text to speech.
-----------------------------------------------------------------------------*/

void USDLViewport::TextToSpeech( const FString& Text, FLOAT Duration )
{
	guard(USDLViewport::TextToSpeech);
#if defined(__APPLE__)
	USDLClient* C = GetOuterUSDLClient();
	if( !C->MacNativeTextToSpeech )
		return;
	UpdateSpeech();

	// Fresh minimal backend: hand the utterance to the system speech
	// service and keep its pid around for non-blocking reaping.
	TArray<ANSICHAR> Utf8;
	TextToUtf8( *Text, Utf8 );
	const pid_t Child = fork();
	if( Child == 0 )
	{
		execl( "/usr/bin/say", "say", &Utf8(0), (char*)NULL );
		_exit( 127 );
	}
	else if( Child > 0 )
	{
		SpeechPid = Child;
	}
#else
	(void)Text;
	(void)Duration;
#endif
	unguard;
}

void USDLViewport::UpdateSpeech()
{
	guard(USDLViewport::UpdateSpeech);
#if defined(__APPLE__)
	if( SpeechPid > 0 )
	{
		// Non-blocking reap; a finished speaker is simply forgotten.
		const pid_t Done = waitpid( SpeechPid, NULL, WNOHANG );
		if( Done == SpeechPid || (Done < 0 && errno == ECHILD) )
			SpeechPid = -1;
	}
#endif
	unguard;
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

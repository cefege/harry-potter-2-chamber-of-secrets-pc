/*=============================================================================
	SDLViewport.cpp: SDL-based platform viewport.
=============================================================================*/

#include "SDLDrv.h"
#include "UnRenDev.h"
#include "hp2_rs.h"
#include "FConfigCacheIni.h"
#include <stddef.h>
#include <errno.h>
#include <string.h>
#if __UNIX__
	#include <sys/wait.h>
	#include <unistd.h>
#endif

/*-----------------------------------------------------------------------------
	Static registration.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(USDLViewport);

/*-----------------------------------------------------------------------------
	Locals.
-----------------------------------------------------------------------------*/

// Layout mirror of hp2-ffi's FfiViewportSize (crates/hp2-ffi/src/viewport.rs).
static_assert(sizeof(FfiViewportSize) == 16, "FfiViewportSize drifted from hp2-ffi");
static_assert(offsetof(FfiViewportSize, blit_flags) == 0, "FfiViewportSize drifted from hp2-ffi");
static_assert(offsetof(FfiViewportSize, size_x) == 4, "FfiViewportSize drifted from hp2-ffi");
static_assert(offsetof(FfiViewportSize, size_y) == 8, "FfiViewportSize drifted from hp2-ffi");
static_assert(offsetof(FfiViewportSize, color_bytes) == 12, "FfiViewportSize drifted from hp2-ffi");
static_assert(USDLViewport::NumSystemCursors == 7, "hp2_rs_viewport_mode_cursor_index assumes 7 system cursors");
// Mirrors of the input pump's C layouts (crates/hp2-ffi/src/input.rs) and of
// the SDL constants crates/hp2-platform/src/input_pump.rs reads. Rust asserts
// the same sizes and offsets.
static_assert(SDL_KEYDOWN == 0x300 && SDL_KEYUP == 0x301 && SDL_TEXTINPUT == 0x303, "SDL key/text event types drifted from input_pump.rs");
static_assert(SDL_MOUSEMOTION == 0x400 && SDL_MOUSEBUTTONDOWN == 0x401 && SDL_MOUSEBUTTONUP == 0x402 && SDL_MOUSEWHEEL == 0x403, "SDL mouse event types drifted from input_pump.rs");
static_assert(SDL_JOYAXISMOTION == 0x600 && SDL_JOYHATMOTION == 0x602 && SDL_JOYBUTTONDOWN == 0x603 && SDL_JOYBUTTONUP == 0x604, "SDL joystick event types drifted from input_pump.rs");
static_assert(SDL_WINDOWEVENT == 0x200, "SDL_WINDOWEVENT drifted from input_pump.rs");
static_assert(SDL_WINDOWEVENT_SHOWN == 1 && SDL_WINDOWEVENT_HIDDEN == 2 && SDL_WINDOWEVENT_RESIZED == 5 && SDL_WINDOWEVENT_MINIMIZED == 7 && SDL_WINDOWEVENT_RESTORED == 9 && SDL_WINDOWEVENT_FOCUS_GAINED == 12 && SDL_WINDOWEVENT_FOCUS_LOST == 13, "SDL window event ids drifted from input_pump.rs");
static_assert(SDL_WINDOW_HIDDEN == 0x8 && SDL_WINDOW_MINIMIZED == 0x40 && SDL_WINDOW_INPUT_FOCUS == 0x200, "SDL window flags drifted from input_pump.rs");
static_assert(SDL_TEXTINPUTEVENT_TEXT_SIZE == 32, "SDL_TEXTINPUTEVENT_TEXT_SIZE drifted from input.rs");
static_assert(sizeof(FfiSdlEvent) == 84, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, kind) == 0, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, key_sym) == 4, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, text) == 8, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, motion_x) == 40, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, motion_y) == 44, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, motion_state) == 48, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, button) == 52, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, wheel_y) == 56, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, joy_axis) == 60, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, joy_axis_value) == 64, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, joy_button) == 68, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, joy_hat) == 69, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, window_event) == 70, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, window_data1) == 72, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, window_data2) == 76, "FfiSdlEvent drifted from hp2-ffi");
static_assert(offsetof(FfiSdlEvent, window_flags) == 80, "FfiSdlEvent drifted from hp2-ffi");
static_assert(sizeof(FfiInputContext) == 40, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, mouse_is_grabbed) == 0, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, lost_grab) == 1, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, capture_mouse) == 2, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, has_window) == 3, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, has_engine) == 4, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, has_actor) == 5, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, joy_hat_folds_to_buttons) == 6, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, mouse_scale_x) == 8, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, mouse_scale_y) == 12, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, size_x) == 16, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, size_y) == 20, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, joystick_dead_zone) == 24, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, scale_jbx) == 28, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, scale_jby) == 32, "FfiInputContext drifted from hp2-ffi");
static_assert(offsetof(FfiInputContext, last_joy_hat) == 36, "FfiInputContext drifted from hp2-ffi");
static_assert(sizeof(FfiInputAction) == 40, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, kind) == 0, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, key) == 4, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, action) == 8, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, delta) == 12, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, buttons) == 16, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, x) == 20, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, y) == 24, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, width) == 28, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, height) == 32, "FfiInputAction drifted from hp2-ffi");
static_assert(offsetof(FfiInputAction, flag) == 36, "FfiInputAction drifted from hp2-ffi");
static_assert(InputActionCause == 0 && InputActionEngineKey == 1 && InputActionEngineMousePosition == 2 && InputActionUpdateMouseGrab == 3 && InputActionSetLostGrab == 4 && InputActionSetLastJoyHat == 5 && InputActionReleaseAll == 6 && InputActionResizeViewport == 7 && InputActionQueryKeyDown == 8, "FfiInputActionKind drifted from hp2-ffi");
static_assert(sizeof(FfiReleaseAllCursor) == 12, "FfiReleaseAllCursor drifted from hp2-ffi");
static_assert(offsetof(FfiReleaseAllCursor, phase) == 0, "FfiReleaseAllCursor drifted from hp2-ffi");
static_assert(offsetof(FfiReleaseAllCursor, index) == 4, "FfiReleaseAllCursor drifted from hp2-ffi");
static_assert(offsetof(FfiReleaseAllCursor, awaiting_key) == 8, "FfiReleaseAllCursor drifted from hp2-ffi");

// Window pixels per viewport pixel; identity until both sizes are known.
static void UpdateMouseScale( USDLViewport& Viewport )
{
	INT WindowX = 0, WindowY = 0;
	if( Viewport.SdlWindow )
		SDL_GetWindowSize( Viewport.SdlWindow, &WindowX, &WindowY );
	hp2_rs_viewport_mouse_scale( WindowX, WindowY, Viewport.SizeX, Viewport.SizeY, &Viewport.MouseScaleX, &Viewport.MouseScaleY );
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
	SDL_DisplayMode DesktopMode;
	memset( &DesktopMode, 0, sizeof(DesktopMode) );
	const bool bHasDesktopMode = SDL_GetDesktopDisplayMode( 0, &DesktopMode ) == 0 && DesktopMode.format != SDL_PIXELFORMAT_UNKNOWN;
	INT DesktopRgb565 = 0;
	hp2_rs_viewport_desktop_color_depth( bHasDesktopMode ? 1 : 0, bHasDesktopMode ? SDL_BITSPERPIXEL( DesktopMode.format ) : 0, &ColorBytes, &DesktopRgb565 );
	if( DesktopRgb565 )
		Caps |= CC_RGB565;

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
	// RenDev before SdlWindow: a render device's wgpu/GL/Vulkan
	// resources can hold a reference into the window's own surface
	// (e.g. WgpuDrv's wgpu::Surface borrows the SDL_MetalView's
	// CAMetalLayer -- WgpuDrv.h's own field comment), so it must be
	// torn down first, not left for GC to reach independently after
	// SDL_DestroyWindow has already freed what it pointed into. Same
	// explicit Exit()/delete/NULL pattern already used at
	// USDLClient::Destroy and USDLViewport::TryRenderDevice -- this
	// call site was simply missing it (a real, pre-existing gap
	// outside those two, not introduced here): confirmed directly via
	// a real -testticks run that UWgpuRenderDevice::Exit was never
	// reached at all before this fix -- USDLClient::Destroy's own
	// RenDev-cleanup loop found Viewports.Num() already 0 by the time
	// it ran, meaning the viewport (and the RenDev it owned) had
	// already been independently garbage-collected via this method,
	// bypassing both of the other two cleanup sites entirely.
	if( RenDev )
	{
		RenDev->Exit();
		delete RenDev;
		RenDev = NULL;
	}
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
	// Not RenDev->Exit()/delete RenDev the way Destroy above does:
	// this runs inside crash handling (appError/GError), where the
	// established UE1 convention is minimal teardown and no object
	// deletion -- UObject::StaticShutdownAfterError already walks
	// every live object (including RenDev, a real ConstructObject-
	// created UObject) and calls ConditionalShutdownAfterError on each
	// during the crash sweep. XOpenGL implements its own
	// ShutdownAfterError (XOpenGL.cpp:3249) for exactly this purpose,
	// gated behind its own "really want noncritical cleanup" flag;
	// UWgpuRenderDevice does not override it yet (inherits UObject's
	// no-op default), so RustDevice/MetalView simply leak on a crash
	// today -- acceptable: the process is terminating anyway and the
	// OS reclaims everything, matching how XOpenGL's own
	// ShutdownAfterError deliberately skips some cleanup by default
	// too.
	//
	// RenDev->ConditionalShutdownAfterError(), not the bare virtual
	// RenDev->ShutdownAfterError(): the conditional wrapper
	// (UnObj.cpp) sets the RF_ErrorShutdown flag and wraps the call in
	// try/catch, specifically so the later global
	// StaticShutdownAfterError sweep (which also calls
	// ConditionalShutdownAfterError on every live object, RenDev
	// included) sees the flag already set and skips calling it again.
	// Calling the bare virtual directly here -- an earlier version of
	// this fix did exactly that -- leaves the flag unset, so that
	// later sweep calls ShutdownAfterError on RenDev a *second* time;
	// XOpenGL's own version runs XOpenGLDestroyContext(glContext)
	// unconditionally on non-WGL builds, so a second call would
	// destroy the GL context twice during an already-crashing process
	// (caught on review, not self-caught -- a real double-destroy
	// risk the bare call would have reintroduced even after the
	// Exit()+delete removal above already fixed the worse risk).
	if( RenDev )
		RenDev->ConditionalShutdownAfterError();
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
	if( !hp2_rs_viewport_lock_allowed( RenDev != NULL, SizeX, SizeY ) )
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

	FfiViewportSize State = { BlitFlags, SizeX, SizeY, ColorBytes };
	hp2_rs_viewport_resize( &State, BlitType, NewX, NewY, NewColorBytes );
	BlitFlags = State.blit_flags;
	SizeX = State.size_x;
	SizeY = State.size_y;
	ColorBytes = State.color_bytes;

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
	const INT Index = hp2_rs_viewport_mode_cursor_index( (INT)SelectedCursor );
	SDL_Cursor* Cursor = SystemCursors[Index];
	if( Cursor )
		SDL_SetCursor( Cursor );
	unguard;
}

void USDLViewport::UpdateWindowFrame()
{
	guard(USDLViewport::UpdateWindowFrame);
	hp2_rs_platform_update_window_frame( SdlWindow, GetOuterUSDLClient()->BorderlessWindow != 0 );
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
	INT Width = 0, Height = 0;
	hp2_rs_viewport_open_window_size( NewX, NewY, OpenX, OpenY, C->WindowedViewportX, C->WindowedViewportY, &Width, &Height );

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
#if defined(__EMSCRIPTEN__)
		// Emscripten creates the WebGL2 context for this window. An explicit
		// alpha size of 0 and no multisampling match the framebuffer the driver
		// renders into, and avoid the multisample renderbuffer path.
		SDL_GL_SetAttribute( SDL_GL_ALPHA_SIZE, 0 );
		SDL_GL_SetAttribute( SDL_GL_MULTISAMPLEBUFFERS, 0 );
		SDL_GL_SetAttribute( SDL_GL_MULTISAMPLESAMPLES, 0 );
#endif

		DWORD Flags = hp2_rs_viewport_window_flags( Temporary ? 1 : 0, C->BorderlessWindow ? 1 : 0 );
		SdlWindow = SDL_CreateWindow(
			"Harry Potter 2",
			SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
			Width, Height, Flags );
		if( !SdlWindow )
			appErrorf( TEXT("SDL_CreateWindow failed: %s"), ANSI_TO_TCHAR(SDL_GetError()) );
	}

	SizeX = Width;
	SizeY = Height;
	ColorBytes = hp2_rs_viewport_window_color_bytes( C->WindowedColorBits );
	BlitFlags &= ~BLIT_Fullscreen;
	if( hp2_rs_viewport_startup_fullscreen( C->StartupFullscreen, C->UseDesktopResolution ) )
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
			appErrorf( TEXT("No render device could be initialized (see the WgpuDrv messages above for the failing reason)") );
	}
	unguard;
}

void USDLViewport::CloseWindow()
{
	// SDL windows need no explicit close choreography.
}

// Room for every action one polled event can produce; must match
// input_pump::MAX_ACTIONS (crates/hp2-platform/src/input_pump.rs).
enum { InputPumpMaxActions = 4 };

// Per-event snapshot the Rust pump translates against. Built fresh for each
// event so it sees the state the previous event's actions left behind.
static FfiInputContext MakeInputContext( const USDLViewport& Viewport, const USDLClient* C )
{
	FfiInputContext Ctx;
	memset( &Ctx, 0, sizeof(Ctx) );
	Ctx.mouse_is_grabbed = Viewport.MouseIsGrabbed != 0;
	Ctx.lost_grab = Viewport.LostGrab != 0;
	Ctx.capture_mouse = C->CaptureMouse != 0;
	Ctx.has_window = Viewport.SdlWindow != NULL;
	Ctx.has_engine = C->Engine != NULL;
	Ctx.has_actor = Viewport.Actor != NULL;
	Ctx.joy_hat_folds_to_buttons = C->JoyHatFoldsToButtons != 0;
	Ctx.mouse_scale_x = Viewport.MouseScaleX;
	Ctx.mouse_scale_y = Viewport.MouseScaleY;
	Ctx.size_x = Viewport.SizeX;
	Ctx.size_y = Viewport.SizeY;
	Ctx.joystick_dead_zone = C->JoystickDeadZone;
	Ctx.scale_jbx = C->ScaleJBX;
	Ctx.scale_jby = C->ScaleJBY;
	Ctx.last_joy_hat = Viewport.LastJoyHat;
	return Ctx;
}

// Flattens one SDL_Event into the plain FfiSdlEvent layout. Only the fields
// the pump reads for that event kind are copied.
static FfiSdlEvent FlattenSdlEvent( const USDLViewport& Viewport, const SDL_Event& Event )
{
	FfiSdlEvent Flat;
	memset( &Flat, 0, sizeof(Flat) );
	Flat.kind = Event.type;
	switch( Event.type )
	{
		case SDL_KEYDOWN:
		case SDL_KEYUP:
			Flat.key_sym = Event.key.keysym.sym;
			break;
		case SDL_TEXTINPUT:
			memcpy( Flat.text, Event.text.text, sizeof(Flat.text) );
			break;
		case SDL_MOUSEMOTION:
			Flat.motion_x = Event.motion.x;
			Flat.motion_y = Event.motion.y;
			Flat.motion_state = Event.motion.state;
			break;
		case SDL_MOUSEBUTTONDOWN:
		case SDL_MOUSEBUTTONUP:
			Flat.button = Event.button.button;
			break;
		case SDL_MOUSEWHEEL:
			Flat.wheel_y = Event.wheel.y;
			break;
		case SDL_JOYAXISMOTION:
			Flat.joy_axis = Event.jaxis.axis;
			Flat.joy_axis_value = Event.jaxis.value;
			break;
		case SDL_JOYBUTTONDOWN:
		case SDL_JOYBUTTONUP:
			Flat.joy_button = Event.jbutton.button;
			break;
		case SDL_JOYHATMOTION:
			Flat.joy_hat = Event.jhat.value;
			break;
		case SDL_WINDOWEVENT:
			Flat.window_event = Event.window.event;
			Flat.window_data1 = Event.window.data1;
			Flat.window_data2 = Event.window.data2;
			if( Viewport.SdlWindow )
				Flat.window_flags = SDL_GetWindowFlags( Viewport.SdlWindow );
			break;
		default:
			break;
	}
	return Flat;
}

// Runs the actions the pump returned, in order. Cause still goes through
// CauseInputEvent, which re-checks Engine and Actor at call time.
static void ExecuteInputActions( USDLViewport& Viewport, USDLClient* C, const FfiInputAction* Actions, INT Count )
{
	for( INT Index = 0; Index < Count; ++Index )
	{
		const FfiInputAction& Step = Actions[Index];
		switch( Step.kind )
		{
			case InputActionCause:
				Viewport.CauseInputEvent( Step.key, (EInputAction)Step.action, Step.delta );
				break;
			case InputActionEngineKey:
				C->Engine->Key( &Viewport, (EInputKey)Step.key );
				break;
			case InputActionEngineMousePosition:
				C->Engine->MousePosition( &Viewport, Step.buttons, Step.x, Step.y );
				break;
			case InputActionUpdateMouseGrab:
				Viewport.UpdateMouseGrabState( Step.flag ? 1 : 0 );
				break;
			case InputActionSetLostGrab:
				Viewport.LostGrab = Step.flag ? 1 : 0;
				break;
			case InputActionSetLastJoyHat:
				Viewport.LastJoyHat = (EInputKey)Step.key;
				break;
			case InputActionReleaseAll:
				Viewport.ReleaseAllInput();
				break;
			case InputActionResizeViewport:
				Viewport.ResizeViewport( BLIT_NoWindowChange, Step.width, Step.height, INDEX_NONE );
				break;
			default:
				// InputActionQueryKeyDown is produced only by the release stepper.
				break;
		}
	}
}

void USDLViewport::UpdateInput( UBOOL Reset )
{
	guard(USDLViewport::UpdateInput);
	USDLClient* C = GetOuterUSDLClient();

	if( Reset )
		ReleaseAllInput();

	// Relative mouse deltas are sampled once per pump while grabbed. Rust maps
	// them to IK_MouseX/IK_MouseY with the original sign and scale.
	if( MouseIsGrabbed && SdlWindow )
	{
		INT DeltaX = 0, DeltaY = 0;
		SDL_GetRelativeMouseState( &DeltaX, &DeltaY );
		const FfiInputContext Ctx = MakeInputContext( *this, C );
		FfiInputAction Actions[InputPumpMaxActions];
		const INT Count = hp2_rs_input_relative_mouse( &Ctx, DeltaX, DeltaY, Actions );
		ExecuteInputActions( *this, C, Actions, Count );
	}

	// Each polled event is flattened, translated against the state it arrives
	// in, and its actions run before the next event is read.
	SDL_Event Event;
	while( SDL_PollEvent(&Event) )
	{
		const FfiSdlEvent Flat = FlattenSdlEvent( *this, Event );
		const FfiInputContext Ctx = MakeInputContext( *this, C );
		FfiInputAction Actions[InputPumpMaxActions];
		const INT Count = hp2_rs_input_translate_event( &Ctx, &Flat, Actions );
		ExecuteInputActions( *this, C, Actions, Count );
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
		// the window-event pump's FOCUS_GAINED restore (input_pump.rs, through
		// mouse_capture::on_restore) completes it once focus genuinely arrives.
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
	return hp2_rs_platform_get_viewport_button_flags();
}

void USDLViewport::GetMouseState( FLOAT& X, FLOAT& Y, DWORD& Buttons )
{
	Buttons = hp2_rs_platform_get_mouse_state( MouseScaleX, MouseScaleY, &X, &Y );
}

void USDLViewport::UpdateMouseGrabState( UBOOL Capture )
{
	guard(USDLViewport::UpdateMouseGrabState);
	// UWindow's console only adopts an absolute pointer position when this is
	// set (WindowConsole.uc RenderUWindow); otherwise it accumulates IK_MouseX/Y
	// deltas, which this driver emits only while grabbed, so a released-for-menu
	// cursor would never move. Mirrors UWindowsViewport::SetMouseCapture.
	//
	// hp2_rs_platform_update_mouse_grab_state (crates/hp2-platform/src/
	// mouse_state.rs) replaces this function's own body: the early-return
	// guard, SDL_SetWindowGrab/SetRelativeMouseMode/ShowCursor calls, and
	// the bWindowsMouseAvailable/MouseIsGrabbed transition logic all now
	// live there. This call site only owns the UObject bitfields Rust
	// cannot touch directly.
	bool bOutWindowsMouseAvailable = false;
	const bool bNewGrabbed = hp2_rs_platform_update_mouse_grab_state(
		SdlWindow, Capture != 0, MouseIsGrabbed != 0, &bOutWindowsMouseAvailable );
	bWindowsMouseAvailable = bOutWindowsMouseAvailable;
	MouseIsGrabbed = bNewGrabbed;
	unguard;
}

void USDLViewport::ReleaseAllInput()
{
	guard(USDLViewport::ReleaseAllInput);

	// Order lives in Rust (input_pump::release_all_next): held keys release
	// first in ascending IK order, then the joystick axes neutralize and
	// LastJoyHat resets, then the mouse axes. The shell only answers key-held
	// queries and executes the steps in order.
	FfiReleaseAllCursor Cursor = hp2_rs_input_release_all_begin( Input != NULL );
	bool bHeld = false;
	FfiInputAction Step;
	while( hp2_rs_input_release_all_next( &Cursor, bHeld, &Step ) )
	{
		bHeld = false;
		switch( Step.kind )
		{
			case InputActionQueryKeyDown:
				bHeld = Input->KeyDown( Step.key ) != 0;
				break;
			case InputActionCause:
				CauseInputEvent( Step.key, (EInputAction)Step.action, Step.delta );
				break;
			case InputActionSetLastJoyHat:
				LastJoyHat = (EInputKey)Step.key;
				break;
			default:
				break;
		}
	}

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
	return hp2_rs_viewport_is_fullscreen( BlitFlags );
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
	TArray<ANSICHAR> Utf8;
	TextToUtf8( Title, Utf8 );
	hp2_rs_platform_set_title_bar( SdlWindow, reinterpret_cast<const uint8_t*>(&Utf8(0)), strlen(&Utf8(0)) );
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

/*=============================================================================
	PlatformMouseStateTests.cpp: SDL main-thread checks for
	hp2_rs_platform_get_viewport_button_flags/get_mouse_state/
	update_mouse_grab_state (crates/hp2-platform/src/mouse_state.rs),
	replacing USDLViewport::GetViewportButtonFlags/GetMouseState/
	UpdateMouseGrabState (SDLViewport.cpp).

	No existing C++ oracle test exercises these three functions (they
	are UObject member functions of USDLViewport, which none of
	Tests/InputContractTests.cpp's headless harness constructs), so
	this is new coverage, not a port of an existing test. Runs on the
	dummy SDL video driver for the same reasons as
	PlatformClipboardTests.cpp: no real window/display touched, works
	headless on linux-arm64 CI.

	Two real SDL calls inside update_mouse_grab_state cannot be
	verified through their own readback under this driver (confirmed
	empirically with throwaway probes against the pinned SDL2 build
	before writing these assertions, not assumed):
	  - SDL_SetRelativeMouseMode fails outright under dummy
	    ("No relative mode implementation available";
	    SDL_GetRelativeMouseMode stays false regardless of what was
	    requested) -- the dummy driver provides neither
	    SetRelativeMouseMode nor WarpMouse (pinned SDL_mouse.c).
	  - SDL_SetWindowGrab silently has no effect under dummy either;
	    SDL_GetWindowGrab reports false no matter what was requested.
	SDL_ShowCursor(SDL_QUERY) *does* work reliably under dummy (it is
	a process-global SDL_Mouse toggle, not a per-video-driver window
	property) and is asserted below for both transitions plus the
	no-transition early-return case. A real (hidden) SDL_Window is
	still created and passed through, so SDL_SetWindowGrab's call path
	is genuinely exercised (not skipped via a null-window guard), even
	though its effect isn't independently checkable here.
=============================================================================*/

#include "SDLDrv.h"
#include "hp2_rs.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>

namespace
{

void Require( bool Condition, const char* Message )
{
	if( !Condition )
	{
		std::fprintf( stderr, "PlatformMouseStateTests: FAIL: %s\n", Message );
		std::exit( 1 );
	}
}

} // namespace

int main()
{
	SDL_SetHint( SDL_HINT_VIDEODRIVER, "dummy" );
	Require( SDL_Init( SDL_INIT_VIDEO ) == 0, "SDL_Init(SDL_INIT_VIDEO) failed" );
	const char* ActiveDriver = SDL_GetCurrentVideoDriver();
	Require( ActiveDriver && std::strcmp( ActiveDriver, "dummy" ) == 0, "expected the dummy SDL video driver, got a real one" );

	// GetViewportButtonFlags / GetMouseState: no real pointing device
	// exists under the dummy driver, so the only real assertion is
	// that the real linked SDL_GetMouseState call round-trips without
	// crashing and reports no buttons held.
	const DWORD Flags = hp2_rs_platform_get_viewport_button_flags();
	Require( Flags == 0, "expected no mouse buttons held under the dummy driver" );

	FLOAT X = -1.f, Y = -1.f;
	const DWORD Buttons = hp2_rs_platform_get_mouse_state( 2.f, 3.f, &X, &Y );
	Require( Buttons == 0, "GetMouseState buttons should match GetViewportButtonFlags (none held)" );
	Require( X == 0.f && Y == 0.f, "expected the dummy driver's mouse position to scale to (0,0)" );

	// UpdateMouseGrabState's transition contract, matching the real
	// C++'s own early-return guard (HP2MouseCapturePolicy.h's "Current
	// truth" note: capture state changes only on an actual
	// grabbed/released transition, not every call). A real hidden
	// window exercises the SDL_SetWindowGrab call path genuinely
	// (see the file header for why its effect isn't independently
	// checkable under the dummy driver).
	SDL_Window* Window = SDL_CreateWindow( "mouse-state-test", 0, 0, 64, 64, SDL_WINDOW_HIDDEN );
	Require( Window != NULL, "SDL_CreateWindow failed under the dummy driver" );

	bool bWindowsMouseAvailable = false;

	// Not grabbed -> grab: a real transition. Cursor must hide.
	bool bGrabbed = hp2_rs_platform_update_mouse_grab_state( Window, true, false, &bWindowsMouseAvailable );
	Require( bGrabbed == true, "expected grab to engage on a false->true transition" );
	Require( bWindowsMouseAvailable == false, "expected bWindowsMouseAvailable false while grabbed" );
	Require( SDL_ShowCursor( SDL_QUERY ) == SDL_DISABLE, "expected the cursor hidden after grabbing" );

	// Already grabbed -> grab again: no transition (idempotent).
	// "Cursor visibility must stay untouched" (HP2MouseCapturePolicy.h):
	// show the cursor by hand first, then confirm the no-op call does
	// NOT hide it again.
	SDL_ShowCursor( SDL_ENABLE );
	bGrabbed = hp2_rs_platform_update_mouse_grab_state( Window, true, true, &bWindowsMouseAvailable );
	Require( bGrabbed == true, "expected grab to remain engaged on a true->true call" );
	Require( bWindowsMouseAvailable == false, "bWindowsMouseAvailable must still reflect capture=true" );
	Require( SDL_ShowCursor( SDL_QUERY ) == SDL_ENABLE, "no-transition call must leave cursor visibility untouched" );

	// Grabbed -> release: a real transition. Cursor must show. Force
	// it to DISABLE first so the ENABLE check below actually proves
	// release changed it, rather than coincidentally observing the
	// ENABLE state the prior no-transition check already left behind.
	SDL_ShowCursor( SDL_DISABLE );
	bGrabbed = hp2_rs_platform_update_mouse_grab_state( Window, false, true, &bWindowsMouseAvailable );
	Require( bGrabbed == false, "expected grab to release on a true->false transition" );
	Require( bWindowsMouseAvailable == true, "expected bWindowsMouseAvailable true once released" );
	Require( SDL_ShowCursor( SDL_QUERY ) == SDL_ENABLE, "expected the cursor visible after releasing" );

	// Null window: must still no-op safely, matching the real C++'s
	// own `if (SdlWindow)` guard around SDL_SetWindowGrab specifically.
	bGrabbed = hp2_rs_platform_update_mouse_grab_state( NULL, false, true, &bWindowsMouseAvailable );
	Require( bGrabbed == false, "null window must not prevent the grab/release transition itself" );

	SDL_DestroyWindow( Window );
	SDL_Quit();
	std::puts( "PlatformMouseStateTests: PASS" );
	return 0;
}

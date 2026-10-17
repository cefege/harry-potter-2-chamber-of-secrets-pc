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
	headless on linux-arm64 CI, and the transition behavior (does a
	call to UpdateMouseGrabState change anything) does not depend on a
	real window existing -- the real C++ itself guards
	SDL_SetWindowGrab specifically with `if (SdlWindow)`, so a null
	window here exercises exactly that guarded path.
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
	// grabbed/released transition, not every call).
	bool bWindowsMouseAvailable = false;

	// Not grabbed -> grab: a real transition.
	bool bGrabbed = hp2_rs_platform_update_mouse_grab_state( NULL, true, false, &bWindowsMouseAvailable );
	Require( bGrabbed == true, "expected grab to engage on a false->true transition" );
	Require( bWindowsMouseAvailable == false, "expected bWindowsMouseAvailable false while grabbed" );

	// Already grabbed -> grab again: no transition (idempotent).
	bGrabbed = hp2_rs_platform_update_mouse_grab_state( NULL, true, true, &bWindowsMouseAvailable );
	Require( bGrabbed == true, "expected grab to remain engaged on a true->true call" );
	Require( bWindowsMouseAvailable == false, "bWindowsMouseAvailable must still reflect capture=true" );

	// Grabbed -> release: a real transition.
	bGrabbed = hp2_rs_platform_update_mouse_grab_state( NULL, false, true, &bWindowsMouseAvailable );
	Require( bGrabbed == false, "expected grab to release on a true->false transition" );
	Require( bWindowsMouseAvailable == true, "expected bWindowsMouseAvailable true once released" );

	SDL_Quit();
	std::puts( "PlatformMouseStateTests: PASS" );
	return 0;
}

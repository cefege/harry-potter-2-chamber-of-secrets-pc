/*=============================================================================
	PlatformWindowChromeTests.cpp: SDL main-thread checks for
	hp2_rs_platform_update_window_frame/set_title_bar
	(crates/hp2-platform/src/window_chrome.rs), replacing
	USDLViewport::UpdateWindowFrame/SetTitleBar (SDLViewport.cpp).

	No existing C++ oracle exercises these two functions (USDLViewport
	member functions; Tests/InputContractTests.cpp's headless harness
	never constructs a real USDLViewport instance), so this is new
	coverage. Creates a real (off-screen, invisible) SDL_Window under
	the dummy video driver -- unlike PlatformClipboardTests.cpp/
	PlatformMouseStateTests.cpp, which never need one. set_title_bar
	gets a genuine round trip (set, then read back via
	SDL_GetWindowTitle, compare); update_window_frame only gets a
	does-not-crash check here, because the dummy driver does not
	update SDL_GetWindowFlags' SDL_WINDOW_BORDERLESS bit in response
	to SDL_SetWindowBordered (confirmed empirically before writing
	this file, not assumed) -- there is nothing stronger to assert
	through SDL state under this driver for that specific call. The
	`borderless` -> SDL_TRUE/SDL_FALSE inversion itself (the only real
	logic in update_window_frame) is independently pinned by a plain
	Rust unit test instead:
	crates/hp2-platform/src/window_chrome.rs's
	sdl_bordered_flag_inverts_borderless, which needs no SDL driver at
	all.
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
		std::fprintf( stderr, "PlatformWindowChromeTests: FAIL: %s\n", Message );
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

	// Null window: both functions must no-op, matching the real C++'s
	// own `if (SdlWindow)` guards -- must not crash.
	hp2_rs_platform_update_window_frame( NULL, true );
	const uint8_t NullTitle[] = "ignored";
	hp2_rs_platform_set_title_bar( NULL, NullTitle, sizeof(NullTitle) - 1 );

	// A real (off-screen) window under the dummy driver.
	SDL_Window* Window = SDL_CreateWindow( "initial", 0, 0, 64, 64, SDL_WINDOW_HIDDEN );
	Require( Window != NULL, "SDL_CreateWindow failed under the dummy driver" );

	const uint8_t Title[] = "hp2-platform window chrome round-trip";
	hp2_rs_platform_set_title_bar( Window, Title, sizeof(Title) - 1 );
	const char* ReadBack = SDL_GetWindowTitle( Window );
	Require( ReadBack && std::strcmp( ReadBack, reinterpret_cast<const char*>(Title) ) == 0, "window title did not round-trip" );

	// SDL's "dummy" video driver (no real windowing backend) does not
	// update SDL_GetWindowFlags' SDL_WINDOW_BORDERLESS bit in response
	// to SDL_SetWindowBordered -- confirmed empirically with a
	// throwaway probe directly against the pinned SDL2 build before
	// writing this assertion: SDL_GetWindowFlags reports the same
	// value before and after the call under this driver. The real
	// C++ this replaces doesn't check the result either (fire-and-
	// forget, same as here); what's left to verify under this driver
	// is that the real linked hp2_rs_platform_update_window_frame ->
	// SDL_SetWindowBordered call happens and does not crash.
	hp2_rs_platform_update_window_frame( Window, true );
	hp2_rs_platform_update_window_frame( Window, false );

	SDL_DestroyWindow( Window );
	SDL_Quit();
	std::puts( "PlatformWindowChromeTests: PASS" );
	return 0;
}

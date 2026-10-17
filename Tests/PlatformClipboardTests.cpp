/*=============================================================================
	PlatformClipboardTests.cpp: SDL main-thread round-trip for
	hp2_rs_platform_get_clipboard_text/set_clipboard_text
	(crates/hp2-platform/src/clipboard.rs), replacing
	USDLClient::GetClipboardText/SetClipboardText (SDLClient.cpp).

	Exercises the compiled FFI functions directly (not a constructed
	UObject USDLClient instance -- GetClipboardText/SetClipboardText are
	a three-line pass-through to these same two functions plus UE1's own
	already-covered TCHAR/UTF-8 conversion helpers, so the thing actually
	worth a dedicated regression test is the FFI/SDL linkage itself: that
	hp2_ffi's SDL2 symbol references resolve against this binary's one
	SDL2::SDL2-static, not a second copy, and that a round trip through
	SDL's own clipboard storage works end to end). Runs SDL_Init on this
	process's actual main thread, same as the real engine's own startup
	path -- unlike a Rust `#[test]` in hp2-platform's own suite, which
	runs on a libtest worker thread and cannot call into SDL's video
	subsystem at all on macOS (see clipboard.rs's module doc for why).

	Forces the "dummy" SDL video driver (no real window/display backend)
	before SDL_Init, for two reasons: (1) the real Cocoa driver's
	clipboard is the actual, global, per-user macOS pasteboard --
	every run of this test would silently overwrite whatever the person
	running it had just copied, a real side effect a ctest must never
	have; (2) a headless `linux-arm64` CI runner has no real display to
	open at all, so a real video driver would fail SDL_Init outright.
	SDL2's SDL_SetClipboardText/GetClipboardText fall back to an
	in-process copy when the active driver supplies no clipboard hooks,
	which the dummy driver does not -- confirmed by reading
	src/video/SDL_clipboard.c's SDL_SetClipboardText/GetClipboardText
	against `_this->SetClipboardText`/`GetClipboardText` being NULL for
	the dummy driver. So this still exercises the real
	hp2_rs_platform_* -> SDL_* path through the one linked SDL2, just
	without a real OS-level clipboard underneath. Asserts
	SDL_GetCurrentVideoDriver() actually reports "dummy" so the test
	can't silently fall onto a real backend if that ever changes.
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
		std::fprintf( stderr, "PlatformClipboardTests: FAIL: %s\n", Message );
		std::exit( 1 );
	}
}

struct FCapturedText
{
	char Buffer[256];
	size_t Len;
};

void CaptureTextSink( void* Ctx, const uint8_t* Text, size_t Len )
{
	FCapturedText* Captured = static_cast<FCapturedText*>( Ctx );
	const size_t Copy = Len < sizeof(Captured->Buffer) - 1 ? Len : sizeof(Captured->Buffer) - 1;
	std::memcpy( Captured->Buffer, Text, Copy );
	Captured->Buffer[Copy] = 0;
	Captured->Len = Copy;
}

} // namespace

int main()
{
	SDL_SetHint( SDL_HINT_VIDEODRIVER, "dummy" );
	Require( SDL_Init( SDL_INIT_VIDEO ) == 0, "SDL_Init(SDL_INIT_VIDEO) failed" );
	const char* ActiveDriver = SDL_GetCurrentVideoDriver();
	Require( ActiveDriver && std::strcmp( ActiveDriver, "dummy" ) == 0, "expected the dummy SDL video driver, got a real one" );

	// Includes a two-byte UTF-8 sequence (U+00E9, "e" with acute accent) so
	// the round trip exercises more than the pure-ASCII fast path.
	const char* Marker = "hp2-platform clipboard round-trip: caf\xC3\xA9";
	Require(
		hp2_rs_platform_set_clipboard_text( reinterpret_cast<const uint8_t*>(Marker), std::strlen(Marker) ),
		"hp2_rs_platform_set_clipboard_text failed" );

	FCapturedText Captured{};
	const bool HadText = hp2_rs_platform_get_clipboard_text( &CaptureTextSink, &Captured );
	Require( HadText, "hp2_rs_platform_get_clipboard_text reported no text after a successful set" );
	Require( std::strcmp( Captured.Buffer, Marker ) == 0, "round-tripped clipboard text does not match what was set" );

	SDL_Quit();
	std::puts( "PlatformClipboardTests: PASS" );
	return 0;
}

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
	SDL2::SDL2-static, not a second copy, and that the round trip through
	SDL's real clipboard works end to end). Runs SDL_Init on this
	process's actual main thread, same as the real engine's own startup
	path -- unlike a Rust `#[test]` in hp2-platform's own suite, which
	runs on a libtest worker thread and cannot call into SDL's video
	subsystem at all on macOS (see clipboard.rs's module doc for why).
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
	Require( SDL_Init( SDL_INIT_VIDEO ) == 0, "SDL_Init(SDL_INIT_VIDEO) failed" );

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

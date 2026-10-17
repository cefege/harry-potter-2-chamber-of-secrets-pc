/*=============================================================================
	WgpuSurfaceTests.cpp: real-window check for
	hp2_rs_wgpu_create_surface_from_ns_view_for_testing
	(crates/hp2-wgpu/src/lib.rs's macos module), macOS only, gated the
	same as WgpuDrv itself (HP2_ENABLE_WGPU_DRIVER).

	Unlike every other SDL-touching ctest in this project, this one
	cannot use the "dummy" video driver: SDL_Metal_CreateView needs a
	genuine windowing backend to attach a CAMetalLayer-backed NSView
	to, which the dummy driver (headless, no real window system) does
	not provide. Uses the real default video driver instead, relying
	on this being run on a machine with an actual display (true for
	the macOS runners/dev machines this experimental driver targets;
	HP2_ENABLE_WGPU_DRIVER defaults OFF everywhere else).

	No existing C++ oracle: this logic (SDL_Metal_CreateView ->
	SDL_Metal_GetLayer's NSView pointer -> wgpu::Surface) has no
	XOpenGL/Vulkan equivalent to port from; it is new.
=============================================================================*/

#include "SDLDrv.h"
#include "hp2_rs.h"

#include <SDL_metal.h>

#include <cstdio>
#include <cstdlib>

namespace
{

void Require( bool Condition, const char* Message )
{
	if( !Condition )
	{
		std::fprintf( stderr, "WgpuSurfaceTests: FAIL: %s\n", Message );
		std::exit( 1 );
	}
}

} // namespace

int main()
{
	Require( SDL_Init( SDL_INIT_VIDEO ) == 0, "SDL_Init(SDL_INIT_VIDEO) failed" );

	// Null view: must not crash, matching the real Rust function's own
	// `NonNull::new(...).expect(...)` -- wait, a null view is a logic
	// error (the real caller, UWgpuRenderDevice::Init, would never
	// pass one once that integration lands), not a value this
	// function is meant to handle gracefully. Not tested here for
	// that reason -- it is documented as a precondition (`# Safety`
	// on create_surface_from_ns_view), not a runtime-checked case.

	SDL_Window* Window = SDL_CreateWindow( "wgpu surface test", 0, 0, 64, 64, SDL_WINDOW_HIDDEN | SDL_WINDOW_METAL );
	Require( Window != NULL, "SDL_CreateWindow with SDL_WINDOW_METAL failed" );

	SDL_MetalView MetalView = SDL_Metal_CreateView( Window );
	Require( MetalView != NULL, "SDL_Metal_CreateView failed" );

	void* NsView = MetalView; // "can be cast directly to a NSView or UIView" -- SDL_metal.h's own doc comment
	void* Layer = SDL_Metal_GetLayer( MetalView );
	Require( Layer != NULL, "SDL_Metal_GetLayer returned NULL -- no CAMetalLayer attached" );

	const bool Created = hp2_rs_wgpu_create_surface_from_ns_view_for_testing( NsView );
	Require( Created, "hp2_rs_wgpu_create_surface_from_ns_view_for_testing failed to create a wgpu::Surface from a real SDL_Metal_CreateView NSView" );

	SDL_Metal_DestroyView( MetalView );
	SDL_DestroyWindow( Window );
	SDL_Quit();
	std::puts( "WgpuSurfaceTests: PASS" );
	return 0;
}

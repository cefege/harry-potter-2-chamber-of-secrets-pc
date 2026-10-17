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

	// Precondition, not a runtime-checked case: ns_view must be non-null
	// (create_surface_from_ns_view's own # Safety doc), so a null view
	// is not exercised here.

	// SDL_WINDOW_OPENGL, not SDL_WINDOW_METAL: the real engine always
	// creates its window with SDL_WINDOW_OPENGL (SDLViewport.cpp:268,
	// unconditional regardless of which render device is selected).
	// SDL_Metal_CreateView converts it to Metal on the fly (confirmed
	// directly against the pinned SDL2 source, SDL_video.c:4795-4809:
	// it clears SDL_WINDOW_OPENGL, calls SDL_GL_UnloadLibrary(), and
	// sets SDL_WINDOW_METAL) -- that conversion is one-way, so once
	// UWgpuRenderDevice::Init calls it on the real game window (not
	// yet wired, see hp2-wgpu's own module doc), a later fallback to
	// XOpenGL on that same window would have no GL library loaded.
	// Using SDL_WINDOW_OPENGL here, not SDL_WINDOW_METAL directly,
	// exercises the actual conversion path Init will take instead of a
	// window that was already Metal from the start.
	SDL_Window* Window = SDL_CreateWindow( "wgpu surface test", 0, 0, 64, 64, SDL_WINDOW_HIDDEN | SDL_WINDOW_OPENGL );
	Require( Window != NULL, "SDL_CreateWindow with SDL_WINDOW_OPENGL failed" );

	SDL_MetalView MetalView = SDL_Metal_CreateView( Window );
	Require( MetalView != NULL, "SDL_Metal_CreateView failed" );

	void* NsView = MetalView; // "can be cast directly to a NSView or UIView" -- SDL_metal.h's own doc comment
	void* Layer = SDL_Metal_GetLayer( MetalView );
	Require( Layer != NULL, "SDL_Metal_GetLayer returned NULL -- no CAMetalLayer attached" );

	// A pass here proves the raw-handle construction and the real
	// device/adapter sharing one Instance with the surface (see
	// hp2_rs_wgpu_create_surface_from_ns_view_for_testing's own doc) --
	// not that the surface actually works for presentation. The
	// stronger check (surface.configure(&device, ...) plus
	// get_current_texture() succeeding) is still open; see hp2-wgpu's
	// own module doc.
	const bool Created = hp2_rs_wgpu_create_surface_from_ns_view_for_testing( NsView );
	Require( Created, "hp2_rs_wgpu_create_surface_from_ns_view_for_testing failed to create a wgpu::Surface from a real SDL_Metal_CreateView NSView" );

	SDL_Metal_DestroyView( MetalView );
	SDL_DestroyWindow( Window );
	SDL_Quit();
	std::puts( "WgpuSurfaceTests: PASS" );
	return 0;
}

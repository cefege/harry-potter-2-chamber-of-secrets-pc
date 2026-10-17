/*=============================================================================
	WgpuSurfaceTests.cpp: real-window check for hp2_rs_wgpu_create's
	window-surface path (crates/hp2-wgpu/src/lib.rs's macos module),
	macOS only, gated the same as WgpuDrv itself (HP2_ENABLE_WGPU_DRIVER).
	Exercises the exact same hp2_rs_wgpu_create/hp2_rs_wgpu_destroy pair
	UWgpuRenderDevice::Init/Exit call -- not a parallel test-only
	function (an earlier version of this test called a now-deleted
	hp2_rs_wgpu_create_surface_from_ns_view_for_testing; merged into
	the real function once Init actually needed the same ns_view
	parameter, caught on review as worth consolidating rather than
	keeping two near-identical code paths). Also exercises
	hp2_rs_wgpu_present_once_for_testing: configure/acquire/present a
	real frame, the stronger proof noted as still-open in hp2-wgpu's
	own module doc, beyond "the surface is compatible with the
	adapter" (which create_device_sync itself already enforces).

	Unlike every other SDL-touching ctest in this project, this one
	cannot use the "dummy" video driver: SDL_Metal_CreateView needs a
	genuine windowing backend to attach a CAMetalLayer-backed NSView
	to, which the dummy driver (headless, no real window system) does
	not provide. Uses the real default video driver instead, relying
	on this being run on a machine with an actual display (true for
	the macOS runners/dev machines this experimental driver targets;
	HP2_ENABLE_WGPU_DRIVER defaults OFF everywhere else).

	No existing C++ oracle: this logic (SDL_Metal_CreateView ->
	SDL_Metal_GetLayer's NSView pointer -> wgpu::Surface, and the
	configure/acquire/present sequence) has no XOpenGL/Vulkan
	equivalent to port from; it is new.
=============================================================================*/

#include "SDLDrv.h"
#include "hp2_rs.h"

#include <SDL_metal.h>

#include <cstdint>
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
	// macOS 14+ changed SDL's own default: unbundled command-line
	// binaries are now treated as a "background app" by default
	// (SDL_cocoaevents.m's own Cocoa_RegisterApp, called from
	// SDL_Init(SDL_INIT_VIDEO)'s Cocoa_VideoInit), which skips
	// [NSApp activateIgnoringOtherApps:YES]. Set here as a precaution.
	// On its own (a single, non-retried present-once attempt) this did
	// not stop this project's own dev environment from reporting
	// Occluded, same as SDL_RaiseWindow and SDL_WINDOW_ALWAYS_ON_TOP
	// below on their own, and same as an isolated AppKit diagnostic
	// run outside this codebase (app.activate(ignoringOtherApps: true)
	// plus run-loop pumping did not make NSApp.isActive true;
	// window.level = .floating did not set occlusionState's Visible
	// bit). What does work reliably, confirmed over several repeated
	// runs of this actual test binary: all of these precautions
	// together, plus polling the present-once call itself for up to
	// ~2 seconds (below) rather than trying it once. Which single
	// element of that combination is the one actually necessary was
	// not isolated (no ablation study was run) -- all are kept rather
	// than guessing which to drop. Must be set before SDL_Init if it
	// is to have any effect at all: Cocoa_RegisterApp runs during
	// video init, not window creation.
	SDL_SetHint( SDL_HINT_MAC_BACKGROUND_APP, "0" );
	Require( SDL_Init( SDL_INIT_VIDEO ) == 0, "SDL_Init(SDL_INIT_VIDEO) failed" );

	// SDL_WINDOW_OPENGL, not SDL_WINDOW_METAL: the real engine always
	// creates its window with SDL_WINDOW_OPENGL (SDLViewport.cpp:268,
	// unconditional regardless of which render device is selected).
	// SDL_Metal_CreateView converts it to Metal on the fly (confirmed
	// directly against the pinned SDL2 source, SDL_video.c:4795-4809:
	// it clears SDL_WINDOW_OPENGL, calls SDL_GL_UnloadLibrary(), and
	// sets SDL_WINDOW_METAL) -- that conversion is one-way. Using
	// SDL_WINDOW_OPENGL here, not SDL_WINDOW_METAL directly, exercises
	// the actual conversion path UWgpuRenderDevice::Init takes instead
	// of a window that was already Metal from the start.
	//
	// Not SDL_WINDOW_HIDDEN: SDLViewport.cpp:268 only sets that flag
	// for a short-lived "Temporary" resolution-probe window, never the
	// real game window. Using a real, visible window here instead
	// (matching Build/run_wgpu_smoke.py's own -window run) -- a window
	// genuinely appearing briefly during this ctest is expected, same
	// as that smoke test.
	//
	// SDL_WINDOW_ALWAYS_ON_TOP: also a precaution, also not a
	// confirmed fix, same caveat as the SDL_HINT_MAC_BACKGROUND_APP
	// comment above -- the equivalent NSWindow.level = .floating,
	// tested directly in the same isolated AppKit diagnostic, did not
	// set occlusionState's Visible bit either.
	SDL_Window* Window = SDL_CreateWindow( "wgpu surface test", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, 64, 64, SDL_WINDOW_OPENGL | SDL_WINDOW_ALWAYS_ON_TOP );
	Require( Window != NULL, "SDL_CreateWindow with SDL_WINDOW_OPENGL failed" );

	// SDL_RaiseWindow: Cocoa_RaiseWindow (SDL_cocoawindow.m) calls
	// [NSApp activateIgnoringOtherApps:YES] plus makeKeyAndOrderFront:
	// itself, on top of the SDL_HINT_MAC_BACKGROUND_APP precaution
	// above -- same caveat, not confirmed to change the outcome here.
	// Then wait for SDL_WINDOWEVENT_EXPOSED (or FOCUS_GAINED, since
	// activation may arrive as a focus change rather than a fresh
	// expose) before asking for a drawable. Bounded to 2 seconds so a
	// genuine failure to ever activate still fails fast rather than
	// hanging the ctest.
	SDL_RaiseWindow( Window );
	{
		Uint32 StartTicks = SDL_GetTicks();
		bool Ready = false;
		while( !Ready && SDL_GetTicks() - StartTicks < 2000 )
		{
			SDL_Event Event;
			while( SDL_PollEvent( &Event ) )
			{
				if( Event.type == SDL_WINDOWEVENT
					&& ( Event.window.event == SDL_WINDOWEVENT_EXPOSED || Event.window.event == SDL_WINDOWEVENT_FOCUS_GAINED ) )
					Ready = true;
			}
			if( !Ready )
				SDL_Delay( 10 );
		}
	}

	SDL_MetalView MetalView = SDL_Metal_CreateView( Window );
	Require( MetalView != NULL, "SDL_Metal_CreateView failed" );

	void* NsView = MetalView; // "can be cast directly to a NSView or UIView" -- SDL_metal.h's own doc comment
	void* Layer = SDL_Metal_GetLayer( MetalView );
	Require( Layer != NULL, "SDL_Metal_GetLayer returned NULL -- no CAMetalLayer attached" );

	// The real create/destroy pair UWgpuRenderDevice::Init/Exit use.
	// create_device_sync (crates/hp2-wgpu) itself now validates
	// surface.get_capabilities(&adapter).formats is non-empty before
	// returning a device at all (an earlier version of this check
	// lived only in this test and was vacuous -- caught on review,
	// see hp2-wgpu's own create_device_sync doc), so a non-null
	// handle here already proves the surface is genuinely usable with
	// the adapter, not just that a Result was constructed.
	FfiWgpuCreateResult Result = hp2_rs_wgpu_create( NsView );
	Require( Result.handle != NULL, "hp2_rs_wgpu_create failed to create a real wgpu device from a real SDL_Metal_CreateView NSView" );
	Require( Result.adapter_name_len > 0, "hp2_rs_wgpu_create returned an empty adapter name" );

	// Stronger proof than "the surface is compatible with the
	// adapter" (already proven inside create_device_sync itself, see
	// hp2-wgpu's own doc): actually configure the surface, acquire a
	// frame, and present it -- the real per-frame sequence any
	// renderer's draw loop will need, once draw-call forwarding
	// exists (hp2-wgpu's own module doc on what's still open). 0 is
	// the only success code (hp2_wgpu::PresentOnceOutcome::Presented);
	// every other value is printed so a failure says exactly which
	// step failed instead of forcing a guess.
	//
	// Polled, not a single call: wgpu-hal's own Metal backend
	// (wgpu-hal-30.0.1/src/metal/surface.rs, a workaround for
	// gfx-rs/wgpu#8309) reports Occluded (outcome code 4) whenever
	// NSWindow.occlusionState lacks the Visible bit. Whether that bit
	// can still set a moment later than this point, on a machine where
	// it sets at all, is not confirmed either way -- polling for ~2
	// seconds before giving up is a defensive allowance for that
	// possibility, not a claim that it is what actually happens. Only
	// a *persistent* Occluded after that polling window is treated as
	// environment-dependent and skipped (ctest SKIP_RETURN_CODE 77,
	// configured in HP2Targets.cmake) rather than failed -- any other
	// non-Presented code fails immediately, on the first observation,
	// with no retry.
	uint32_t PresentOutcome = 4; // Occluded, until proven otherwise below
	{
		Uint32 StartTicks = SDL_GetTicks();
		do
		{
			SDL_PumpEvents();
			PresentOutcome = hp2_rs_wgpu_present_once_for_testing( Result.handle, 64, 64 );
			if( PresentOutcome == 0 || PresentOutcome != 4 )
				break;
			SDL_Delay( 100 );
		} while( SDL_GetTicks() - StartTicks < 2000 );
	}
	if( PresentOutcome == 4 )
	{
		std::fprintf( stderr, "WgpuSurfaceTests: SKIP: hp2_rs_wgpu_present_once_for_testing reported Occluded even after SDL_WINDOW_ALWAYS_ON_TOP and ~2s of polling -- this execution environment never sets NSWindow.occlusionState's Visible bit (confirmed via an isolated AppKit diagnostic outside this codebase), not a code regression\n" );
		SDL_Metal_DestroyView( MetalView );
		SDL_DestroyWindow( Window );
		SDL_Quit();
		return 77;
	}
	if( PresentOutcome != 0 )
	{
		std::fprintf( stderr, "WgpuSurfaceTests: FAIL: hp2_rs_wgpu_present_once_for_testing returned outcome code %u (0 = Presented; see hp2_wgpu::PresentOnceOutcome's own doc for what the rest mean)\n", PresentOutcome );
		std::exit( 1 );
	}

	hp2_rs_wgpu_destroy( Result.handle );

	SDL_Metal_DestroyView( MetalView );
	SDL_DestroyWindow( Window );
	SDL_Quit();
	std::puts( "WgpuSurfaceTests: PASS" );
	return 0;
}

/*=============================================================================
	WgpuDrv.cpp: wgpu-backed render device implementation.
=============================================================================*/

#include "WgpuDrv.h"
#include "hp2_rs.h"

#if MACOSX
#include <SDL_metal.h>
#endif

IMPLEMENT_PACKAGE(WgpuDrv);
IMPLEMENT_CLASS(UWgpuRenderDevice);

namespace
{
// Matches SDLClient.cpp's ClipboardSinkToFString: build a NUL-terminated
// ANSI buffer from the raw UTF-8 bytes+length Rust returns, then convert
// with the real appFromUtf8InPlace -- not a fabricated length-aware
// TCHAR-conversion macro (no such macro exists in this codebase; one
// was assumed here, then caught and replaced with the real, already-
// established pattern before this was ever compiled).
FString Utf8SpanToFString( const BYTE* Text, SIZE_T Len )
{
	TArray<ANSICHAR> Utf8;
	Utf8.AddZeroed( (INT)Len + 1 );
	if( Len )
		appMemcpy( &Utf8(0), Text, (INT)Len );
	TArray<TCHAR> Buffer;
	Buffer.AddZeroed( (INT)Len + 1 );
	appFromUtf8InPlace( &Buffer(0), &Utf8(0), Buffer.Num() );
	return FString( &Buffer(0) );
}
} // namespace

void UWgpuRenderDevice::StaticConstructor()
{
	guard(UWgpuRenderDevice::StaticConstructor);
	unguard;
}

UBOOL UWgpuRenderDevice::Init( UViewport* InViewport, INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen )
{
	guard(UWgpuRenderDevice::Init);
	Viewport = InViewport;

	// UE1 calls Init again on every fullscreen toggle without an Exit in
	// between (see XOpenGL.cpp:370's own comment on this). Destroy any
	// existing device/view first so repeated toggles don't leak one
	// wgpu device (or SDL_MetalView) per call. RustDevice before
	// MetalView: RustDevice's wgpu::Surface borrows the view's
	// CAMetalLayer, so the view must outlive it (WgpuDrv.h's own
	// field comment).
	if( RustDevice )
	{
		hp2_rs_wgpu_destroy( RustDevice );
		RustDevice = NULL;
	}
#if MACOSX
	if( MetalView )
	{
		SDL_Metal_DestroyView( (SDL_MetalView)MetalView );
		MetalView = NULL;
	}

	// SDL_WINDOW_OPENGL -> SDL_WINDOW_METAL is a one-way conversion
	// (confirmed against the pinned SDL2 source, SDL_video.c:4795-
	// 4809): SDLViewport.cpp:268 always creates the window with
	// SDL_WINDOW_OPENGL regardless of which render device is
	// selected, and SDL_Metal_CreateView flips it to Metal on the
	// fly, clearing SDL_WINDOW_OPENGL and unloading the GL library.
	// A later fallback to XOpenGL on this same window would have no
	// GL library loaded -- not a concern today (HP2_ENABLE_WGPU_DRIVER
	// is an opt-in alternate GameRenderDevice, not a fallback chain).
	void* NsView = NULL;
	SDL_Window* Window = (SDL_Window*)Viewport->GetWindow();
	if( Window )
	{
		SDL_MetalView View = SDL_Metal_CreateView( Window );
		if( View )
		{
			MetalView = View;
			NsView = View; // "can be cast directly to a NSView" -- SDL_metal.h's own doc comment
		}
		else
		{
			debugf( NAME_Init, TEXT("WgpuDrv: SDL_Metal_CreateView failed, continuing headless") );
		}
	}
	else
	{
		// Viewport->GetWindow() returning NULL silently skipped this
		// whole block before this log existed, so a later success
		// line ("adapter ...") looked identical whether or not a real
		// surface was ever attempted -- AGENTS.md's Forbidden Actions
		// rules out swallowing a real condition like this to make a
		// run look clean (caught on review).
		debugf( NAME_Init, TEXT("WgpuDrv: Viewport->GetWindow() returned NULL, continuing headless") );
	}
#else
	void* NsView = NULL;
#endif

	FfiWgpuCreateResult Result = hp2_rs_wgpu_create( NsView );
	if( !Result.handle )
	{
#if MACOSX
		if( MetalView )
		{
			SDL_Metal_DestroyView( (SDL_MetalView)MetalView );
			MetalView = NULL;
		}
#endif
		appErrorf( TEXT("WgpuDrv: no compatible GPU adapter (tried Auto)") );
	}

	RustDevice = Result.handle;
	debugf( NAME_Init, TEXT("WgpuDrv: adapter \"%ls\" backend %ls"),
		*Utf8SpanToFString( Result.adapter_name, Result.adapter_name_len ),
		*Utf8SpanToFString( Result.backend_name, Result.backend_name_len ) );
	// Positive evidence a real window surface was created, not just
	// the absence of a "continuing headless" line -- the latter alone
	// is not proof: a NULL Viewport->GetWindow() (now logged above
	// too) would also leave this function on the headless path with
	// no "continuing headless" line ever printed, making that
	// absence alone misleading evidence (caught on review).
	if( NsView )
		debugf( NAME_Init, TEXT("WgpuDrv: surface created on SDL_MetalView") );
	return 1;
	unguard;
}

UBOOL UWgpuRenderDevice::SetRes( INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen )
{
	guard(UWgpuRenderDevice::SetRes);
	return 1;
	unguard;
}

void UWgpuRenderDevice::Exit()
{
	guard(UWgpuRenderDevice::Exit);
	if( RustDevice )
	{
		// Queried before destroy: the counts live inside RustDevice's
		// own RenderLoop, gone once hp2_rs_wgpu_destroy runs. Correct
		// for a real interactive shutdown (window close/quit), but
		// confirmed NOT reached at all in a -testticks run (verified
		// directly: an unconditional debugf placed at the very top of
		// this function never printed in a real -testticks=60 run --
		// appRequestExit's fast-exit path does not reach
		// USDLViewport::Destroy/RenDev->Exit the way a normal
		// interactive quit does), so Lock below also logs the running
		// tally every call -- Build/run_wgpu_smoke.py reads that one,
		// not this one, since -testticks is exactly the scenario this
		// summary line cannot help with.
		const FrameCounts Counts = hp2_rs_wgpu_frame_counts( RustDevice );
		debugf( NAME_Init, TEXT("WgpuDrv: frames presented=%u occluded=%u outdated=%u other_failure=%u"),
			Counts.presented, Counts.occluded, Counts.outdated, Counts.other_failure );
		hp2_rs_wgpu_destroy( RustDevice );
		RustDevice = NULL;
	}
#if MACOSX
	if( MetalView )
	{
		SDL_Metal_DestroyView( (SDL_MetalView)MetalView );
		MetalView = NULL;
	}
#endif
	unguard;
}

void UWgpuRenderDevice::Flush( UBOOL AllowPrecache )
{
	guard(UWgpuRenderDevice::Flush);
	unguard;
}

void UWgpuRenderDevice::Lock( FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData, INT* HitSize )
{
	guard(UWgpuRenderDevice::Lock);
	// ScreenClear, not FlashScale/FlashFog: XOpenGL.cpp's own Lock
	// uses exactly ScreenClear.X/Y/Z/W as the real glClearColor value
	// (confirmed by reading it, not assumed) -- FlashScale/FlashFog
	// are a separate post-process tint that XOpenGL stores for later,
	// not the buffer clear. No draw-call forwarding exists yet (this
	// class's own header doc), so FlashScale/FlashFog/HitData/HitSize/
	// RenderLockFlags go unused here for now, same as every other
	// still-stub virtual in this class.
	//
	// Real physical pixel dimensions, not Viewport->SizeX/SizeY
	// (logical/point units): on a Retina display those differ by the
	// backing scale factor, and configuring the swapchain at the
	// logical size would under-run the real drawable by that factor.
	// XOpenGL's own present path makes exactly this distinction --
	// SDL_GL_GetDrawableSize for the real blit target, Viewport->
	// SizeX/SizeY only for its own internal FBO sizing -- confirmed by
	// reading XOpenGL.cpp before using this pattern, not guessed (an
	// earlier version of this function used Viewport->SizeX/SizeY
	// directly, matching XOpenGL's FBO-sizing use but not its present-
	// time one, caught on review). SetRes does not need to push a new
	// size into Rust separately -- the next Lock call simply queries
	// the window's current drawable size again, and hp2_rs_wgpu_lock
	// only reconfigures the surface when the requested size actually
	// changed.
#if MACOSX
	INT PixelWidth = 0, PixelHeight = 0;
	if( SDL_Window* Window = (SDL_Window*)Viewport->GetWindow() )
		SDL_Metal_GetDrawableSize( Window, &PixelWidth, &PixelHeight );
	if( PixelWidth <= 0 || PixelHeight <= 0 )
	{
		// No window, or SDL reported nothing useful -- not expected
		// once Lock is reachable at all (Init already required a
		// live window to get this far with a real surface), but fall
		// back to the logical size rather than passing 0 through
		// (Max(1, ...) below still guards the final values either
		// way).
		PixelWidth = Viewport->SizeX;
		PixelHeight = Viewport->SizeY;
	}
#else
	const INT PixelWidth = Viewport->SizeX;
	const INT PixelHeight = Viewport->SizeY;
#endif
	const DWORD Outcome = hp2_rs_wgpu_lock( RustDevice,
		(DWORD)Max( 1, PixelWidth ), (DWORD)Max( 1, PixelHeight ),
		ScreenClear.X, ScreenClear.Y, ScreenClear.Z, ScreenClear.W );
	// Logged every call, success or failure, with the running tally --
	// not only on failure. Exit's own tally log (WgpuDrv.cpp's own
	// Exit comment) is confirmed unreachable in a -testticks run
	// (appRequestExit's fast-exit path skips RenDev->Exit entirely),
	// so Build/run_wgpu_smoke.py needs a per-tick line it can rely on
	// seeing regardless of how the process exits; this is that line
	// (caught on review: an earlier version only logged on failure,
	// which would have given run_wgpu_smoke.py nothing to read at all
	// in exactly the all-Occluded case it most needs to detect).
	// NAME_Dev: the only general developer-log category that exists
	// and fits here (UnNames.h's REGISTER_NAME list has no graphics-
	// specific one; a fabricated NAME_DevGraphics was caught here
	// before this ever compiled).
	const FrameCounts Counts = hp2_rs_wgpu_frame_counts( RustDevice );
	debugf( NAME_Dev, TEXT("WgpuDrv: Lock outcome=%u tally presented=%u occluded=%u outdated=%u other_failure=%u"),
		Outcome, Counts.presented, Counts.occluded, Counts.outdated, Counts.other_failure );
	unguard;
}

void UWgpuRenderDevice::Unlock( UBOOL Blit )
{
	guard(UWgpuRenderDevice::Unlock);
	// Safe to call even if Lock above did not actually begin a frame
	// (Outcome != 0): hp2_rs_wgpu_unlock/unlock_frame's own contract
	// is to return false, not panic or corrupt state, when there is no
	// frame in progress.
	hp2_rs_wgpu_unlock( RustDevice, Blit != 0 );
	unguard;
}

void UWgpuRenderDevice::DrawComplexSurface( FSceneNode* Frame, FSurfaceInfo& Surface, FSurfaceFacet& Facet, DWORD PolyFlags, BYTE cAlpha )
{
	guard(UWgpuRenderDevice::DrawComplexSurface);
	unguard;
}

INT UWgpuRenderDevice::MaxVertices()
{
	return 256;
}

void UWgpuRenderDevice::DrawTriangles( FSceneNode* Frame, FTextureInfo& Info, FTransTexture** Pts, INT NumPts, _WORD* Indices, INT NumIndices, DWORD PolyFlags, FSpanBuffer* Span )
{
	guard(UWgpuRenderDevice::DrawTriangles);
	unguard;
}

void UWgpuRenderDevice::DrawTile( FSceneNode* Frame, FTextureInfo& Info, FLOAT X, FLOAT Y, FLOAT XL, FLOAT YL, FLOAT U, FLOAT V, FLOAT UL, FLOAT VL, FSpanBuffer* Span, FLOAT Z, FPlane Color, FPlane Fog, DWORD PolyFlags )
{
	guard(UWgpuRenderDevice::DrawTile);
	unguard;
}

void UWgpuRenderDevice::Draw2DLine( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FVector P1, FVector P2 )
{
	guard(UWgpuRenderDevice::Draw2DLine);
	unguard;
}

void UWgpuRenderDevice::Draw2DPoint( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FLOAT X1, FLOAT Y1, FLOAT X2, FLOAT Y2, FLOAT Z )
{
	guard(UWgpuRenderDevice::Draw2DPoint);
	unguard;
}

void UWgpuRenderDevice::ClearZ( FSceneNode* Frame )
{
	guard(UWgpuRenderDevice::ClearZ);
	unguard;
}

void UWgpuRenderDevice::PushHit( const BYTE* Data, INT Count )
{
	guard(UWgpuRenderDevice::PushHit);
	unguard;
}

void UWgpuRenderDevice::PopHit( INT Count, UBOOL bForce )
{
	guard(UWgpuRenderDevice::PopHit);
	unguard;
}

void UWgpuRenderDevice::GetStats( TCHAR* Result )
{
	guard(UWgpuRenderDevice::GetStats);
	*Result = 0;
	unguard;
}

void UWgpuRenderDevice::ReadPixels( FColor* Pixels )
{
	guard(UWgpuRenderDevice::ReadPixels);
	unguard;
}

void autoInitializeRegistrantsWgpuDrv(void)
{
	UWgpuRenderDevice::StaticClass();
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

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
	unguard;
}

void UWgpuRenderDevice::Unlock( UBOOL Blit )
{
	guard(UWgpuRenderDevice::Unlock);
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

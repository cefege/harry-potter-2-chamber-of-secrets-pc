/*=============================================================================
	WgpuDrv.cpp: wgpu-backed render device implementation.
=============================================================================*/

#include "WgpuDrv.h"
#include "hp2_rs.h"

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
	// existing device first so repeated toggles don't leak one wgpu
	// device per call.
	if( RustDevice )
	{
		hp2_rs_wgpu_destroy( RustDevice );
		RustDevice = NULL;
	}

	FfiWgpuCreateResult Result = hp2_rs_wgpu_create();
	if( !Result.handle )
		appErrorf( TEXT("WgpuDrv: no compatible GPU adapter (tried Auto)") );

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

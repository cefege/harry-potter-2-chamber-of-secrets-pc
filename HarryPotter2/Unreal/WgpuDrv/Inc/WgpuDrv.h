/*=============================================================================
	WgpuDrv.h: wgpu-backed render device, forwarding to crates/hp2-wgpu.
	Phase 3.2 of local://rust-inplace-port-plan.md.

	Scope today: Init creates a real wgpu adapter+device, and on macOS
	(MACOSX) also a real window surface via SDL_Metal_CreateView on the
	real game window (Viewport->GetWindow()), logging the plan's
	"WgpuDrv: adapter "<name>" backend <X>" line. Non-macOS builds stay
	headless (no Metal API to create a view from) until a Vulkan/DX12
	surface path exists. There is still no draw-call forwarding and no
	shader pipeline -- every other virtual is a safe no-op stub. This
	class exists to prove the registration/build/ini-selection/surface
	chain works end to end, not to render a frame yet.
=============================================================================*/

#ifndef _INCL_WGPUDRV_H_
#define _INCL_WGPUDRV_H_

#include "Engine.h"
#include "UnRender.h"

extern "C" { void autoInitializeRegistrantsWgpuDrv(void); }
#define AUTO_INITIALIZE_REGISTRANTS_WGPUDRV autoInitializeRegistrantsWgpuDrv();

class UWgpuRenderDevice : public URenderDevice
{
	DECLARE_CLASS(UWgpuRenderDevice, URenderDevice, CLASS_Config, WgpuDrv)

	// Opaque handle to the boxed Rust CreatedDevice (crates/hp2-wgpu);
	// this class never looks inside it, only passes it back to
	// hp2_rs_wgpu_destroy in Exit. NULL when no device is live.
	void* RustDevice;

	// Opaque SDL_MetalView (macOS only; always NULL elsewhere). Must
	// outlive RustDevice's wgpu::Surface -- destroyed only after
	// RustDevice, never before (hp2-wgpu::CreatedDevice's own doc on
	// why: the Surface borrows the view's CAMetalLayer).
	void* MetalView;

	void StaticConstructor();

	// URenderDevice pure virtuals -- every one of URenderDevice's
	// non-defaulted methods (Engine/Inc/UnRenDev.h:172-197) must be
	// overridden for this class to be concrete at all.
	UBOOL Init( UViewport* InViewport, INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen );
	UBOOL SetRes( INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen );
	void Exit();
	void Flush( UBOOL AllowPrecache );
	void Lock( FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData, INT* HitSize );
	void Unlock( UBOOL Blit );
	void DrawComplexSurface( FSceneNode* Frame, FSurfaceInfo& Surface, FSurfaceFacet& Facet, DWORD PolyFlags, BYTE cAlpha );
	INT MaxVertices();
	void DrawTriangles( FSceneNode* Frame, FTextureInfo& Info, FTransTexture** Pts, INT NumPts, _WORD* Indices, INT NumIndices, DWORD PolyFlags, FSpanBuffer* Span );
	void DrawTile( FSceneNode* Frame, FTextureInfo& Info, FLOAT X, FLOAT Y, FLOAT XL, FLOAT YL, FLOAT U, FLOAT V, FLOAT UL, FLOAT VL, class FSpanBuffer* Span, FLOAT Z, FPlane Color, FPlane Fog, DWORD PolyFlags );
	void Draw2DLine( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FVector P1, FVector P2 );
	void Draw2DPoint( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FLOAT X1, FLOAT Y1, FLOAT X2, FLOAT Y2, FLOAT Z );
	void ClearZ( FSceneNode* Frame );
	void PushHit( const BYTE* Data, INT Count );
	void PopHit( INT Count, UBOOL bForce );
	void GetStats( TCHAR* Result );
	void ReadPixels( FColor* Pixels );
};

#endif
/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

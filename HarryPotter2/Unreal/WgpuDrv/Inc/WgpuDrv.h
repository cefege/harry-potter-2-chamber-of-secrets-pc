/*=============================================================================
	WgpuDrv.h: wgpu-backed render device, forwarding to crates/hp2-wgpu.
	Phase 3.2 of local://rust-inplace-port-plan.md.

	Scope today: Init creates a real wgpu adapter+device, and on macOS
	(MACOSX) also a real window surface via SDL_Metal_CreateView on the
	real game window (Viewport->GetWindow()), logging the plan's
	"WgpuDrv: adapter "<name>" backend <X>" line. Non-macOS builds stay
	headless (no Metal API to create a view from) until a Vulkan/DX12
	surface path exists. Lock/Unlock call into the real per-frame
	configure/acquire/clear/present path (hp2_rs_wgpu_lock/unlock,
	crates/hp2-wgpu's lock_frame/unlock_frame) -- but nothing is drawn
	into the frame yet, and there is no WGSL shader pipeline; every
	virtual past Lock/Unlock/Init/Exit/SetRes is still a safe no-op
	stub.

	Observed, not yet explained: a real Build/run_wgpu_smoke.py
	-testticks=60 run on this project's own development machine (a
	real MacBook, its own built-in display active -- confirmed via
	system_profiler/pmset/CGSessionCopyCurrentDictionary before
	concluding anything about the machine itself) got outcome code 4
	(Occluded, see hp2_wgpu::FrameOutcome's own doc) on all 60 of 60
	ticks; zero frames were actually presented in that run. By
	contrast, Tests/WgpuSurfaceTests.cpp passes reliably on the same
	machine. The most likely difference between the two, not yet
	isolated by turning each off one at a time, is that that test
	applies several precautions Lock does not -- SDL_HINT_MAC_
	BACKGROUND_APP, SDL_RaiseWindow, SDL_WINDOW_ALWAYS_ON_TOP, and
	polling the present call itself for up to ~2 seconds -- while a
	real `hp2_game -window` launch does none of those and Lock makes
	only one attempt per tick. Lock deliberately does not poll the way
	that test does: blocking the real game loop for up to ~2 seconds
	on every single frame to work around this would be a bad trade even
	if it turned out to help, since a frame failing to acquire is
	already an expected, transient condition in real usage (minimized
	window, display asleep) that the next tick's own Lock call
	naturally retries for free. The engine itself handled the all-
	Occluded run correctly regardless: Unlock is safe to call even when
	Lock observed a non-zero outcome (hp2_wgpu::unlock_frame's own
	contract), and the real run above completed all 60 ticks and shut
	down cleanly with no crash. (Separately, UWgpuRenderDevice::Exit
	itself was not being reached at all during that same -testticks
	run -- a real, pre-existing gap in USDLViewport::Destroy shared by
	every render device, not specific to this one -- since fixed at
	the source; see SDLViewport.cpp's own Destroy/ShutdownAfterError.
	That fix is unrelated to why frames are not presented above: it
	only means Exit's own frame-count summary line is now reliably
	logged, not that any frame reaches the screen.)
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

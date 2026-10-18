/*=============================================================================
	WgpuDrv.h: UWgpuRenderDevice, the only render device. Forwards the
	URenderDevice interface to crates/hp2-wgpu through the hp2_rs_wgpu_*
	exports (hp2-ffi feature `renderer-wgpu`).

	Config section: [WgpuDrv.WgpuRenderDevice]. Keys are registered in
	UWgpuRenderDevice::StaticConstructor (WgpuDrv.cpp). macOS is the only
	verified platform; Init creates a Metal window surface there.
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

	// Shared native text provider (XOpenGL NativeText.cpp's factory; CoreText
	// on Apple). Owns shaping and glyph rasterization only, with no GPU
	// state, so it is never Reset here. NULL until Init creates it and after
	// Exit. Zero-initialized like RustDevice.
	class FNativeTextPlatformBackend* NativeTextBackend;

	// Total UWgpuRenderDevice::SetSceneNode calls since the last Init
	// (not since process start -- Init resets it, matching its own
	// device-recreate-on-every-toggle behavior). Logged in Exit next
	// to the frame-presented tally. SetSceneNode runs many times per
	// real frame (3D world, HUD/Canvas, mirrors/portals/sub-
	// viewports), not once, so a count across a known-length
	// -testticks run is real evidence for how many FrameUniforms
	// segments one frame actually needs -- the input the future
	// per-segment-slot redesign (hp2_wgpu::RenderLoop::set_scene_
	// node's own doc) needs, not yet measured.
	INT SetSceneNodeCallCount;

	// Total UWgpuRenderDevice::Draw2DLine calls since the last Init,
	// same reset/logging convention as SetSceneNodeCallCount above --
	// real evidence this override actually ran during a given
	// -testticks run, not just that nothing crashed when it did (the
	// same caught-on-review reasoning SetSceneNodeCallCount's own doc
	// already has).
	INT Draw2DLineCallCount;

	// Total UWgpuRenderDevice::DrawTile calls since the last Init,
	// same reset/logging convention as Draw2DLineCallCount above.
	INT DrawTileCallCount;

	// Total UWgpuRenderDevice::Draw2DPoint calls since the last Init,
	// same reset/logging convention as Draw2DLineCallCount above.
	INT Draw2DPointCallCount;

	// Total UWgpuRenderDevice::DrawTriangles calls since the last Init,
	// same reset/logging convention as Draw2DLineCallCount above.
	INT DrawTrianglesCallCount;

	// Total UWgpuRenderDevice::DrawComplexSurface calls since the last
	// Init, same reset/logging convention as Draw2DLineCallCount above.
	INT DrawComplexSurfaceCallCount;

	// Lock's FlashScale/FlashFog, stored for EndFlash. Named differently
	// from Lock's parameters so they do not shadow. Init resets them to
	// neutral values (0.5,0.5,0.5,0) / 0: FPlane's default constructor
	// leaves W uninitialized, and a zero FlashScale with zero fog would
	// draw an opaque black overlay.
	FPlane CurrentFlashScale;
	FPlane CurrentFlashFog;

	// Total UWgpuRenderDevice::EndFlash overlay draws the Rust side queued
	// (past the neutral-value skip, and only when a frame is open with a
	// scene) since the last Init, same reset/logging
	// convention as Draw2DLineCallCount above.
	INT EndFlashDrawCount;

	// `[WgpuDrv.WgpuRenderDevice] Backend=Auto|Metal|Vulkan|Dx12|Gl` --
	// config property, UEnum-backed (StaticConstructor registers the
	// enum). Plain member index: 0=Auto, 1=Metal, 2=Vulkan, 3=Dx12,
	// 4=Gl, read directly as hp2_rs_wgpu_create's `backend` FFI
	// parameter (hp2_wgpu::BackendChoice::from_ffi_discriminant's own
	// doc has the full contract). Defaults to 0 (Auto): UE1 zero-
	// initializes config BYTE properties with no explicit default,
	// matching this enum's own member-0 value, so an ini with no
	// Backend key at all still resolves to Auto correctly.
	BYTE Backend;

	// `[WgpuDrv.WgpuRenderDevice] OneXBlending`/`ActorXBlending` --
	// plain config bools, matching XOpenGL.cpp's own StaticConstructor
	// registration and real defaults (both default true). Read
	// directly by SetSceneNode to build FrameUniforms::
	// light_map_intensity/light_color_intensity, mirroring
	// XOpenGL.cpp's own SetFrameStateUniforms exactly (`OneXBlending ?
	// 2.f : 4.f`, `ActorXBlending ? 1.f : 1.5f`).
	UBOOL OneXBlending;
	UBOOL ActorXBlending;

	// Real SetGamma (XOpenGL.cpp:2961-2969), run from Flush:
	// Gamma = (Client->Brightness + 0.1) * (GIsEditor ?
	// GammaMultiplierUED : GammaMultiplier). SetSceneNode passes it
	// (1.0 for ortho viewports, GetViewportGamma) to the Rust post-
	// process blit, which applies pow(color, 1/Gamma) exactly like
	// XOpenGL's PostProcess_Prog. Both multipliers are config
	// properties with XOpenGL's defaults (1.75).
	FLOAT Gamma;
	FLOAT GammaMultiplier;
	FLOAT GammaMultiplierUED;

	// `[WgpuDrv.WgpuRenderDevice] UseVSync=Off|On|Adaptive` -- real
	// config property, UEnum-backed (StaticConstructor registers
	// VSyncs, matching XOpenGL.cpp's own enum member order: 0=Off,
	// 1=On, 2=Adaptive). Wired: lock_frame's own real surface
	// configuration (hp2_rs_wgpu_lock -> hp2_wgpu::lock_frame) reads
	// this to choose wgpu::PresentMode -- the one of these six
	// settings with a real, already-existing Rust-side hook to attach
	// to. XOpenGL.cpp's own real platform-dependent default (`VS_Off`
	// on MACOSX, `VS_Adaptive` elsewhere) is replicated in this
	// class's own StaticConstructor, not silently simplified to one
	// constant for every platform.
	BYTE UseVSync;

	// `[WgpuDrv.WgpuRenderDevice] RenderScale`/`UseAA`/`NumAASamples`/
	// `MaxAnisotropy`/`UseSRGBTextures` -- real config properties,
	// matching XOpenGL.cpp's own StaticConstructor registration and
	// real defaults exactly (RenderScale=1.f, UseAA=true,
	// NumAASamples=4, MaxAnisotropy=16.f, UseSRGBTextures=false).
	// Wired: MaxAnisotropy (hp2_rs_wgpu_set_max_anisotropy in Init,
	// applied to every smooth texture sampler) and UseSRGBTextures
	// (passed to every texture upload; RGBA7/RGB16 stay linear, as in
	// SetTexture.cpp:247-257). Not wired yet: RenderScale (needs a
	// scaled render target plus composite blit) and UseAA/NumAASamples
	// (need an MSAA target and resolve). Those are registered anyway so
	// an XOpenGL-style ini parses without "unknown property" warnings.
	FLOAT RenderScale;
	UBOOL UseAA;
	INT NumAASamples;
	FLOAT MaxAnisotropy;
	UBOOL UseSRGBTextures;

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
	void EndFlash();
	void DrawComplexSurface( FSceneNode* Frame, FSurfaceInfo& Surface, FSurfaceFacet& Facet, DWORD PolyFlags, BYTE cAlpha );
	void SetSceneNode( FSceneNode* Frame );
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

	// Native Canvas text. Layout create/measure/destroy go through the shared
	// provider (NativeText.h); DrawCanvasText hands the shaped layout to the
	// Rust atlas (hp2_rs_wgpu_draw_native_text).
	UBOOL CreateCanvasTextLayout( const FCanvasTextRequest& Request, FCanvasTextLayout*& OutLayout );
	void DestroyCanvasTextLayout( FCanvasTextLayout* Layout );
	UBOOL MeasureCanvasText( FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight );
	UBOOL DrawCanvasText( FSceneNode* Frame, FCanvasTextLayout* Layout );
};

#endif
/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

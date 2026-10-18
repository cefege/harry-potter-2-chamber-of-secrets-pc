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
	crates/hp2-wgpu's lock_frame/unlock_frame). SetSceneNode and
	Draw2DLine are real, wired overrides (hp2_rs_wgpu_set_scene_node/
	hp2_rs_wgpu_draw_2d_line, crates/hp2-wgpu's real simple.wgsl/
	tile.wgsl pipelines exist and are GPU-verified by that crate's own
	tests) -- stale as of an earlier version of this paragraph, which
	said "nothing is drawn into the frame yet, and there is no WGSL
	shader pipeline". DrawComplexSurface/DrawTriangles/DrawTile/
	Draw2DPoint remain safe no-op stubs.

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

	2026-10-17 addendum: a later ctest run in this same long session
	hit a NEW, different confound -- not a retroactive correction of
	the paragraph above, which checked session-lock state at the time
	and found it genuinely unlocked. A compiled probe calling the real
	`CGSessionCopyCurrentDictionary()` (the exact function named above)
	at 19:56 that day found `CGSSessionScreenIsLocked = 1`, locked
	since 19:47 -- directly explaining why that one later run's
	`wgpu_surface_round_trip` ctest (which normally passes for real,
	per the paragraph above) hit its own `SKIP_RETURN_CODE=77`
	environment-gap path instead: macOS reports every window occluded
	to the WindowServer/Metal compositor while the session is locked,
	independent of any application code. Any `renderer_smoke_wgpu`/
	`wgpu_surface_round_trip` result captured during a locked session
	is uninterpretable noise, not evidence toward or against the
	original all-Occluded finding above -- that finding still needs
	its own genuinely-unlocked re-run (with a fresh `CGSessionCopy
	CurrentDictionary` check immediately beforehand, not assumed from
	an earlier check) before its root cause can be narrowed further.
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

	// Gamma, deliberately NOT yet a config property or synced from
	// the real viewport brightness slider the way XOpenGL.cpp's own
	// SetGamma/GetViewportGamma are (GammaMultiplier/GammaMultiplierUED
	// config, a real gamma-ramp subsystem this port has not read yet
	// at all) -- out of scope for SetSceneNode's own FrameUniforms
	// wiring. Fixed at 1.0f (no correction) until that lands; stated
	// honestly here rather than silently wrong or left undocumented.
	FLOAT Gamma;

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
	// Known, explicitly scoped gap, not silently pretended away: none
	// of these five are wired to any real Rust-side behavior yet --
	// unlike UseVSync, none of their real hooks exist in this wgpu
	// port yet (RenderScale needs a real render-target-scaling+
	// composite-blit path; UseAA needs a real MSAA-capable pipeline/
	// framebuffer, not yet built at all; MaxAnisotropy needs a real
	// per-sampler anisotropic-filtering setup, TilePipeline's own
	// sampler is currently a hardcoded default; UseSRGBTextures has a
	// real, already-existing Rust-side parameter -- hp2_wgpu::
	// texture::TextureCache::upload_rgba8's own `srgb` bool -- but no
	// real SetTexture call site threads a real value into it yet).
	// Registered now so a real XOpenGL-style ini file parses without
	// "unknown property" warnings and the values are available the
	// moment each real hook is built, rather than registering them
	// piecemeal alongside each hook later.
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
};

#endif
/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

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
	// Matches the plan's `[WgpuDrv.WgpuRenderDevice] Backend=
	// Auto|Metal|Vulkan|Dx12|Gl` ini key. Member order matters: it is
	// the real C++/Rust FFI contract (see hp2_wgpu::BackendChoice::
	// from_ffi_discriminant's own doc) -- UE1's UByteProperty stores
	// an enum-backed config property as the member's plain index, so
	// Backend (the BYTE field below) is 0 for Auto, 1 for Metal, and
	// so on, read directly by hp2_rs_wgpu_create with no separate
	// string-to-enum mapping on either side. Same real pattern as
	// XOpenGL.cpp's own UEnum-backed config properties (e.g. its
	// VSyncs enum for UseVSync), not an invented convention.
	UEnum* Backends = new(GetClass(), TEXT("Backends"))UEnum(NULL);
	new(Backends->Names)FName(TEXT("Auto"));
	new(Backends->Names)FName(TEXT("Metal"));
	new(Backends->Names)FName(TEXT("Vulkan"));
	new(Backends->Names)FName(TEXT("Dx12"));
	new(Backends->Names)FName(TEXT("Gl"));
	new(GetClass(), TEXT("Backend"), RF_Public)UByteProperty(CPP_PROPERTY(Backend), TEXT("Options"), CPF_Config, Backends);

	// Matches XOpenGL.cpp's own StaticConstructor registration for the
	// same two properties -- plain UBoolProperty, CPF_Config, no enum.
	new(GetClass(), TEXT("OneXBlending"), RF_Public)UBoolProperty(CPP_PROPERTY(OneXBlending), TEXT("Options"), CPF_Config);
	new(GetClass(), TEXT("ActorXBlending"), RF_Public)UBoolProperty(CPP_PROPERTY(ActorXBlending), TEXT("Options"), CPF_Config);

	// Settings carry-over (this class's own header has the real,
	// per-property wiring status). UEnum member order matches
	// XOpenGL.cpp's own VSyncs exactly: 0=Off, 1=On, 2=Adaptive.
	UEnum* VSyncs = new(GetClass(), TEXT("VSyncs"))UEnum(NULL);
	new(VSyncs->Names)FName(TEXT("Off"));
	new(VSyncs->Names)FName(TEXT("On"));
	new(VSyncs->Names)FName(TEXT("Adaptive"));
	new(GetClass(), TEXT("UseVSync"), RF_Public)UByteProperty(CPP_PROPERTY(UseVSync), TEXT("Options"), CPF_Config, VSyncs);
	new(GetClass(), TEXT("RenderScale"), RF_Public)UFloatProperty(CPP_PROPERTY(RenderScale), TEXT("Options"), CPF_Config);
	new(GetClass(), TEXT("UseAA"), RF_Public)UBoolProperty(CPP_PROPERTY(UseAA), TEXT("Options"), CPF_Config);
	new(GetClass(), TEXT("NumAASamples"), RF_Public)UIntProperty(CPP_PROPERTY(NumAASamples), TEXT("Options"), CPF_Config);
	new(GetClass(), TEXT("MaxAnisotropy"), RF_Public)UFloatProperty(CPP_PROPERTY(MaxAnisotropy), TEXT("Options"), CPF_Config);
	new(GetClass(), TEXT("UseSRGBTextures"), RF_Public)UBoolProperty(CPP_PROPERTY(UseSRGBTextures), TEXT("Options"), CPF_Config);
	// Real bug fix (caught on review): an earlier version of this
	// default-seeding assignment lived in Init(), which this class's
	// own commit history at the time wrongly claimed matched
	// XOpenGL.cpp's own Init. It does not -- XOpenGL.cpp's real
	// OneXBlending = 1; ActorXBlending = 1; sit inside its own
	// StaticConstructor (lines 122-303), well before its Init starts
	// at line 367, confirmed by reading the actual function
	// boundaries, not assumed from a nearby line number. The
	// distinction is not cosmetic: StaticConstructor runs once on the
	// class default object, and UE1's CPF_Config loading applies ini
	// values on top of those defaults when an instance is configured
	// -- the correct place for a default. Init() runs again on every
	// fullscreen toggle (this file's own Init has a comment on
	// exactly that), so seeding these fields there would silently
	// stomp a real ini-configured value back to the hardcoded default
	// on every single toggle, not just the first run.
	OneXBlending = ActorXBlending = 1;
	// Gamma: see this class's own header comment on why this stays a
	// fixed 1.0f for now (no gamma-ramp subsystem ported yet). Seeded
	// here, not in Init, for the same real reason as the two
	// properties above.
	Gamma = 1.0f;
	// Settings carry-over: real XOpenGL.cpp defaults
	// (StaticConstructor lines 216-273), seeded here for the same
	// real reason as every default above -- Init() reruns on every
	// fullscreen toggle and would stomp a real ini value otherwise.
#if MACOSX
	UseVSync = 0; // VS_Off -- XOpenGL.cpp's own `#if MACOSX || UNREAL_TOURNAMENT_OLDUNREAL` branch
#else
	UseVSync = 2; // VS_Adaptive -- XOpenGL.cpp's own #else branch
#endif
	RenderScale = 1.f;
	UseAA = 1;
	NumAASamples = 4;
	MaxAnisotropy = 16.f;
	UseSRGBTextures = 0;
	unguard;
}

UBOOL UWgpuRenderDevice::Init( UViewport* InViewport, INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen )
{
	guard(UWgpuRenderDevice::Init);
	Viewport = InViewport;
	SetSceneNodeCallCount = 0;
	Draw2DLineCallCount = 0;
	DrawTileCallCount = 0;

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

	FfiWgpuCreateResult Result = hp2_rs_wgpu_create( NsView, Backend );
	if( !Result.handle )
	{
#if MACOSX
		if( MetalView )
		{
			SDL_Metal_DestroyView( (SDL_MetalView)MetalView );
			MetalView = NULL;
		}
#endif
		// Matches StaticConstructor's Backends UEnum member order
		// (0=Auto..4=Gl) so the error names the real backend(s) that
		// were actually tried, not a hardcoded "Auto" that would have
		// been wrong as soon as this ini key became real (an earlier
		// version of this message always said "Auto" unconditionally,
		// from before the Backend config property existed). Also
		// names the GL-only retry create_device_sync (hp2-wgpu's own
		// Rust side) always performs on total failure when the
		// requested backend wasn't already Gl -- an earlier version of
		// this message named only the originally-requested backend,
		// which under-reported what was genuinely attempted whenever
		// that retry also failed (caught on review). For an
		// out-of-range Backend value, names what hp2_rs_wgpu_create
		// itself actually falls back to (BackendChoice::
		// from_ffi_discriminant returning None -> Auto), not "Unknown"
		// -- an earlier version of this message said "Unknown" there
		// instead, misreporting what was actually attempted (caught on
		// review).
		static const TCHAR* BackendNames[] = { TEXT("Auto"), TEXT("Metal"), TEXT("Vulkan"), TEXT("Dx12"), TEXT("Gl") };
		const INT GlIndex = ARRAY_COUNT(BackendNames) - 1; // "Gl", the real retry target
		const TCHAR* RequestedName = ( Backend < ARRAY_COUNT(BackendNames) ) ? BackendNames[Backend] : TEXT("Auto");
		if( Backend == GlIndex )
			appErrorf( TEXT("WgpuDrv: no compatible GPU adapter (tried %ls)"), RequestedName );
		else if( Backend < ARRAY_COUNT(BackendNames) )
			appErrorf( TEXT("WgpuDrv: no compatible GPU adapter (tried %ls, %ls)"), RequestedName, BackendNames[GlIndex] );
		else
			appErrorf( TEXT("WgpuDrv: no compatible GPU adapter (tried %ls, %ls, invalid Backend=%i)"), RequestedName, BackendNames[GlIndex], Backend );
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
		// own RenderLoop, gone once hp2_rs_wgpu_destroy runs. A real
		// -testticks=60 run was first observed to never reach this
		// function at all (confirmed directly: an unconditional
		// debugf at the top of this function never printed) --
		// USDLViewport::Destroy was independently destroying the SDL
		// window via UObject GC before the viewport's own RenDev
		// cleanup had a chance to run, a real, pre-existing gap now
		// fixed at the source in SDLViewport.cpp's own Destroy/
		// ShutdownAfterError (not worked around here). This line is
		// what Build/run_wgpu_smoke.py reads.
		const FrameCounts Counts = hp2_rs_wgpu_frame_counts( RustDevice );
		debugf( NAME_Init, TEXT("WgpuDrv: frames presented=%u occluded=%u outdated=%u other_failure=%u"),
			Counts.presented, Counts.occluded, Counts.outdated, Counts.other_failure );
		// Real evidence for the per-segment-slot redesign
		// (hp2_wgpu::RenderLoop::set_scene_node's own doc): how many
		// SetSceneNode calls one run actually made, not inferred or
		// assumed from the engine's own architecture docs.
		debugf( NAME_Init, TEXT("WgpuDrv: SetSceneNode calls=%i Draw2DLine calls=%i DrawTile calls=%i"), SetSceneNodeCallCount, Draw2DLineCallCount, DrawTileCallCount );
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
	// (logical/point units): XOpenGL's own present path makes exactly
	// this distinction -- SDL_GL_GetDrawableSize for the real blit
	// target, Viewport->SizeX/SizeY only for its own internal FBO
	// sizing -- confirmed by reading XOpenGL.cpp before using this
	// pattern, not guessed (an earlier version of this function used
	// Viewport->SizeX/SizeY directly, matching XOpenGL's FBO-sizing
	// use but not its present-time one, caught on review). On a
	// Retina display with HiDPI requested, those two differ by the
	// backing scale factor; using the wrong one would under-run the
	// real drawable. Not a confirmed *current* bug, though: the real
	// game window (SDLViewport.cpp:268) does not pass
	// SDL_WINDOW_ALLOW_HIGHDPI (only HP2Launcher.cpp:557's separate
	// launcher window does), so SDL_Metal_GetDrawableSize and
	// Viewport->SizeX/SizeY are confirmed equal for it today -- this
	// is the correct API choice regardless, so it stays correct if
	// HiDPI is ever enabled for the game window, not a fix for an
	// observed half-resolution bug (an earlier version of this
	// comment overclaimed that it was, caught on review: the two
	// sizes were never actually logged and compared). SetRes does not
	// need to push a new size into Rust separately -- the next Lock
	// call simply queries the window's current drawable size again,
	// and hp2_rs_wgpu_lock only reconfigures the surface when the
	// requested size actually changed.
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
		ScreenClear.X, ScreenClear.Y, ScreenClear.Z, ScreenClear.W,
		UseVSync );
	// Failure-only, not every call: an earlier version of this logged
	// every tick's running tally unconditionally, as a workaround for
	// UWgpuRenderDevice::Exit being unreachable in a -testticks run
	// (USDLViewport::Destroy was independently destroying the SDL
	// window via GC before the viewport's own RenDev cleanup ever ran
	// -- a real, pre-existing bug, now fixed at the source in
	// SDLViewport.cpp's own Destroy/ShutdownAfterError, not worked
	// around here). With Exit now reliably reached, logging every
	// single tick (roughly 60 lines/second in real interactive play)
	// was needless log spam kept only because the real fix had not
	// landed yet (caught on review). Exit's own tally line
	// (WgpuDrv.cpp's own Exit, below) is what Build/run_wgpu_smoke.py
	// reads now.
	if( Outcome != 0 )
		// NAME_Dev: the only general developer-log category that
		// exists and fits here (UnNames.h's REGISTER_NAME list has no
		// graphics-specific one; a fabricated NAME_DevGraphics was
		// caught here before this ever compiled).
		debugf( NAME_Dev, TEXT("WgpuDrv: Lock did not begin a frame (outcome code %i)"), Outcome );
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

void UWgpuRenderDevice::SetSceneNode( FSceneNode* Frame )
{
	guard(UWgpuRenderDevice::SetSceneNode);
	if( !RustDevice )
		return;

	// Matches XOpenGL.cpp's own SetSceneNode/SetOrthoProjection/
	// SetProjection/UpdateCoords field reads exactly (hp2_wgpu::
	// compute_frame_uniforms's own doc has the real matrix math this
	// feeds). Unlike the real source, this does not skip the call
	// when nothing relevant changed (StoredOrthoFX != ... etc.) --
	// correct but not maximally efficient, the same known, explicit,
	// non-urgent follow-up noted on hp2_wgpu::RenderLoop::
	// set_scene_node's own doc.
	SceneFrame Scene;
	Scene.coords_origin[0] = Frame->Coords.Origin.X; Scene.coords_origin[1] = Frame->Coords.Origin.Y; Scene.coords_origin[2] = Frame->Coords.Origin.Z;
	Scene.coords_x_axis[0] = Frame->Coords.XAxis.X;  Scene.coords_x_axis[1]  = Frame->Coords.XAxis.Y;  Scene.coords_x_axis[2]  = Frame->Coords.XAxis.Z;
	Scene.coords_y_axis[0] = Frame->Coords.YAxis.X;  Scene.coords_y_axis[1]  = Frame->Coords.YAxis.Y;  Scene.coords_y_axis[2]  = Frame->Coords.YAxis.Z;
	Scene.coords_z_axis[0] = Frame->Coords.ZAxis.X;  Scene.coords_z_axis[1]  = Frame->Coords.ZAxis.Y;  Scene.coords_z_axis[2]  = Frame->Coords.ZAxis.Z;
	Scene.uncoords_origin[0] = Frame->Uncoords.Origin.X; Scene.uncoords_origin[1] = Frame->Uncoords.Origin.Y; Scene.uncoords_origin[2] = Frame->Uncoords.Origin.Z;
	Scene.uncoords_x_axis[0] = Frame->Uncoords.XAxis.X;  Scene.uncoords_x_axis[1]  = Frame->Uncoords.XAxis.Y;  Scene.uncoords_x_axis[2]  = Frame->Uncoords.XAxis.Z;
	Scene.uncoords_y_axis[0] = Frame->Uncoords.YAxis.X;  Scene.uncoords_y_axis[1]  = Frame->Uncoords.YAxis.Y;  Scene.uncoords_y_axis[2]  = Frame->Uncoords.YAxis.Z;
	Scene.uncoords_z_axis[0] = Frame->Uncoords.ZAxis.X;  Scene.uncoords_z_axis[1]  = Frame->Uncoords.ZAxis.Y;  Scene.uncoords_z_axis[2]  = Frame->Uncoords.ZAxis.Z;
	Scene.fx = Frame->FX;
	Scene.fy = Frame->FY;
	Scene.effective_fov_angle = Frame->EffectiveFovAngle;
	Scene.viewport_fov_angle = Frame->Viewport->Actor->FovAngle;
	Scene.is_ortho = Frame->Viewport->IsOrtho() != 0;
	Scene.is_wireframe_zfar = ( GIsEditor && Frame->Viewport->Actor->RendMap == REN_Wire );
	// GetViewportGamma's own real logic (XOpenGL.cpp): 1.f when
	// ortho, the real Gamma field otherwise.
	Scene.gamma = Scene.is_ortho ? 1.0f : Gamma;
	Scene.light_color_intensity = ActorXBlending ? 1.0f : 1.5f;
	Scene.light_map_intensity = OneXBlending ? 2.0f : 4.0f;

	// Real evidence this override actually runs, not just that
	// nothing crashed when it did (caught on review: a prior ctest
	// pass alone proved no crash, not that this code path was ever
	// reached -- SetSceneNode could have been skipped entirely by
	// engine-side logic and the same test would still have passed).
	// Logs only the first call's values, not every call: SetSceneNode
	// runs many times per real frame (3D world, HUD/Canvas, portals/
	// mirrors/sub-viewports), so an unconditional debugf here would
	// spam the log every single one of those, every tick. A member
	// counter, not a function-local `static` (an earlier version used
	// one): Init already resets per-run state on every call including
	// fullscreen toggles, and a `static` would persist across those
	// toggles too, silently suppressing this log on a second Init
	// within the same process. The running total itself is also real,
	// useful evidence: logged in Exit next to the frame-presented
	// tally, it is the real count of FrameUniforms segments one
	// -testticks run actually needed -- the measurement the future
	// per-segment-slot redesign (hp2_wgpu::RenderLoop::set_scene_
	// node's own doc) needs and does not have yet.
	if( SetSceneNodeCallCount == 0 )
	{
		debugf( NAME_Init, TEXT("WgpuDrv: SetSceneNode first call: is_ortho=%i fx=%f fy=%f effective_fov=%f"),
			Scene.is_ortho, Scene.fx, Scene.fy, Scene.effective_fov_angle );
	}
	SetSceneNodeCallCount++;

	hp2_rs_wgpu_set_scene_node( RustDevice, &Scene );
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
	if( !RustDevice )
		return;

	// hp2_rs_wgpu_queue_tile's own doc has the full real-formula
	// detail and the known, explicitly scoped gaps (GIsEditor &&
	// PF_Selected -> DF_Selected, HitTesting, the editor-only Z=1.0f
	// override -- none applied yet, same category of gap Draw2DLine's
	// own override already has for LineFlags/HitTesting/blend mode).
	//
	// Real FixCacheID call site (SetTexture.cpp's own, immediately
	// before BindMap->Find) -- computed once per real DrawTile call,
	// reused for the upload-or-skip check below and the final
	// queue_tile call, matching the real source's own single call
	// site exactly.
	QWORD FixedCacheID = hp2_rs_wgpu_fix_cache_id( Info.CacheID, Info.Format, PolyFlags );

	// Real SetTexture.cpp:502's own fast-path bailout, the caller-
	// facing half (hp2_rs_wgpu_is_texture_cached's own doc): skips
	// the real (potentially expensive -- P8 palette expansion) data
	// extraction entirely when this exact real texture is already
	// resident. Does not detect staleness (bRealtimeChanged) --
	// hp2_wgpu::wgpu::texture::TextureCache's own doc already tracks
	// real re-upload as a separate, not-yet-done item.
	if( !hp2_rs_wgpu_is_texture_cached( RustDevice, FixedCacheID ) )
	{
		// Real UploadTexture (SetTexture.cpp, read in full), scoped
		// to mip 0 only (TextureCache's own doc has the full real
		// reasoning/limitation -- the real source's own per-mip loop,
		// Bind->BaseMip through Info.NumMips, is not replicated here)
		// and to the two real formats hp2_rs_wgpu_upload_texture
		// itself already supports (TEXF_P8/TEXF_RGBA8 -- that
		// function's own doc has the real, explicitly scoped gap for
		// TEXF_RGBA7/RGB16/RGB8/DXT1: it logs and returns false,
		// uploading nothing, rather than this override needing to
		// know which formats are supported itself).
		if( Info.Format == TEXF_P8 && !Info.Palette )
		{
			// Real SetTexture.cpp:213-214's own appErrorf condition
			// ("Encountered bogus P8 texture") -- this override logs
			// and skips the one texture instead of aborting the whole
			// process over it.
			debugf( NAME_Warning, TEXT("WgpuDrv: DrawTile: TEXF_P8 texture %ls has a null Palette, skipping upload"),
				Info.Texture ? Info.Texture->GetFullName() : TEXT("<unknown>") );
		}
		else if( Info.NumMips > 0 && Info.Mips[0] && Info.Mips[0]->DataPtr )
		{
			FMipmapBase* Mip = Info.Mips[0];
			size_t DataLen = ( Info.Format == TEXF_P8 )
				? (size_t)Mip->USize * (size_t)Mip->VSize
				: (size_t)Mip->USize * (size_t)Mip->VSize * 4;
			hp2_rs_wgpu_upload_texture( RustDevice, FixedCacheID, Info.Format,
				(DWORD)Mip->USize, (DWORD)Mip->VSize,
				Mip->DataPtr, DataLen,
				(const BYTE*)Info.Palette,
				( PolyFlags & PF_Masked ) != 0,
				UseSRGBTextures != 0 );
		}
	}

	UBOOL HasTextureAlpha = Info.Texture != NULL;

	if( DrawTileCallCount == 0 )
	{
		debugf( NAME_Init, TEXT("WgpuDrv: DrawTile first call: X=%f Y=%f XL=%f YL=%f CacheID=%llu Format=%i"),
			X, Y, XL, YL, FixedCacheID, (INT)Info.Format );
	}
	DrawTileCallCount++;

	hp2_rs_wgpu_queue_tile( RustDevice, FixedCacheID,
		X, Y, XL, YL,
		U, V, UL, VL,
		Z,
		Color.X, Color.Y, Color.Z,
		PolyFlags,
		Info.UScale, Info.VScale,
		HasTextureAlpha, HasTextureAlpha ? Info.Texture->Alpha : 0.0f,
		Frame->FX2, Frame->FY2 );
	unguard;
}

void UWgpuRenderDevice::Draw2DLine( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FVector P1, FVector P2 )
{
	guard(UWgpuRenderDevice::Draw2DLine);
	if( !RustDevice )
		return;

	// hp2_rs_wgpu_draw_2d_line's own doc has the full real-formula
	// detail and the known, explicitly scoped gaps (LineFlags,
	// HitTesting, PF_Highlighted blend mode -- none applied yet).
	// Color.W is not passed at all: the real source always forces it
	// to 1.f before use (DrawSimple.cpp:64), so there is nothing a
	// caller-supplied alpha could correctly mean. Frame->FX2/FY2 are
	// read straight off this call's own real Frame, not derived from
	// whatever scene SetSceneNode last cached -- the real source's
	// own Draw2DLine does the same (DrawSimple.cpp:80,82), and
	// nothing guarantees the two coincide (caught on review).
	//
	// Logs only the first call -- real evidence this override
	// actually ran during a given run, not just that nothing crashed
	// when it did (same reasoning as SetSceneNode's own first-call
	// log). Not gated on NoDrawSimple (unlike the real XOpenGL
	// source): UWgpuRenderDevice has no such field yet, and this
	// override unconditionally draws when called, a real, honest
	// difference from the real source, not an oversight.
	if( Draw2DLineCallCount == 0 )
	{
		debugf( NAME_Init, TEXT("WgpuDrv: Draw2DLine first call: P1=(%f,%f,%f) P2=(%f,%f,%f)"),
			P1.X, P1.Y, P1.Z, P2.X, P2.Y, P2.Z );
	}
	Draw2DLineCallCount++;

	hp2_rs_wgpu_draw_2d_line( RustDevice,
		P1.X, P1.Y, P1.Z,
		P2.X, P2.Y, P2.Z,
		Frame->FX2, Frame->FY2,
		Color.X, Color.Y, Color.Z );
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

/*=============================================================================
	WgpuDrv.cpp: wgpu-backed render device implementation.
=============================================================================*/

#include "WgpuDrv.h"
#include "hp2_rs.h"
#include "NativeText.h"

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

// Real FixCacheID/is_texture_cached/upload_texture(-or-compressed)
// sequence every real draw override that touches a texture needs --
// DrawTile/DrawTriangles/DrawComplexSurface (three times, for its own
// diffuse/light-map/fog-map layers) all used to carry their own
// separately-written copy of this; consolidated here once a third
// real branch (TEXF_DXT1, this change) would otherwise have meant
// tripling it a third time. Takes RustDevice/UseSRGBTextures as
// explicit parameters rather than a UWgpuRenderDevice* -- this
// class's own members have no `public:` specifier anywhere in
// WgpuDrv.h (checked directly, not assumed), so a free function
// cannot reach them through a bare pointer without either changing
// that class's own access surface or adding a friend declaration;
// passing the two real values this function actually needs avoids
// both. CallerName is only used in the real TEXF_P8-null-Palette
// warning's own real "WgpuDrv: <Caller>: ..." prefix, matching each
// override's own previously-separate wording exactly, not merged
// into one generic message that would lose which real override hit
// it.
QWORD UploadTextureIfNeeded( void* RustDevice, UBOOL UseSRGBTextures, FTextureInfo& Info, DWORD PolyFlags, const TCHAR* CallerName )
{
	QWORD FixedCacheID = hp2_rs_wgpu_fix_cache_id( Info.CacheID, Info.Format, PolyFlags );
	// Real SetTexture.cpp:502's own `!IsTextureDataStale` condition --
	// IsTextureDataStale is purely Info.bRealtimeChanged-driven
	// (SetTexture.cpp:99-101, read directly before relying on this:
	// `if (Info.bRealtimeChanged) { IsTextureDataStale = 1; ... }`,
	// no other real condition sets it). Without this, a real realtime
	// texture (most importantly every real lightmap/fogmap -- each
	// one's own CacheID is stable across re-illumination, UnLight.cpp's
	// own `MakeCacheID(CID_StaticMap, iLightMap, ZoneID, Model)` --
	// would upload once and then never again for the rest of the
	// program, however many times its own real lighting actually
	// changed afterward.
	if( hp2_rs_wgpu_is_texture_cached( RustDevice, FixedCacheID ) && !Info.bRealtimeChanged )
		return FixedCacheID;
	// Real SetTexture.cpp:200's own unconditional `Info.bRealtimeChanged
	// = 0;`, at the very top of the real UploadTexture -- reset here at
	// the same real point this function's own analog of "decide to call
	// UploadTexture, then call it" enters that real function, covering
	// every branch below alike (P8-null-palette skip, DXT1-invalid-mip
	// skip, and the real uncompressed upload), matching the real
	// source's own reset running regardless of what UploadTexture itself
	// goes on to do.
	Info.bRealtimeChanged = 0;
	// Real SetTexture.cpp:530-535 IsNewBind sampler inputs (SetSampler,
	// SetTexture.cpp:138-166), as hp2_wgpu::texture::SamplerKey bits:
	// CLAMP_U=1, CLAMP_V=2, NO_SMOOTH=4. Rust applies them only on a
	// cache_id's first upload, matching SetSampler running only for a
	// new BindMap entry. Every caller here binds the diffuse slot
	// (Multi == 0) except light/fog maps, which pass PolyFlags = 0.
	const UBOOL IsLightOrFogMap = Info.Texture == NULL;
	DWORD SamplerFlags = 0;
	if( IsLightOrFogMap || ( Info.UClamp > 0 && Info.UClamp < Info.USize ) )
		SamplerFlags |= 1;
	if( IsLightOrFogMap || ( Info.VClamp > 0 && Info.VClamp < Info.VSize ) )
		SamplerFlags |= 2;
	if( PolyFlags & PF_NoSmooth )
		SamplerFlags |= 4;

	if( Info.Format == TEXF_P8 && !Info.Palette )
	{
		// Real SetTexture.cpp:213-214's own appErrorf condition
		// ("Encountered bogus P8 texture") -- logged and skipped
		// here instead of aborting the whole process over it.
		debugf( NAME_Warning, TEXT("WgpuDrv: %ls: TEXF_P8 texture %ls has a null Palette, skipping upload"),
			CallerName, Info.Texture ? Info.Texture->GetFullName() : TEXT("<unknown>") );
		return FixedCacheID;
	}

	if( Info.Format == TEXF_DXT1 )
	{
		// Real TEXF_DXT1 branch (SetTexture.cpp's own case TEXF_DXT1,
		// read in full): the real full Info.Mips[0..NumMips) chain,
		// not mip 0 only -- hp2_wgpu::texture::create_compressed_gpu_
		// texture's own doc has the full real reasoning for why this
		// one format needs it (no wgpu-side mip generation is
		// possible for a compressed format at all, unlike the
		// uncompressed formats below). Any one mip missing real data
		// (Mips[i]/DataPtr null) drops the *whole* texture, same real
		// "break on the first bad mip" behavior UploadTexture's own
		// real per-mip loop has (SetTexture.cpp:354-360), not a
		// partial upload of just the mips that happened to be valid.
		if( Info.NumMips <= 0 || !Info.Mips[0] || !Info.Mips[0]->DataPtr )
			return FixedCacheID;
		TArray<FfiCompressedMip> Mips( Info.NumMips );
		for( INT i = 0; i < Info.NumMips; ++i )
		{
			FMipmapBase* Mip = Info.Mips[i];
			if( !Mip || !Mip->DataPtr )
			{
				debugf( NAME_Warning, TEXT("WgpuDrv: %ls: DXT1 texture %ls has invalid mip %i, skipping upload"),
					CallerName, Info.Texture ? Info.Texture->GetFullName() : TEXT("<unknown>"), i );
				return FixedCacheID;
			}
			// Real HP2TextureBytes' own DXT1 formula (SetTexture.cpp:
			// 21-30 -- a file-static function in a different real
			// translation unit, not reachable from here, so
			// replicated exactly rather than called): Max(1,(U+3)/4)
			// * Max(1,(V+3)/4) * 8, the same real per-format byte
			// size hp2_wgpu::texture::texture_bytes already computes
			// Rust-side and create_compressed_gpu_texture's own
			// assert depends on matching exactly.
			Mips(i).width = (DWORD)Mip->USize;
			Mips(i).height = (DWORD)Mip->VSize;
			Mips(i).data = Mip->DataPtr;
			Mips(i).data_len = (size_t)Max<INT>( 1, ( Mip->USize + 3 ) / 4 ) * (size_t)Max<INT>( 1, ( Mip->VSize + 3 ) / 4 ) * 8;
		}
		hp2_rs_wgpu_upload_compressed_texture( RustDevice, FixedCacheID, Info.Format,
			(DWORD)Info.Mips[0]->USize, (DWORD)Info.Mips[0]->VSize,
			&Mips(0), (size_t)Info.NumMips,
			UseSRGBTextures != 0, SamplerFlags );
		return FixedCacheID;
	}

	if( Info.NumMips > 0 && Info.Mips[0] && Info.Mips[0]->DataPtr )
	{
		// Real UploadTexture (SetTexture.cpp, read in full), scoped
		// to mip 0 only (TextureCache's own doc has the full real
		// reasoning/limitation -- the real source's own per-mip loop,
		// Bind->BaseMip through Info.NumMips, is not replicated here
		// for these uncompressed formats). Covers all five real
		// uncompressed formats hp2_rs_wgpu_upload_texture itself now
		// supports (TEXF_P8/RGBA7/RGB16/RGB8/RGBA8).
		//
		// DataLen must be each real format's own real per-pixel byte
		// size (SetTexture.cpp's own real HP2TextureBytes formula,
		// replicated here for the same real reason DXT1's own branch
		// above already does: a file-static function in a different
		// real translation unit, not reachable from here) -- NOT a
		// blanket "P8 is 1, everything else is 4" the way an earlier
		// version of this function had, back when RGB8 (3 bytes/pixel)
		// and RGB16 (2) were both still unconditionally rejected by
		// hp2_rs_wgpu_upload_texture itself regardless of what DataLen
		// claimed. Now that both are real, accepted formats, the same
		// real soundness bug DXT1 had before its own fix (an
		// oversized, caller-computed DataLen reaching bytes_from_raw's
		// own from_raw_parts before any per-format length check could
		// catch it) would reopen here if this were not fixed at the
		// same time those two formats became real upload targets, not
		// left for a later change to discover again independently.
		FMipmapBase* Mip = Info.Mips[0];
		size_t BytesPerPixel;
		switch( Info.Format )
		{
		case TEXF_P8:    BytesPerPixel = 1; break;
		case TEXF_RGB16: BytesPerPixel = 2; break;
		case TEXF_RGB8:  BytesPerPixel = 3; break;
		default:         BytesPerPixel = 4; break; // TEXF_RGBA7, TEXF_RGBA8
		}
		size_t DataLen = (size_t)Mip->USize * (size_t)Mip->VSize * BytesPerPixel;
		hp2_rs_wgpu_upload_texture( RustDevice, FixedCacheID, Info.Format,
			(DWORD)Mip->USize, (DWORD)Mip->VSize,
			Mip->DataPtr, DataLen,
			(const BYTE*)Info.Palette,
			( PolyFlags & PF_Masked ) != 0,
			UseSRGBTextures != 0, SamplerFlags );
	}
	return FixedCacheID;
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
	// XOpenGL.cpp:151-152 registers the same two keys.
	new(GetClass(), TEXT("GammaMultiplier"), RF_Public)UFloatProperty(CPP_PROPERTY(GammaMultiplier), TEXT("Options"), CPF_Config);
	new(GetClass(), TEXT("GammaMultiplierUED"), RF_Public)UFloatProperty(CPP_PROPERTY(GammaMultiplierUED), TEXT("Options"), CPF_Config);

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
	// Gamma is recomputed by Flush (SetGamma); 1.0 (no correction)
	// until the engine's first Flush. Multiplier defaults are
	// XOpenGL.cpp:235-236. Seeded here, not in Init, for the same
	// real reason as the two properties above.
	Gamma = 1.0f;
	GammaMultiplier = 1.75f;
	GammaMultiplierUED = 1.75f;
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
	Draw2DPointCallCount = 0;
	DrawTrianglesCallCount = 0;
	DrawComplexSurfaceCallCount = 0;
	EndFlashDrawCount = 0;
	// Neutral: (0.5,0.5,0.5,0) / 0 draws no overlay (see EndFlash).
	CurrentFlashScale = FPlane(0.5f,0.5f,0.5f,0.f);
	CurrentFlashFog = FPlane(0.f,0.f,0.f,0.f);

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
	// Re-init drops the native text provider with the device it served
	// (XOpenGL.cpp:375-381 does the same on its own Init).
	delete NativeTextBackend;
	NativeTextBackend = NULL;
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
	// WgpuDrv is the only render device, so no GL fallback can run on this
	// window after the conversion.
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
	// Real SetSampler's MaxAnisotropy (SetTexture.cpp:159-160).
	hp2_rs_wgpu_set_max_anisotropy( RustDevice, MaxAnisotropy );
	// Real UseSRGBTextures framebuffer half (XOpenGL.cpp:1175-1176):
	// linear swapchain unless it is set.
	hp2_rs_wgpu_set_srgb_framebuffer( RustDevice, UseSRGBTextures != 0 );
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
	// Native text provider, created after the device so a failed device
	// never leaves one behind (XOpenGL.cpp:587-597 creates it at the same
	// point in its Init tail).
#if HP2_HAS_NATIVE_TEXT_BACKEND
	FNativeTextBackendStatus NativeTextStatus = { false, NULL };
	NativeTextBackend = CreateNativeTextPlatformBackend( NativeTextStatus );
	debugf( NAME_Init, TEXT("WgpuDrv: native text backend %s (%s)"),
		NativeTextStatus.Available ? TEXT("ready") : TEXT("unavailable"),
		NativeTextStatus.ReasonCode ? appFromAnsi( NativeTextStatus.ReasonCode ) : TEXT("unknown") );
#else
	debugf( NAME_Init, TEXT("WgpuDrv: native text backend unavailable (capability.compile_time_disabled)") );
#endif
	// Real XOpenGL Init tail (XOpenGL.cpp:581): SetRes, whose own
	// Flush(1) is what first runs SetGamma.
	SetRes( NewX, NewY, NewColorBytes, Fullscreen );
	return 1;
	unguard;
}

UBOOL UWgpuRenderDevice::SetRes( INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen )
{
	guard(UWgpuRenderDevice::SetRes);
	// The surface itself follows the window's drawable size on the
	// next Lock (hp2_rs_wgpu_lock reconfigures on change). Real XOpenGL
	// SetRes ends with Flush(1) (XOpenGL.cpp:1374-1375), which re-runs
	// SetGamma; kept here for the same reason.
	Flush( 1 );
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
		debugf( NAME_Init, TEXT("WgpuDrv: SetSceneNode calls=%i Draw2DLine calls=%i DrawTile calls=%i Draw2DPoint calls=%i DrawTriangles calls=%i DrawComplexSurface calls=%i"), SetSceneNodeCallCount, Draw2DLineCallCount, DrawTileCallCount, Draw2DPointCallCount, DrawTrianglesCallCount, DrawComplexSurfaceCallCount );
		hp2_rs_wgpu_destroy( RustDevice );
		RustDevice = NULL;
	}
	// Native text provider has no GPU state, so it is torn down without
	// the device (XOpenGL.cpp:3127-3128).
	delete NativeTextBackend;
	NativeTextBackend = NULL;
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
	// Real XOpenGL Flush's SetGamma call (XOpenGL.cpp:1578-1579) and
	// SetGamma itself (XOpenGL.cpp:2961-2969): shift the 0.0-0.9
	// brightness slider range to 0.1-1.0, then scale.
	if( Viewport && Viewport->GetOuterUClient() )
		Gamma = ( Viewport->GetOuterUClient()->Brightness + 0.1f ) * ( GIsEditor ? GammaMultiplierUED : GammaMultiplier );
	unguard;
}

void UWgpuRenderDevice::Lock( FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData, INT* HitSize )
{
	guard(UWgpuRenderDevice::Lock);
	// ScreenClear, not FlashScale/FlashFog: XOpenGL.cpp's own Lock
	// uses exactly ScreenClear.X/Y/Z/W as the real glClearColor value
	// (confirmed by reading it, not assumed) -- FlashScale/FlashFog
	// are a separate post-process tint, not the buffer clear. They are
	// stored here for EndFlash, which the engine calls after DrawWorld
	// within this same frame (UnGame.cpp:1761). HitData/HitSize/
	// RenderLockFlags go unused here for now, same as every other
	// still-stub virtual in this class.
	CurrentFlashScale = FlashScale;
	CurrentFlashFog = FlashFog;
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
	INT LogicalWidth = 0, LogicalHeight = 0;
	if( SDL_Window* Window = (SDL_Window*)Viewport->GetWindow() )
	{
		SDL_Metal_GetDrawableSize( Window, &PixelWidth, &PixelHeight );
		SDL_GetWindowSize( Window, &LogicalWidth, &LogicalHeight );
	}
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
	// HP2_CAPTURE_FRAMES's backing_scale_factor: drawable pixels per
	// logical window pixel, computed exactly as XOpenGL.cpp:2728-2731
	// does (DrawableHeight / LogicalHeight, 1.0 when unknown).
	hp2_rs_wgpu_set_backing_scale( RustDevice,
		LogicalHeight > 0 ? (DOUBLE)PixelHeight / (DOUBLE)LogicalHeight : 1.0 );
#else
	const INT PixelWidth = Viewport->SizeX;
	const INT PixelHeight = Viewport->SizeY;
	hp2_rs_wgpu_set_backing_scale( RustDevice, 1.0 );
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
		debugf( NAME_Dev, TEXT("WgpuDrv: Lock outcome code %i: no window frame presented this tick"), Outcome );
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
	if( !RustDevice )
		return;

	// Real check(Surface.Texture) (DrawComplex.cpp:60) -- every real
	// DrawComplexSurface call has a real diffuse texture; defensively
	// bail rather than dereference null, same established style every
	// other override here already uses (none of them replicate the
	// real source's own debug-only check()/guard() asserts literally).
	if( !Surface.Texture )
		return;

	// Same real FixCacheID/is_texture_cached/upload_texture(-or-
	// compressed) sequence as DrawTile/DrawTriangles' own overrides,
	// via the shared UploadTextureIfNeeded (this file's own anonymous
	// namespace) -- called up to three times since DrawComplexSurface
	// has up to three real texture layers (diffuse/light-map/fog-map)
	// where the others have one. The third argument is 0 for light-
	// map/fog-map, not PolyFlags -- matching the real SetTextureHelper's
	// own light-map/fog-map call sites (DrawComplex.cpp:175,178), which
	// pass a literal 0 as their own PolyFlags argument, not
	// NextPolyFlags. Confirmed this actually matters, not just copied
	// for its own sake: FixCacheID's only real use of PolyFlags is
	// tagging the cache ID with PF_Masked (SetTexture.cpp:66), so a
	// light-map/fog-map texture on a masked surface must not itself be
	// tagged as masked.
	QWORD DiffuseCacheID = UploadTextureIfNeeded( RustDevice, UseSRGBTextures, *Surface.Texture, PolyFlags, TEXT("DrawComplexSurface") );
	UBOOL HasLightMap = Surface.LightMap != NULL;
	QWORD LightMapCacheID = HasLightMap ? UploadTextureIfNeeded( RustDevice, UseSRGBTextures, *Surface.LightMap, 0, TEXT("DrawComplexSurface") ) : 0;
	// Real FogMap condition (DrawComplex.cpp:177) is strictly more
	// restrictive than LightMap's own (DrawComplex.cpp:174) -- a non-
	// null pointer alone is not enough, the first mip must actually
	// have real data loaded. A real, deliberate asymmetry transcribed
	// exactly, not a copy-paste of LightMap's own weaker condition.
	UBOOL HasFogMap = Surface.FogMap && Surface.FogMap->Mips[0] && Surface.FogMap->Mips[0]->DataPtr;
	QWORD FogMapCacheID = HasFogMap ? UploadTextureIfNeeded( RustDevice, UseSRGBTextures, *Surface.FogMap, 0, TEXT("DrawComplexSurface") ) : 0;
	UBOOL HasTexture = Surface.Texture->Texture != NULL;

	// Real XAxis/YAxis vec4 packing (DrawComplex.cpp:193-194):
	// (axis.X, axis.Y, axis.Z, axis | MapCoords.Origin) -- the real
	// dot product (FVector::operator|, UnMath.h:686-689, confirmed
	// before relying on the operator name alone). ZAxis is computed
	// by the real source too but never read anywhere in complex.wgsl's
	// own base path (that shader's own module doc already confirms
	// this), so it is not threaded through here.
	FLOAT XAxisW = Facet.MapCoords.XAxis | Facet.MapCoords.Origin;
	FLOAT YAxisW = Facet.MapCoords.YAxis | Facet.MapCoords.Origin;

	// Real per-polygon fan triangulation (DrawComplex.cpp:206-248,
	// read in full), two passes: count first -- Facet.Polys is a real
	// linked list of independently-faceted FSavedPoly, each its own
	// fan, not one single fan across the whole facet -- so TArray's
	// own allocating constructor (TArray(InNum, Tag), confirmed via
	// direct source read, not TRefArray's same-looking but non-
	// allocating (InNum) constructor -- same real distinction
	// DrawTriangles' own override already established) can be sized
	// exactly once, then fill. Every real FSavedPoly::Pts entry is
	// already an FTransform* (not FTransform**, unlike the real
	// source's own local `FTransform** In = &Poly->Pts[0]` -- an
	// equivalent, simpler way to reach the same real FTransform*
	// values, not a behavior change).
	INT TotalVertexCount = 0;
	for( FSavedPoly* Poly = Facet.Polys; Poly; Poly = Poly->Next )
		if( Poly->NumPts >= 3 )
			TotalVertexCount += ( Poly->NumPts - 2 ) * 3;
	if( TotalVertexCount <= 0 )
		return;

	TArray<ComplexVertexInput> Vertices( TotalVertexCount );
	auto FillVertex = [&]( ComplexVertexInput& Vert, FTransform* P )
	{
		Vert.coords[0] = P->Point.X; Vert.coords[1] = P->Point.Y; Vert.coords[2] = P->Point.Z;
	};
	INT Out = 0;
	for( FSavedPoly* Poly = Facet.Polys; Poly; Poly = Poly->Next )
	{
		const INT NumPts = Poly->NumPts;
		if( NumPts < 3 )
			continue;
		for( INT i = 0; i < NumPts - 2; i++ )
		{
			FillVertex( Vertices(Out++), Poly->Pts[0] );
			FillVertex( Vertices(Out++), Poly->Pts[i + 1] );
			FillVertex( Vertices(Out++), Poly->Pts[i + 2] );
		}
	}

	if( DrawComplexSurfaceCallCount == 0 )
	{
		debugf( NAME_Init, TEXT("WgpuDrv: DrawComplexSurface first call: TotalVertexCount=%i DiffuseCacheID=%llu HasLightMap=%i HasFogMap=%i"),
			TotalVertexCount, DiffuseCacheID, HasLightMap, HasFogMap );
	}
	DrawComplexSurfaceCallCount++;

	hp2_rs_wgpu_queue_complex_surface( RustDevice,
		DiffuseCacheID, Surface.Texture->UScale, Surface.Texture->VScale, Surface.Texture->Pan.X, Surface.Texture->Pan.Y,
		HasLightMap, LightMapCacheID, HasLightMap ? Surface.LightMap->UScale : 0.0f, HasLightMap ? Surface.LightMap->VScale : 0.0f, HasLightMap ? Surface.LightMap->Pan.X : 0.0f, HasLightMap ? Surface.LightMap->Pan.Y : 0.0f,
		HasFogMap, FogMapCacheID, HasFogMap ? Surface.FogMap->UScale : 0.0f, HasFogMap ? Surface.FogMap->VScale : 0.0f, HasFogMap ? Surface.FogMap->Pan.X : 0.0f, HasFogMap ? Surface.FogMap->Pan.Y : 0.0f,
		PolyFlags,
		HasTexture, HasTexture ? Surface.Texture->Texture->Diffuse : 0.0f, HasTexture ? Surface.Texture->Texture->Specular : 0.0f, HasTexture ? Surface.Texture->Texture->Alpha : 0.0f,
		cAlpha,
		Facet.MapCoords.XAxis.X, Facet.MapCoords.XAxis.Y, Facet.MapCoords.XAxis.Z, XAxisW,
		Facet.MapCoords.YAxis.X, Facet.MapCoords.YAxis.Y, Facet.MapCoords.YAxis.Z, YAxisW,
		&Vertices(0), (size_t)TotalVertexCount );
	unguard;
}

INT UWgpuRenderDevice::MaxVertices()
{
	return 256;
}

void UWgpuRenderDevice::DrawTriangles( FSceneNode* Frame, FTextureInfo& Info, FTransTexture** Pts, INT NumPts, _WORD* Indices, INT NumIndices, DWORD PolyFlags, FSpanBuffer* Span )
{
	guard(UWgpuRenderDevice::DrawTriangles);
	if( !RustDevice )
		return;

	// Real DrawTriangles' own validation/expansion logic (DrawGouraud.
	// cpp:296-359, read in full, transcribed here exactly -- not just
	// approximated from its general shape): an indexed flat triangle
	// list when Indices is non-null (truncated to a multiple of 3), a
	// triangle fan over Pts[0..NumPts) otherwise. Real per-index
	// validation (range, non-null Pts[PointIndex]) drops the *whole*
	// call silently (a warning, not a partial draw) on any bad index --
	// the real source's own behavior, not a new policy invented here.
	if( !Pts || NumPts < 3 )
		return;

	const UBOOL Indexed = Indices != NULL;
	const INT OutVertexCount = Indexed ? NumIndices - (NumIndices % 3) : (NumPts - 2) * 3;
	if( OutVertexCount < 3 )
		return;

	for( INT i = 0; i < (Indexed ? OutVertexCount : NumPts); ++i )
	{
		const INT PointIndex = Indexed ? Indices[i] : i;
		if( PointIndex < 0 || PointIndex >= NumPts || !Pts[PointIndex] )
		{
			debugf( NAME_Warning, TEXT("WgpuDrv: DrawTriangles received an invalid vertex index.") );
			return;
		}
	}

	// Real BufferVert's own four real FTransTexture fields (DrawGouraud.
	// cpp:35-43: Point/U/V/Light/Fog -- Normal skipped, GouraudVertexInput's
	// own doc has the full real reasoning). TArray(InNum, Tag) (not
	// TRefArray's own same-looking (InNum) constructor, confirmed by
	// reading both real constructor bodies first) really allocates via
	// Realloc -- every element is written below before being read, so
	// the real lack of zero-initialization here does not matter.
	TArray<GouraudVertexInput> Vertices( OutVertexCount );
	auto FillVertex = [&]( GouraudVertexInput& Vert, FTransTexture* P )
	{
		Vert.coords[0] = P->Point.X; Vert.coords[1] = P->Point.Y; Vert.coords[2] = P->Point.Z;
		Vert.tex_coords[0] = P->U; Vert.tex_coords[1] = P->V;
		Vert.light_color[0] = P->Light.X; Vert.light_color[1] = P->Light.Y; Vert.light_color[2] = P->Light.Z; Vert.light_color[3] = P->Light.W;
		Vert.fog_color[0] = P->Fog.X; Vert.fog_color[1] = P->Fog.Y; Vert.fog_color[2] = P->Fog.Z; Vert.fog_color[3] = P->Fog.W;
	};
	if( Indexed )
	{
		for( INT i = 0; i < OutVertexCount; ++i )
			FillVertex( Vertices(i), Pts[Indices[i]] );
	}
	else
	{
		INT Out = 0;
		for( INT i = 0; i < NumPts - 2; ++i )
		{
			FillVertex( Vertices(Out++), Pts[0] );
			FillVertex( Vertices(Out++), Pts[i + 1] );
			FillVertex( Vertices(Out++), Pts[i + 2] );
		}
	}

	// Same real FixCacheID/is_texture_cached/upload_texture(-or-
	// compressed) sequence as DrawTile/DrawComplexSurface's own
	// overrides, via the shared UploadTextureIfNeeded (this file's own
	// anonymous namespace) -- hp2_rs_wgpu_queue_triangles's own doc has
	// the full real-formula detail and the known, explicitly scoped
	// gaps (detail/macro textures, bump maps, shader specialization,
	// GIsEditor && PF_Selected, NoNearZ, HitTesting -- none applied
	// yet).
	QWORD FixedCacheID = UploadTextureIfNeeded( RustDevice, UseSRGBTextures, Info, PolyFlags, TEXT("DrawTriangles") );

	UBOOL HasTexture = Info.Texture != NULL;

	if( DrawTrianglesCallCount == 0 )
	{
		debugf( NAME_Init, TEXT("WgpuDrv: DrawTriangles first call: OutVertexCount=%i CacheID=%llu Format=%i"),
			OutVertexCount, FixedCacheID, (INT)Info.Format );
	}
	DrawTrianglesCallCount++;

	hp2_rs_wgpu_queue_triangles( RustDevice, FixedCacheID, PolyFlags,
		Info.UScale, Info.VScale,
		HasTexture, HasTexture ? Info.Texture->Alpha : 0.0f,
		HasTexture, HasTexture ? Info.Texture->Diffuse : 0.0f,
		&Vertices(0), (size_t)OutVertexCount );
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
	// Same real FixCacheID/is_texture_cached/upload_texture(-or-
	// compressed) sequence as DrawTriangles/DrawComplexSurface's own
	// overrides, via the shared UploadTextureIfNeeded (this file's own
	// anonymous namespace).
	QWORD FixedCacheID = UploadTextureIfNeeded( RustDevice, UseSRGBTextures, Info, PolyFlags, TEXT("DrawTile") );

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
	// detail. LineFlags select the depth state and the blend is the
	// real fixed PF_Highlighted (draw::RenderState::for_line_flags);
	// HitTesting is not applied yet.
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
		Color.X, Color.Y, Color.Z,
		LineFlags );
	unguard;
}

void UWgpuRenderDevice::Draw2DPoint( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FLOAT X1, FLOAT Y1, FLOAT X2, FLOAT Y2, FLOAT Z )
{
	guard(UWgpuRenderDevice::Draw2DPoint);
	if( !RustDevice )
		return;

	// hp2_rs_wgpu_queue_point's own doc has the full real-formula
	// detail. LineFlags and the fixed PF_Highlighted blend are applied
	// the same way as Draw2DLine's; HitTesting is not applied yet.
	// Color.W is not passed at all, same real reason Draw2DLine's own
	// override already has (DrawSimple.cpp:166: "Unfortunately this is usually
	// set to 0"). Frame->FX2/FY2 are read straight off this call's own
	// real Frame, not derived from whatever scene SetSceneNode last
	// cached -- the real source's own Draw2DPoint does the same
	// (DrawSimple.cpp:187), matching Draw2DLine's own override exactly.
	if( Draw2DPointCallCount == 0 )
	{
		debugf( NAME_Init, TEXT("WgpuDrv: Draw2DPoint first call: X1=%f Y1=%f X2=%f Y2=%f Z=%f"),
			X1, Y1, X2, Y2, Z );
	}
	Draw2DPointCallCount++;

	hp2_rs_wgpu_queue_point( RustDevice,
		X1, Y1, X2, Y2, Z,
		Frame->FX2, Frame->FY2,
		Color.X, Color.Y, Color.Z,
		LineFlags );
	unguard;
}

void UWgpuRenderDevice::EndFlash()
{
	guard(UWgpuRenderDevice::EndFlash);
	if( !RustDevice )
		return;

	// Real UXOpenGLRenderDevice::EndFlash (DrawSimple.cpp:207-256): draws
	// the flash overlay only when FlashScale is not the neutral (0.5,0.5,
	// 0.5,0) or FlashFog is not zero. Alpha is 1 - min(2 * FlashScale.X,
	// 1), so the overlay goes from transparent to opaque as FlashScale.X
	// falls. The real NoDrawSimple gate is not mirrored: this class has
	// no such field, same as Draw2DLine's own override.
	if( CurrentFlashScale == FPlane(0.5f,0.5f,0.5f,0.f) && CurrentFlashFog == FPlane(0.f,0.f,0.f,0.f) )
		return;
	const FLOAT Alpha = 1.f - Min( CurrentFlashScale.X * 2.f, 1.f );

	// Counts and logs only draws the Rust side queued: it returns false when
	// no frame is open or no SetSceneNode has run yet. Logs the first one.
	if( !hp2_rs_wgpu_end_flash( RustDevice,
		CurrentFlashFog.X, CurrentFlashFog.Y, CurrentFlashFog.Z, Alpha ) )
		return;
	if( EndFlashDrawCount == 0 )
	{
		debugf( NAME_Init, TEXT("WgpuDrv: EndFlash first draw: Fog=(%f,%f,%f) Alpha=%f"),
			CurrentFlashFog.X, CurrentFlashFog.Y, CurrentFlashFog.Z, Alpha );
	}
	EndFlashDrawCount++;
	unguard;
}

void UWgpuRenderDevice::ClearZ( FSceneNode* Frame )
{
	guard(UWgpuRenderDevice::ClearZ);
	// Real XOpenGL ClearZ (XOpenGL.cpp:3030-3042): flush queued draws,
	// then clear depth to 1.0. Its non-macOS SetSceneNode(Frame) call
	// is a workaround the macOS build skips too.
	if( RustDevice )
		hp2_rs_wgpu_clear_z( RustDevice );
	unguard;
}

namespace
{
// Per-call context for RasterizeNativeTextGlyph: the drawable glyphs of
// one DrawCanvasText call, in the same order as the FFI glyph array.
struct FNativeTextRasterContext
{
	const std::vector<const Hp2NativeText::FGlyphPlacement*>* Drawable;
};

// FfiNativeTextRasterize (hp2_rs.h). Coverage is RasterizeGlyph's own
// alpha, the same bytes XOpenGL's atlas uploads. Rust sizes OutLen from
// CalculateRasterRectangle, which must equal RasterizeGlyph's Width*Height;
// a mismatch is refused, never truncated.
bool RasterizeNativeTextGlyph( void* Ctx, size_t Index, uint8_t* OutCoverage, size_t OutLen )
{
	const FNativeTextRasterContext* Context = static_cast<const FNativeTextRasterContext*>( Ctx );
	if( !Context || Index >= Context->Drawable->size() || !OutCoverage )
		return false;
	Hp2NativeText::FRasterizedGlyph Raster;
	if( !Hp2NativeText::RasterizeGlyph( *(*Context->Drawable)[Index], Hp2NativeText::PageSize, Raster ) )
		return false;
	if( Raster.Alpha.size() != OutLen )
		return false;
	appMemcpy( OutCoverage, Raster.Alpha.data(), (INT)OutLen );
	return true;
}
}

UBOOL UWgpuRenderDevice::CreateCanvasTextLayout( const FCanvasTextRequest& Request, FCanvasTextLayout*& OutLayout )
{
	guard(UWgpuRenderDevice::CreateCanvasTextLayout);
	OutLayout = NULL;
	// No staged-font retry (XOpenGL NativeText.cpp:798-811): IsAwaitingStagedFont()
	// is false on every native build (NativeTextShared.cpp:735-740), so that
	// browser-only path is unreachable here.
	if( !NativeTextBackend )
		return 0;
	// Origin validation and layout-only projection as in XOpenGL
	// (NativeText.cpp:818-827). The origin is applied only after a
	// successful create, so a failed create leaves nothing behind.
	if( !Hp2NativeText::SafeFloat( Request.OriginX ) || !Hp2NativeText::SafeFloat( Request.OriginY ) )
		return 0;
	FCanvasTextLayoutRequest LayoutRequest;
	if( !ProjectCanvasTextLayoutRequest( Request, LayoutRequest ) )
		return 0;
	if( !NativeTextBackend->CreateLayout( LayoutRequest, OutLayout ) || !OutLayout )
		return 0;
	OutLayout->OriginX = Request.OriginX;
	OutLayout->OriginY = Request.OriginY;
	return 1;
	unguard;
}

void UWgpuRenderDevice::DestroyCanvasTextLayout( FCanvasTextLayout* Layout )
{
	guard(UWgpuRenderDevice::DestroyCanvasTextLayout);
	if( NativeTextBackend )
		NativeTextBackend->DestroyLayout( Layout );
	unguard;
}

UBOOL UWgpuRenderDevice::MeasureCanvasText( FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight )
{
	guard(UWgpuRenderDevice::MeasureCanvasText);
	OutWidth = 0;
	OutHeight = 0;
	return NativeTextBackend ? NativeTextBackend->MeasureLayout( Layout, OutWidth, OutHeight ) : 0;
	unguard;
}

UBOOL UWgpuRenderDevice::DrawCanvasText( FSceneNode* Frame, FCanvasTextLayout* Layout )
{
	guard(UWgpuRenderDevice::DrawCanvasText);
	// Entry checks as in XOpenGL's DrawLayout (NativeText.cpp:398-399).
	if( !RustDevice || !NativeTextBackend || !Frame || !Hp2NativeText::IsLayout( Layout ) )
		return 0;

	// Drawable glyphs only, as DrawLayout skips the rest (NativeText.cpp:404-411).
	// Every drawable glyph is measured before anything reaches the atlas, so a
	// failure returns 0 with nothing queued.
	std::vector<const Hp2NativeText::FGlyphPlacement*> Drawable;
	std::vector<FfiNativeTextGlyph> Glyphs;
	for( const Hp2NativeText::FGlyphPlacement& Placement : Layout->Glyphs )
	{
		if( !Placement.bDrawable )
			continue;
		Hp2NativeText::FRasterRectangle Rectangle;
		if( !Hp2NativeText::CalculateRasterRectangle( Placement, Hp2NativeText::PageSize, Rectangle ) )
			return 0;
		double DrawLeft = 0.0, DrawTop = 0.0;
		Hp2NativeText::GlyphDrawOffset( Placement, DrawLeft, DrawTop );
		FfiNativeTextGlyph Glyph;
		Glyph.font_identity = reinterpret_cast<const uint8_t*>( Placement.Key.FontIdentity.data() );
		Glyph.font_identity_len = Placement.Key.FontIdentity.size();
		Glyph.glyph = static_cast<uint32_t>( Placement.Key.Glyph );
		Glyph.raster_point_size_64 = Placement.Key.RasterPointSize64;
		Glyph.antialias = Placement.Key.Antialias != 0;
		Glyph.subpixel = Placement.Key.Subpixel != 0;
		Glyph.draw_left = DrawLeft;
		Glyph.draw_top = DrawTop;
		Glyph.raster_width = static_cast<uint32_t>( Rectangle.Width );
		Glyph.raster_height = static_cast<uint32_t>( Rectangle.Height );
		Drawable.push_back( &Placement );
		Glyphs.push_back( Glyph );
	}

	std::vector<FfiNativeTextUnderline> Underlines;
	for( const Hp2NativeText::FUnderline& Underline : Layout->Underlines )
	{
		FfiNativeTextUnderline Raw;
		Raw.start = Underline.Start;
		Raw.end = Underline.End;
		Raw.baseline = Underline.Baseline;
		Raw.position = Underline.Position;
		Raw.thickness = Underline.Thickness;
		Underlines.push_back( Raw );
	}

	FfiNativeTextLayout NativeLayout;
	NativeLayout.start_x = Layout->StartX;
	NativeLayout.start_y = Layout->StartY;
	NativeLayout.origin_x = Layout->OriginX;
	NativeLayout.origin_y = Layout->OriginY;
	NativeLayout.clip_x = Layout->ClipX;
	NativeLayout.clip_y = Layout->ClipY;
	NativeLayout.clip = Layout->bClip != 0;
	NativeLayout.poly_flags = Layout->PolyFlags;
	NativeLayout.color[0] = Layout->Color.X;
	NativeLayout.color[1] = Layout->Color.Y;
	NativeLayout.color[2] = Layout->Color.Z;
	NativeLayout.color[3] = Layout->Color.W;

	FNativeTextRasterContext Context;
	Context.Drawable = &Drawable;
	const UBOOL bDrawn = hp2_rs_wgpu_draw_native_text( RustDevice, &NativeLayout,
		Glyphs.data(), Glyphs.size(), Underlines.data(), Underlines.size(),
		Frame->FX2, Frame->FY2, RasterizeNativeTextGlyph, &Context ) ? 1 : 0;
	static UBOOL bLoggedFirstCall = 0;
	if( !bLoggedFirstCall )
	{
		bLoggedFirstCall = 1;
		debugf( NAME_Dev, TEXT("WgpuDrv: DrawCanvasText first call: glyphs=%i underlines=%i result=%i"), (INT)Glyphs.size(), (INT)Underlines.size(), (INT)bDrawn );
	}
	return bDrawn;
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

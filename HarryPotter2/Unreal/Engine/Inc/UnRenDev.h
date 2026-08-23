/*=============================================================================
	UnRenDev.h: 3D rendering device class.

	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.
	Compiled with Visual C++ 4.0. Best viewed with Tabs=4.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#ifndef _UNRENDEV_H_
#define _UNRENDEV_H_

/*------------------------------------------------------------------------------------
	URenderDevice.
------------------------------------------------------------------------------------*/

// Flags for locking a rendering device.
enum ELockRenderFlags
{
	LOCKR_ClearScreen	    = 1,
	LOCKR_LightDiminish     = 2,
};
enum EDescriptionFlags
{
	RDDESCF_Certified       = 1,
	RDDESCF_Incompatible    = 2,
	RDDESCF_LowDetailWorld  = 4,
	RDDESCF_LowDetailSkins  = 8,
	RDDESCF_LowDetailActors = 16,
};
class UFont;
struct FSurfaceInfo;
struct FSurfaceFacet;
struct FTransTexture;
struct FCanvasTextLayout;
class FNativeTextPlatformBackend;
struct FCanvasTextRequest;
// Layout-only text request.  Draw-side state (Canvas origin, cursor rules)
// never travels in this struct: layout creation is pure, and drawing stays
// driven by the existing Canvas flow against the returned layout.  TextLength
// is always the full, non-negative source span; wrapping is requested
// exclusively through Mode -- never through the sign of TextLength.
enum ECanvasTextLayoutMode
{
	CanvasLayout_Compute = 0,
	CanvasLayout_Wrapped = 1,
};
struct FCanvasTextLayoutRequest
{
	UFont* Font;
	const TCHAR* Text;
	INT TextLength;
	FLOAT TextScale;
	FLOAT SpaceX;
	FLOAT SpaceY;
	FLOAT ClipX;
	FLOAT ClipY;
	INT StartX;
	INT StartY;
	DWORD PolyFlags;
	FPlane Color;
	UBOOL bClip;
	UBOOL bCenter;
	UBOOL bHandleAmpersand;
	INT VisibleSourceCharacters;
	ECanvasTextLayoutMode Mode;

	FCanvasTextLayoutRequest()
	: Font(NULL), Text(NULL), TextLength(0), TextScale(1.f), SpaceX(0.f), SpaceY(0.f)
	, ClipX(0.f), ClipY(0.f), StartX(0), StartY(0), PolyFlags(0), Color(0.f,0.f,0.f,0.f)
	, bClip(0), bCenter(0), bHandleAmpersand(0), VisibleSourceCharacters(0), Mode(CanvasLayout_Compute)
	{}
	FCanvasTextLayoutRequest( UFont* InFont, const TCHAR* InText, INT InTextLength, ECanvasTextLayoutMode InMode )
	: Font(InFont), Text(InText), TextLength(InTextLength), TextScale(1.f), SpaceX(0.f), SpaceY(0.f)
	, ClipX(0.f), ClipY(0.f), StartX(0), StartY(0), PolyFlags(0), Color(0.f,0.f,0.f,0.f)
	, bClip(0), bCenter(0), bHandleAmpersand(0), VisibleSourceCharacters(0), Mode(InMode)
	{}
	static FCanvasTextLayoutRequest Computed( UFont* InFont, const TCHAR* InText, INT InTextLength )
	{
		return FCanvasTextLayoutRequest( InFont, InText, InTextLength, CanvasLayout_Compute );
	}
	static FCanvasTextLayoutRequest Wrapped( UFont* InFont, const TCHAR* InText, INT InTextLength )
	{
		return FCanvasTextLayoutRequest( InFont, InText, InTextLength, CanvasLayout_Wrapped );
	}
};

// Projects a full Canvas text request onto the layout-only request consumed by
// FNativeTextPlatformBackend.  Draw-side OriginX/Y are validated but dropped
// here; returns 0 (leaving OutRequest untouched) for negative lengths or
// non-finite origin/clip bounds.
ENGINE_API UBOOL ProjectCanvasTextLayoutRequest( const FCanvasTextRequest& CanvasRequest, FCanvasTextLayoutRequest& OutRequest );

struct FCanvasTextRequest
{
	UFont* Font;
	const TCHAR* Text;
	INT TextLength;
	FLOAT TextScale;
	FLOAT SpaceX;
	FLOAT SpaceY;
	FLOAT OriginX;
	FLOAT OriginY;
	FLOAT ClipX;
	FLOAT ClipY;
	INT StartX;
	INT StartY;
	DWORD PolyFlags;
	FPlane Color;
	UBOOL bClip;
	UBOOL bCenter;
	UBOOL bHandleAmpersand;
	INT VisibleSourceCharacters;
	ECanvasTextLayoutMode Mode;
};

// Deterministic test seam for the Canvas-native policy boundary.  It owns no
// renderer state; production Canvas operations and contract tests use the
// same request-dispatch routine.
struct FCanvasNativeTextTestState
{
	UBOOL NativeText;
	FCanvasTextRequest Request;
	INT CurX;
	INT CurY;
	INT CurYL;
	// 0=DrawText, 1=DrawTextClipped, 2=TextSize/StrLen,
	// 3=WrappedPrint, 4=WrappedStrLenf.
	INT Operation;
	UBOOL bCR;
	UBOOL bClipped;
};

ENGINE_API UBOOL RunCanvasNativeTextCompatibilityForTests( FNativeTextPlatformBackend* Backend, FCanvasNativeTextTestState& State, INT& OutWidth, INT& OutHeight );


//
// A low-level 3D rendering device.
//
class ENGINE_API URenderDevice : public USubsystem
{
	DECLARE_ABSTRACT_CLASS(URenderDevice,USubsystem,CLASS_Config,Engine)

	// Variables.
	BYTE			DecompFormat;
	INT				RecommendedLOD;
	UViewport*		Viewport;
	FString			Description;
	DWORD			DescFlags;
	BITFIELD		SpanBased;
	BITFIELD		FullscreenOnly;
	BITFIELD		SupportsFogMaps;
	BITFIELD		SupportsDistanceFog;
	BITFIELD		VolumetricLighting;
	BITFIELD		ShinySurfaces;
	BITFIELD		Coronas;
	BITFIELD		HighDetailActors;
	BITFIELD		SupportsTC;
	BITFIELD		PrecacheOnFlip;
	BITFIELD		SupportsLazyTextures;
	BITFIELD		PrefersDeferredLoad;
	BITFIELD		DetailTextures;
	BITFIELD		Pad1[8];
	DWORD			Pad0[8];

	// Constructors.
	void StaticConstructor();

	// URenderDevice low-level functions that drivers must implement.
	virtual void Placeholder() {}
	virtual UBOOL Init( UViewport* InViewport, INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen )=0;
	virtual UBOOL SetRes( INT NewX, INT NewY, INT NewColorBytes, UBOOL Fullscreen )=0;
	virtual void Exit()=0;
	virtual void Flush( UBOOL AllowPrecache )=0;
	virtual UBOOL Exec( const TCHAR* Cmd, FOutputDevice& Ar );
	virtual void Lock( FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData, INT* HitSize )=0;
	virtual void Unlock( UBOOL Blit )=0;
	
	virtual void DrawComplexSurface( FSceneNode* Frame, FSurfaceInfo& Surface, FSurfaceFacet& Facet, DWORD PolyFlags, BYTE cAlpha )=0;
	virtual INT MaxVertices()=0;
	virtual void DrawTriangles( FSceneNode* Frame, FTextureInfo& Info, FTransTexture** Pts, INT NumPts, _WORD* Indices, INT NumIndices, DWORD PolyFlags, FSpanBuffer* Span )=0;
	virtual void DrawGouraudPolygon( FSceneNode* Frame, FTextureInfo& Info, FTransTexture** Pts, int NumPts, DWORD PolyFlags, FSpanBuffer* Span );
	virtual UBOOL CreateCanvasTextLayout( const FCanvasTextRequest&, FCanvasTextLayout*& OutLayout ) { OutLayout=NULL; return 0; }
	virtual void DestroyCanvasTextLayout( FCanvasTextLayout* ) {}
	virtual UBOOL MeasureCanvasText( FCanvasTextLayout*, INT& OutWidth, INT& OutHeight ) { return 0; }
	virtual UBOOL DrawCanvasText( FSceneNode*, FCanvasTextLayout* ) { return 0; }
	virtual void DrawTile( FSceneNode* Frame, FTextureInfo& Info, FLOAT X, FLOAT Y, FLOAT XL, FLOAT YL, FLOAT U, FLOAT V, FLOAT UL, FLOAT VL, class FSpanBuffer* Span, FLOAT Z, FPlane Color, FPlane Fog, DWORD PolyFlags )=0;
	virtual void Draw3DLine( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FVector OrigP, FVector OrigQ );
	virtual void Draw2DClippedLine( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FVector P1, FVector P2 );
	virtual void Draw2DLine( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FVector P1, FVector P2 )=0;
	virtual void Draw2DPoint( FSceneNode* Frame, FPlane Color, DWORD LineFlags, FLOAT X1, FLOAT Y1, FLOAT X2, FLOAT Y2, FLOAT Z )=0;
	virtual void ClearZ( FSceneNode* Frame )=0;
	virtual void PushHit( const BYTE* Data, INT Count )=0;
	virtual void PopHit( INT Count, UBOOL bForce )=0;
	virtual void GetStats( TCHAR* Result )=0;
	virtual void ReadPixels( FColor* Pixels )=0;
	virtual void EndFlash() {}
	virtual void BeginUI( FSceneNode* Frame ) {}
	virtual void DrawStats( FSceneNode* Frame ) {}
	virtual void SetSceneNode( FSceneNode* Frame ) {}
	virtual void PrecacheTexture( FTextureInfo& Info, DWORD PolyFlags ) {}
	virtual void* GetOsSurface( void* Release = NULL, bool bFrontBuffer=false) { return NULL; }

	// Padding.
	virtual void vtblPad0() {}
	virtual void vtblPad1() {}
	virtual void vtblPad2() {}
	virtual void vtblPad3() {}
	virtual void vtblPad4() {}
	virtual void vtblPad5() {}
	virtual void vtblPad6() {}
	virtual void vtblPad7() {}
};

#endif
/*------------------------------------------------------------------------------------
	The End.
------------------------------------------------------------------------------------*/

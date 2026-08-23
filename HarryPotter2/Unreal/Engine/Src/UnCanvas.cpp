/*=============================================================================
	UnCanvas.cpp: Unreal canvas rendering.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Tim Sweeney
	* 31/3/99 Updated revision history - Jack Porter
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "NativeText.h"
#include <cmath>

void UCanvas::StaticConstructor()
{
	guard(UCanvas::StaticConstructor);
	UStruct* ColorStruct = FindObject<UStruct>(UObject::StaticClass(),TEXT("Color"));
	if( !ColorStruct )
	{
		ColorStruct = new(UObject::StaticClass(),TEXT("Color"),RF_Public)UStruct(NULL);
		ColorStruct->SetPropertiesSize(sizeof(FColor));
		new(ColorStruct,TEXT("R"),RF_Public)UByteProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FColor,R)),TEXT(""),0);
		new(ColorStruct,TEXT("G"),RF_Public)UByteProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FColor,G)),TEXT(""),0);
		new(ColorStruct,TEXT("B"),RF_Public)UByteProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FColor,B)),TEXT(""),0);
		new(ColorStruct,TEXT("A"),RF_Public)UByteProperty(EC_CppProperty,static_cast<INT>(__builtin_offsetof(FColor,A)),TEXT(""),0);
		FArchive ArDummy;
		ColorStruct->Link(ArDummy,0);
	}
	new(GetClass(),TEXT("Font"),RF_Public)UObjectProperty(CPP_PROPERTY(Font),TEXT("Canvas"),0,UFont::StaticClass());
	new(GetClass(),TEXT("SpaceX"),RF_Public)UFloatProperty(CPP_PROPERTY(SpaceX),TEXT("Canvas"),0);
	new(GetClass(),TEXT("SpaceY"),RF_Public)UFloatProperty(CPP_PROPERTY(SpaceY),TEXT("Canvas"),0);
	new(GetClass(),TEXT("OrgX"),RF_Public)UFloatProperty(CPP_PROPERTY(OrgX),TEXT("Canvas"),0);
	new(GetClass(),TEXT("OrgY"),RF_Public)UFloatProperty(CPP_PROPERTY(OrgY),TEXT("Canvas"),0);
	new(GetClass(),TEXT("ClipX"),RF_Public)UFloatProperty(CPP_PROPERTY(ClipX),TEXT("Canvas"),0);
	new(GetClass(),TEXT("ClipY"),RF_Public)UFloatProperty(CPP_PROPERTY(ClipY),TEXT("Canvas"),0);
	new(GetClass(),TEXT("CurX"),RF_Public)UFloatProperty(CPP_PROPERTY(CurX),TEXT("Canvas"),0);
	new(GetClass(),TEXT("CurY"),RF_Public)UFloatProperty(CPP_PROPERTY(CurY),TEXT("Canvas"),0);
	new(GetClass(),TEXT("Z"),RF_Public)UFloatProperty(CPP_PROPERTY(Z),TEXT("Canvas"),0);
	new(GetClass(),TEXT("Style"),RF_Public)UByteProperty(CPP_PROPERTY(Style),TEXT("Canvas"),0);
	new(GetClass(),TEXT("CurYL"),RF_Public)UFloatProperty(CPP_PROPERTY(CurYL),TEXT("Canvas"),0);
	new(GetClass(),TEXT("DrawColor"),RF_Public)UStructProperty(CPP_PROPERTY(Color),TEXT("Canvas"),0,ColorStruct);
	const INT BoolOffset = static_cast<INT>(__builtin_offsetof(UCanvas, Color) + sizeof(FColor));
	UBoolProperty* Center = new(GetClass(),TEXT("bCenter"),RF_Public)UBoolProperty(EC_CppProperty,BoolOffset,TEXT("Canvas"),0);
	Center->BitMask = 1;
	UBoolProperty* NoSmooth = new(GetClass(),TEXT("bNoSmooth"),RF_Public)UBoolProperty(EC_CppProperty,BoolOffset,TEXT("Canvas"),0);
	NoSmooth->BitMask = 2;
	new(GetClass(),TEXT("SizeX"),RF_Public)UIntProperty(CPP_PROPERTY(X),TEXT("Canvas"),0);
	new(GetClass(),TEXT("SizeY"),RF_Public)UIntProperty(CPP_PROPERTY(Y),TEXT("Canvas"),0);
	new(GetClass(),TEXT("SmallFont"),RF_Public)UObjectProperty(CPP_PROPERTY(SmallFont),TEXT("Canvas"),0,UFont::StaticClass());
	new(GetClass(),TEXT("MedFont"),RF_Public)UObjectProperty(CPP_PROPERTY(MedFont),TEXT("Canvas"),0,UFont::StaticClass());
	new(GetClass(),TEXT("BigFont"),RF_Public)UObjectProperty(CPP_PROPERTY(BigFont),TEXT("Canvas"),0,UFont::StaticClass());
	new(GetClass(),TEXT("LargeFont"),RF_Public)UObjectProperty(CPP_PROPERTY(LargeFont),TEXT("Canvas"),0,UFont::StaticClass());
	new(GetClass(),TEXT("Viewport"),RF_Public)UObjectProperty(CPP_PROPERTY(Viewport),TEXT("Canvas"),0,UViewport::StaticClass());
	new(GetClass(),TEXT("FramePtr"),RF_Public)UIntProperty(CPP_PROPERTY(Frame),TEXT("Canvas"),0);
	new(GetClass(),TEXT("RenderPtr"),RF_Public)UIntProperty(CPP_PROPERTY(Render),TEXT("Canvas"),0);
	unguard;
}

/*-----------------------------------------------------------------------------
	UCanvas scaled sprites.
-----------------------------------------------------------------------------*/

//
// Draw arbitrary aligned rectangle.
//
void UCanvas::DrawTile
(
	UTexture*		Texture,
	FLOAT			X,
	FLOAT			Y,
	FLOAT			XL,
	FLOAT			YL,
	FLOAT			U,
	FLOAT			V,
	FLOAT			UL,
	FLOAT			VL,
	FSpanBuffer*	SpanBuffer,
	FLOAT			Z,
	FPlane			Color,
	FPlane			Fog,
	DWORD			PolyFlags
)
{
	guard(UCanvas::DrawTile);
	check(Texture);

	// Compute clipping region.
	FLOAT ClipY0 = /*SpanBuffer ? SpanBuffer->StartY :*/ 0;
	FLOAT ClipY1 = /*SpanBuffer ? SpanBuffer->EndY   :*/ Frame->FY;

	// Reject.
	if( XL<=0.f || YL<=0.f || X+XL<=0.f || Y+YL<=ClipY0 || X>=Frame->FX || Y>=ClipY1 )
		return;

	// Clip.
	if( X<0.f )
		{FLOAT C=X*UL/XL; U-=C; UL+=C; XL+=X; X=0.f;}
	if( Y<0.f )
		{FLOAT C=Y*VL/YL; V-=C; VL+=C; YL+=Y; Y=0.f;}
	if( XL>Frame->FX-X )
		{UL+=(Frame->FX-X-XL)*UL/XL; XL=Frame->FX-X;}
	if( YL>Frame->FY-Y )
		{VL+=(Frame->FY-Y-YL)*VL/YL; YL=Frame->FY-Y;}

	// Draw it.
	FTextureInfo Info;
	if( !GIsEditor )
		Texture = Texture->Get( Viewport->CurrentTime );
	Texture->Lock( Info, Viewport->CurrentTime, -1, Viewport->RenDev );
	FLOAT UF = Info.UScale * Info.USize / Texture->USize; U *= UF; UL *= UF;
	FLOAT VF = Info.VScale * Info.VSize / Texture->VSize; V *= VF; VL *= VF;
	Viewport->RenDev->DrawTile( Frame, Info, X, Y, XL, YL, U, V, UL, VL, SpanBuffer, Z, Color, Fog, PolyFlags | (Texture->PolyFlags&PF_Masked) );
	Texture->Unlock( Info );

	unguard;
}

//
// Draw titling pattern.
//
void UCanvas::DrawPattern
(
	UTexture*		Texture,
	FLOAT			X,
	FLOAT			Y,
	FLOAT			XL,
	FLOAT			YL,
	FLOAT			Scale,
	FLOAT			OrgX,
	FLOAT			OrgY,
	FSpanBuffer*	SpanBuffer,
	FLOAT			Z,
	FPlane			Color,
	FPlane			Fog,
	DWORD			PolyFlags
)
{
	guard(UCanvas::DrawPattern);
	DrawTile( Texture, X, Y, XL, YL, (X-OrgX)*Scale + Texture->USize, (Y-OrgY)*Scale + Texture->VSize, XL*Scale, YL*Scale, SpanBuffer, Z, Color, Fog, PolyFlags );
	unguard;
}

//
// Draw a scaled sprite.  Takes care of clipping.
// XSize and YSize are in pixels.
//
void UCanvas::DrawIcon
(
	UTexture*			Texture,
	FLOAT				ScreenX, 
	FLOAT				ScreenY, 
	FLOAT				XSize, 
	FLOAT				YSize, 
	FSpanBuffer*		SpanBuffer,
	FLOAT				Z,
	FPlane				Color,
	FPlane				Fog,
	DWORD				PolyFlags
)
{
	guard(UCanvas::DrawIcon);
	DrawTile( Texture, ScreenX, ScreenY, XSize, YSize, 0, 0, Texture->USize, Texture->VSize, SpanBuffer, Z, Color, Fog, PolyFlags );
	unguard;
}

/*-----------------------------------------------------------------------------
	Clip window.
-----------------------------------------------------------------------------*/

void UCanvas::SetClip( INT X, INT Y, INT XL, INT YL )
{
	guard(UCanvas::SetClip);

	CurX  = 0;
	CurY  = 0;
	OrgX  = X;
	OrgY  = Y;
	ClipX = XL;
	ClipY = YL;

	unguard;
}

/*-----------------------------------------------------------------------------
	UCanvas basic text functions.
-----------------------------------------------------------------------------*/

static inline FLOAT GetCanvasTextScale( const UCanvas* Canvas )
{
	const FLOAT Width = Canvas && Canvas->Frame ? Canvas->Frame->FX : (Canvas ? (FLOAT)Canvas->X : 0.f);
	FLOAT UIScale = 1.f;
	if( Canvas && Canvas->Viewport && Canvas->Viewport->GetOuterUClient() )
		UIScale = Canvas->Viewport->GetOuterUClient()->GetUIScale();
	if( appIsNan(UIScale) )
		UIScale = 1.f;
	UIScale = Clamp(UIScale, 0.75f, 2.f);
	return Max(1.f, Width/640.f) * UIScale;
}

static inline UBOOL IsSafeCanvasTextFloat( FLOAT Value )
{
	const double WideValue = static_cast<double>(Value);
	return !appIsNan(Value)
		&& WideValue >= -static_cast<double>(MAXINT) - 1.0
		&& WideValue <= static_cast<double>(MAXINT);
}

static inline UBOOL HasSafeCanvasTextRequestFloats( const UCanvas* Canvas )
{
	if( !Canvas )
		return 0;
	const FLOAT TextScale = GetCanvasTextScale(Canvas);
	return IsSafeCanvasTextFloat(TextScale)
		&& IsSafeCanvasTextFloat(Canvas->SpaceX * TextScale)
		&& IsSafeCanvasTextFloat(Canvas->SpaceY * TextScale)
		&& IsSafeCanvasTextFloat(Canvas->OrgX)
		&& IsSafeCanvasTextFloat(Canvas->OrgY)
		&& IsSafeCanvasTextFloat(Canvas->ClipX)
		&& IsSafeCanvasTextFloat(Canvas->ClipY)
		&& IsSafeCanvasTextFloat(Canvas->CurX)
		&& IsSafeCanvasTextFloat(Canvas->CurY);
}

static inline INT ScaleFontMetric( INT Value, FLOAT Scale )
{
	return Value>0 ? Max(1, appRound(Value*Scale)) : 0;
}

//
// Draw a character.
//
static inline void DrawChar
(
	DWORD			Flags,
	UCanvas*		Canvas,
	FTextureInfo&	Info,
	INT				X,
	INT				Y,
	INT				XL,
	INT				YL,
	INT				U, 
	INT				V, 
	INT				UL, 
	INT				VL, 
	FPlane			Color
)
{
	guardSlow(DrawChar);

	// Reject.
	FSceneNode* Frame=Canvas->Frame;
	if( !(Flags & PF_Invisible) && X+XL>0 && Y+YL>0 && X<Frame->X && Y<Frame->Y && XL>0 && YL>0 )
	{
		// Clip.
		if( X<0 )
			{INT C=X*UL/XL; U-=C; UL+=C; XL+=X; X=0;}
		if( Y<0 )
			{INT C=Y*VL/YL; V-=C; VL+=C; YL+=Y; Y=0;}
		if( XL>Frame->X-X )
			{UL+=(Frame->X-X-XL)*UL/XL; XL=Frame->X-X;}
		if( YL>Frame->Y-Y )
			{VL+=(Frame->Y-Y-YL)*VL/YL; YL=Frame->Y-Y;}

		// Draw.
		Canvas->Viewport->RenDev->DrawTile( Frame, Info, X, Y, XL, YL, U, V, UL, VL, NULL, Canvas->Z, Color, FPlane(0,0,0,0), Flags );
	}
	unguardSlow;
}

//
// Get a character's dimensions.
//
static inline void GetCharSize( UFont* Font, TCHAR InCh, FLOAT TextScale, INT& Width, INT& Height )
{
	guardSlow(GetCharSize);
	Width = 0;
	Height = 0;
	if( !Font || Font->CharactersPerPage<=0 )
		return;
	INT Ch    = (TCHARU)Font->RemapChar(InCh);
	INT Page  = Ch / Font->CharactersPerPage;
	INT Index = Ch - Page * Font->CharactersPerPage;
	if( Page<Font->Pages.Num() && Index<Font->Pages(Page).Characters.Num() )
	{
		FFontCharacter& Char = Font->Pages(Page).Characters(Index);
		Width = ScaleFontMetric(Char.USize, TextScale);
		Height = ScaleFontMetric(Char.VSize, TextScale);
	}
	unguardSlow;
}
class FCanvasTextLayoutDispatcher
{
public:
	virtual ~FCanvasTextLayoutDispatcher() {}
	virtual UBOOL Create( const FCanvasTextRequest& Request, FCanvasTextLayout*& Layout ) = 0;
	virtual void Destroy( FCanvasTextLayout* Layout ) = 0;
	virtual UBOOL Measure( FCanvasTextLayout* Layout, INT& Width, INT& Height ) = 0;
	virtual UBOOL Draw( FSceneNode* Frame, FCanvasTextLayout* Layout ) = 0;
};

class FRenderDeviceCanvasTextDispatcher : public FCanvasTextLayoutDispatcher
{
	URenderDevice* RenderDevice;
public:
	FRenderDeviceCanvasTextDispatcher( URenderDevice* InRenderDevice ) : RenderDevice(InRenderDevice) {}
	UBOOL Create( const FCanvasTextRequest& Request, FCanvasTextLayout*& Layout ) override { return RenderDevice->CreateCanvasTextLayout(Request, Layout); }
	void Destroy( FCanvasTextLayout* Layout ) override { RenderDevice->DestroyCanvasTextLayout(Layout); }
	UBOOL Measure( FCanvasTextLayout* Layout, INT& Width, INT& Height ) override { return RenderDevice->MeasureCanvasText(Layout, Width, Height); }
	UBOOL Draw( FSceneNode* Frame, FCanvasTextLayout* Layout ) override { return RenderDevice->DrawCanvasText(Frame, Layout); }
};

class FBackendCanvasTextDispatcher : public FCanvasTextLayoutDispatcher
{
	FNativeTextPlatformBackend* Backend;
public:
	FBackendCanvasTextDispatcher( FNativeTextPlatformBackend* InBackend ) : Backend(InBackend) {}
	UBOOL Create( const FCanvasTextRequest& Request, FCanvasTextLayout*& Layout ) override
	{
		FCanvasTextLayoutRequest LayoutRequest;
		if( !ProjectCanvasTextLayoutRequest(Request, LayoutRequest) )
			return 0;
		return Backend->CreateLayout(LayoutRequest, Layout);
	}
	void Destroy( FCanvasTextLayout* Layout ) override { Backend->DestroyLayout(Layout); }
	UBOOL Measure( FCanvasTextLayout* Layout, INT& Width, INT& Height ) override { return Backend->MeasureLayout(Layout, Width, Height); }
	UBOOL Draw( FSceneNode*, FCanvasTextLayout* ) override { return 1; }
};

static UBOOL DispatchCanvasTextLayout( FCanvasTextLayoutDispatcher& Dispatcher, FSceneNode* Frame, const FCanvasTextRequest& Request, UBOOL bDraw, INT& OutWidth, INT& OutHeight )
{
	FCanvasTextLayout* Layout = NULL;
	UBOOL Success = Dispatcher.Create(Request, Layout) && Dispatcher.Measure(Layout, OutWidth, OutHeight);
	if( Success && bDraw )
		Success = Dispatcher.Draw(Frame, Layout);
	if( Layout )
		Dispatcher.Destroy(Layout);
	if( !Success )
		OutWidth = OutHeight = 0;
	return Success;
}
UBOOL ProjectCanvasTextLayoutRequest( const FCanvasTextRequest& CanvasRequest, FCanvasTextLayoutRequest& OutRequest )
{
	static const FLOAT MaxSafe = (FLOAT)0x7fffffff;
	const double WideClipX = CanvasRequest.ClipX, WideClipY = CanvasRequest.ClipY;
	if( !CanvasRequest.Font || !CanvasRequest.Text
	||	CanvasRequest.TextLength < 0
	||	!std::isfinite((double)CanvasRequest.OriginX) || !std::isfinite((double)CanvasRequest.OriginY)
	||	!std::isfinite(WideClipX) || !std::isfinite(WideClipY)
	||	WideClipX < -MaxSafe || WideClipX > MaxSafe || WideClipY < -MaxSafe || WideClipY > MaxSafe )
		return 0;

	OutRequest = FCanvasTextLayoutRequest::Computed(CanvasRequest.Font, CanvasRequest.Text, CanvasRequest.TextLength);
	OutRequest.Mode = CanvasRequest.Mode;
	OutRequest.TextScale = CanvasRequest.TextScale;
	OutRequest.SpaceX = CanvasRequest.SpaceX;
	OutRequest.SpaceY = CanvasRequest.SpaceY;
	OutRequest.ClipX = CanvasRequest.ClipX;
	OutRequest.ClipY = CanvasRequest.ClipY;
	OutRequest.StartX = CanvasRequest.StartX;
	OutRequest.StartY = CanvasRequest.StartY;
	OutRequest.PolyFlags = CanvasRequest.PolyFlags;
	OutRequest.Color = CanvasRequest.Color;
	OutRequest.bClip = CanvasRequest.bClip;
	OutRequest.bCenter = CanvasRequest.bCenter;
	OutRequest.bHandleAmpersand = CanvasRequest.bHandleAmpersand;
	OutRequest.VisibleSourceCharacters = CanvasRequest.VisibleSourceCharacters;
	return 1;
}

static UBOOL TryNativeCanvasText
(
	DWORD Flags,
	UCanvas* Canvas,
	UFont* Font,
	INT DrawX,
	INT DrawY,
	const TCHAR* Text,
	FPlane Color,
	UBOOL bClip,
	UBOOL bHandleAmpersand,
	UBOOL bCenter,
	UBOOL bWrap,
	INT VisibleSourceCharacters,
	INT& OutWidth,
	INT& OutHeight,
	FCanvasTextLayoutDispatcher* TestDispatcher = NULL,
	const FCanvasTextRequest* TestRequest = NULL,
	UBOOL TestNativeText = 1
)
{
	OutWidth = OutHeight = 0;
	if( TestDispatcher )
		return TestRequest && TestRequest->Font
			&& (TestNativeText || TestRequest->Font->FontName.Len()>0)
			&& DispatchCanvasTextLayout(*TestDispatcher, NULL, *TestRequest, 0, OutWidth, OutHeight);
#if !HP2_HAS_NATIVE_TEXT_BACKEND
	return 0;
#endif
	if( !Canvas || !HasSafeCanvasTextRequestFloats(Canvas) || !Canvas->Viewport || !Canvas->Viewport->RenDev || !Font || !Text || !*Text )
		return 0;

	UClient* Client = Canvas->Viewport->GetOuterUClient();
	if( !Client || (!Client->NativeText && Font->FontName.Len()==0) )
		return 0;

	FCanvasTextRequest Request = {};
	const INT SourceLength = appStrlen(Text);
	Request.Font = Font;
	Request.Text = Text;
	Request.TextLength = SourceLength;
	Request.TextScale = GetCanvasTextScale(Canvas);
	Request.SpaceX = Canvas->SpaceX * Request.TextScale;
	Request.SpaceY = Canvas->SpaceY * Request.TextScale;
	Request.OriginX = Canvas->OrgX;
	Request.OriginY = Canvas->OrgY;
	Request.ClipX = Canvas->ClipX;
	Request.ClipY = Canvas->ClipY;
	Request.StartX = DrawX;
	Request.StartY = DrawY;
	Request.PolyFlags = Flags;
	Request.Color = Color;
	Request.bClip = bClip;
	Request.bCenter = bCenter;
	Request.bHandleAmpersand = bHandleAmpersand;
	Request.VisibleSourceCharacters = VisibleSourceCharacters > 0
		? Min(VisibleSourceCharacters, SourceLength)
		: SourceLength;
	Request.Mode = bWrap ? CanvasLayout_Wrapped : CanvasLayout_Compute;

	FRenderDeviceCanvasTextDispatcher Dispatcher(Canvas->Viewport->RenDev);
	return DispatchCanvasTextLayout(Dispatcher, Canvas->Frame, Request, !(Flags & PF_Invisible), OutWidth, OutHeight);
}

UBOOL RunCanvasNativeTextCompatibilityForTests( FNativeTextPlatformBackend* Backend, FCanvasNativeTextTestState& State, INT& OutWidth, INT& OutHeight )
{
	OutWidth = OutHeight = 0;
	if( !Backend || !State.Request.Font || !State.Request.Text || !*State.Request.Text )
		return 0;
	FBackendCanvasTextDispatcher Dispatcher(Backend);
	if( !TryNativeCanvasText(0, NULL, NULL, 0, 0, NULL, FPlane(0,0,0,0), 0, 0, 0, 0, 0,
		OutWidth, OutHeight, &Dispatcher, &State.Request, State.NativeText) )
		return 0;

	if( !State.bClipped )
	{
		State.CurX += OutWidth;
		State.CurYL = Max(State.CurYL, OutHeight);
		if( State.bCR )
		{
			State.CurX = 0;
			State.CurY += State.CurYL;
			State.CurYL = 0;
		}
	}
	return 1;
}



//
// Draw a string of characters.
// - returns pixels drawn
//
static INT DrawString
(
	DWORD			Flags,
	UCanvas*		Canvas,
	UFont*			Font,
	INT				DrawX,
	INT				DrawY,
	const TCHAR*	Text,
	FPlane			Color,
	UBOOL			bClip,
	UBOOL			bHandleApersand
)
{
	if( !*Text || !HasSafeCanvasTextRequestFloats(Canvas) )
		return 0;

	INT NativeWidth, NativeHeight;
	if( TryNativeCanvasText(Flags, Canvas, Font, DrawX, DrawY, Text, Color, bClip, bHandleApersand, 0, 0, appStrlen(Text), NativeWidth, NativeHeight) )
		return NativeWidth;

	if( Font->FontName )
	{
		INT OldX = DrawX;
		Canvas->Viewport->DrawString( Flags, Font, DrawX, DrawY, Text, Color );
		return DrawX - OldX;
	}
	if( Font->CharactersPerPage<=0 )
		return 0;
	guardSlow(DrawString);

	const FLOAT TextScale = GetCanvasTextScale(Canvas);
	const INT ScaledSpaceX = appRound(Canvas->SpaceX * TextScale);

	// Font texture pages.
	FTextureInfo Infos[5];
	Infos[0].Texture=Infos[1].Texture=Infos[2].Texture=Infos[3].Texture=Infos[4].Texture=NULL;

	// Draw all characters in string.
	INT LineX = 0;
	INT bDrawUnderline = 0;
	INT UnderlineWidth = 0;
	for( INT i=0; Text[i]; i++ )
	{
		INT bUnderlineNext = 0;
		INT Ch = (TCHARU)Font->RemapChar(Text[i]);

		// Handle ampersand underlining.
		if( bHandleApersand )
		{
			if( bDrawUnderline )
				Ch = (TCHARU)Font->RemapChar('_');
			if( Text[i]=='&' )
			{
				if( !Text[i+1] )
					break;
				if( Text[i+1]!='&' )
				{
					bUnderlineNext = 1;
					Ch = (TCHARU)Font->RemapChar(Text[i+1]);
				}
			}
		}

		// Process character if it's valid.
	try_char:
		INT NewPage = Ch / Font->CharactersPerPage;
		UTexture* Tex;
		if( NewPage<Font->Pages.Num() && (Tex=Font->Pages(NewPage).Texture)!=NULL )
		{
			INT        Index    = Ch - NewPage*Font->CharactersPerPage;
			FFontPage& PageInfo = Font->Pages(NewPage);
			if( Index<PageInfo.Characters.Num() )
			{
				// Get proper font page.
				FTextureInfo& Info = Infos[Min(NewPage,4)];
				if( Info.Texture!=Tex )
				{
					if( Info.Texture )
						Info.Texture->Unlock( Info );
					Tex->Lock( Info, Canvas->Viewport->CurrentTime, 0, Canvas->Viewport->RenDev );
				}
				FFontCharacter& Char = PageInfo.Characters(Index);

				// Try upper case.
				if( Char.USize==0 && Ch >= 'a' && Ch <= 'z' )
				{
					Ch += 'A'-'a';
					goto try_char;
				}

				// Underlines use the underscore glyph but retain the underlined character's advance.
				INT CharWidth = bDrawUnderline ? Min(UnderlineWidth, Char.USize) : Char.USize;
				INT DrawWidth = ScaleFontMetric(CharWidth, TextScale);
				INT DrawHeight = ScaleFontMetric(Char.VSize, TextScale);

				INT X = LineX + DrawX;
				INT Y = DrawY;
				INT CU = Char.StartU;
				INT CV = Char.StartV;
				INT CUSize = CharWidth;
				INT CVSize = Char.VSize;

				if( (!bClip) || (X+DrawWidth>0 && X<=Canvas->ClipX && Y+DrawHeight>0 && Y<=Canvas->ClipY) )
				{
					if( bClip && DrawWidth>0 && DrawHeight>0 )
					{
						if( X<0 )
						{
							const INT Clip = Min(-X, DrawWidth);
							const INT SourceClip = Min(CUSize, appRound((FLOAT)Clip*CUSize/DrawWidth));
							X += Clip; DrawWidth -= Clip; CU += SourceClip; CUSize -= SourceClip;
						}
						if( Y<0 )
						{
							const INT Clip = Min(-Y, DrawHeight);
							const INT SourceClip = Min(CVSize, appRound((FLOAT)Clip*CVSize/DrawHeight));
							Y += Clip; DrawHeight -= Clip; CV += SourceClip; CVSize -= SourceClip;
						}
						if( X+DrawWidth > Canvas->ClipX && DrawWidth>0 )
						{
							const INT Keep = Max(0, (INT)(Canvas->ClipX-X));
							CUSize = Min(CUSize, appRound((FLOAT)Keep*CUSize/DrawWidth));
							DrawWidth = Keep;
						}
						if( Y+DrawHeight > Canvas->ClipY && DrawHeight>0 )
						{
							const INT Keep = Max(0, (INT)(Canvas->ClipY-Y));
							CVSize = Min(CVSize, appRound((FLOAT)Keep*CVSize/DrawHeight));
							DrawHeight = Keep;
						}
					}
					if( DrawWidth>0 && DrawHeight>0 && CUSize>0 && CVSize>0 )
						DrawChar( Flags, Canvas, Info, (INT)(Canvas->OrgX+X), (INT)(Canvas->OrgY+Y), DrawWidth, DrawHeight, CU, CV, CUSize, CVSize, Color );
				}

				if( bDrawUnderline )
					CharWidth = UnderlineWidth;

				if( !bUnderlineNext )
					LineX += ScaleFontMetric(CharWidth, TextScale) + ScaledSpaceX;
				else
					UnderlineWidth = Char.USize;

				bDrawUnderline = bUnderlineNext;
			}
		}
	}

	// Unlock font pages.
	for( INT i=0; i<5; i++ )
		if( Infos[i].Texture )
			Infos[i].Texture->Unlock( Infos[i] );

	return LineX;
	unguardSlow;
}

//
// Compute size and optionally print text with word wrap.
//!!For the next generation, redesign to ignore CurX,CurY.
//
void VARARGS UCanvas::WrappedPrint( ERenderStyle Style, INT& XL, INT& YL, UFont* Font, UBOOL Center, const TCHAR* Text, INT numChars, UBOOL bCursor )
{
	guard(UCanvas::WrappedPrint);

	bool bDoTeletype = numChars ? true : false;

	if( !HasSafeCanvasTextRequestFloats(this) || ClipX<0 || ClipY<0 || !*Text )
	{
		XL = YL = 0;
		return;
	}
	if( (Font==LargeFont || Font==BigFont) && appStricmp(UObject::GetLanguage(),TEXT("INT")) )
		Font = MedFont;//BigFont;!!
	check(Font);
	FPlane DrawColor = Color.Plane();

	// Generate flags.
	DWORD PolyFlags
	=	(PF_NoSmooth | PF_Masked | PF_RenderHint)
	|(	(Style==STY_None       ) ? PF_Invisible
	:	(Style==STY_Translucent) ? PF_Translucent
	:	(Style==STY_Modulated  ) ? PF_Modulated
	:	                           0);

	INT NativeWidth, NativeHeight;
	if( TryNativeCanvasText(PolyFlags, this, Font, CurX, CurY, Text, DrawColor, 0, 0, Center, 1, numChars, NativeWidth, NativeHeight) )
	{
		XL = NativeWidth;
		YL = NativeHeight;
		CurY += NativeHeight;
		return;
	}
	if( Font->FontName.Len()>0 )
	{
		// Viewport.DrawString handles word wrapping for inherently native fonts.
		INT X = CurX, Y = CurY;
		if( Center )
			PolyFlags |= PF_TwoSided;
		Viewport->DrawString( PolyFlags, Font, X, Y, Text, DrawColor );
		XL = X-CurX;  YL = Y-CurY;
		return;
	}
	const FLOAT TextScale = GetCanvasTextScale(this);
	const INT ScaledSpaceX = appRound(SpaceX * TextScale);
	const INT ScaledSpaceY = appRound(SpaceY * TextScale);


	// Process each word until the current line overflows.
	XL = YL = 0;
	float OrigX = CurX;
	do
	{
		INT iCleanWordEnd = 0, iTestWord;
		INT TestXL = Center ? (INT)0 : (INT)CurX, CleanXL = 0;
		INT TestYL = 0, CleanYL = 0;
		UBOOL GotWord=0;
		for( iTestWord=0; Text[iTestWord]!=0 && Text[iTestWord]!='\n'; )
		{
			INT ChW, ChH;
			GetCharSize(Font, Text[iTestWord], TextScale, ChW, ChH);
			if( ChW==0 )
				// Try upper case.
				if( Text[iTestWord] >= 'a' && Text[iTestWord] <= 'z' )
					GetCharSize(Font, Text[iTestWord]+'A'-'a', TextScale, ChW, ChH);
			TestXL              += ChW + ScaledSpaceX;
			TestYL               = Max( TestYL, ChH + ScaledSpaceY );
			if( TestXL>ClipX )
				break;
			iTestWord++;
			UBOOL WordBreak = Text[iTestWord]==' ' || Text[iTestWord]=='\n' || Text[iTestWord]==0;
			if( WordBreak || !GotWord )
			{
				iCleanWordEnd = iTestWord;
				CleanXL       = TestXL;
				CleanYL       = TestYL;
				GotWord       = GotWord || WordBreak;
			}
		}
		if( iCleanWordEnd==0 )
			break;

		if (bDoTeletype)
		{
			if (iCleanWordEnd > numChars)
				iCleanWordEnd = numChars;	
		}


		// Sucessfully split this line, now draw it.
		if( Style!=STY_None && OrgY+CurY<Frame->Y && OrgY+CurY+CleanYL>0 )
		{
			FString TextLine(Text);
			INT LineX = Center ? (INT) (CurX-CleanXL/2) : (INT) (CurX);
			LineX += DrawString( PolyFlags, this, Font, LineX, (INT) CurY, *(TextLine.Left(iCleanWordEnd)), DrawColor, 0, 0 );
			CurX = LineX;
		}

		// Update position.
		CurX  = OrigX;
		CurY += CleanYL;
		YL   += CleanYL;
		XL    = Max(XL,CleanXL);
		Text += iCleanWordEnd;

		if (bDoTeletype)
		{
			numChars -= iCleanWordEnd;
			if (!numChars)
				break;
		}

		// Skip whitespace after word wrap.
		while( *Text==' ' )
			Text++;
	}
	while( *Text );

	unguardf(( TEXT("(%s)"), Text ));
}

/*-----------------------------------------------------------------------------
	UCanvas derived text functions.
-----------------------------------------------------------------------------*/

//
// Calculate the size of a string built from a font, word wrapped
// to a specified region.
//
void UCanvas::WrappedStrLenf( UFont* Font, INT& XL, INT& YL, const TCHAR* Fmt, ... )
{
	TCHAR Text[4096];
	GET_VARARGS( Text, ARRAY_COUNT(Text), Fmt );

	guard(UCanvas::WrappedStrLenf);
	WrappedPrint( STY_None, XL, YL, Font, 0, Text );
	unguard;
}

//
// Wrapped printf.
//
void VARARGS UCanvas::WrappedPrintf( UFont* Font, UBOOL Center, const TCHAR* Fmt, ... )
{
	TCHAR Text[4096];
	GET_VARARGS( Text, ARRAY_COUNT(Text), Fmt );

	guard(UCanvas::WrappedPrintf);
	INT XL=0, YL=0;
	WrappedPrint( STY_Normal, XL, YL, Font, Center, Text );
	unguard;
}

/*-----------------------------------------------------------------------------
	UCanvas object functions.
-----------------------------------------------------------------------------*/

void UCanvas::Init( UViewport* InViewport )
{
	guard(UCanvas::UCanvas);
	Viewport = InViewport;
	unguard;
}
void UCanvas::Update( FSceneNode* InFrame )
{
	guard(UCanvas::Update);

	// Call UnrealScript to reset.
	eventReset();

	// Copy size parameters from viewport.
	Frame = InFrame;
	ClipX = Frame->X;
	X = (INT) ClipX;
	ClipY = Frame->Y;
	Y = (INT) ClipY;

	unguard;
}

/*-----------------------------------------------------------------------------
	UCanvas natives.
-----------------------------------------------------------------------------*/

void UCanvas::execStrLen( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execStrLen);

	P_GET_STR(InText);
	P_GET_FLOAT_REF(XL);
	P_GET_FLOAT_REF(YL);
	P_FINISH;

	INT XLi, YLi;
	FLOAT OldCurX, OldCurY;
	OldCurX = CurX;
	OldCurY = CurY;
	CurX = 0;
	CurY = 0;
	WrappedStrLenf( Font, XLi, YLi, TEXT("%s"), *InText );
	CurY = OldCurY;
	CurX = OldCurX;
	*XL = XLi;
	*YL = YLi;

	unguard;
}
IMPLEMENT_FUNCTION( UCanvas, 464, execStrLen );

void UCanvas::execDrawText( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execDrawText);
	P_GET_STR(InText);
	P_GET_UBOOL_OPTX(CR,1);
	P_GET_INT_OPTX(numChars,0);
	P_GET_UBOOL_OPTX(bCursor,0);

	P_FINISH;
	if( !Font )
	{
		Stack.Logf( TEXT("DrawText: No font") );
		return;
	}
	INT XL=0, YL=0;
	if( Style!=STY_None )
		WrappedPrint( (ERenderStyle)Style, XL, YL, Font, bCenter, *InText, numChars, bCursor );
	CurX += XL;
	CurYL = Max(CurYL,(FLOAT)YL);
	if( CR )
	{
		CurX  = 0;
		CurY += CurYL;
		CurYL = 0;
	}

	unguardexec;
}
IMPLEMENT_FUNCTION( UCanvas, 465, execDrawText );

void UCanvas::execDrawTile( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execDrawTile);
	P_GET_OBJECT(UTexture,Tex);
	P_GET_FLOAT(XL);
	P_GET_FLOAT(YL);
	P_GET_FLOAT(U);
	P_GET_FLOAT(V);
	P_GET_FLOAT(UL);
	P_GET_FLOAT(VL);
	P_FINISH;
	if( !Tex )
	{
		Stack.Logf( TEXT("DrawTile: Missing Texture") );
		return;
	}
	if( Style!=STY_None ) DrawTile
	(
		Tex,
		OrgX+CurX,
		OrgY+CurY,
		XL,
		YL,
		U,
		V,
		UL,
		VL,
		NULL,
		Z,
		Color.Plane(),
		FPlane(0,0,0,0),
		PF_TwoSided | (Style==STY_Translucent ? PF_Translucent : Style==STY_Modulated ? PF_Modulated : 0) | (bNoSmooth ? PF_NoSmooth : 0)
	);
	CurX += XL + SpaceX;
	CurYL = Max(CurYL,YL);
	unguardexec;
}
IMPLEMENT_FUNCTION( UCanvas, 466, execDrawTile );

void UCanvas::execDrawActor( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execDrawActor);
	P_GET_OBJECT(AActor, Actor);
	P_GET_UBOOL(WireFrame);
	P_GET_UBOOL_OPTX(ClearZ, 0);
	P_FINISH;

	INT OldRendMap;
	OldRendMap = Viewport->Actor->RendMap;
	if( WireFrame )
		Viewport->Actor->RendMap = REN_Wire;
	Actor->bHidden = 0;
	if( ClearZ )
		Viewport->RenDev->ClearZ( Frame );
	Render->DrawActor( Frame, Actor );
	Actor->bHidden = 1;
	Viewport->Actor->RendMap = OldRendMap;

	unguardexec;
}
IMPLEMENT_FUNCTION( UCanvas, 467, execDrawActor );

void UCanvas::execDrawClippedActor( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execDrawClippedActor);
	P_GET_OBJECT(AActor, Actor);
	P_GET_UBOOL(WireFrame);
	P_GET_INT(X);
	P_GET_INT(Y);
	P_GET_INT(XB);
	P_GET_INT(YB);
	P_GET_UBOOL_OPTX(ClearZ, 0);
	P_FINISH;
	
	INT OldX, OldY, OldXB, OldYB;
	INT OldRendMap;

	OldX = Frame->X;
	OldY = Frame->Y;
	OldXB = Frame->XB;
	OldYB = Frame->YB;

	Frame->X = X;
	Frame->Y = Y;
	Frame->XB = XB;
	Frame->YB = YB;

	FVector V(0,0,0);
	FRotator R(0,0,0);

	Frame->ComputeRenderCoords( V, R );
	Frame->ComputeRenderSize();

	OldRendMap = Viewport->Actor->RendMap;
	if (WireFrame)
		Viewport->Actor->RendMap = REN_Wire;
	Actor->bHidden = 0;
	if (ClearZ)
		Viewport->RenDev->ClearZ(Frame);
	Render->DrawActor(Frame, Actor);
	Actor->bHidden = 1;
	Viewport->Actor->RendMap = OldRendMap;
	
	Frame->X = OldX;
	Frame->Y = OldY;
	Frame->XB = OldXB;
	Frame->YB = OldYB;
	Frame->ComputeRenderSize();

	unguardexec;
}
IMPLEMENT_FUNCTION( UCanvas, 471, execDrawClippedActor );

void UCanvas::execDrawTileClipped( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execDrawTileClipped);
	P_GET_OBJECT(UTexture,Tex);
	P_GET_FLOAT(XL);
	P_GET_FLOAT(YL);
	P_GET_FLOAT(U);
	P_GET_FLOAT(V);
	P_GET_FLOAT(UL);
	P_GET_FLOAT(VL);
	P_FINISH;
	if( !Tex )
	{
		Stack.Logf( TEXT("DrawTileClipped: Missing Texture") );
		return;
	}

	// Clip to ClipX and ClipY
	if( XL > 0 && YL > 0 )
	{		
		if( CurX<0 )
			{FLOAT C=CurX*UL/XL; U-=C; UL+=C; XL+=CurX; CurX=0;}
		if( CurY<0 )
			{FLOAT C=CurY*VL/YL; V-=C; VL+=C; YL+=CurY; CurY=0;}
		if( XL>ClipX-CurX )
			{UL+=(ClipX-CurX-XL)*UL/XL; XL=ClipX-CurX;}
		if( YL>ClipY-CurY )
			{VL+=(ClipY-CurY-YL)*VL/YL; YL=ClipY-CurY;}
	
		if( Style!=STY_None ) 
			DrawTile
			(
				Tex,
				OrgX+CurX,
				OrgY+CurY,
				XL,
				YL,
				U,
				V,
				UL,
				VL,
				NULL,
				Z,
				Color.Plane(),
				FPlane(0,0,0,0),
				PF_TwoSided | (/*Style==STY_Masked ? PF_Masked :*/ Style==STY_Translucent ? PF_Translucent : Style==STY_Modulated ? PF_Modulated : 0) | (bNoSmooth ? PF_NoSmooth : 0)
			);

		CurX += XL + SpaceX;
		CurYL = Max(CurYL,YL);
	}

	unguardexec;
}
IMPLEMENT_FUNCTION( UCanvas, 468, execDrawTileClipped );

void UCanvas::execDrawTextClipped( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execDrawTextClipped);
	P_GET_STR(InText);
	P_GET_UBOOL_OPTX(CheckHotKey, 0);
	P_FINISH;

	if( !Font )
	{
		Stack.Logf( TEXT("DrawTextClipped: No font") );
		return;
	}

	if( (Font==LargeFont || Font==BigFont) && appStricmp(UObject::GetLanguage(),TEXT("INT")) )
		Font = MedFont;//BigFont;//!!
	check(Font);

	// Generate flags.
	DWORD PolyFlags
	=	(PF_NoSmooth | PF_Masked | PF_RenderHint)
	|(	(Style==STY_None       ) ? PF_Invisible
	:	(Style==STY_Translucent) ? PF_Translucent
	:	(Style==STY_Modulated  ) ? PF_Modulated
	:	                           0);

	if( !HasSafeCanvasTextRequestFloats(this) )
		return;

	FPlane DrawColor = Color.Plane();
	DrawString( PolyFlags, this, Font, (INT) CurX, (INT) CurY, *InText, DrawColor, 1, CheckHotKey );

	unguardexec;
}
IMPLEMENT_FUNCTION( UCanvas, 469, execDrawTextClipped );

void UCanvas::execTextSize( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execTextSize);
	P_GET_STR(InText);
	P_GET_FLOAT_REF(XL);
	P_GET_FLOAT_REF(YL);
	P_FINISH;

	if( !Font )
	{
		Stack.Logf( TEXT("TextSize: No font") );
		return;
	}

	INT XLi, YLi;
	FLOAT OldCurX, OldCurY, OldClipX = ClipX;
	OldCurX = CurX;
	OldCurY = CurY;
	CurX = 0;
	CurY = 0;
	ClipX = 32767;
	WrappedPrint( STY_None, XLi, YLi, Font, 0, *InText );
	CurY = OldCurY;
	CurX = OldCurX;
	ClipX = OldClipX;
	*XL = XLi;
	*YL = YLi;
	unguardexec;
}
IMPLEMENT_FUNCTION( UCanvas, 470, execTextSize );

// execDrawPortal
// Written by Andrew Scheidecker
// Edited by Brandon Reinhart
void UCanvas::execDrawPortal( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execDrawPortal);
	P_GET_INT(X);
	P_GET_INT(Y);
	P_GET_INT(Width);
	P_GET_INT(Height);
	P_GET_OBJECT(AActor,CamActor);
	P_GET_VECTOR(CamLocation);
	P_GET_ROTATOR(CamRotation);
	P_GET_INT_OPTX(FOV, 90);
	P_GET_UBOOL_OPTX(ClearZ, 1);
	P_FINISH;

	FSceneNode* NewNode;
	FScreenBounds Bounds;

	FLOAT SavedFovAngle = Viewport->Actor->FovAngle;
	Viewport->Actor->FovAngle = FOV;
	NewNode = Render->CreateMasterFrame
	(
		Viewport,
		CamLocation,
		CamRotation,
		&Bounds
	);
	check( NewNode );

	NewNode->XB = X;
	NewNode->YB = Y;
	NewNode->X = Width;
	NewNode->Y = Height;
	NewNode->ComputeRenderSize();

	if( ClearZ )
		GRenderDevice->ClearZ( NewNode );

	Render->DrawWorld( NewNode );
	Render->FinishMasterFrame();

	Viewport->Actor->FovAngle = SavedFovAngle;

	unguardexec;
}
IMPLEMENT_FUNCTION( UCanvas, 480, execDrawPortal );

IMPLEMENT_CLASS(UCanvas);

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

/*=============================================================================
	UnCanvas.cpp: Unreal canvas rendering.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Tim Sweeney
	* 31/3/99 Updated revision history - Jack Porter
=============================================================================*/

#include "EnginePrivate.h"
#include "HP2EngineServices.h"
#include "UnRender.h"
#include "NativeText.h"
#include "Hp2CanvasServices.h"
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

	// Clip and reject against the frame. The math lives in Rust (hp2-engine canvas.rs).
	Hp2CanvasTileRect Rect = { X, Y, XL, YL, U, V, UL, VL };
	if( !hp2_rs_canvas_clip_tile( Frame->FX, Frame->FY, &Rect ) )
		return;
	X = Rect.x; Y = Rect.y; XL = Rect.xl; YL = Rect.yl;
	U = Rect.u; V = Rect.v; UL = Rect.ul; VL = Rect.vl;

	// Draw it.
	FTextureInfo Info;
	if( !GIsEditor )
		Texture = Texture->Get( Viewport->CurrentTime );
	Texture->Lock( Info, Viewport->CurrentTime, -1, Viewport->RenDev );
	FLOAT UF = hp2_rs_canvas_uv_factor( Info.UScale, Info.USize, Texture->USize ); U *= UF; UL *= UF;
	FLOAT VF = hp2_rs_canvas_uv_factor( Info.VScale, Info.VSize, Texture->VSize ); V *= VF; VL *= VF;
	Viewport->RenDev->DrawTile( Frame, Info, X, Y, XL, YL, U, V, UL, VL, SpanBuffer, Z, Color, Fog, PolyFlags | (Texture->PolyFlags&PF_Masked) );
	Texture->Unlock( Info );

	unguard;
}

/*-----------------------------------------------------------------------------
	UCanvas basic text functions.
-----------------------------------------------------------------------------*/

/*-----------------------------------------------------------------------------
	UCanvas text port glue. Layout, clipping and drawing live in Rust
	(hp2-engine canvas_text.rs); these helpers copy UCanvas state into
	Hp2CanvasView, pass the Hp2CanvasServices context, and write back the
	fields the Rust side changes. Engine touch-points are in Hp2CanvasServices.cpp.
-----------------------------------------------------------------------------*/

static Hp2Plane CanvasPlane( const FPlane& P )
{
	Hp2Plane Result = { P.X, P.Y, P.Z, P.W };
	return Result;
}

// Copies at most MaxChars of a host string into the NUL-terminated UTF-16 buffer the Rust port reads.
static void CanvasTextUnits( const TCHAR* Text, TArray<UNICHAR>& Units, INT MaxChars )
{
	const INT Len = Min( appStrlen(Text), MaxChars );
	Units.Empty();
	Units.Add( Len + 1 );
	for( INT i=0; i<Len; i++ )
		Units(i) = ToUnicode( Text[i] );
	Units(Len) = 0;
}

static void FillCanvasTextView( const UCanvas* Canvas, UFont* Font, Hp2CanvasView& View )
{
	const FPlane Color = Canvas->Color.Plane();
	View.font       = Font;
	View.large_font = Canvas->LargeFont;
	View.big_font   = Canvas->BigFont;
	View.med_font   = Canvas->MedFont;
	View.cur_x      = Canvas->CurX;
	View.cur_y      = Canvas->CurY;
	View.cur_yl     = Canvas->CurYL;
	View.clip_x     = Canvas->ClipX;
	View.clip_y     = Canvas->ClipY;
	View.org_x      = Canvas->OrgX;
	View.org_y      = Canvas->OrgY;
	View.space_x    = Canvas->SpaceX;
	View.space_y    = Canvas->SpaceY;
	View.z          = Canvas->Z;
	View.x          = Canvas->X;
	View.has_frame  = Canvas->Frame ? 1 : 0;
	View.frame_x    = Canvas->Frame ? Canvas->Frame->X : 0;
	View.frame_y    = Canvas->Frame ? Canvas->Frame->Y : 0;
	View.frame_fx   = Canvas->Frame ? Canvas->Frame->FX : 0.f;
	View.frame_fy   = Canvas->Frame ? Canvas->Frame->FY : 0.f;
	View.style      = Canvas->Style;
	View.center     = Canvas->bCenter ? 1 : 0;
	View.no_smooth  = Canvas->bNoSmooth ? 1 : 0;
	View.color      = CanvasPlane( Color );
}

static void WriteBackCanvasTextView( UCanvas* Canvas, const Hp2CanvasView& View )
{
	Canvas->CurX  = View.cur_x;
	Canvas->CurY  = View.cur_y;
	Canvas->CurYL = View.cur_yl;
	Canvas->ClipX = View.clip_x;
	Canvas->ClipY = View.clip_y;
	Canvas->OrgX  = View.org_x;
	Canvas->OrgY  = View.org_y;
}

// One UCanvas text or tile call: the view copy, the DrawString texture slots and the engine context.
struct FCanvasTextCall
{
	Hp2CanvasView    View;
	FTextureInfo     Slots[HP2_CANVAS_TEXT_SLOTS];
	Hp2CanvasTextCtx Ctx;

	FCanvasTextCall( UCanvas* Canvas, UFont* Font, FFrame* Stack )
	{
		FillCanvasTextView( Canvas, Font, View );
		Ctx.services = &GHp2CanvasServices;
		Ctx.canvas   = Canvas;
		Ctx.slots    = Slots;
		Ctx.stack    = Stack;
	}
};

/*-----------------------------------------------------------------------------
	UCanvas pattern, icon and clip window. Math lives in Rust (hp2-engine canvas_text.rs);
	the tile itself is drawn by UCanvas::DrawTile through the services table.
-----------------------------------------------------------------------------*/

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
	check(Texture);
	FCanvasTextCall Call( this, NULL, NULL );
	Hp2CanvasPatternArgs Args = { Texture, X, Y, XL, YL, Scale, OrgX, OrgY, SpanBuffer, Z, CanvasPlane(Color), CanvasPlane(Fog), PolyFlags };
	hp2_rs_canvas_draw_pattern( &Call.Ctx, &Call.View, &Args );
	unguard;
}

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
	check(Texture);
	FCanvasTextCall Call( this, NULL, NULL );
	Hp2CanvasPatternArgs Args = { Texture, ScreenX, ScreenY, XSize, YSize, 1.f, 0.f, 0.f, SpanBuffer, Z, CanvasPlane(Color), CanvasPlane(Fog), PolyFlags };
	hp2_rs_canvas_draw_icon( &Call.Ctx, &Call.View, &Args );
	unguard;
}

// Clip window.
void UCanvas::SetClip( INT X, INT Y, INT XL, INT YL )
{
	guard(UCanvas::SetClip);
	FCanvasTextCall Call( this, NULL, NULL );
	hp2_rs_canvas_set_clip( &Call.View, X, Y, XL, YL );
	WriteBackCanvasTextView( this, Call.View );
	unguard;
}

static FLOAT GetCanvasTextScale( UCanvas* Canvas )
{
	if( !Canvas )
		return 1.f;
	Hp2CanvasView View;
	FillCanvasTextView( Canvas, Canvas->Font, View );
	return hp2_rs_canvas_text_scale( &View, GHp2CanvasServices.ui_scale(Canvas) );
}

static UBOOL HasSafeCanvasTextRequestFloats( UCanvas* Canvas )
{
	if( !Canvas )
		return 0;
	Hp2CanvasView View;
	FillCanvasTextView( Canvas, Canvas->Font, View );
	return hp2_rs_canvas_has_safe_request_floats( &View, GetCanvasTextScale(Canvas) ) ? 1 : 0;
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

// Services entry point (Hp2CanvasServices.cpp): one native text backend attempt.
UBOOL CanvasTryNativeTextForServices( UCanvas* Canvas, const Hp2NativeTextRequest& Req, UFont* Font, const TCHAR* Text, FPlane Color, INT& OutWidth, INT& OutHeight )
{
	return TryNativeCanvasText( Req.flags, Canvas, Font, Req.draw_x, Req.draw_y, Text, Color, Req.clip!=0, Req.amp!=0, Req.center!=0, Req.wrap!=0, Req.visible, OutWidth, OutHeight );
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
// Compute size and optionally print text with word wrap.
//!!For the next generation, redesign to ignore CurX,CurY.
//
void VARARGS UCanvas::WrappedPrint( ERenderStyle Style, INT& XL, INT& YL, UFont* Font, UBOOL Center, const TCHAR* Text, INT numChars, UBOOL bCursor )
{
	guard(UCanvas::WrappedPrint);

	FCanvasTextCall Call( this, Font, NULL );
	TArray<UNICHAR> Units;
	CanvasTextUnits( Text, Units, MAXINT );
	const Hp2CanvasSize Size = hp2_rs_canvas_wrapped_print( &Call.Ctx, &Call.View, Font, Style, Center ? 1 : 0, &Units(0), numChars );
	WriteBackCanvasTextView( this, Call.View );
	XL = Size.w;
	YL = Size.h;

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

	FCanvasTextCall Call( this, Font, &Stack );
	TArray<UNICHAR> Units;
	// WrappedStrLenf formats through a 4096-TCHAR buffer, so at most 4095 units are measured.
	CanvasTextUnits( *InText, Units, 4095 );
	const Hp2CanvasSize Size = hp2_rs_canvas_native_str_len( &Call.Ctx, &Call.View, &Units(0) );
	WriteBackCanvasTextView( this, Call.View );
	*XL = Size.w;
	*YL = Size.h;

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

	FCanvasTextCall Call( this, Font, &Stack );
	TArray<UNICHAR> Units;
	CanvasTextUnits( *InText, Units, MAXINT );
	hp2_rs_canvas_native_draw_text( &Call.Ctx, &Call.View, &Units(0), CR ? 1 : 0, numChars );
	WriteBackCanvasTextView( this, Call.View );
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

	FCanvasTextCall Call( this, Font, &Stack );
	const Hp2CanvasTileRect Rect = { 0.f, 0.f, XL, YL, U, V, UL, VL };
	hp2_rs_canvas_native_draw_tile( &Call.Ctx, &Call.View, Tex, &Rect );
	WriteBackCanvasTextView( this, Call.View );
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

	FCanvasTextCall Call( this, Font, &Stack );
	const Hp2CanvasTileRect Rect = { 0.f, 0.f, XL, YL, U, V, UL, VL };
	hp2_rs_canvas_native_draw_tile_clipped( &Call.Ctx, &Call.View, Tex, &Rect );
	WriteBackCanvasTextView( this, Call.View );
	unguardexec;
}
IMPLEMENT_FUNCTION( UCanvas, 468, execDrawTileClipped );

void UCanvas::execDrawTextClipped( FFrame& Stack, RESULT_DECL )
{
	guard(UCanvas::execDrawTextClipped);
	P_GET_STR(InText);
	P_GET_UBOOL_OPTX(CheckHotKey, 0);
	P_FINISH;

	FCanvasTextCall Call( this, Font, &Stack );
	TArray<UNICHAR> Units;
	CanvasTextUnits( *InText, Units, MAXINT );
	hp2_rs_canvas_native_draw_text_clipped( &Call.Ctx, &Call.View, &Units(0), CheckHotKey ? 1 : 0 );
	Font = (UFont*)Call.View.font;
	WriteBackCanvasTextView( this, Call.View );
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

	FCanvasTextCall Call( this, Font, &Stack );
	TArray<UNICHAR> Units;
	CanvasTextUnits( *InText, Units, MAXINT );
	Hp2CanvasSize Size = { 0, 0 };
	if( hp2_rs_canvas_native_text_size( &Call.Ctx, &Call.View, &Units(0), &Size ) )
	{
		*XL = Size.w;
		*YL = Size.h;
	}
	WriteBackCanvasTextView( this, Call.View );
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

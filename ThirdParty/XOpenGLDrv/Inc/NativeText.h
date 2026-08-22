/*=============================================================================
	NativeText.h: Platform-neutral native Canvas text backend contract.
=============================================================================*/
#pragma once

class UXOpenGLRenderDevice;
struct FCanvasTextLayout;
struct FCanvasTextRequest;

enum ENativeTextRole
{
	NTROLE_Tiny,
	NTROLE_Small,
	NTROLE_Body,
	NTROLE_BodyBold,
	NTROLE_Heading,
	NTROLE_HeadingBold,
	NTROLE_HPMenuMedium,
	NTROLE_HPMenuLarge,
	NTROLE_Console,
	NTROLE_Subtitle,
};

// Platform implementations own shaped-layout and rasterizer objects entirely.
// This boundary intentionally carries no CoreText, DirectWrite, or Pango types.
class FNativeTextPlatformBackend
{
public:
	virtual ~FNativeTextPlatformBackend() {}
	virtual UBOOL CreateLayout( const FCanvasTextRequest& Request, FCanvasTextLayout*& OutLayout ) = 0;
	virtual void DestroyLayout( FCanvasTextLayout* Layout ) = 0;
	virtual UBOOL MeasureLayout( FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight ) = 0;
	virtual UBOOL DrawLayout( UXOpenGLRenderDevice& Renderer, FSceneNode* Frame, FCanvasTextLayout* Layout ) = 0;
	virtual void Reset( UXOpenGLRenderDevice& Renderer ) = 0;
	virtual UBOOL RunRuntimeSmoke( UXOpenGLRenderDevice& Renderer, FSceneNode* Frame ) { return 0; }
};

// Runtime availability report for the platform text stack.  Available=false
// means the renderer must keep using the per-draw Boolean Canvas fallback;
// ReasonCode is a stable dotted-lowercase diagnostic string backed by static
// storage and safe to log or persist.
struct FNativeTextBackendStatus
{
	bool Available;
	const char* ReasonCode;
};

// Status-reporting factory.  Returns NULL on an unusable platform text stack,
// filling OutStatus with a stable reason code; never crashes when unavailable.
FNativeTextPlatformBackend* CreateNativeTextPlatformBackend( FNativeTextBackendStatus& OutStatus );

// Source-compatibility wrapper around the status factory.  A NULL return means
// unavailable; callers wanting the reason code use the overload above.
FNativeTextPlatformBackend* CreateNativeTextPlatformBackend();

ENativeTextRole NativeTextRoleForFont( const UFont* Font );
// These diagnostics expose facts from an opaque shaped layout so the native
// typography contract can be exercised without creating a GL window.
struct FNativeTextLayoutTestInfo
{
	INT Width;
	INT Height;
	INT UTF16Length;
	INT VisibleSourceEnd;
	INT ClusterCount;
	INT GlyphKeyCount;
	INT ResolvedFontCount;
	INT UnderlineCount;
	QWORD GlyphKeyFingerprint;
};

UBOOL NativeTextCopyUTF16ForTests( const TCHAR* Text, INT TextLength, UNICHAR* OutText, INT OutCapacity, INT& OutLength );
UBOOL NativeTextInspectLayoutForTests( const FCanvasTextLayout* Layout, FNativeTextLayoutTestInfo& OutInfo, INT* OutClusterSourceEnds, INT OutCapacity );

// The game executable drives this state while -TESTNATIVETEXT is active.  It
// is deliberately renderer-owned: a successful result means a real GL Canvas
// draw completed, rather than only a CoreText layout unit test.
enum ENativeTextRuntimeSmokeState
{
	NativeTextRuntimeSmokeIdle,
	NativeTextRuntimeSmokePending,
	NativeTextRuntimeSmokePassed,
	NativeTextRuntimeSmokeFailed,
};
enum ENativeTextRuntimeSmokeStage
{
	NativeTextRuntimeSmokeStageNone,
	NativeTextRuntimeSmokeStageFonts,
	NativeTextRuntimeSmokeStagePageDraw,
	NativeTextRuntimeSmokeStageNativeDraw,
	NativeTextRuntimeSmokeStageAtlas,
	NativeTextRuntimeSmokeStageFallback,
	NativeTextRuntimeSmokeStageReset,
	NativeTextRuntimeSmokeStageRecreate,
	NativeTextRuntimeSmokeStageFinalReset,
	NativeTextRuntimeSmokeStageGLError,
	NativeTextRuntimeSmokeStageRecreateLayout,
	NativeTextRuntimeSmokeStageRecreateMeasure,
	NativeTextRuntimeSmokeStageRecreateSubmit,
	NativeTextRuntimeSmokeStageRecreatePage,
	NativeTextRuntimeSmokeStageFallbackAllocation,
	NativeTextRuntimeSmokeStageFallbackCursor,
};

void BeginNativeTextRuntimeSmoke();
ENativeTextRuntimeSmokeState GetNativeTextRuntimeSmokeState();
ENativeTextRuntimeSmokeStage GetNativeTextRuntimeSmokeStage();
void SetNativeTextRuntimeSmokeStage( ENativeTextRuntimeSmokeStage Stage );
void CompleteNativeTextRuntimeSmoke( UBOOL Success );

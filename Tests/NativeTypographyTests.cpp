#include <stdlib.h>

#include "Engine.h"
#include "UnRenDev.h"
#include "NativeText.h"
#include "NativeTextShared.h"
#include "HP2Paths.h"
#include "FMallocAnsi.h"
#include "FFileManagerUnix.h"
#include "FFeedbackContextAnsi.h"
#include "FConfigCacheIni.h"

#include <array>
#include <cstdio>
#include <cstring>
#include <memory>
#include <limits>
#include <vector>
extern "C" { TCHAR GPackage[64] = TEXT("NativeTypographyTests"); }
INT GFilesOpen = 0;
INT GFilesOpened = 0;

namespace
{
void Check( bool Condition, const char* Message );

class FSilentLog : public FOutputDevice
{
public:
	void Serialize( const TCHAR*, EName ) override {}
};

class FCapturingError : public FOutputDeviceError
{
public:
	FCapturingError() { Message[0] = 0; }
	void Serialize( const TCHAR* Text, EName ) override
	{
		if( !Message[0] )
			appStrncpy(Message, Text, ARRAY_COUNT(Message));
		throw 1;
	}
	void HandleError() override {}
	TCHAR Message[1024];
};

FSilentLog RuntimeLog;
FCapturingError RuntimeError;
FFeedbackContextAnsi RuntimeWarn;
FFileManagerUnix RuntimeFileManager;
FMallocAnsi RuntimeMalloc;

bool InitializeRuntime( int ArgC, char** ArgV )
{
	if( !PrepareHP2Paths(ArgC, ArgV) )
	{
		std::fprintf(stderr, "native_typography: path bootstrap failed\n");
		return false;
	}
#if !_MSC_VER
	__Context::StaticInit();
	std::strncpy(GModule, ArgV[0], sizeof(GModule) - 1);
	GModule[sizeof(GModule) - 1] = 0;
#endif

	TCHAR CmdLine[2048] = {};
	for( INT Index = 1; Index < ArgC; ++Index )
	{
		const TCHAR* Argument = ANSI_TO_TCHAR(ArgV[Index]);
		if( appStrlen(CmdLine) + appStrlen(Argument) + 2 >= ARRAY_COUNT(CmdLine) )
		{
			std::fprintf(stderr, "native_typography: runtime command line is too long\n");
			return false;
		}
		if( CmdLine[0] )
			appStrcat(CmdLine, TEXT(" "));
		appStrcat(CmdLine, Argument);
	}

	GIsStarted = 1;
	GIsGuarded = 1;
	try
	{
		appInit(TEXT("NativeTypographyTests"), CmdLine, &RuntimeMalloc, &RuntimeLog, &RuntimeError,
			&RuntimeWarn, &RuntimeFileManager, FConfigCacheIni::Factory, 1);
		return true;
	}
	catch( ... )
	{
		std::fprintf(stderr, "native_typography: runtime bootstrap failed%s%s\n",
			RuntimeError.Message[0] ? ": " : "",
			RuntimeError.Message[0] ? TCHAR_TO_ANSI(RuntimeError.Message) : "");
		return false;
	}
}

UFont* CreateFontFixture( const TCHAR* Name, INT Height )
{
	UFont* Font = Cast<UFont>(UObject::StaticConstructObject(
		UFont::StaticClass(), UObject::GetTransientPackage(), NAME_None, RF_Transient));
	Check(Font != NULL, "transient font fixture allocation failed");
	if( Font )
	{
		Font->FontName = Name;
		Font->FontHeight = Height;
	}
	return Font;
}

struct FRuntimeExit
{
	~FRuntimeExit()
	{
		GIsScriptable = 0;
		GIsGuarded = 0;
		if( GIsStarted )
			appExit();
	}
};

int Failures = 0;
void Check( bool Condition, const char* Message )
{
	if( !Condition )
	{
		std::fprintf( stderr, "native_typography: %s\n", Message );
		++Failures;
	}
}

FCanvasTextLayoutRequest RequestFor( UFont& Font, const TCHAR* Text, INT Length = 0 )
{
	// TextLength is always the non-negative source span; wrapping is chosen
	// with an explicit Wrapped() request, never by a negative length.
	return FCanvasTextLayoutRequest::Computed( &Font, Text, Length > 0 ? Length : appStrlen(Text) );
}

struct FLayout
{
	FNativeTextPlatformBackend& Backend;
	FCanvasTextLayout* Value{};
	~FLayout() { if( Value ) Backend.DestroyLayout(Value); }
};

FNativeTextLayoutTestInfo Shape( FNativeTextPlatformBackend& Backend, const FCanvasTextLayoutRequest& Request, const char* Message, std::vector<INT>* Ends = NULL )
{
	FLayout Layout = { Backend, NULL };
	Check( Backend.CreateLayout(Request, Layout.Value) && Layout.Value, Message );
	FNativeTextLayoutTestInfo Info = {};
	if( !Layout.Value ) return Info;
	INT Width = 0, Height = 0;
	Check( Backend.MeasureLayout(Layout.Value, Width, Height), "created layout did not measure" );
	std::array<INT, 64> SourceEnds = {};
	Check( NativeTextInspectLayoutForTests(Layout.Value, Info, SourceEnds.data(), static_cast<INT>(SourceEnds.size())), "backend layout inspection failed" );
	Check( Width == Info.Width && Height == Info.Height, "inspection disagrees with backend measurement" );
	if( Ends ) Ends->assign(SourceEnds.begin(), SourceEnds.begin() + Info.ClusterCount);
	return Info;
}

void TestUTF16Conversion()
{
	const std::array<TCHAR, 2> Bmp = {{ static_cast<TCHAR>('A'), 0 }};
	const std::array<TCHAR, 2> NonBmp = {{ static_cast<TCHAR>(0x1f9d9u), 0 }};
	const std::array<TCHAR, 3> Invalid = {{ static_cast<TCHAR>(0xd800u), static_cast<TCHAR>(0x110000u), 0 }};
	std::array<UNICHAR, 4> Out = {};
	INT Count = 0;
	Check( NativeTextCopyUTF16ForTests(Bmp.data(), 1, Out.data(), 4, Count) && Count == 1 && Out[0] == 'A', "BMP UTF-32 conversion changed its unit" );
	Check( NativeTextCopyUTF16ForTests(NonBmp.data(), 1, Out.data(), 4, Count) && Count == 2 && Out[0] == 0xd83e && Out[1] == 0xddd9, "non-BMP UTF-32 conversion did not emit a surrogate pair" );
	Check( NativeTextCopyUTF16ForTests(Invalid.data(), 2, Out.data(), 4, Count) && Count == 2 && Out[0] == 0xfffd && Out[1] == 0xfffd, "invalid UTF-32 scalars did not become U+FFFD" );
	Check( !NativeTextCopyUTF16ForTests(NonBmp.data(), 1, Out.data(), 1, Count) && Count == 2, "UTF-16 conversion accepted a short output buffer" );
}

void TestRejectedRequests()
{
	std::unique_ptr<FNativeTextPlatformBackend> Backend(CreateNativeTextPlatformBackend());
	Check( Backend.get() != NULL, "CoreText backend was not created" );
	if( !Backend ) return;
	UFont* Font = CreateFontFixture(TEXT("Times"), 18);
	if( !Font ) return;

	const TCHAR Text[] = { 'A', 0 };
	auto Reject = [&]( const FCanvasTextLayoutRequest& Request, const char* Message )
	{
		FCanvasTextLayout* Layout = reinterpret_cast<FCanvasTextLayout*>(1);
		const UBOOL Created = Backend->CreateLayout(Request, Layout);
		Check( !Created && Layout == NULL, Message );
		if( Created && Layout )
			Backend->DestroyLayout(Layout);
	};

	// The negative-length sentinel is gone: TextLength is contractually
	// non-negative, and the backend still rejects violations defensively
	// instead of negating them into a wrapped request.
	FCanvasTextLayoutRequest Request = FCanvasTextLayoutRequest::Wrapped( Font, Text, std::numeric_limits<INT>::min() );
	Reject(Request, "negative wrapped length was negated instead of rejected");

	Request = RequestFor(*Font, Text);
	Request.TextScale = std::numeric_limits<FLOAT>::infinity();
	Reject(Request, "non-finite text scale reached CoreText");

	Request = RequestFor(*Font, Text);
	Request.TextScale = std::numeric_limits<FLOAT>::max();
	Reject(Request, "finite out-of-domain text scale reached CoreText");

	Request = RequestFor(*Font, Text);
	Request.SpaceX = std::numeric_limits<FLOAT>::infinity();
	Reject(Request, "non-finite horizontal spacing reached CoreText");

	Request = RequestFor(*Font, Text);
	Request.SpaceY = std::numeric_limits<FLOAT>::quiet_NaN();
	Reject(Request, "non-finite vertical spacing reached CoreText");

	Request = RequestFor(*Font, Text);
	Request.SpaceY = std::numeric_limits<FLOAT>::max();
	Reject(Request, "overflowing vertical spacing reached CoreText");
}

void TestShaping()
{
	std::unique_ptr<FNativeTextPlatformBackend> Backend(CreateNativeTextPlatformBackend());
	Check( Backend.get() != NULL, "CoreText backend was not created" );
	if( !Backend ) return;
	UFont* Times = CreateFontFixture(TEXT("Times"), 18);
	if( !Times ) return;

	const TCHAR Ligature[] = { 'f', 'i', 0 };
	const TCHAR Combining[] = { 'e', static_cast<TCHAR>(0x0301u), 0 };
	const TCHAR Variation[] = { static_cast<TCHAR>(0x2764u), static_cast<TCHAR>(0xfe0fu), 0 };
	const TCHAR Rtl[] = { static_cast<TCHAR>(0x05d0u), static_cast<TCHAR>(0x05d1u), 0 };
	std::vector<INT> Ends;
	const FNativeTextLayoutTestInfo LigatureInfo = Shape(*Backend, RequestFor(*Times, Ligature), "ligature layout failed", &Ends);
	Check( LigatureInfo.ClusterCount == 1 && Ends.size() == 1 && Ends[0] == 2, "ligature was split at a non-CTLine boundary" );
	const FNativeTextLayoutTestInfo CombiningInfo = Shape(*Backend, RequestFor(*Times, Combining), "combining layout failed", &Ends);
	Check( CombiningInfo.ClusterCount == 1 && Ends.size() == 1 && Ends[0] == 2, "combining sequence was split" );
	const FNativeTextLayoutTestInfo VariationInfo = Shape(*Backend, RequestFor(*Times, Variation), "variation layout failed", &Ends);
	Check( VariationInfo.ClusterCount == 1 && Ends.size() == 1 && Ends[0] == 2, "variation-selector sequence was split" );
	const FNativeTextLayoutTestInfo RtlInfo = Shape(*Backend, RequestFor(*Times, Rtl), "RTL layout failed", &Ends);
	Check( RtlInfo.ClusterCount == 2 && Ends.size() == 2 && Ends[0] == 1 && Ends[1] == 2, "RTL clusters lost source order" );

	const TCHAR Fallback[] = { 'A', static_cast<TCHAR>(0x4e2du), 0 };
	const FNativeTextLayoutTestInfo FallbackInfo = Shape(*Backend, RequestFor(*Times, Fallback), "fallback layout failed");
	Check( FallbackInfo.ResolvedFontCount >= 2, "CoreText fallback font run was not retained" );
	const TCHAR A[] = { 'A', 0 };
	UFont* Helvetica = CreateFontFixture(TEXT("Helvetica"), 18);
	if( !Helvetica ) return;
	const FNativeTextLayoutTestInfo TimesInfo = Shape(*Backend, RequestFor(*Times, A), "Times layout failed");
	const FNativeTextLayoutTestInfo HelveticaInfo = Shape(*Backend, RequestFor(*Helvetica, A), "Helvetica layout failed");
	Check( TimesInfo.GlyphKeyCount && HelveticaInfo.GlyphKeyCount && TimesInfo.GlyphKeyFingerprint != HelveticaInfo.GlyphKeyFingerprint, "glyph cache identity ignored resolved CTFont identity" );
}

void TestGlyphCoverage()
{
	std::unique_ptr<FNativeTextPlatformBackend> Backend(CreateNativeTextPlatformBackend());
	Check( Backend.get() != NULL, "text backend was not created" );
	if( !Backend ) return;
	UFont* Times = CreateFontFixture(TEXT("Times"), 18);
	if( !Times ) return;

	const TCHAR A[] = { 'A', 0 };
	FLayout Layout = { *Backend, NULL };
	Check( Backend->CreateLayout(RequestFor(*Times, A), Layout.Value) && Layout.Value, "coverage layout failed" );
	if( !Layout.Value ) return;

	const Hp2NativeText::FGlyphPlacement* Drawable = NULL;
	for( const Hp2NativeText::FGlyphPlacement& Placement : Layout.Value->Glyphs )
	{
		if( Placement.bDrawable )
		{
			Drawable = &Placement;
			break;
		}
	}
	Check( Drawable != NULL, "glyph 'A' produced no drawable placement" );
	if( !Drawable ) return;

	Hp2NativeText::FRasterizedGlyph Raster;
	Check( Hp2NativeText::RasterizeGlyph(*Drawable, Hp2NativeText::PageSize, Raster), "glyph rasterization failed" );
	Check( Raster.Width > 0 && Raster.Height > 0 &&
		Raster.Alpha.size() == static_cast<size_t>(Raster.Width) * static_cast<size_t>(Raster.Height),
		"glyph rasterization produced no coverage" );

	bool HasFullCoverage = false;
	bool HasPartialCoverage = false;
	for( BYTE Alpha : Raster.Alpha )
	{
		if( Alpha >= 250 ) HasFullCoverage = true;
		else if( Alpha > 0 ) HasPartialCoverage = true;
	}
	Check( HasFullCoverage, "glyph coverage never reached full opacity" );
	Check( HasPartialCoverage, "glyph coverage was not antialiased" );
}

void TestLayoutSemantics()
{
	std::unique_ptr<FNativeTextPlatformBackend> Backend(CreateNativeTextPlatformBackend());
	if( !Backend ) { Check(false, "CoreText backend was not created"); return; }
	UFont* Font = CreateFontFixture(TEXT("Times"), 18);
	if( !Font ) return;
	const TCHAR A[] = { 'A', 0 };
	const FNativeTextLayoutTestInfo Plain = Shape(*Backend, RequestFor(*Font, A), "plain layout failed");
	const TCHAR Markup[] = { '&', 'A', 0 };
	FCanvasTextLayoutRequest MarkupRequest = RequestFor(*Font, Markup);
	MarkupRequest.bHandleAmpersand = 1;
	const FNativeTextLayoutTestInfo Underlined = Shape(*Backend, MarkupRequest, "ampersand layout failed");
	Check( Underlined.Width == Plain.Width && Underlined.Height == Plain.Height && Underlined.UnderlineCount == 1, "ampersand markup changed advance or missed underline" );
	const TCHAR Newline[] = { 'A', '\n', 'A', 0 };
	const FNativeTextLayoutTestInfo NewlineInfo = Shape(*Backend, RequestFor(*Font, Newline), "newline layout failed");
	Check( NewlineInfo.Width == Plain.Width && NewlineInfo.Height > Plain.Height, "newline did not form an independent line" );
	const TCHAR Wrapped[] = { 'w', 'o', 'r', 'd', ' ', 'w', 'o', 'r', 'd', 0 };
	FCanvasTextLayoutRequest WrapRequest = FCanvasTextLayoutRequest::Wrapped( Font, Wrapped, appStrlen(Wrapped) );
	WrapRequest.ClipX = Plain.Width + 4;
	const FNativeTextLayoutTestInfo WrappedInfo = Shape(*Backend, WrapRequest, "wrapped layout failed");
	Check( WrappedInfo.Height > Plain.Height && WrappedInfo.ClusterCount == appStrlen(Wrapped), "wrap did not preserve all shaped source clusters" );

	// Mode, not TextLength sign or clip width alone, decides wrapping: the
	// identical positive-length request in Computed mode must stay one line.
	FCanvasTextLayoutRequest UnwrappedRequest = FCanvasTextLayoutRequest::Computed( Font, Wrapped, appStrlen(Wrapped) );
	UnwrappedRequest.ClipX = Plain.Width + 4;
	const FNativeTextLayoutTestInfo UnwrappedInfo = Shape(*Backend, UnwrappedRequest, "computed layout failed");
	Check( UnwrappedInfo.Height == Plain.Height && UnwrappedInfo.Width > WrappedInfo.Width,
		"Computed mode must stay one wide line where Wrapped mode broke" );
	const TCHAR Truncated[] = { 'A', static_cast<TCHAR>(0x1f9d9u), 'B', 0 };
	FCanvasTextLayoutRequest VisibleRequest = RequestFor(*Font, Truncated);
	VisibleRequest.VisibleSourceCharacters = 2;
	const FNativeTextLayoutTestInfo Visible = Shape(*Backend, VisibleRequest, "visible-source layout failed");
	Check( Visible.UTF16Length == 3 && Visible.VisibleSourceEnd == 2, "visible-source truncation split a non-BMP cluster" );
}
}

int main( int ArgC, char** ArgV )
{
	if( !InitializeRuntime(ArgC, ArgV) )
		return 1;
	FRuntimeExit RuntimeExit;
	TestUTF16Conversion();
	TestRejectedRequests();
	TestShaping();
	TestGlyphCoverage();
	TestLayoutSemantics();
	return Failures == 0 ? 0 : 1;
}

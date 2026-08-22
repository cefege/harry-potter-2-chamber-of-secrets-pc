#include <stdlib.h>

#include "Engine.h"
#include "UnRenDev.h"
#include "NativeText.h"
#include "HP2Paths.h"
#include "FMallocAnsi.h"
#include "FFileManagerUnix.h"
#include "FFeedbackContextAnsi.h"
#include "FConfigCacheIni.h"

#include <CommonCrypto/CommonDigest.h>
#include <array>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <filesystem>
#include <memory>
#include <limits>
#include <string>
#include <type_traits>

extern "C" { TCHAR GPackage[64] = TEXT("CanvasCompatibilityTests"); }
INT GFilesOpen = 0;
INT GFilesOpened = 0;

namespace
{
int Failures = 0;
void Check( bool Condition, const char* Message )
{
	if( !Condition ) { std::fprintf(stderr, "canvas_compatibility: %s\n", Message); ++Failures; }
}
class FSilentLog : public FOutputDevice { public: void Serialize( const TCHAR*, EName ) override {} };
class FCapturingError : public FOutputDeviceError
{
public:
	FCapturingError() { Message[0] = 0; }
	void Serialize( const TCHAR* Text, EName ) override { if( !Message[0] ) appStrncpy(Message, Text, ARRAY_COUNT(Message)); throw 1; }
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
	if( !PrepareHP2Paths(ArgC, ArgV) ) return false;
#if !_MSC_VER
	__Context::StaticInit();
	std::strncpy(GModule, ArgV[0], sizeof(GModule) - 1);
	GModule[sizeof(GModule) - 1] = 0;
#endif
	GIsStarted = 1; GIsGuarded = 1;
	try { appInit(TEXT("CanvasCompatibilityTests"), TEXT(""), &RuntimeMalloc, &RuntimeLog, &RuntimeError, &RuntimeWarn, &RuntimeFileManager, FConfigCacheIni::Factory, 1); return true; }
	catch( ... ) { return false; }
}
struct FRuntimeExit { ~FRuntimeExit() { GIsScriptable = 0; GIsGuarded = 0; if( GIsStarted ) appExit(); } };

UFont* CreateNativeFontFixture( const TCHAR* Name, INT Height )
{
	UFont* Font = Cast<UFont>(UObject::StaticConstructObject(UFont::StaticClass(), UObject::GetTransientPackage(), NAME_None, RF_Transient));
	Check(Font != NULL, "transient native font fixture allocation failed");
	if( Font ) { Font->FontName = Name; Font->FontHeight = Height; }
	return Font;
}
UFont* CreatePageBackedFontFixture()
{
	UFont* Font = Cast<UFont>(UObject::StaticConstructObject(UFont::StaticClass(), UObject::GetTransientPackage(), NAME_None, RF_Transient));
	Check(Font != NULL, "transient page-backed font fixture allocation failed");
	if( Font )
	{
		Font->FontName = TEXT("");
		Font->CharactersPerPage = 256;
		Font->Pages.AddZeroed(1);
		Font->Pages(0).Characters.AddZeroed(256);
		Font->Pages(0).Characters('A').USize = 9;
		Font->Pages(0).Characters('A').VSize = 18;
	}
	return Font;
}
FCanvasTextRequest RequestFor( UFont& Font, const TCHAR* Text, INT Length = 0 )
{
	FCanvasTextRequest Request = {};
	Request.Font = &Font; Request.Text = Text; Request.TextLength = Length ? Length : appStrlen(Text);
	Request.TextScale = 1.f; Request.ClipX = 512; Request.ClipY = 128; Request.Color = FPlane(1.f, 1.f, 1.f, 1.f);
	Request.VisibleSourceCharacters = appStrlen(Text);
	return Request;
}
struct FLayout { FNativeTextPlatformBackend& Backend; FCanvasTextLayout* Value{}; ~FLayout() { if( Value ) Backend.DestroyLayout(Value); } };
FNativeTextLayoutTestInfo Shape( FNativeTextPlatformBackend& Backend, const FCanvasTextRequest& Request, const char* Message )
{
	FLayout Layout = { Backend, NULL }; Check(Backend.CreateLayout(Request, Layout.Value) && Layout.Value, Message);
	FNativeTextLayoutTestInfo Info = {}; if( !Layout.Value ) return Info;
	INT Width = 0, Height = 0; Check(Backend.MeasureLayout(Layout.Value, Width, Height), "layout measurement failed");
	std::array<INT, 128> ClusterEnds = {};
	Check(NativeTextInspectLayoutForTests(Layout.Value, Info, ClusterEnds.data(), static_cast<INT>(ClusterEnds.size())), "layout inspection failed");
	Check(Width == Info.Width && Height == Info.Height, "measure and layout inspection differ"); return Info;
}
std::string SHA256( const std::string& Path )
{
	std::ifstream Input(Path, std::ios::binary); if( !Input ) return std::string();
	CC_SHA256_CTX Context; CC_SHA256_Init(&Context); std::array<char, 65536> Buffer = {};
	while( Input.good() ) { Input.read(Buffer.data(), static_cast<std::streamsize>(Buffer.size())); const std::streamsize Count = Input.gcount(); if( Count > 0 ) CC_SHA256_Update(&Context, Buffer.data(), static_cast<CC_LONG>(Count)); }
	std::array<unsigned char, CC_SHA256_DIGEST_LENGTH> Digest = {}; CC_SHA256_Final(Digest.data(), &Context);
	static const char Hex[] = "0123456789abcdef"; std::string Result(CC_SHA256_DIGEST_LENGTH * 2, '0');
	for( size_t Index = 0; Index < Digest.size(); ++Index ) { Result[Index * 2] = Hex[Digest[Index] >> 4]; Result[Index * 2 + 1] = Hex[Digest[Index] & 15]; }
	return Result;
}
void TestImmutableFontPackages( const std::filesystem::path& RepositoryRoot )
{
	const std::array<std::pair<const char*, const char*>, 3> Packages = {{
		{ "HarryPotter2/Unreal/System/Engine.u", "c75c3e849bd07a3a315d3e87f24bb5d8960b5ce32f8fc1438d2c1c3906f93182" },
		{ "HarryPotter2/Unreal/System/HGame.u", "6c996cd90d7841cba6ea1bc5edece67471307ff47fc6d856774106b01afc22a2" },
		{ "HarryPotter2/Unreal/System/UWindow.u", "711656cdfe4ed45ca29cec4e87748b2ef668c675b25a1480619a7f784cccc94a" },
	}};
	for( const auto& Package : Packages ) { const std::string Actual = SHA256((RepositoryRoot / Package.first).string()); Check(!Actual.empty(), "immutable package fixture is unavailable"); if( !Actual.empty() ) Check(Actual == Package.second, "native text changed an immutable font package"); }
}
void TestCanvasPolicyAndCursorContract( FNativeTextPlatformBackend& Backend )
{
	UFont* PageFont = CreatePageBackedFontFixture();
	UFont* NativeFont = CreateNativeFontFixture(TEXT("Times"), 18);
	if( !PageFont || !NativeFont ) return;
	const TCHAR Text[] = { 'A', 0 };
	FCanvasNativeTextTestState State = {};
	State.NativeText = 0; State.Request = RequestFor(*PageFont, Text); State.CurX = 9; State.CurY = 11; State.CurYL = 3;
	INT Width = 0, Height = 0;
	Check(!RunCanvasNativeTextCompatibilityForTests(&Backend, State, Width, Height) && State.CurX == 9 && State.CurY == 11 && State.CurYL == 3,
		"NativeText=False did not retain the page-backed Canvas path and cursor");

	State.Request = RequestFor(*NativeFont, Text); State.CurX = 9; State.CurY = 11; State.CurYL = 3;
	Check(RunCanvasNativeTextCompatibilityForTests(&Backend, State, Width, Height) && Width > 0 && Height > 0 && State.CurX == 9 + Width && State.CurY == 11 && State.CurYL >= Height,
		"NativeText=False did not dispatch an inherently native font safely");

	FCanvasNativeTextTestState UnavailableState = {};
	UnavailableState.NativeText = 0; UnavailableState.Request = RequestFor(*NativeFont, Text);
	UnavailableState.CurX = 9; UnavailableState.CurY = 11; UnavailableState.CurYL = 3;
	Width = Height = 0;
	Check(!RunCanvasNativeTextCompatibilityForTests(NULL, UnavailableState, Width, Height) && Width == 0 && Height == 0 &&
		UnavailableState.CurX == 9 && UnavailableState.CurY == 11 && UnavailableState.CurYL == 3,
		"unavailable native backend changed Canvas dimensions or cursor");

	State.NativeText = 1; State.Request = RequestFor(*PageFont, Text); State.CurX = 9; State.CurY = 11; State.CurYL = 3;
	Check(RunCanvasNativeTextCompatibilityForTests(&Backend, State, Width, Height) && Width > 0 && Height > 0 && State.CurX == 9 + Width && State.CurY == 11 && State.CurYL >= Height,
		"native DrawText did not use shaped metrics for cursor advance");
	State.bCR = 1; State.CurX = 9; State.CurY = 11; State.CurYL = 0;
	Check(RunCanvasNativeTextCompatibilityForTests(&Backend, State, Width, Height) && State.CurX == 0 && State.CurY == 11 + Height && State.CurYL == 0,
		"native DrawText CR did not use the measured line height");
	State.bCR = 0; State.bClipped = 1; State.CurX = 9; State.CurY = 11; State.CurYL = 3;
	Check(RunCanvasNativeTextCompatibilityForTests(&Backend, State, Width, Height) && State.CurX == 9 && State.CurY == 11 && State.CurYL == 3,
		"DrawTextClipped changed Canvas cursor state");
}
void TestCanvasLayoutContract( FNativeTextPlatformBackend& Backend )
{
	UFont* Font = CreateNativeFontFixture(TEXT("Times"), 18); if( !Font ) return;
	const TCHAR PlainText[] = { 'A', ' ', 'B', 0 };
	const FNativeTextLayoutTestInfo Plain = Shape(Backend, RequestFor(*Font, PlainText), "plain Canvas layout failed");
	static_assert(
		std::is_same<decltype(FCanvasTextRequest::OriginX), FLOAT>::value &&
		std::is_same<decltype(FCanvasTextRequest::OriginY), FLOAT>::value &&
		std::is_same<decltype(FCanvasTextRequest::ClipX), FLOAT>::value &&
		std::is_same<decltype(FCanvasTextRequest::ClipY), FLOAT>::value,
		"Canvas native bounds must remain floating point");
	FCanvasTextRequest FractionalRequest = RequestFor(*Font, PlainText);
	FractionalRequest.OriginX = 37.25f; FractionalRequest.OriginY = -19.5f;
	FractionalRequest.ClipX = 511.75f; FractionalRequest.ClipY = 127.5f;
	const FNativeTextLayoutTestInfo Fractional = Shape(Backend, FractionalRequest, "finite fractional Canvas bounds were rejected");
	Check(Fractional.Width == Plain.Width && Fractional.Height == Plain.Height, "fractional Canvas bounds changed text metrics");

	auto RejectBounds = [&]( const FCanvasTextRequest& Request, const char* Message )
	{
		FCanvasTextLayout* Layout = reinterpret_cast<FCanvasTextLayout*>(1);
		const UBOOL Created = Backend.CreateLayout(Request, Layout);
		Check(!Created && Layout == NULL, Message);
		if( Created && Layout )
			Backend.DestroyLayout(Layout);
	};
	FCanvasTextRequest InvalidBounds = RequestFor(*Font, PlainText);
	InvalidBounds.OriginX = std::numeric_limits<FLOAT>::infinity();
	RejectBounds(InvalidBounds, "non-finite Canvas origin reached CoreText");
	InvalidBounds = RequestFor(*Font, PlainText);
	InvalidBounds.OriginY = std::numeric_limits<FLOAT>::quiet_NaN();
	RejectBounds(InvalidBounds, "NaN Canvas origin reached CoreText");
	InvalidBounds = RequestFor(*Font, PlainText);
	InvalidBounds.ClipX = std::numeric_limits<FLOAT>::max();
	RejectBounds(InvalidBounds, "finite out-of-domain Canvas clip reached CoreText");
	InvalidBounds = RequestFor(*Font, PlainText);
	InvalidBounds.ClipY = -std::numeric_limits<FLOAT>::max();
	RejectBounds(InvalidBounds, "negative out-of-domain Canvas clip reached CoreText");
	FCanvasTextRequest SpacedRequest = RequestFor(*Font, PlainText); SpacedRequest.SpaceX = 3.f;
	const FNativeTextLayoutTestInfo Spaced = Shape(Backend, SpacedRequest, "spaced Canvas layout failed");
	Check(Spaced.Width > Plain.Width && Spaced.Height == Plain.Height, "Canvas SpaceX was not included in shaped advance");
	FCanvasTextRequest CenteredRequest = RequestFor(*Font, PlainText); CenteredRequest.bCenter = 1; CenteredRequest.StartX = 200;
	const FNativeTextLayoutTestInfo Centered = Shape(Backend, CenteredRequest, "centered Canvas layout failed");
	Check(Centered.Width == Plain.Width && Centered.Height == Plain.Height, "centering changed Canvas text metrics");
	FCanvasTextRequest ClippedRequest = RequestFor(*Font, PlainText); ClippedRequest.bClip = 1; ClippedRequest.OriginX = 37; ClippedRequest.OriginY = 19; ClippedRequest.StartX = -11; ClippedRequest.StartY = -5; ClippedRequest.ClipX = 9; ClippedRequest.ClipY = 7;
	const FNativeTextLayoutTestInfo Clipped = Shape(Backend, ClippedRequest, "clipped Canvas layout failed");
	const FNativeTextLayoutTestInfo PlainAgain = Shape(Backend, RequestFor(*Font, PlainText), "post-clip Canvas layout failed");
	Check(Clipped.Width == Plain.Width && Clipped.Height == Plain.Height && PlainAgain.Width == Plain.Width && PlainAgain.Height == Plain.Height, "local clipping changed or leaked Canvas cursor metrics");
	const TCHAR Ampersand[] = { '&', 'A', 0 }; const TCHAR EscapedAmpersand[] = { '&', '&', 0 };
	FCanvasTextRequest AmpersandRequest = RequestFor(*Font, Ampersand); AmpersandRequest.bHandleAmpersand = 1;
	const FNativeTextLayoutTestInfo Underlined = Shape(Backend, AmpersandRequest, "ampersand Canvas layout failed");
	const FNativeTextLayoutTestInfo A = Shape(Backend, RequestFor(*Font, TEXT("A")), "single character Canvas layout failed");
	FCanvasTextRequest EscapedRequest = RequestFor(*Font, EscapedAmpersand); EscapedRequest.bHandleAmpersand = 1;
	const FNativeTextLayoutTestInfo Escaped = Shape(Backend, EscapedRequest, "escaped ampersand Canvas layout failed");
	const FNativeTextLayoutTestInfo Literal = Shape(Backend, RequestFor(*Font, TEXT("&")), "literal ampersand Canvas layout failed");
	Check(Underlined.Width == A.Width && Underlined.UnderlineCount == 1, "ampersand markup changed advance or omitted underline");
	Check(Escaped.Width == Literal.Width && Escaped.UnderlineCount == 0, "&& did not produce exactly one literal ampersand");
	const TCHAR WrappedText[] = { 'w', 'o', 'r', 'd', ' ', 'w', 'o', 'r', 'd', 0 };
	FCanvasTextRequest WrappedRequest = RequestFor(*Font, WrappedText, -appStrlen(WrappedText)); WrappedRequest.ClipX = Plain.Width + 4;
	const FNativeTextLayoutTestInfo Wrapped = Shape(Backend, WrappedRequest, "wrapped Canvas layout failed");
	const TCHAR NewlineText[] = { 'A', '\n', 'A', 0 }; const FNativeTextLayoutTestInfo Newline = Shape(Backend, RequestFor(*Font, NewlineText), "newline Canvas layout failed");
	Check(Wrapped.Height > Plain.Height && Newline.Height > A.Height, "wrapped/newline Canvas layout did not create independent lines");
	const TCHAR Teletype[] = { 'A', static_cast<TCHAR>(0x1f9d9u), 'B', 0 }; FCanvasTextRequest VisibleRequest = RequestFor(*Font, Teletype); VisibleRequest.VisibleSourceCharacters = 2;
	const FNativeTextLayoutTestInfo Visible = Shape(Backend, VisibleRequest, "visible-source Canvas layout failed");
	Check(Visible.VisibleSourceEnd == 2 && Visible.UTF16Length == 3, "numChars did not stop at a legal shaped source cluster");
}
}
int main( int ArgC, char** ArgV )
{
	const std::filesystem::path RepositoryRoot = std::filesystem::current_path();
	if( !InitializeRuntime(ArgC, ArgV) ) return 1; FRuntimeExit RuntimeExit;
	std::unique_ptr<FNativeTextPlatformBackend> Backend(CreateNativeTextPlatformBackend()); Check(Backend.get() != NULL, "CoreText backend was not created");
	if( Backend ) { TestCanvasPolicyAndCursorContract(*Backend); TestCanvasLayoutContract(*Backend); } TestImmutableFontPackages(RepositoryRoot); return Failures == 0 ? 0 : 1;
}

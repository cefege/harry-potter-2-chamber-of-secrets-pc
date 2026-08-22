/*=============================================================================
	AbiTests.cpp: arm64 host/package ABI and static-registration contracts.
=============================================================================*/

#include <stdlib.h>

#include "Engine.h"
#include "Render.h"
#include "UnCon.h"
#include "UnEngineNative.h"
#include "SDLDrv.h"
#include "HP2Paths.h"
#include "HP2StaticPackages.h"
#include "FMallocAnsi.h"
#include "FFileManagerUnix.h"
#include "FFeedbackContextAnsi.h"
#include "FConfigCacheIni.h"

#include <cstddef>
#include <cstdarg>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <new>
#include <type_traits>

// Keep the pre-appInit path bootstrap in this executable's sole translation
// unit rather than requiring a separate test-only CMake source edge.
#include "HP2Paths.cpp"


extern "C" { TCHAR GPackage[64] = TEXT("AbiTests"); }
INT GFilesOpen = 0;
INT GFilesOpened = 0;


extern CORE_API INT GNativeDuplicate;
extern UObjectNativeInfo GCoreUObjectNatives[];
extern UCommandletNativeInfo GCoreUCommandletNatives[];
extern AActorNativeInfo GEngineAActorNatives[];
extern APawnNativeInfo GEngineAPawnNatives[];
extern APlayerPawnNativeInfo GEngineAPlayerPawnNatives[];
extern ADecalNativeInfo GEngineADecalNatives[];
extern AStatLogNativeInfo GEngineAStatLogNatives[];
extern AStatLogFileNativeInfo GEngineAStatLogFileNatives[];
extern AZoneInfoNativeInfo GEngineAZoneInfoNatives[];
extern AWarpZoneInfoNativeInfo GEngineAWarpZoneInfoNatives[];
extern ALevelInfoNativeInfo GEngineALevelInfoNatives[];
extern AGameInfoNativeInfo GEngineAGameInfoNatives[];
extern ANavigationPointNativeInfo GEngineANavigationPointNatives[];
extern UCanvasNativeInfo GEngineUCanvasNatives[];
extern UConsoleNativeInfo GEngineUConsoleNatives[];
extern UScriptedTextureNativeInfo GEngineUScriptedTextureNatives[];

namespace
{
	const char* GTestName = "abi";

	int Fail(const char* Format, ...)
	{
		std::fprintf(stderr, "%s: ", GTestName);
		va_list Args;
		va_start(Args, Format);
		std::vfprintf(stderr, Format, Args);
		va_end(Args);
		std::fputc('\n', stderr);
		return 1;
	}

	int CheckWidth(const char* Name, std::size_t Actual, std::size_t Expected)
	{
		return Actual == Expected ? 0 : Fail("sizeof(%s): expected %zu, got %zu", Name, Expected, Actual);
	}

	struct FCppPropertyProbe
	{
		BYTE Prefix;
		void* HostPointer;
		TCHAR HostText;
		UNICHAR WireText;
		INT Tail;
	};

	INT ProbeCppPropertyOffset()
	{
		struct FCapture
		{
			static INT Offset(ECppProperty, INT InOffset) { return InOffset; }
		};
		return FCapture::Offset(EC_CppProperty, static_cast<INT>(offsetof(FCppPropertyProbe, HostPointer)));
	}

	int TestAbiWidths()
	{
		GTestName = "abi_widths";
		if (int Result = CheckWidth("BYTE", sizeof(BYTE), 1)) return Result;
		if (int Result = CheckWidth("SBYTE", sizeof(SBYTE), 1)) return Result;
		if (int Result = CheckWidth("_WORD", sizeof(_WORD), 2)) return Result;
		if (int Result = CheckWidth("SWORD", sizeof(SWORD), 2)) return Result;
		if (int Result = CheckWidth("UNICHAR", sizeof(UNICHAR), 2)) return Result;
		if (int Result = CheckWidth("INT", sizeof(INT), 4)) return Result;
		if (int Result = CheckWidth("NAME_INDEX", sizeof(NAME_INDEX), 4)) return Result;
		if (int Result = CheckWidth("FName", sizeof(FName), sizeof(NAME_INDEX))) return Result;
		if (int Result = CheckWidth("DWORD", sizeof(DWORD), 4)) return Result;
		if (int Result = CheckWidth("LONG", sizeof(LONG), 4)) return Result;
		if (int Result = CheckWidth("FLOAT", sizeof(FLOAT), 4)) return Result;
		if (int Result = CheckWidth("QWORD", sizeof(QWORD), 8)) return Result;
		if (int Result = CheckWidth("SQWORD", sizeof(SQWORD), 8)) return Result;
		if (int Result = CheckWidth("void*", sizeof(void*), 8)) return Result;
		if (int Result = CheckWidth("PTRINT", sizeof(PTRINT), sizeof(void*))) return Result;
		if (int Result = CheckWidth("UPTRINT", sizeof(UPTRINT), sizeof(void*))) return Result;
		if (int Result = CheckWidth("SIZE_T", sizeof(SIZE_T), sizeof(void*))) return Result;
		if (int Result = CheckWidth("TCHAR", sizeof(TCHAR), 4)) return Result;
		if (!std::is_signed<INT>::value) return Fail("INT must be signed");
		if (!std::is_signed<PTRINT>::value) return Fail("PTRINT must be signed");
		if (std::is_signed<UPTRINT>::value) return Fail("UPTRINT must be unsigned");
		if (sizeof(TCHAR) == sizeof(UNICHAR)) return Fail("host TCHAR must be distinct from wire UNICHAR");
		if (alignof(FCppPropertyProbe) != alignof(void*))
			return Fail("non-reflected packing leaked: probe alignment expected %zu, got %zu", alignof(void*), alignof(FCppPropertyProbe));
		if (alignof(AActor) != 4 || alignof(UClient) != alignof(void*))
			return Fail("packing boundary changed: generated AActor=%zu host UClient=%zu", alignof(AActor), alignof(UClient));
		const INT CppOffset = ProbeCppPropertyOffset();
		const INT BuiltinOffset = static_cast<INT>(offsetof(FCppPropertyProbe, HostPointer));
		if (CppOffset != BuiltinOffset)
			return Fail("CPP_PROPERTY host offset expected %i, got %i", BuiltinOffset, CppOffset);
		if (CppOffset != 8)
			return Fail("CPP_PROPERTY pointer-aware offset expected 8, got %i", CppOffset);
		if (offsetof(FCppPropertyProbe, HostText) != 16 || offsetof(FCppPropertyProbe, WireText) != 20)
			return Fail("host/wire member separation changed: TCHAR offset=%zu UNICHAR offset=%zu",
				offsetof(FCppPropertyProbe, HostText), offsetof(FCppPropertyProbe, WireText));
		return 0;
	}

	int TestRenderClipClassification()
	{
		GTestName = "render_clip";
		const FLOAT Epsilon = 0.000001f;
		if (RenderClipEdgeCrosses(-0.0f, 0.0f)
			|| RenderClipEdgeCrosses(0.0f, -0.0f))
			return Fail("signed zero must describe one clipping-plane side");
		if (!RenderClipEdgeCrosses(-Epsilon, Epsilon)
			|| !RenderClipEdgeCrosses(Epsilon, -Epsilon))
			return Fail("opposite clipping-plane sides did not produce an edge crossing");
		if (RenderClipEdgeCrosses(-Epsilon, -Epsilon)
			|| RenderClipEdgeCrosses(Epsilon, Epsilon))
			return Fail("matching clipping-plane sides produced a false edge crossing");
		return 0;
	}
	int TestProjectionFov()
	{
		GTestName = "projection_fov";
		const FLOAT AuthoredFov = 90.f;
		if (UClient::GetEffectiveFovAngle(AuthoredFov, 800, 600, 1) != AuthoredFov)
			return Fail("4:3 projection changed from authored FOV");
		if (UClient::GetEffectiveFovAngle(AuthoredFov, 1920, 1080, 0) != AuthoredFov)
			return Fail("disabled vertical-FOV maintenance changed legacy projection");
		if (UClient::GetEffectiveFovAngle(AuthoredFov, 1280, 1024, 1) != AuthoredFov)
			return Fail("narrow aspect projection changed from authored FOV");

		const FLOAT WideFov = UClient::GetEffectiveFovAngle(AuthoredFov, 1920, 1080, 1);
		const FLOAT WideError = WideFov>106.2602f ? WideFov-106.2602f : 106.2602f-WideFov;
		if (WideError > 0.001f)
			return Fail("16:9 90-degree projection expected 106.2602, got %.6f", static_cast<double>(WideFov));

		const FLOAT VerticalScale = appTan(WideFov * PI/360.f) / (4.f/3.f);
		const FLOAT VerticalError = VerticalScale>1.f ? VerticalScale-1.f : 1.f-VerticalScale;
		if (VerticalError > 0.0001f)
			return Fail("16:9 projection did not retain the authored vertical view");
		if (UClient::GetEffectiveFovAngle(160.f, 7680, 1080, 1) >= 170.f)
			return Fail("ultrawide projection exceeded the safe FOV limit");
		if (UGameEngine::GetConsoleUIScale(1920.f, 1080.f, 1.f) != 2.25f)
			return Fail("16:9 menu UI did not fit the authored 640x480 height");
		if (UGameEngine::GetConsoleUIScale(1280.f, 1024.f, 1.f) != 2.f)
			return Fail("5:4 menu UI did not fit the authored 640x480 width");
		return 0;
	}
	int TestCommandLineLoad()
	{
		GTestName = "command_line_load";
		static const TCHAR* InvalidCommandLines[] =
		{
			TEXT("Startup.unr -SAVESLOT=0"),
			TEXT("Startup.unr -LOAD="),
			TEXT("Startup.unr -LOAD=-1"),
			TEXT("Startup.unr -LOAD=+1"),
			TEXT("Startup.unr -LOAD=12x"),
			TEXT("Startup.unr -LOAD=2147483648"),
			TEXT("Startup.unr -NOT-LOAD=0"),
			TEXT("\"-LOAD=0")
		};
		for (INT Index = 0; Index < ARRAY_COUNT(InvalidCommandLines); ++Index)
		{
			INT Slot = 12345;
			if (appConsumeCommandLineLoadSlot(InvalidCommandLines[Index], Slot))
				return Fail("malformed or absent load argument %i was accepted", Index);
			if (Slot != 12345)
				return Fail("rejected load argument %i changed the output slot", Index);
		}

		INT Slot = INDEX_NONE;
		if (!appConsumeCommandLineLoadSlot(TEXT("Startup.unr -LOAD=0 -SAVESLOT=0"), Slot))
			return Fail("valid map-first load argument was not consumed");
		if (Slot != 0)
			return Fail("valid load argument expected slot 0, got %i", Slot);
		TCHAR LoadURL[32];
		if (!appFormatLoadGameURL(0, LoadURL, ARRAY_COUNT(LoadURL))
			|| appStrcmp(LoadURL, TEXT("?load=0")) != 0
			|| !appFormatLoadGameURL(17, LoadURL, ARRAY_COUNT(LoadURL))
			|| appStrcmp(LoadURL, TEXT("?load=17")) != 0)
			return Fail("load-game travel must use the relative ?load=<slot> FURL");

		TCHAR FileToken[128];
		if (!appFilePathToFURLToken(
				TEXT("/tmp/Profile/Save0.usa"), FileToken, ARRAY_COUNT(FileToken))
			|| appStrcmp(FileToken, TEXT("\\tmp\\Profile\\Save0.usa")) != 0)
			return Fail("absolute native file path was not encoded as an FURL token");
		if (!appFilePathToFURLToken(
				TEXT("../Maps/Entry.unr?Name=Player"), FileToken, ARRAY_COUNT(FileToken))
			|| appStrcmp(FileToken, TEXT("..\\Maps\\Entry.unr?Name=Player")) != 0)
			return Fail("map path conversion changed FURL options");
		if (appFilePathToFURLToken(
				TEXT("unreal://example.invalid/Map"), FileToken, ARRAY_COUNT(FileToken)))
			return Fail("network URL was accepted by the file-path FURL boundary");
		if (appStrcmp(
				appPathLeaf(TEXT("..\\Maps/Entryhall_hub.unr")),
				TEXT("Entryhall_hub.unr")) != 0)
			return Fail("path leaf did not recognize both separator styles");

		Slot = 12345;
		if (appConsumeCommandLineLoadSlot(TEXT("Startup.unr -LOAD=7"), Slot))
			return Fail("a second load argument was consumed in the same process");
		if (Slot != 12345)
			return Fail("re-consumption changed the output slot");
		return 0;
	}


	class FFixedArchive : public FArchive
	{
	public:
		FFixedArchive()
		: Position(0), Length(0)
		{
			ArIsSaving = 1;
			ArIsPersistent = 1;
			std::memset(Bytes, 0, sizeof(Bytes));
		}

		FFixedArchive(const BYTE* Source, INT SourceLength)
		: Position(0), Length(SourceLength)
		{
			ArIsLoading = 1;
			ArIsPersistent = 1;
			std::memset(Bytes, 0, sizeof(Bytes));
			if (SourceLength < 0 || SourceLength > static_cast<INT>(sizeof(Bytes)))
			{
				ArIsError = 1;
				Length = 0;
			}
			else if (SourceLength)
			{
				std::memcpy(Bytes, Source, static_cast<std::size_t>(SourceLength));
			}
		}

		void Serialize(void* Data, INT Count) override
		{
			if (Count < 0)
			{
				ArIsError = 1;
				return;
			}
			const INT Limit = ArIsLoading ? Length : static_cast<INT>(sizeof(Bytes));
			if (Position < 0 || Position > Limit || Count > Limit - Position)
			{
				ArIsError = 1;
				if (ArIsLoading && Count > 0)
					std::memset(Data, 0, static_cast<std::size_t>(Count));
				Position = Limit;
				return;
			}
			if (Count)
			{
				if (ArIsLoading)
					std::memcpy(Data, Bytes + Position, static_cast<std::size_t>(Count));
				else
					std::memcpy(Bytes + Position, Data, static_cast<std::size_t>(Count));
			}
			Position += Count;
			if (ArIsSaving && Position > Length)
				Length = Position;
		}

		INT Tell() override { return Position; }
		INT TotalSize() override { return Length; }
		void Seek(INT NewPosition) override
		{
			const INT Limit = ArIsLoading ? Length : static_cast<INT>(sizeof(Bytes));
			if (NewPosition < 0 || NewPosition > Limit)
				ArIsError = 1;
			else
				Position = NewPosition;
		}

		const BYTE* Data() const { return Bytes; }
		INT Size() const { return Length; }

	private:
		BYTE Bytes[256];
		INT Position;
		INT Length;
	};

	int CheckBytes(const char* CaseName, const BYTE* Actual, INT ActualSize, const BYTE* Expected, INT ExpectedSize)
	{
		if (ActualSize != ExpectedSize)
			return Fail("%s size expected %i, got %i", CaseName, ExpectedSize, ActualSize);
		for (INT Index = 0; Index < ExpectedSize; ++Index)
			if (Actual[Index] != Expected[Index])
				return Fail("%s byte %i expected 0x%02x, got 0x%02x", CaseName, Index,
					static_cast<unsigned>(Expected[Index]), static_cast<unsigned>(Actual[Index]));
		return 0;
	}

	int CheckCompact(INT Value, const BYTE* Expected, INT ExpectedSize)
	{
		FFixedArchive Save;
		INT SavedValue = Value;
		Save << AR_INDEX(SavedValue);
		char CaseName[64];
		std::snprintf(CaseName, sizeof(CaseName), "compact %i encode", Value);
		if (Save.IsError()) return Fail("%s archive error", CaseName);
		if (SavedValue != Value) return Fail("%s mutated value to %i", CaseName, SavedValue);
		if (int Result = CheckBytes(CaseName, Save.Data(), Save.Size(), Expected, ExpectedSize)) return Result;

		FFixedArchive Load(Expected, ExpectedSize);
		INT LoadedValue = 0x13572468;
		Load << AR_INDEX(LoadedValue);
		if (Load.IsError()) return Fail("compact %i decode archive error", Value);
		if (Load.Tell() != ExpectedSize) return Fail("compact %i decode consumed %i of %i bytes", Value, Load.Tell(), ExpectedSize);
		if (LoadedValue != Value) return Fail("compact decode expected %i, got %i", Value, LoadedValue);
		return 0;
	}

	bool IsCanonicalCompact(const BYTE* Bytes, INT Size)
	{
		FFixedArchive Load(Bytes, Size);
		INT Value = 0;
		Load << AR_INDEX(Value);
		if (Load.IsError() || Load.Tell() != Size)
			return false;
		FFixedArchive Save;
		Save << AR_INDEX(Value);
		return !Save.IsError() && Save.Size() == Size && std::memcmp(Save.Data(), Bytes, static_cast<std::size_t>(Size)) == 0;
	}

	int TestCompactIndex()
	{
		GTestName = "compact_index";
		static const BYTE Z[] = {0x00};
		static const BYTE P63[] = {0x3f};
		static const BYTE N63[] = {0xbf};
		static const BYTE P64[] = {0x40, 0x01};
		static const BYTE N64[] = {0xc0, 0x01};
		static const BYTE P8191[] = {0x7f, 0x7f};
		static const BYTE N8191[] = {0xff, 0x7f};
		static const BYTE P8192[] = {0x40, 0x80, 0x01};
		static const BYTE N8192[] = {0xc0, 0x80, 0x01};
		if (int Result = CheckCompact(0, Z, ARRAY_COUNT(Z))) return Result;
		if (int Result = CheckCompact(63, P63, ARRAY_COUNT(P63))) return Result;
		if (int Result = CheckCompact(-63, N63, ARRAY_COUNT(N63))) return Result;
		if (int Result = CheckCompact(64, P64, ARRAY_COUNT(P64))) return Result;
		if (int Result = CheckCompact(-64, N64, ARRAY_COUNT(N64))) return Result;
		if (int Result = CheckCompact(8191, P8191, ARRAY_COUNT(P8191))) return Result;
		if (int Result = CheckCompact(-8191, N8191, ARRAY_COUNT(N8191))) return Result;
		if (int Result = CheckCompact(8192, P8192, ARRAY_COUNT(P8192))) return Result;
		if (int Result = CheckCompact(-8192, N8192, ARRAY_COUNT(N8192))) return Result;

		static const BYTE NegativeZero[] = {0x80};
		static const BYTE LongZero[] = {0x40, 0x00};
		static const BYTE Overflow[] = {0x40, 0x80, 0x80, 0x80, 0x10};
		if (IsCanonicalCompact(NegativeZero, ARRAY_COUNT(NegativeZero))) return Fail("negative-zero encoding was accepted");
		if (IsCanonicalCompact(LongZero, ARRAY_COUNT(LongZero))) return Fail("noncanonical long-zero encoding was accepted");
		if (IsCanonicalCompact(Overflow, ARRAY_COUNT(Overflow))) return Fail("overflow encoding was accepted");

		static const BYTE Truncated1[] = {0x40};
		static const BYTE Truncated2[] = {0x40, 0x80};
		static const BYTE Truncated3[] = {0x40, 0x80, 0x80};
		static const BYTE Truncated4[] = {0x40, 0x80, 0x80, 0x80};
		if (IsCanonicalCompact(Truncated1, ARRAY_COUNT(Truncated1))) return Fail("truncated compact index at offset 1 was accepted");
		if (IsCanonicalCompact(Truncated2, ARRAY_COUNT(Truncated2))) return Fail("truncated compact index at offset 2 was accepted");
		if (IsCanonicalCompact(Truncated3, ARRAY_COUNT(Truncated3))) return Fail("truncated compact index at offset 3 was accepted");
		if (IsCanonicalCompact(Truncated4, ARRAY_COUNT(Truncated4))) return Fail("truncated compact index at offset 4 was accepted");
		return 0;
	}

	int SaveStringAndCheck(const char* CaseName, const TCHAR* Text, const BYTE* Expected, INT ExpectedSize)
	{
		FString Value(Text);
		FFixedArchive Save;
		Save << Value;
		if (Save.IsError()) return Fail("%s save archive error", CaseName);
		return CheckBytes(CaseName, Save.Data(), Save.Size(), Expected, ExpectedSize);
	}

	int TestFStringArchive()
	{
		GTestName = "fstring_archive";
		FMallocAnsi Allocator;
		FMalloc* PreviousMalloc = GMalloc;
		Allocator.Init();
		GMalloc = &Allocator;
		int Result = 0;
		{
			static const BYTE Ansi[] = {0x04, 0x48, 0x50, 0x32, 0x00};
			static const BYTE Utf16[] = {0x83, 0x41, 0x00, 0xa9, 0x03, 0x00, 0x00};
			static const BYTE NonBmp[] = {0x83, 0x3d, 0xd8, 0x42, 0xde, 0x00, 0x00};
			if (!Result) Result = SaveStringAndCheck("ANSI FString", TEXT("HP2"), Ansi, ARRAY_COUNT(Ansi));
			if (!Result) Result = SaveStringAndCheck("UTF-16LE FString", TEXT("A\u03a9"), Utf16, ARRAY_COUNT(Utf16));
			if (!Result) Result = SaveStringAndCheck("non-BMP FString", TEXT("\U0001f642"), NonBmp, ARRAY_COUNT(NonBmp));

			if (!Result)
			{
				FFixedArchive Load(NonBmp, ARRAY_COUNT(NonBmp));
				FString Value;
				Load << Value;
				if (Load.IsError()) Result = Fail("non-BMP FString load archive error");
				else if (Load.Tell() != ARRAY_COUNT(NonBmp)) Result = Fail("non-BMP FString consumed %i of %zu bytes", Load.Tell(), ARRAY_COUNT(NonBmp));
				else if (Value.Len() != 1 || (*Value)[0] != static_cast<TCHAR>(0x1f642))
					Result = Fail("non-BMP FString decode expected U+1F642 as one host TCHAR");
				else
				{
					FFixedArchive Save;
					Save << Value;
					Result = CheckBytes("non-BMP FString roundtrip", Save.Data(), Save.Size(), NonBmp, ARRAY_COUNT(NonBmp));
				}
			}

			if (!Result)
			{
				static const BYTE LoneHighSurrogate[] = {0x82, 0x00, 0xd8, 0x00, 0x00};
				FFixedArchive Load(LoneHighSurrogate, ARRAY_COUNT(LoneHighSurrogate));
				FString Value;
				Load << Value;
				if (Load.IsError()) Result = Fail("malformed UTF-16 replacement load archive error");
				else if (Value.Len() != 1 || (*Value)[0] != static_cast<TCHAR>(0xfffd))
					Result = Fail("malformed UTF-16 expected one U+FFFD replacement");
			}

			if (!Result)
			{
				static const BYTE Truncated[] = {0x82, 0x41, 0x00};
				FFixedArchive Load(Truncated, ARRAY_COUNT(Truncated));
				FString Value;
				Load << Value;
				if (!Load.IsError()) Result = Fail("truncated UTF-16 FString at offset 3 was accepted");
			}

			if (!Result)
			{
				static const ANSICHAR ExpectedUtf8[] =
				{ 'A', static_cast<ANSICHAR>(0xce), static_cast<ANSICHAR>(0xa9),
				  static_cast<ANSICHAR>(0xf0), static_cast<ANSICHAR>(0x9f),
				  static_cast<ANSICHAR>(0x99), static_cast<ANSICHAR>(0x82), 0 };
				ANSICHAR Encoded[ARRAY_COUNT(ExpectedUtf8)];
				TCHAR Decoded[4];
				if (!appToUtf8InPlace(Encoded, TEXT("A\u03a9\U0001f642"), ARRAY_COUNT(Encoded)))
					Result = Fail("host TCHAR to UTF-8 conversion failed");
				else if (std::memcmp(Encoded, ExpectedUtf8, sizeof(ExpectedUtf8)) != 0)
					Result = Fail("host TCHAR to UTF-8 bytes differ");
				else if (!appFromUtf8InPlace(Decoded, ExpectedUtf8, ARRAY_COUNT(Decoded)))
					Result = Fail("UTF-8 to host TCHAR conversion failed");
				else if (Decoded[0] != TEXT('A') || Decoded[1] != static_cast<TCHAR>(0x03a9)
					|| Decoded[2] != static_cast<TCHAR>(0x1f642) || Decoded[3] != 0)
					Result = Fail("UTF-8 to host TCHAR scalars differ");
			}

			if (!Result)
			{
				static const ANSICHAR OverlongNul[] =
					{ static_cast<ANSICHAR>(0xc0), static_cast<ANSICHAR>(0x80), 0 };
				ANSICHAR SmallEncode[7];
				TCHAR SmallDecode[3];
				TCHAR InvalidDecode[2];
				if (appToUtf8InPlace(SmallEncode, TEXT("A\u03a9\U0001f642"), ARRAY_COUNT(SmallEncode)))
					Result = Fail("undersized UTF-8 destination was accepted");
				else if (appFromUtf8InPlace(SmallDecode, "A\xce\xa9\xf0\x9f\x99\x82", ARRAY_COUNT(SmallDecode)))
					Result = Fail("undersized TCHAR destination was accepted");
				else if (appFromUtf8InPlace(InvalidDecode, OverlongNul, ARRAY_COUNT(InvalidDecode)))
					Result = Fail("overlong UTF-8 sequence was accepted");
			}
		}
		GMalloc = PreviousMalloc;
		Allocator.Exit();
		return Result;
	}

	Native ReadLookupNative(const TCHAR* Name)
	{
		void* Stored = FindNative(Name);
		Native Result = NULL;
		if (Stored)
			std::memcpy(&Result, Stored, sizeof(Result));
		return Result;
	}

	int CheckLookup(const char* Label, const TCHAR* Name, Native Expected)
	{
		Native Actual = ReadLookupNative(Name);
		if (!Actual) return Fail("native lookup missing %s", Label);
		if (Actual != Expected) return Fail("native lookup mismatch for %s", Label);
		return 0;
	}
	template<class InfoType>
	int CheckLookupTable(const char* Label, InfoType* Table, INT ExpectedCount)
	{
		INT Count = 0;
		while (Count <= ExpectedCount && Table[Count].Name)
		{
			if (!Table[Count].Pointer)
				return Fail("%s entry %i has a null function", Label, Count);
			for (INT Prior = 0; Prior < Count; ++Prior)
				if (appStrcmp(Table[Prior].Name, Table[Count].Name) == 0)
					return Fail("%s entries %i and %i duplicate %ls", Label, Prior, Count, Table[Count].Name);
			Native Expected = (Native)Table[Count].Pointer;
			Native Actual = ReadLookupNative(Table[Count].Name);
			if (!Actual)
				return Fail("%s entry %i is unreachable: %ls", Label, Count, Table[Count].Name);
			if (Actual != Expected)
				return Fail("%s entry %i resolves to the wrong function: %ls", Label, Count, Table[Count].Name);
			++Count;
		}
		if (Count != ExpectedCount)
			return Fail("%s entry count expected %i, got %i", Label, ExpectedCount, Count);
		return 0;
	}

	int CheckGeneratedNative(const char* Label, const TCHAR* LookupName, INT Index, Native Expected, BYTE* Seen)
	{
		// Every unindexed native depends on name lookup. For indexed natives,
		// validate the lookup too whenever the owning handler exports it.
		Native Lookup = ReadLookupNative(LookupName);
		if (Index == INDEX_NONE)
		{
			if (!Lookup) return Fail("native lookup missing %s", Label);
			if (Lookup != Expected) return Fail("native lookup mismatch for %s", Label);
			return 0;
		}
		if (Lookup && Lookup != Expected)
			return Fail("native lookup mismatch for %s", Label);
		if (Index < 0 || Index >= EX_Max)
			return Fail("generated native %s has out-of-range slot %i", Label, Index);
		if (Seen[Index])
			return Fail("generated native slot %i is duplicated by %s", Index, Label);
		Seen[Index] = 1;
		if (GNatives[Index] == &UObject::execUndefined)
			return Fail("native slot %i missing for %s", Index, Label);
		if (GNatives[Index] != Expected)
			return Fail("native slot %i mismatch for %s", Index, Label);
		return 0;
	}

	int CheckAllGeneratedNatives()
	{
		BYTE Seen[EX_Max];
		std::memset(Seen, 0, sizeof(Seen));
		int Result = 0;
#define ABI_STRINGIZE_INNER(Value) #Value
#define ABI_STRINGIZE(Value) ABI_STRINGIZE_INNER(Value)
#define NAMES_ONLY
#define AUTOGENERATE_NAME(Name)
#define AUTOGENERATE_FUNCTION(Class,Index,Name) \
		if (!Result) Result = CheckGeneratedNative(ABI_STRINGIZE(Class) "." ABI_STRINGIZE(Name), NATIVE_NAME(Class,Name), Index, (Native)&Class::Name, Seen)
#include "EngineClasses.h"
#undef AUTOGENERATE_FUNCTION
#undef AUTOGENERATE_NAME
#undef NAMES_ONLY
#undef ABI_STRINGIZE
#undef ABI_STRINGIZE_INNER
		return Result;
	}

	class FSilentLog : public FOutputDevice
	{
	public:
		void Serialize(const TCHAR*, EName) override {}
	};

	class FCapturingError : public FOutputDeviceError
	{
	public:
		FCapturingError() { Message[0] = 0; }
		void Serialize(const TCHAR* Text, EName) override
		{
			if (!Message[0])
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

	class FRecordingBrushTracker : public FMovingBrushTrackerBase
	{
	public:
		FRecordingBrushTracker()
		: UpdateCount(0)
		, LastActor(NULL)
		{}
		void Update(AActor* Actor) override
		{
			++UpdateCount;
			LastActor = Actor;
			AMover* Mover = Cast<AMover>(Actor);
			if (Mover)
				Mover->SavedPos = Mover->Location;
		}
		void Flush(AActor*) override {}
		UBOOL SurfIsDynamic(INT) override { return 0; }
		void CountBytes(FArchive&) override {}
		INT UpdateCount;
		AActor* LastActor;
	};

	class FRecordingCollisionHash : public FCollisionHashBase
	{
	public:
		FRecordingCollisionHash()
		: AddCount(0)
		, RemoveCount(0)
		, ContainedActor(NULL)
		{}
		void Tick() override {}
		void AddActor(AActor* Actor) override { ++AddCount; ContainedActor = Actor; }
		void RemoveActor(AActor* Actor, bool) override
		{
			++RemoveCount;
			if (ContainedActor == Actor)
				ContainedActor = NULL;
		}
		FCheckResult* ActorLineCheck(FMemStack&, FVector, FVector, FVector, BYTE) override { return NULL; }
		FCheckResult* ActorPointCheck(FMemStack&, FVector, FVector, DWORD) override { return NULL; }
		FCheckResult* ActorRadiusCheck(FMemStack&, FVector, FLOAT, DWORD) override { return NULL; }
		FCheckResult* ActorEncroachmentCheck(FMemStack&, AActor*, FVector, FRotator, DWORD) override { return NULL; }
		void CheckActorNotReferenced(AActor*) override {}
		INT AddCount;
		INT RemoveCount;
		AActor* ContainedActor;
	};

	class FMoveActorTestLevel : public ULevel
	{
	public:
		FMoveActorTestLevel()
		: ULevel()
		{
			Hash = NULL;
			BrushTracker = NULL;
			Model = NULL;
			InTick = Ticked = 0;
			NumMoves = MoveCycles = 0;
		}
		void SetActorZone(AActor*, UBOOL, UBOOL) override {}
		FCheckResult* MultiLineCheck(FMemStack&, FVector, FVector, FVector, UBOOL, ALevelInfo*, BYTE) override { return NULL; }
	};

	int CheckReflectedProperty(UClass* Owner, const TCHAR* PropertyName, INT ExpectedOffset)
	{
		UProperty* Property = FindField<UProperty>(Owner, PropertyName);
		if (!Property) return Fail("reflected property %ls.%ls is missing", Owner->GetName(), PropertyName);
		if (!Property->CppProperty) return Fail("reflected property %ls.%ls is not a CPP_PROPERTY", Owner->GetName(), PropertyName);
		if (Property->Offset != ExpectedOffset)
			return Fail("reflected property %ls.%ls offset expected %i, got %i", Owner->GetName(), PropertyName, ExpectedOffset, Property->Offset);
		return 0;
	}

	int CheckReflectedByteProperty(UStruct* Owner, const TCHAR* PropertyName, INT ExpectedOffset)
	{
		UByteProperty* Property = FindField<UByteProperty>(Owner, PropertyName);
		if (!Property) return Fail("reflected byte property %ls.%ls is missing", Owner->GetName(), PropertyName);
		if (!Property->CppProperty) return Fail("reflected byte property %ls.%ls is not a CPP_PROPERTY", Owner->GetName(), PropertyName);
		if (Property->Offset != ExpectedOffset)
			return Fail("reflected byte property %ls.%ls offset expected %i, got %i", Owner->GetName(), PropertyName, ExpectedOffset, Property->Offset);
		if (Property->ElementSize != sizeof(BYTE))
			return Fail("reflected byte property %ls.%ls size expected %zu, got %i", Owner->GetName(), PropertyName, sizeof(BYTE), Property->ElementSize);
		return 0;
	}

	int CheckReflectedBoolProperty(
		UClass* Owner,
		const TCHAR* PropertyName,
		INT ExpectedOffset,
		DWORD ExpectedMask,
		UBOOL ExpectedCppProperty=1)
	{
		UBoolProperty* Property = FindField<UBoolProperty>(Owner, PropertyName);
		if (!Property) return Fail("reflected bool property %ls.%ls is missing", Owner->GetName(), PropertyName);
		if (Property->CppProperty != ExpectedCppProperty)
			return Fail("reflected bool property %ls.%ls CPP_PROPERTY expected %i, got %i",
				Owner->GetName(), PropertyName, ExpectedCppProperty, Property->CppProperty);
		if (Property->Offset != ExpectedOffset)
			return Fail("reflected bool property %ls.%ls offset expected %i, got %i", Owner->GetName(), PropertyName, ExpectedOffset, Property->Offset);
		if (Property->ArrayDim != 1 || Property->ElementSize != sizeof(BITFIELD))
			return Fail("reflected bool property %ls.%ls layout expected dim=1 size=%zu, got dim=%i size=%i",
				Owner->GetName(), PropertyName, sizeof(BITFIELD), Property->ArrayDim, Property->ElementSize);
		if (Property->BitMask != ExpectedMask)
			return Fail("reflected bool property %ls.%ls mask expected %u, got %u", Owner->GetName(), PropertyName, ExpectedMask, Property->BitMask);
		return 0;
	}

	int CheckReflectedStructProperty(UClass* Owner, const TCHAR* PropertyName, INT ExpectedOffset, const TCHAR* ExpectedStruct, INT ExpectedSize)
	{
		UStructProperty* Property = FindField<UStructProperty>(Owner, PropertyName);
		if (!Property) return Fail("reflected struct property %ls.%ls is missing", Owner->GetName(), PropertyName);
		if (!Property->CppProperty) return Fail("reflected struct property %ls.%ls is not a CPP_PROPERTY", Owner->GetName(), PropertyName);
		if (Property->Offset != ExpectedOffset)
			return Fail("reflected struct property %ls.%ls offset expected %i, got %i", Owner->GetName(), PropertyName, ExpectedOffset, Property->Offset);
		if (!Property->Struct || appStricmp(Property->Struct->GetName(), ExpectedStruct) != 0)
			return Fail("reflected struct property %ls.%ls expected struct %ls", Owner->GetName(), PropertyName, ExpectedStruct);
		if (Property->ElementSize != ExpectedSize)
			return Fail("reflected struct property %ls.%ls size expected %i, got %i", Owner->GetName(), PropertyName, ExpectedSize, Property->ElementSize);
		return 0;
	}

	int CheckReflectedPropertyLayout(
		UClass* Owner,
		const TCHAR* PropertyName,
		UClass* ExpectedPropertyClass,
		INT ExpectedOffset,
		INT ExpectedArrayDim,
		INT ExpectedElementSize)
	{
		UProperty* Property = FindField<UProperty>(Owner, PropertyName);
		if (!Property)
			return Fail("reflected property %ls.%ls is missing", Owner->GetName(), PropertyName);
		if (!Property->CppProperty)
			return Fail("reflected property %ls.%ls is not a CPP_PROPERTY", Owner->GetName(), PropertyName);
		if (Property->GetClass() != ExpectedPropertyClass)
			return Fail("reflected property %ls.%ls type expected %ls, got %ls",
				Owner->GetName(), PropertyName, ExpectedPropertyClass->GetName(), Property->GetClass()->GetName());
		if (Property->Offset != ExpectedOffset)
			return Fail("reflected property %ls.%ls offset expected %i, got %i",
				Owner->GetName(), PropertyName, ExpectedOffset, Property->Offset);
		if (Property->ArrayDim != ExpectedArrayDim || Property->ElementSize != ExpectedElementSize)
			return Fail("reflected property %ls.%ls layout expected dim=%i size=%i, got dim=%i size=%i",
				Owner->GetName(), PropertyName, ExpectedArrayDim, ExpectedElementSize, Property->ArrayDim, Property->ElementSize);
		return 0;
	}

	int CheckReflectedObjectPropertyClass(UClass* Owner, const TCHAR* PropertyName, UClass* ExpectedObjectClass)
	{
		UObjectProperty* Property = FindField<UObjectProperty>(Owner, PropertyName);
		if (!Property)
			return Fail("reflected object property %ls.%ls is missing", Owner->GetName(), PropertyName);
		if (Property->PropertyClass != ExpectedObjectClass)
			return Fail("reflected object property %ls.%ls class expected %ls, got %ls",
				Owner->GetName(), PropertyName, ExpectedObjectClass->GetName(),
				Property->PropertyClass ? Property->PropertyClass->GetName() : TEXT("None"));
		return 0;
	}

	int CheckReflectedStructPropertyType(UClass* Owner, const TCHAR* PropertyName, const TCHAR* ExpectedStructName)
	{
		UStructProperty* Property = FindField<UStructProperty>(Owner, PropertyName);
		if (!Property)
			return Fail("reflected struct property %ls.%ls is missing", Owner->GetName(), PropertyName);
		if (!Property->Struct || appStricmp(Property->Struct->GetName(), ExpectedStructName) != 0)
			return Fail("reflected struct property %ls.%ls struct expected %ls, got %ls",
				Owner->GetName(), PropertyName, ExpectedStructName,
				Property->Struct ? Property->Struct->GetName() : TEXT("None"));
		return 0;
	}

	int CheckReflectedBytePropertyEnum(UClass* Owner, const TCHAR* PropertyName, const TCHAR* ExpectedEnumName)
	{
		UByteProperty* Property = FindField<UByteProperty>(Owner, PropertyName);
		if (!Property)
			return Fail("reflected byte property %ls.%ls is missing", Owner->GetName(), PropertyName);
		if (!Property->Enum || appStricmp(Property->Enum->GetName(), ExpectedEnumName) != 0)
			return Fail("reflected byte property %ls.%ls enum expected %ls, got %ls",
				Owner->GetName(), PropertyName, ExpectedEnumName,
				Property->Enum ? Property->Enum->GetName() : TEXT("None"));
		return 0;
	}

	int CheckMoverHostBoolStorage(AMover* Mover)
	{
		const INT StateOffset = static_cast<INT>(__builtin_offsetof(AMover, PlayerBumpEvent) - sizeof(BITFIELD));
		const INT MotionOffset = static_cast<INT>(__builtin_offsetof(AMover, RecommendedTrigger) - sizeof(BITFIELD));
		const INT CorralOffset = static_cast<INT>(__builtin_offsetof(AMover, MoverSpringTime) - sizeof(BITFIELD));
		BITFIELD* State = reinterpret_cast<BITFIELD*>(reinterpret_cast<BYTE*>(Mover) + StateOffset);
		BITFIELD* Motion = reinterpret_cast<BITFIELD*>(reinterpret_cast<BYTE*>(Mover) + MotionOffset);
		BITFIELD* Corral = reinterpret_cast<BITFIELD*>(reinterpret_cast<BYTE*>(Mover) + CorralOffset);

		*State = 0; Mover->bKeepRotationDirection = 1;
		if (*State != 1u) return Fail("host AMover.bKeepRotationDirection mask expected 1, got %u", *State);
		*State = 0; Mover->bTriggerOnceOnly = 1;
		if (*State != 2u) return Fail("host AMover.bTriggerOnceOnly mask expected 2, got %u", *State);
		*State = 0; Mover->bSlave = 1;
		if (*State != 4u) return Fail("host AMover.bSlave mask expected 4, got %u", *State);
		*State = 0; Mover->bUseTriggered = 1;
		if (*State != 8u) return Fail("host AMover.bUseTriggered mask expected 8, got %u", *State);
		*State = 0; Mover->bDamageTriggered = 1;
		if (*State != 16u) return Fail("host AMover.bDamageTriggered mask expected 16, got %u", *State);
		*State = 0; Mover->bDynamicLightMover = 1;
		if (*State != 32u) return Fail("host AMover.bDynamicLightMover mask expected 32, got %u", *State);
		*State = 0;

		*Motion = 0; Mover->bOpening = 1;
		if (*Motion != 1u) return Fail("host AMover.bOpening mask expected 1, got %u", *Motion);
		*Motion = 0; Mover->bDelaying = 1;
		if (*Motion != 2u) return Fail("host AMover.bDelaying mask expected 2, got %u", *Motion);
		*Motion = 0; Mover->bClientPause = 1;
		if (*Motion != 4u) return Fail("host AMover.bClientPause mask expected 4, got %u", *Motion);
		*Motion = 0; Mover->bPlayerOnly = 1;
		if (*Motion != 8u) return Fail("host AMover.bPlayerOnly mask expected 8, got %u", *Motion);
		*Motion = 0;

		*Corral = 0; Mover->bCorralMover = 1;
		if (*Corral != 1u) return Fail("host AMover.bCorralMover mask expected 1, got %u", *Corral);
		*Corral = 0; Mover->bCorraledFlag = 1;
		if (*Corral != 2u) return Fail("host AMover.bCorraledFlag mask expected 2, got %u", *Corral);
		*Corral = 0;
		return 0;
	}

	void InvokeGetCurrentKeyState(AActor* Actor, BYTE Key, UBOOL& Result)
	{
		BYTE Code[] = { EX_ByteConst, Key, EX_EndFunctionParms, 0 };
		FFrame Stack(Actor);
		Stack.Code = Code;
		Actor->execGetCurrentKeyState(Stack, &Result);
	}

	int CheckLegacyNullContextNameClear()
	{
		struct FNameClearProbe
		{
			DWORD Before;
			FName Value;
			DWORD After;

			FNameClearProbe()
			: Before(0x13579bdf)
			, Value(TEXT("LegacyContextName"))
			, After(0x2468ace0)
			{}
		};

		FNameClearProbe Probe;
		BYTE Code[] =
		{
			EX_NoObject,
			1, 0,
			4,
			EX_Nothing
		};
		FFrame Stack(UObject::GetTransientPackage());
		Stack.Code = Code;
		UObject::GetTransientPackage()->execContext(Stack, &Probe.Value);
		if (Probe.Before != 0x13579bdf || Probe.After != 0x2468ace0)
			return Fail("legacy null context overwrote adjacent Name storage");
		if (Probe.Value != NAME_None)
			return Fail("legacy null context did not clear the complete Name value");
		return 0;
	}

	int CheckCurrentKeyStateNative()
	{
		AActor* Actor = Cast<AActor>(UObject::StaticConstructObject(
			AMover::StaticClass(), UObject::GetTransientPackage(), NAME_None, RF_Transient));
		if (!Actor)
			return Fail("GetCurrentKeyState non-player fixture is unavailable");

		UBOOL KeyState = 1;
		InvokeGetCurrentKeyState(Actor, IK_A, KeyState);
		if (KeyState)
			return Fail("GetCurrentKeyState returned true for a non-player actor");

		KeyState = 1;
		InvokeGetCurrentKeyState(Actor, 0xff, KeyState);
		if (KeyState)
			return Fail("GetCurrentKeyState accepted an out-of-range key");
		return 0;
	}

	int ValidateStarPlatformFragments(
		ULevel* Level,
		AMover* Mover,
		const char* Stage,
		INT& FragmentCount)
	{
		FragmentCount = 0;
		for (INT NodeIndex = Mover->Brush->MoverLink, ChainLength = 0;
			NodeIndex != INDEX_NONE;)
		{
			if (!Level->Model->Nodes.IsValidIndex(NodeIndex))
				return Fail("%s star platform links an invalid BSP node", Stage);
			const FBspNode& Node = Level->Model->Nodes(NodeIndex);
			if (!Level->Model->Surfs.IsValidIndex(Node.iSurf)
				|| Node.NumVertices < 3
				|| !Level->Model->Verts.IsValidIndex(Node.iVertPool)
				|| !Level->Model->Verts.IsValidIndex(Node.iVertPool + Node.NumVertices - 1))
				return Fail("%s star platform has an invalid BSP fragment", Stage);
			for (INT VertexIndex = 0; VertexIndex < Node.NumVertices; ++VertexIndex)
			{
				const INT PointIndex = Level->Model->Verts(Node.iVertPool + VertexIndex).pVertex;
				if (!Level->Model->Points.IsValidIndex(PointIndex))
					return Fail("%s star platform references an invalid BSP point", Stage);
				const FVector& Point = Level->Model->Points(PointIndex);
				if (!std::isfinite(Point.X)
					|| !std::isfinite(Point.Y)
					|| !std::isfinite(Point.Z))
					return Fail("%s star platform produced a non-finite BSP point", Stage);
			}
			++FragmentCount;
			NodeIndex = Node.iRenderBound;
			if (++ChainLength > Level->Model->Nodes.Num())
				return Fail("%s star platform BSP chain is cyclic", Stage);
		}
		if (!FragmentCount)
			return Fail("%s star platform has no render fragments", Stage);
		return 0;
	}

	int CheckRictusempraStarPlatformMovement()
	{
		UPackage* Package = Cast<UPackage>(UObject::LoadPackage(
			NULL, TEXT("../Maps/Ch1Rictusempra.unr"), LOAD_NoFail));
		ULevel* Level = Package
			? FindObject<ULevel>(Package, TEXT("MyLevel"))
			: NULL;
		AMover* Platform = Package
			? FindObject<AMover>(Package, TEXT("Mover74"))
			: NULL;
		if (!Level || !Level->Model || !Platform || !Platform->Brush)
			return Fail("Rictusempra star-platform fixture is unavailable");
		if (Platform->Tag != FName(TEXT("starplatform01"))
			|| Platform->NumKeys < 2
			|| !Platform->bBlockPlayers
			|| Platform->GetPrimitive() != Platform->Brush)
			return Fail("Rictusempra star-platform fixture contract changed");

		if (Level->BrushTracker)
			delete Level->BrushTracker;
		Level->BrushTracker = GNewBrushTracker(Level);
		if (!Level->BrushTracker)
			return Fail("Rictusempra moving-brush tracker was not created");

		const FVector Start = Platform->BasePos;
		const FVector End = Start + Platform->KeyPos[1];
		const FVector Middle = Start + Platform->KeyPos[1] * 0.5f;
		INT StartFragments = 0;
		INT MiddleFragments = 0;
		INT EndFragments = 0;
		int Result = ValidateStarPlatformFragments(
			Level, Platform, "raised", StartFragments);
		if (!Result)
		{
			Platform->Location = Middle;
			Level->BrushTracker->Update(Platform);
			Result = ValidateStarPlatformFragments(
				Level, Platform, "midpoint", MiddleFragments);
		}
		if (!Result)
		{
			Platform->Location = End;
			Level->BrushTracker->Update(Platform);
			Result = ValidateStarPlatformFragments(
				Level, Platform, "lowered", EndFragments);
		}
		if (!Result && (Platform->SavedPos != End
			|| Platform->GetPrimitive() != Platform->Brush
			|| !Platform->bBlockPlayers))
			Result = Fail("lowered star platform lost transform or collision state");
		delete Level->BrushTracker;
		Level->BrushTracker = NULL;
		return Result;
	}



	int CheckRuntimeProperties(int ArgC, char** ArgV)
	{
		if (!PrepareHP2Paths(ArgC, ArgV))
			return Fail("path bootstrap failed");
#if !_MSC_VER
		__Context::StaticInit();
		std::strncpy(GModule, ArgV[0], sizeof(GModule) - 1);
		GModule[sizeof(GModule) - 1] = 0;
#endif
		TCHAR CmdLine[2048];
		CmdLine[0] = 0;
		for (INT Index = 1; Index < ArgC; ++Index)
		{
			if (std::strncmp(ArgV[Index], "--test=", 7) == 0)
				continue;
			const TCHAR* Argument = ANSI_TO_TCHAR(ArgV[Index]);
			if (appStrlen(CmdLine) + appStrlen(Argument) + 2 >= ARRAY_COUNT(CmdLine))
				return Fail("runtime command line exceeds 2047 TCHARs");
			if (CmdLine[0]) appStrcat(CmdLine, TEXT(" "));
			appStrcat(CmdLine, Argument);
		}
		RuntimeError.Message[0] = 0;
		GIsStarted = 1;
		GIsGuarded = 1;
		int Result = 0;
		try
		{
			appInit(TEXT("AbiTests"), CmdLine, &RuntimeMalloc, &RuntimeLog, &RuntimeError, &RuntimeWarn,
				&RuntimeFileManager, FConfigCacheIni::Factory, 1);
			RegisterHP2RuntimeClasses();
			if (!Result) Result = CheckLegacyNullContextNameClear();
			UClass* CompiledPatrolClass = APatrolPoint::StaticClass();
			if (CompiledPatrolClass->GetSuperClass() != ANavigationPoint::StaticClass())
				Result = Fail("compiled Engine.PatrolPoint superclass is not Engine.NavigationPoint");
			if (!Result && !(CompiledPatrolClass->GetFlags() & RF_Native))
				Result = Fail("compiled Engine.PatrolPoint is not RF_Native");
			if (!Result) Result = CheckReflectedPropertyLayout(
				CompiledPatrolClass, TEXT("ActionKeyword"), UNameProperty::StaticClass(),
				static_cast<INT>(__builtin_offsetof(APatrolPoint, ActionKeyword)), 1, sizeof(FName));
			const INT CompiledActorCollisionOffset = static_cast<INT>(
				__builtin_offsetof(AActor, LightType) - sizeof(BITFIELD));
			if (!Result) Result = CheckReflectedBoolProperty(
				AActor::StaticClass(), TEXT("bAlignBottomAlways"), CompiledActorCollisionOffset, 128);
			if (!Result) Result = CheckReflectedPropertyLayout(
				AInterpolationManager::StaticClass(), TEXT("Last"), UObjectProperty::StaticClass(),
				static_cast<INT>(__builtin_offsetof(AInterpolationManager, Last)), 1, sizeof(UObject*));
			if (!Result) Result = CheckReflectedObjectPropertyClass(
				AInterpolationManager::StaticClass(), TEXT("Last"), AInterpolationPoint::StaticClass());
			const INT CompiledLoadingScreenOffset = static_cast<INT>(
				__builtin_offsetof(APlayerPawn, TotalGameStateTokens) - sizeof(BITFIELD));
			if (!Result) Result = CheckReflectedBoolProperty(
				APlayerPawn::StaticClass(), TEXT("bShowLoadingScreen"), CompiledLoadingScreenOffset, 1);
			const INT CompiledSaveStateOffset = static_cast<INT>(
				__builtin_offsetof(APlayerPawn, bMouseAltFire) - sizeof(BITFIELD));
			if (!Result) Result = CheckReflectedBoolProperty(
				APlayerPawn::StaticClass(), TEXT("bModernThirdPersonControls"), CompiledSaveStateOffset, 4);
			UBoolProperty* ModernThirdPersonControls = FindField<UBoolProperty>(
				APlayerPawn::StaticClass(), TEXT("bModernThirdPersonControls"));
			if (!Result && (ModernThirdPersonControls->PropertyFlags & (CPF_Config | CPF_GlobalConfig)) !=
				(CPF_Config | CPF_GlobalConfig))
				Result = Fail("APlayerPawn.bModernThirdPersonControls is not config globalconfig");
			if (!Result && ModernThirdPersonControls->Category != NAME_None)
				Result = Fail("APlayerPawn.bModernThirdPersonControls category is not empty");
			if (Result)
				return Result;
			if (!UObject::LoadPackage(NULL, TEXT("Engine.u"), LOAD_NoFail))
				return Fail("Engine package load failed");
			if (!Result) Result = CheckRictusempraStarPlatformMovement();
		}
		catch (...)
		{
			GIsGuarded = 0;
			GIsStarted = 0;
			if (RuntimeError.Message[0])
				return Fail("runtime bootstrap failed: %ls", RuntimeError.Message);
			return Fail("runtime bootstrap threw without an engine diagnostic");
		}
		static const TCHAR* ExpectedEngineClasses[] =
		{
			TEXT("Engine.AnimChannel"), TEXT("Engine.ClipMarker"), TEXT("Engine.Wind"),
			TEXT("Engine.PatrolPoint"),
			TEXT("Engine.AlignedOvalCylinder"), TEXT("Engine.Box"), TEXT("Engine.BoxPrim"),
			TEXT("Engine.ChannelDownload"), TEXT("Engine.CheckSumCommandlet"), TEXT("Engine.Download"),
			TEXT("Engine.EngineTextureFactory"), TEXT("Engine.GameSaveInfo"), TEXT("Engine.Gesture"),
			TEXT("Engine.I3DL2Listener"), TEXT("Engine.ImpactSoundSet"), TEXT("Engine.OrientedCylinder"),
			TEXT("Engine.OrientedOvalCylinder"), TEXT("Engine.ParticleListPriv"),
			TEXT("Engine.SkeletalMesh"), TEXT("Engine.SoundContainer")
		};
		Result = 0;
		for (INT Index = 0; !Result && Index < ARRAY_COUNT(ExpectedEngineClasses); ++Index)
			if (!UObject::StaticFindObject(UClass::StaticClass(), ANY_PACKAGE, ExpectedEngineClasses[Index], 1))
				Result = Fail("compiled Engine class was not initialized: %ls", ExpectedEngineClasses[Index]);
		UClass* PatrolClass = Cast<UClass>(UObject::StaticFindObject(
			UClass::StaticClass(), ANY_PACKAGE, TEXT("Engine.PatrolPoint"), 1));
		if (!Result && !PatrolClass)
			Result = Fail("compiled Engine PatrolPoint class was not initialized");
		if (!Result && PatrolClass->GetSuperClass() != ANavigationPoint::StaticClass())
			Result = Fail("Engine.PatrolPoint native superclass is not Engine.NavigationPoint");
		if (!Result && !(PatrolClass->GetFlags() & RF_Native))
			Result = Fail("Engine.PatrolPoint did not retain RF_Native binding");

		struct FPatrolPropertyExpectation
		{
			const TCHAR* Name;
			UClass* PropertyClass;
			INT Offset;
			INT ElementSize;
		};
#define PATROL_PROPERTY(Member, PropertyType, ElementType) \
		{ TEXT(#Member), PropertyType::StaticClass(), static_cast<INT>(__builtin_offsetof(APatrolPoint, Member)), sizeof(ElementType) }
		const FPatrolPropertyExpectation PatrolProperties[] =
		{
			PATROL_PROPERTY(NextPatrol_ObjectName, UNameProperty, FName),
			PATROL_PROPERTY(PauseTime, UFloatProperty, FLOAT),
			PATROL_PROPERTY(lookDir, UStructProperty, FVector),
			PATROL_PROPERTY(PatrolAnim, UNameProperty, FName),
			PATROL_PROPERTY(PauseAnim, UNameProperty, FName),
			PATROL_PROPERTY(PatrolSound, UObjectProperty, UObject*),
			PATROL_PROPERTY(numAnims, UByteProperty, BYTE),
			PATROL_PROPERTY(AnimCount, UIntProperty, INT),
			PATROL_PROPERTY(PrevPatrolPoint, UObjectProperty, UObject*),
			PATROL_PROPERTY(NextPatrolPoint, UObjectProperty, UObject*),
			PATROL_PROPERTY(EventToSend, UNameProperty, FName),
			PATROL_PROPERTY(PatrolPointLinkTag, UNameProperty, FName),
			PATROL_PROPERTY(vFraySplineTangent, UStructProperty, FVector),
			PATROL_PROPERTY(fTanLenIn, UFloatProperty, FLOAT),
			PATROL_PROPERTY(fTanLenOut, UFloatProperty, FLOAT),
			PATROL_PROPERTY(fJumpHorizSpeed, UFloatProperty, FLOAT),
			PATROL_PROPERTY(fJumpVertSpeed, UFloatProperty, FLOAT),
			PATROL_PROPERTY(fJumpAnimMultiplier, UFloatProperty, FLOAT)
		};
#undef PATROL_PROPERTY
		for (INT Index = 0; !Result && Index < ARRAY_COUNT(PatrolProperties); ++Index)
		{
			const FPatrolPropertyExpectation& Expected = PatrolProperties[Index];
			Result = CheckReflectedPropertyLayout(
				PatrolClass, Expected.Name, Expected.PropertyClass,
				Expected.Offset, 1, Expected.ElementSize);
		}
		if (!Result) Result = CheckReflectedObjectPropertyClass(PatrolClass, TEXT("PatrolSound"), USound::StaticClass());
		if (!Result) Result = CheckReflectedObjectPropertyClass(PatrolClass, TEXT("PrevPatrolPoint"), PatrolClass);
		if (!Result) Result = CheckReflectedObjectPropertyClass(PatrolClass, TEXT("NextPatrolPoint"), PatrolClass);
		if (!Result) Result = CheckReflectedStructPropertyType(PatrolClass, TEXT("lookDir"), TEXT("Vector"));
		if (!Result) Result = CheckReflectedStructPropertyType(PatrolClass, TEXT("vFraySplineTangent"), TEXT("Vector"));

		const INT PatrolLookBoolOffset = static_cast<INT>(__builtin_offsetof(APatrolPoint, lookDir) - sizeof(BITFIELD));
		const INT PatrolLeadBoolOffset = static_cast<INT>(__builtin_offsetof(APatrolPoint, PrevPatrolPoint) - sizeof(BITFIELD));
		const INT PatrolChainBoolOffset = static_cast<INT>(__builtin_offsetof(APatrolPoint, PatrolPointLinkTag) - sizeof(BITFIELD));
		const INT PatrolSplineBoolOffset = static_cast<INT>(__builtin_offsetof(APatrolPoint, fTanLenIn) - sizeof(BITFIELD));
		const INT PatrolBossBoolOffset = static_cast<INT>(sizeof(APatrolPoint) - sizeof(BITFIELD));
		if (!Result) Result = CheckReflectedBoolProperty(PatrolClass, TEXT("bUseLookDir"), PatrolLookBoolOffset, 1);
		if (!Result) Result = CheckReflectedBoolProperty(PatrolClass, TEXT("bLeadActorWaitPoint"), PatrolLeadBoolOffset, 1);
		if (!Result) Result = CheckReflectedBoolProperty(PatrolClass, TEXT("bDestroyPawn"), PatrolChainBoolOffset, 1);
		if (!Result) Result = CheckReflectedBoolProperty(PatrolClass, TEXT("bUseOrientationForSplineTan"), PatrolChainBoolOffset, 2);
		if (!Result) Result = CheckReflectedBoolProperty(PatrolClass, TEXT("bStartOfUnlinkedChain"), PatrolChainBoolOffset, 4);
		if (!Result) Result = CheckReflectedBoolProperty(PatrolClass, TEXT("bHasSplineInfo"), PatrolSplineBoolOffset, 1);
		if (!Result) Result = CheckReflectedBoolProperty(PatrolClass, TEXT("bStopBossEncounter"), PatrolBossBoolOffset, 1);

		const INT ActorCollisionBoolOffset = static_cast<INT>(__builtin_offsetof(AActor, LightType) - sizeof(BITFIELD));
		if (!Result) Result = CheckReflectedBoolProperty(AActor::StaticClass(), TEXT("bAlignBottom"), ActorCollisionBoolOffset, 64);
		if (!Result) Result = CheckReflectedBoolProperty(AActor::StaticClass(), TEXT("bBlockCamera"), ActorCollisionBoolOffset, 256);
		if (!Result) Result = CheckReflectedProperty(
			AInterpolationManager::StaticClass(), TEXT("PhysAlpha"),
			static_cast<INT>(__builtin_offsetof(AInterpolationManager, PhysAlpha)));
		if (!Result) Result = CheckReflectedProperty(
			APlayerPawn::StaticClass(), TEXT("TotalGameStateTokens"),
			static_cast<INT>(__builtin_offsetof(APlayerPawn, TotalGameStateTokens)));
		struct FPropertyLayoutExpectation
		{
			const TCHAR* Name;
			UClass* PropertyClass;
			INT Offset;
			INT ArrayDim;
			INT ElementSize;
		};
#define MOVER_PROPERTY(Member, PropertyType, ElementType) \
		{ TEXT(#Member), PropertyType::StaticClass(), static_cast<INT>(__builtin_offsetof(AMover, Member)), 1, sizeof(ElementType) }
#define MOVER_ARRAY_PROPERTY(Member, PropertyType, ElementType, Count) \
		{ TEXT(#Member), PropertyType::StaticClass(), static_cast<INT>(__builtin_offsetof(AMover, Member)), Count, sizeof(ElementType) }
		const FPropertyLayoutExpectation MoverProperties[] =
		{
			MOVER_PROPERTY(MoverEncroachType, UByteProperty, BYTE),
			MOVER_PROPERTY(MoverGlideType, UByteProperty, BYTE),
			MOVER_PROPERTY(BumpType, UByteProperty, BYTE),
			MOVER_PROPERTY(KeyNum, UByteProperty, BYTE),
			MOVER_PROPERTY(PrevKeyNum, UByteProperty, BYTE),
			MOVER_PROPERTY(NumKeys, UByteProperty, BYTE),
			MOVER_PROPERTY(WorldRaytraceKey, UByteProperty, BYTE),
			MOVER_PROPERTY(BrushRaytraceKey, UByteProperty, BYTE),
			MOVER_ARRAY_PROPERTY(MoveTimes, UFloatProperty, FLOAT, 8),
			MOVER_PROPERTY(MoveTime, UFloatProperty, FLOAT),
			MOVER_PROPERTY(StayOpenTime, UFloatProperty, FLOAT),
			MOVER_PROPERTY(OtherTime, UFloatProperty, FLOAT),
			MOVER_PROPERTY(EncroachDamage, UIntProperty, INT),
			MOVER_PROPERTY(PlayerBumpEvent, UNameProperty, FName),
			MOVER_PROPERTY(BumpEvent, UNameProperty, FName),
			MOVER_PROPERTY(SavedTrigger, UObjectProperty, UObject*),
			MOVER_PROPERTY(DamageThreshold, UFloatProperty, FLOAT),
			MOVER_PROPERTY(numTriggerEvents, UIntProperty, INT),
			MOVER_PROPERTY(Leader, UObjectProperty, UObject*),
			MOVER_PROPERTY(Follower, UObjectProperty, UObject*),
			MOVER_PROPERTY(ReturnGroup, UNameProperty, FName),
			MOVER_PROPERTY(DelayTime, UFloatProperty, FLOAT),
			MOVER_PROPERTY(AttachTag, UNameProperty, FName),
			MOVER_PROPERTY(OpeningSound, UObjectProperty, UObject*),
			MOVER_PROPERTY(OpenedSound, UObjectProperty, UObject*),
			MOVER_PROPERTY(ClosingSound, UObjectProperty, UObject*),
			MOVER_PROPERTY(ClosedSound, UObjectProperty, UObject*),
			MOVER_PROPERTY(MoveAmbientSound, UObjectProperty, UObject*),
			MOVER_PROPERTY(FailSound, UObjectProperty, UObject*),
			MOVER_PROPERTY(MoverRadius, UFloatProperty, FLOAT),
			MOVER_PROPERTY(MoverVolume, UByteProperty, BYTE),
			MOVER_PROPERTY(MoverPitch, UByteProperty, BYTE),
			MOVER_ARRAY_PROPERTY(KeyPos, UStructProperty, FVector, 8),
			MOVER_ARRAY_PROPERTY(KeyRot, UStructProperty, FRotator, 8),
			MOVER_PROPERTY(BasePos, UStructProperty, FVector),
			MOVER_PROPERTY(OldPos, UStructProperty, FVector),
			MOVER_PROPERTY(OldPrePivot, UStructProperty, FVector),
			MOVER_PROPERTY(SavedPos, UStructProperty, FVector),
			MOVER_PROPERTY(BaseRot, UStructProperty, FRotator),
			MOVER_PROPERTY(OldRot, UStructProperty, FRotator),
			MOVER_PROPERTY(SavedRot, UStructProperty, FRotator),
			MOVER_PROPERTY(PhysAlpha, UFloatProperty, FLOAT),
			MOVER_PROPERTY(PhysRate, UFloatProperty, FLOAT),
			MOVER_PROPERTY(MyMarker, UObjectProperty, UObject*),
			MOVER_PROPERTY(TriggerActor, UObjectProperty, UObject*),
			MOVER_PROPERTY(TriggerActor2, UObjectProperty, UObject*),
			MOVER_PROPERTY(WaitingPawn, UObjectProperty, UObject*),
			MOVER_PROPERTY(RecommendedTrigger, UObjectProperty, UObject*),
			MOVER_PROPERTY(SimOldPos, UStructProperty, FVector),
			MOVER_PROPERTY(SimOldRotPitch, UIntProperty, INT),
			MOVER_PROPERTY(SimOldRotYaw, UIntProperty, INT),
			MOVER_PROPERTY(SimOldRotRoll, UIntProperty, INT),
			MOVER_PROPERTY(SimInterpolate, UStructProperty, FVector),
			MOVER_PROPERTY(RealPosition, UStructProperty, FVector),
			MOVER_PROPERTY(RealRotation, UStructProperty, FRotator),
			MOVER_PROPERTY(ClientUpdate, UIntProperty, INT),
			MOVER_PROPERTY(MoverSpringTime, UFloatProperty, FLOAT),
			MOVER_PROPERTY(MoverMaxAmplitude, UFloatProperty, FLOAT),
			MOVER_PROPERTY(MoverFluctuations, UByteProperty, BYTE),
			MOVER_PROPERTY(HitPosition, UStructProperty, FVector),
			MOVER_PROPERTY(HitNormal, UStructProperty, FVector)
		};
#undef MOVER_ARRAY_PROPERTY
#undef MOVER_PROPERTY
		for (INT Index = 0; !Result && Index < ARRAY_COUNT(MoverProperties); ++Index)
		{
			const FPropertyLayoutExpectation& Expected = MoverProperties[Index];
			Result = CheckReflectedPropertyLayout(
				AMover::StaticClass(),
				Expected.Name,
				Expected.PropertyClass,
				Expected.Offset,
				Expected.ArrayDim,
				Expected.ElementSize);
		}

		struct FObjectPropertyExpectation
		{
			const TCHAR* Name;
			UClass* ObjectClass;
		};
#define MOVER_OBJECT_PROPERTY(Member, ObjectType) { TEXT(#Member), ObjectType::StaticClass() }
		const FObjectPropertyExpectation MoverObjectProperties[] =
		{
			MOVER_OBJECT_PROPERTY(SavedTrigger, AActor),
			MOVER_OBJECT_PROPERTY(Leader, AMover),
			MOVER_OBJECT_PROPERTY(Follower, AMover),
			MOVER_OBJECT_PROPERTY(OpeningSound, USound),
			MOVER_OBJECT_PROPERTY(OpenedSound, USound),
			MOVER_OBJECT_PROPERTY(ClosingSound, USound),
			MOVER_OBJECT_PROPERTY(ClosedSound, USound),
			MOVER_OBJECT_PROPERTY(MoveAmbientSound, USound),
			MOVER_OBJECT_PROPERTY(FailSound, USound),
			MOVER_OBJECT_PROPERTY(MyMarker, ANavigationPoint),
			MOVER_OBJECT_PROPERTY(TriggerActor, AActor),
			MOVER_OBJECT_PROPERTY(TriggerActor2, AActor),
			MOVER_OBJECT_PROPERTY(WaitingPawn, APawn),
			MOVER_OBJECT_PROPERTY(RecommendedTrigger, ATrigger)
		};
#undef MOVER_OBJECT_PROPERTY
		for (INT Index = 0; !Result && Index < ARRAY_COUNT(MoverObjectProperties); ++Index)
			Result = CheckReflectedObjectPropertyClass(
				AMover::StaticClass(), MoverObjectProperties[Index].Name, MoverObjectProperties[Index].ObjectClass);

		struct FStructPropertyExpectation
		{
			const TCHAR* Name;
			const TCHAR* StructName;
		};
#define MOVER_STRUCT_PROPERTY(Member, StructType) { TEXT(#Member), TEXT(#StructType) }
		const FStructPropertyExpectation MoverStructProperties[] =
		{
			MOVER_STRUCT_PROPERTY(KeyPos, Vector),
			MOVER_STRUCT_PROPERTY(KeyRot, Rotator),
			MOVER_STRUCT_PROPERTY(BasePos, Vector),
			MOVER_STRUCT_PROPERTY(OldPos, Vector),
			MOVER_STRUCT_PROPERTY(OldPrePivot, Vector),
			MOVER_STRUCT_PROPERTY(SavedPos, Vector),
			MOVER_STRUCT_PROPERTY(BaseRot, Rotator),
			MOVER_STRUCT_PROPERTY(OldRot, Rotator),
			MOVER_STRUCT_PROPERTY(SavedRot, Rotator),
			MOVER_STRUCT_PROPERTY(SimOldPos, Vector),
			MOVER_STRUCT_PROPERTY(SimInterpolate, Vector),
			MOVER_STRUCT_PROPERTY(RealPosition, Vector),
			MOVER_STRUCT_PROPERTY(RealRotation, Rotator),
			MOVER_STRUCT_PROPERTY(HitPosition, Vector),
			MOVER_STRUCT_PROPERTY(HitNormal, Vector)
		};
#undef MOVER_STRUCT_PROPERTY
		for (INT Index = 0; !Result && Index < ARRAY_COUNT(MoverStructProperties); ++Index)
			Result = CheckReflectedStructPropertyType(
				AMover::StaticClass(), MoverStructProperties[Index].Name, MoverStructProperties[Index].StructName);

		if (!Result) Result = CheckReflectedBytePropertyEnum(AMover::StaticClass(), TEXT("MoverEncroachType"), TEXT("EMoverEncroachType"));
		if (!Result) Result = CheckReflectedBytePropertyEnum(AMover::StaticClass(), TEXT("MoverGlideType"), TEXT("EMoverGlideType"));
		if (!Result) Result = CheckReflectedBytePropertyEnum(AMover::StaticClass(), TEXT("BumpType"), TEXT("EBumpType"));

		const INT MoverStateBoolOffset = static_cast<INT>(__builtin_offsetof(AMover, PlayerBumpEvent) - sizeof(BITFIELD));
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bKeepRotationDirection"), MoverStateBoolOffset, 1);
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bTriggerOnceOnly"), MoverStateBoolOffset, 2);
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bSlave"), MoverStateBoolOffset, 4);
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bUseTriggered"), MoverStateBoolOffset, 8);
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bDamageTriggered"), MoverStateBoolOffset, 16);
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bDynamicLightMover"), MoverStateBoolOffset, 32);

		const INT MoverMotionBoolOffset = static_cast<INT>(__builtin_offsetof(AMover, RecommendedTrigger) - sizeof(BITFIELD));
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bOpening"), MoverMotionBoolOffset, 1);
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bDelaying"), MoverMotionBoolOffset, 2);
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bClientPause"), MoverMotionBoolOffset, 4);
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bPlayerOnly"), MoverMotionBoolOffset, 8);

		const INT MoverCorralBoolOffset = static_cast<INT>(__builtin_offsetof(AMover, MoverSpringTime) - sizeof(BITFIELD));
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bCorralMover"), MoverCorralBoolOffset, 1);
		if (!Result) Result = CheckReflectedBoolProperty(AMover::StaticClass(), TEXT("bCorraledFlag"), MoverCorralBoolOffset, 2);
		if (!Result) Result = CheckReflectedProperty(UClient::StaticClass(), TEXT("WindowedViewportX"), static_cast<INT>(__builtin_offsetof(UClient, WindowedViewportX)));
		if (!Result) Result = CheckReflectedProperty(UClient::StaticClass(), TEXT("TextureDetail"), static_cast<INT>(__builtin_offsetof(UClient, TextureLODSet[1])));
		if (!Result) Result = CheckReflectedProperty(UClient::StaticClass(), TEXT("SkinDetail"), static_cast<INT>(__builtin_offsetof(UClient, TextureLODSet[2])));
		if (!Result) Result = CheckReflectedProperty(UClient::StaticClass(), TEXT("MaintainVerticalFOV"), static_cast<INT>(__builtin_offsetof(UClient, MaintainVerticalFOV)));
		if (!Result) Result = CheckReflectedBoolProperty(
			UClient::StaticClass(), TEXT("ShowFPS"),
			static_cast<INT>(__builtin_offsetof(UClient, ShowFPS)), 1);
		UProperty* ShowFPSProperty = FindField<UProperty>(UClient::StaticClass(), TEXT("ShowFPS"));
		if (!Result && (!(ShowFPSProperty->PropertyFlags & CPF_Config)
			|| ShowFPSProperty->Category != FName(TEXT("Display"))))
			Result = Fail("reflected UClient.ShowFPS expected Config flags in the Display category");
		if (!Result) Result = CheckReflectedProperty(UEngine::StaticClass(), TEXT("CacheSizeMegs"), static_cast<INT>(__builtin_offsetof(UEngine, CacheSizeMegs)));
		if (!Result) Result = CheckReflectedProperty(UEngine::StaticClass(), TEXT("UseSound"), static_cast<INT>(__builtin_offsetof(UEngine, UseSound)));
		if (!Result) Result = CheckReflectedProperty(UGameEngine::StaticClass(), TEXT("FrameRateLimit"), static_cast<INT>(__builtin_offsetof(UGameEngine, FrameRateLimit)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("Font"), static_cast<INT>(__builtin_offsetof(UCanvas, Font)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("SpaceX"), static_cast<INT>(__builtin_offsetof(UCanvas, SpaceX)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("SpaceY"), static_cast<INT>(__builtin_offsetof(UCanvas, SpaceY)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("OrgX"), static_cast<INT>(__builtin_offsetof(UCanvas, OrgX)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("OrgY"), static_cast<INT>(__builtin_offsetof(UCanvas, OrgY)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("ClipX"), static_cast<INT>(__builtin_offsetof(UCanvas, ClipX)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("ClipY"), static_cast<INT>(__builtin_offsetof(UCanvas, ClipY)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("CurX"), static_cast<INT>(__builtin_offsetof(UCanvas, CurX)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("CurY"), static_cast<INT>(__builtin_offsetof(UCanvas, CurY)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("Z"), static_cast<INT>(__builtin_offsetof(UCanvas, Z)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("Style"), static_cast<INT>(__builtin_offsetof(UCanvas, Style)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("CurYL"), static_cast<INT>(__builtin_offsetof(UCanvas, CurYL)));
		if (!Result) Result = CheckReflectedStructProperty(UCanvas::StaticClass(), TEXT("DrawColor"), static_cast<INT>(__builtin_offsetof(UCanvas, Color)), TEXT("Color"), sizeof(FColor));
		UStructProperty* DrawColorProperty = FindField<UStructProperty>(UCanvas::StaticClass(), TEXT("DrawColor"));
		if (!Result) Result = CheckReflectedByteProperty(DrawColorProperty->Struct, TEXT("R"), static_cast<INT>(__builtin_offsetof(FColor, R)));
		if (!Result) Result = CheckReflectedByteProperty(DrawColorProperty->Struct, TEXT("G"), static_cast<INT>(__builtin_offsetof(FColor, G)));
		if (!Result) Result = CheckReflectedByteProperty(DrawColorProperty->Struct, TEXT("B"), static_cast<INT>(__builtin_offsetof(FColor, B)));
		if (!Result) Result = CheckReflectedByteProperty(DrawColorProperty->Struct, TEXT("A"), static_cast<INT>(__builtin_offsetof(FColor, A)));
		const INT CanvasBoolOffset = static_cast<INT>(__builtin_offsetof(UCanvas, Color) + sizeof(FColor));
		if (!Result) Result = CheckReflectedBoolProperty(UCanvas::StaticClass(), TEXT("bCenter"), CanvasBoolOffset, 1);
		if (!Result) Result = CheckReflectedBoolProperty(UCanvas::StaticClass(), TEXT("bNoSmooth"), CanvasBoolOffset, 2);
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("SizeX"), static_cast<INT>(__builtin_offsetof(UCanvas, X)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("SizeY"), static_cast<INT>(__builtin_offsetof(UCanvas, Y)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("SmallFont"), static_cast<INT>(__builtin_offsetof(UCanvas, SmallFont)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("MedFont"), static_cast<INT>(__builtin_offsetof(UCanvas, MedFont)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("BigFont"), static_cast<INT>(__builtin_offsetof(UCanvas, BigFont)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("LargeFont"), static_cast<INT>(__builtin_offsetof(UCanvas, LargeFont)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("Viewport"), static_cast<INT>(__builtin_offsetof(UCanvas, Viewport)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("FramePtr"), static_cast<INT>(__builtin_offsetof(UCanvas, Frame)));
		if (!Result) Result = CheckReflectedProperty(UCanvas::StaticClass(), TEXT("RenderPtr"), static_cast<INT>(__builtin_offsetof(UCanvas, Render)));
		USDLClient* SDLClientDefaults = Cast<USDLClient>(USDLClient::StaticClass()->GetDefaultObject());
		if (!Result && (!SDLClientDefaults || SDLClientDefaults->ParticleDensity != 1))
			Result = Fail("fresh SDL client particle density expected 1");
		if (!Result && !SDLClientDefaults->MaintainVerticalFOV)
			Result = Fail("fresh SDL client vertical-FOV maintenance expected enabled");
		if (!Result && SDLClientDefaults->ShowFPS)
			Result = Fail("fresh SDL client FPS display expected disabled");
		if (!Result) Result = CheckCurrentKeyStateNative();
		AMover* MoverProbe = Cast<AMover>(UObject::StaticConstructObject(
			AMover::StaticClass(), UObject::GetTransientPackage(), NAME_None, RF_Transient));
		UModel* BrushProbe = Cast<UModel>(UObject::StaticConstructObject(
			UModel::StaticClass(), UObject::GetTransientPackage(), NAME_None, RF_Transient));
		if (!Result && (!MoverProbe || !BrushProbe))
			Result = Fail("transient mover collision fixture is unavailable");
		if (!Result)
			Result = CheckMoverHostBoolStorage(MoverProbe);
		if (!Result)
		{
			alignas(FMoveActorTestLevel) BYTE LevelStorage[sizeof(FMoveActorTestLevel)] = {};
			FMoveActorTestLevel* TestLevel = ::new (LevelStorage) FMoveActorTestLevel;
			FRecordingCollisionHash Hash;
			FRecordingBrushTracker Tracker;
			TestLevel->Hash = &Hash;
			TestLevel->BrushTracker = &Tracker;
			MoverProbe->XLevel = TestLevel;
			MoverProbe->Brush = BrushProbe;
			MoverProbe->CollideType = CT_Shape;
			MoverProbe->Location = FVector(0.f, 0.f, 0.f);
			MoverProbe->SavedPos = MoverProbe->Location;
			MoverProbe->bStatic = 0;
			MoverProbe->bMovable = 1;
			MoverProbe->bCollideActors = 1;
			MoverProbe->bCollideWorld = 0;
			MoverProbe->bBlockActors = 1;
			MoverProbe->bBlockPlayers = 1;
			MoverProbe->StandingCount = 0;
			Hash.AddActor(MoverProbe);
			FCheckResult Hit(1.f);
			const FVector Destination(0.f, 0.f, 80.f);
			if (!TestLevel->MoveActor(MoverProbe, Destination - MoverProbe->Location, MoverProbe->Rotation, Hit, 0, 0, 0, 1))
				Result = Fail("zero-time mover fixture did not move");
			else if (MoverProbe->Location != Destination || Hash.RemoveCount != 1 || Hash.AddCount != 2
				|| Hash.ContainedActor != MoverProbe)
				Result = Fail("moved brush was not re-registered in the collision hash at its destination");
			else if (Tracker.UpdateCount != 1 || Tracker.LastActor != MoverProbe
				|| MoverProbe->SavedPos != Destination)
				Result = Fail("moved brush tracker was not synchronized to the mover destination");
			else if (!MoverProbe->bCollideActors || !MoverProbe->bBlockActors || !MoverProbe->bBlockPlayers
				|| MoverProbe->GetPrimitive() != BrushProbe)
				Result = Fail("moved brush lost its blocking flags or brush primitive");
		}
		GIsGuarded = 0;
		appExit();
		GIsStarted = 0;
		return Result;
	}



	int TestNativeRegistration(int ArgC, char** ArgV)
	{
		GTestName = "native_registration";
		InstallHP2NativeLookups();
		static NativeLookup ExpectedLookups[] =
		{
			&FindCoreUObjectNative, &FindCoreUCommandletNative,
			&FindEngineAActorNative, &FindEngineAPawnNative, &FindEngineAPlayerPawnNative,
			&FindEngineADecalNative, &FindEngineAStatLogNative, &FindEngineAStatLogFileNative,
			&FindEngineAZoneInfoNative, &FindEngineAWarpZoneInfoNative, &FindEngineALevelInfoNative,
			&FindEngineAGameInfoNative, &FindEngineANavigationPointNative,
			&FindEngineUCanvasNative, &FindEngineUConsoleNative, &FindEngineUScriptedTextureNative
		};
		for (INT Index = 0; Index < ARRAY_COUNT(ExpectedLookups); ++Index)
		{
			if (GNativeLookupFuncs[Index] != ExpectedLookups[Index])
				return Fail("lookup slot %i missing or out of order", Index);
			for (INT Prior = 0; Prior < Index; ++Prior)
				if (GNativeLookupFuncs[Index] == GNativeLookupFuncs[Prior])
					return Fail("lookup slots %i and %i are duplicates", Prior, Index);
		}
		for (INT Index = ARRAY_COUNT(ExpectedLookups); Index < ARRAY_COUNT(GNativeLookupFuncs); ++Index)
			if (GNativeLookupFuncs[Index] != NULL)
				return Fail("unexpected native lookup at slot %i", Index);
		if (GNativeDuplicate != 0) return Fail("duplicate native slot %i registered", GNativeDuplicate);

#define CHECK_LOOKUP(Class,Name) do { int R = CheckLookup(#Class "." #Name, NATIVE_NAME(Class,Name), (Native)&Class::Name); if (R) return R; } while (0)
		CHECK_LOOKUP(UObject, execLocalVariable);
		CHECK_LOOKUP(UCommandlet, execMain);
		CHECK_LOOKUP(AActor, execMove);
		CHECK_LOOKUP(AActor, execGetCurrentKeyState);
		CHECK_LOOKUP(APawn, execMoveTo);
		CHECK_LOOKUP(APlayerPawn, execUpdateURL);
		CHECK_LOOKUP(ADecal, execAttachDecal);
		CHECK_LOOKUP(AStatLog, execGetMapFileName);
		CHECK_LOOKUP(AStatLogFile, execOpenLog);
		CHECK_LOOKUP(AZoneInfo, execZoneActors);
		CHECK_LOOKUP(AWarpZoneInfo, execWarp);
		CHECK_LOOKUP(ALevelInfo, execGetLocalURL);
		CHECK_LOOKUP(AGameInfo, execParseKillMessage);
		CHECK_LOOKUP(ANavigationPoint, execdescribeSpec);
		CHECK_LOOKUP(UCanvas, execStrLen);
		CHECK_LOOKUP(UConsole, execConsoleCommand);
		CHECK_LOOKUP(UScriptedTexture, execReplaceTexture);
#undef CHECK_LOOKUP
#define CHECK_SLOT(Index,Class,Name) do { if (GNatives[Index] != (Native)&Class::Name) return Fail("native slot %i mismatch for " #Class "." #Name, static_cast<INT>(Index)); } while (0)
		CHECK_SLOT(EX_LocalVariable, UObject, execLocalVariable);
		CHECK_SLOT(EX_InstanceVariable, UObject, execInstanceVariable);
		CHECK_SLOT(EX_IntConst, UObject, execIntConst);
		CHECK_SLOT(EX_Iterator, UObject, execIterator);
		CHECK_SLOT(330, AActor, execGetCurrentKeyState);
#undef CHECK_SLOT
		if (FindNative(TEXT("intNoSuchClassexecNoSuchFunction")) != NULL)
			return Fail("unknown native lookup unexpectedly resolved");
		if (int Result = CheckAllGeneratedNatives()) return Result;
		if (int Result = CheckLookupTable("Core.UObject", GCoreUObjectNatives, 254)) return Result;
		if (int Result = CheckLookupTable("Core.UCommandlet", GCoreUCommandletNatives, 1)) return Result;
		if (int Result = CheckLookupTable("Engine.AActor", GEngineAActorNatives, 60)) return Result;
		if (int Result = CheckLookupTable("Engine.APawn", GEngineAPawnNatives, 34)) return Result;
		if (int Result = CheckLookupTable("Engine.APlayerPawn", GEngineAPlayerPawnNatives, 9)) return Result;
		if (int Result = CheckLookupTable("Engine.ADecal", GEngineADecalNatives, 2)) return Result;
		if (int Result = CheckLookupTable("Engine.AStatLog", GEngineAStatLogNatives, 10)) return Result;
		if (int Result = CheckLookupTable("Engine.AStatLogFile", GEngineAStatLogFileNatives, 6)) return Result;
		if (int Result = CheckLookupTable("Engine.AZoneInfo", GEngineAZoneInfoNatives, 1)) return Result;
		if (int Result = CheckLookupTable("Engine.AWarpZoneInfo", GEngineAWarpZoneInfoNatives, 2)) return Result;
		if (int Result = CheckLookupTable("Engine.ALevelInfo", GEngineALevelInfoNatives, 2)) return Result;
		if (int Result = CheckLookupTable("Engine.AGameInfo", GEngineAGameInfoNatives, 2)) return Result;
		if (int Result = CheckLookupTable("Engine.ANavigationPoint", GEngineANavigationPointNatives, 1)) return Result;
		if (int Result = CheckLookupTable("Engine.UCanvas", GEngineUCanvasNatives, 9)) return Result;
		if (int Result = CheckLookupTable("Engine.UConsole", GEngineUConsoleNatives, 3)) return Result;
		if (int Result = CheckLookupTable("Engine.UScriptedTexture", GEngineUScriptedTextureNatives, 5)) return Result;
		if (int Result = CheckRuntimeProperties(ArgC, ArgV)) return Result;

		const Native Registered = GNatives[EX_InstanceVariable];
		GNativeDuplicate = 0;
		GRegisterNative(EX_InstanceVariable, Registered);
		if (GNativeDuplicate != EX_InstanceVariable)
			return Fail("duplicate registration probe expected slot %i, got %i", static_cast<INT>(EX_InstanceVariable), GNativeDuplicate);
		if (GNatives[EX_InstanceVariable] != Registered)
			return Fail("duplicate registration probe changed slot %i", static_cast<INT>(EX_InstanceVariable));
		GNativeDuplicate = 0;
		return 0;
	}
}

int main(int ArgC, char** ArgV)
{
	const char* Test = NULL;
	for (int Index = 1; Index < ArgC; ++Index)
	{
		if (std::strncmp(ArgV[Index], "--test=", 7) == 0)
		{
			if (Test) return Fail("multiple --test arguments");
			Test = ArgV[Index] + 7;
		}
	}
	if (!Test || !*Test) return Fail("required argument: --test=<abi_widths|render_clip|projection_fov|command_line_load|compact_index|fstring_archive|native_registration>");
	if (std::strcmp(Test, "abi_widths") == 0) return TestAbiWidths();
	if (std::strcmp(Test, "render_clip") == 0) return TestRenderClipClassification();
	if (std::strcmp(Test, "projection_fov") == 0) return TestProjectionFov();
	if (std::strcmp(Test, "command_line_load") == 0) return TestCommandLineLoad();
	if (std::strcmp(Test, "compact_index") == 0) return TestCompactIndex();
	if (std::strcmp(Test, "fstring_archive") == 0) return TestFStringArchive();
	if (std::strcmp(Test, "native_registration") == 0) return TestNativeRegistration(ArgC, ArgV);
	return Fail("unknown --test value '%s'", Test);
}

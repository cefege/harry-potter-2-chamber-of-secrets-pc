/*=============================================================================
	ReplayRoundTripTests.cpp: Byte-level FReplay::FInputEvent stream framing
	contracts and production-oracle fixture emission.

	Owns the wire format of the replay input stream ONLY:
	  - the production operator<<(FArchive&, FReplay::FInputEvent&) compiled
	    from Engine/Src/UnReplayWire.cpp
	  - deterministic full FReplay fixture emission under HP2_ARTIFACT_DIR
	  - round-trip structural + byte-for-byte equality
	  - truncation and trailing-garbage behavior as it exists today
	UInput dispatch semantics are owned by another test suite and deliberately
	not exercised here.
=============================================================================*/

#include <cstddef>
#include <cstdarg>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <CommonCrypto/CommonDigest.h>
#include <string>
#include <vector>

#include "Core.h"
#include "Engine.h"
#include "FMallocAnsi.h"
#include "FFileManagerUnix.h"
// FReplay arrives via Engine.h -> UnGame.h; UnReplay.h itself is unguarded,
// so it must not be included a second time here.

extern "C" { TCHAR GPackage[64] = TEXT("ReplayRoundTripTests"); }

namespace
{
	const char* GTestName = "replay_roundtrip";
	int GFailures = 0;

	int Fail(const char* Format, ...)
	{
		std::fprintf(stderr, "%s: ", GTestName);
		va_list Args;
		va_start(Args, Format);
		std::vfprintf(stderr, Format, Args);
		va_end(Args);
		std::fputc('\n', stderr);
		++GFailures;
		return 1;
	}

	void Require(bool Condition, const char* Message)
	{
		if (!Condition)
			Fail("%s", Message);
	}
	// This small adapter keeps the probes readable while ensuring every event
	// passes through the production operator linked from UnReplayWire.cpp.
	FArchive& EngineOperatorOracle( FArchive& Ar, FReplay::FInputEvent& IE )
	{
		return Ar << IE;
	}


	// In-memory persistent archive. Saving grows a byte sink; loading reads a
	// fixed buffer and degrades gracefully on overrun (error flag + zero-fill +
	// clamp), mirroring how a short file read must never yield stale bytes.
	class FStreamArchive : public FArchive
	{
	public:
		// Named factories instead of overload pairs: a non-const vector
		// lvalue would silently bind to the saving (non-const) constructor,
		// turning every intended load into a sink-append archive.
		static FStreamArchive Saver(std::vector<BYTE>& Sink)
		{
			return FStreamArchive(Sink);
		}
		static FStreamArchive Loader(const std::vector<BYTE>& Bytes)
		{
			return FStreamArchive(Bytes);
		}


		void Serialize(void* V, INT Count) override
		{
			if (Count < 0)
			{
				ArIsError = 1;
				return;
			}
			if (ArIsLoading)
			{
				if (Position < 0 || Position > Limit || Count > Limit - Position)
				{
					ArIsError = 1;
					std::memset(V, 0, static_cast<std::size_t>(Count));
					Position = Limit;
					return;
				}
				if (Count)
					std::memcpy(V, Source + Position, static_cast<std::size_t>(Count));
			}
			else if (Count)
			{
				Sink->insert(Sink->end(), static_cast<BYTE*>(V), static_cast<BYTE*>(V) + Count);
			}
			Position += Count;
		}
		INT Tell() override { return Position; }
		INT TotalSize() override { return ArIsLoading ? Limit : static_cast<INT>(Sink->size()); }
		void Seek(INT NewPosition) override
		{
			const INT Ceiling = ArIsLoading ? Limit : static_cast<INT>(Sink->size());
			if (NewPosition < 0 || NewPosition > Ceiling)
			{
				ArIsError = 1;
				return;
			}
			Position = NewPosition;
		}

	private:
		explicit FStreamArchive(std::vector<BYTE>& InSink)
		: Sink(&InSink), Source(NULL), Limit(0), Position(0)
		{
			ArIsSaving = 1;
			ArIsPersistent = 1;
		}
		explicit FStreamArchive(const std::vector<BYTE>& Bytes)
		: Sink(NULL), Source(Bytes.empty() ? NULL : &Bytes[0]), Limit(static_cast<INT>(Bytes.size())), Position(0)
		{
			ArIsLoading = 1;
			ArIsPersistent = 1;
		}

		std::vector<BYTE>* Sink;
		const BYTE* Source;
		INT Limit;
		INT Position;
	};

	DWORD BitsOf(FLOAT Value)
	{
		DWORD Bits;
		std::memcpy(&Bits, &Value, sizeof(Bits));
		return Bits;
	}
	FLOAT FloatOf(DWORD Bits)
	{
		FLOAT Value;
		std::memcpy(&Value, &Bits, sizeof(Value));
		return Value;
	}

	bool SameEvent(const FReplay::FInputEvent& A, const FReplay::FInputEvent& B)
	{
		return A.iKey == B.iKey && A.State == B.State && BitsOf(A.Delta) == BitsOf(B.Delta);
	}

	void SaveEvent(std::vector<BYTE>& Sink, const FReplay::FInputEvent& IE)
	{
		Sink.clear();
		FStreamArchive Ar = FStreamArchive::Saver(Sink);
		EngineOperatorOracle(Ar, const_cast<FReplay::FInputEvent&>(IE));
	}

	bool ExpectBytes(const std::vector<BYTE>& Actual, const BYTE* Expected, INT ExpectedCount, const char* Context)
	{
		if (static_cast<INT>(Actual.size()) != ExpectedCount)
			return Fail("%s: expected %i wire bytes, got %zu", Context, ExpectedCount, Actual.size());
		for (INT Index = 0; Index < ExpectedCount; ++Index)
		{
			if (Actual[Index] != Expected[Index])
				return Fail("%s: byte %i expected 0x%02X, got 0x%02X", Context, Index,
					static_cast<unsigned>(Expected[Index]), static_cast<unsigned>(Actual[Index]));
		}
		return 0;
	}

	//-------------------------------------------------------------------------
	// Static ABI of the event record and its wire alphabet.
	//-------------------------------------------------------------------------
	void TestStaticLayout()
	{
		GTestName = "replay_static_layout";

		// Host layout: two int-sized enums followed by a float, no padding to
		// depend on anywhere in the record.
		static_assert(sizeof(EInputKey) == 4 && sizeof(EInputAction) == 4,
			"input enums must stay int-sized for the replay record layout");
		static_assert(sizeof(FLOAT) == 4, "delta payload must stay a 32-bit float");
		static_assert(sizeof(FReplay::FInputEvent) == 12,
			"FReplay::FInputEvent must remain {int enum, int enum, float} with no padding");
		static_assert(alignof(FReplay::FInputEvent) == 4,
			"FReplay::FInputEvent must keep natural 4-byte alignment");
		static_assert(offsetof(FReplay::FInputEvent, iKey) == 0, "wire key must be first member");
		static_assert(offsetof(FReplay::FInputEvent, State) == 4, "wire state must follow the key");
		static_assert(offsetof(FReplay::FInputEvent, Delta) == 8, "wire delta must follow the state");

		// The high bit of the state byte is stolen as the has-delta flag, so
		// the action alphabet plus the flag must fit one unsigned byte, and
		// every key code must survive BYTE truncation.
		static_assert((IK_MAX == 255) && (IST_MAX > 0) && (IST_MAX < 0x7F),
			"state-byte high-bit steal requires IK_MAX<=255 and IST_MAX<128");
	}

	//-------------------------------------------------------------------------
	// Single-event packing against literal expected bytes.
	//---------------------------------------------------------------------
	void TestLiteralPacking()
	{
		GTestName = "replay_literal_packing";

		struct Case
		{
			const char* Name;
			EInputKey Key;
			EInputAction State;
			FLOAT Delta;
			BYTE Wire[6];
			INT WireCount;
		};
		static const Case Cases[] =
		{
			// Zero-delta press/release/hold pack as exactly two bytes.
			{"press_A",        IK_A,        IST_Press,   0.f,          {0x41, 0x01}, 2},
			{"release_A",      IK_A,        IST_Release, 0.f,          {0x41, 0x03}, 2},
			{"hold_W",         IK_W,        IST_Hold,    0.f,          {0x57, 0x02}, 2},
			{"press_F17",      IK_F17,      IST_Press,   0.f,          {0x80, 0x01}, 2},
			{"press_max_key",  IK_MAX,      IST_Press,   0.f,          {0xFF, 0x01}, 2},

			// Axis events with nonzero deltas append a little-endian float.
			{"axis_neg_1_5",   IK_MouseX,   IST_Axis,    -1.5f,        {0xE4, 0x84, 0x00, 0x00, 0xC0, 0xBF}, 6},
			{"axis_quarter",   IK_JoyX,     IST_Axis,    0.25f,        {0xE0, 0x84, 0x00, 0x00, 0x80, 0x3E}, 6},
			{"axis_half",      IK_JoyZ,     IST_Axis,    0.5f,         {0xE2, 0x84, 0x00, 0x00, 0x00, 0x3F}, 6},
			{"axis_max",       IK_JoyZ,     IST_Axis,    340282350000000000000000000000000000000.f,
			                                                {0xE2, 0x84, 0xFF, 0xFF, 0x7F, 0x7F}, 6},
			{"axis_neg_max",   IK_Joy1,     IST_None,    -340282350000000000000000000000000000000.f,
			                                                {0xC8, 0x80, 0xFF, 0xFF, 0x7F, 0xFF}, 6},
		};

		for (INT Index = 0; Index < static_cast<INT>(ARRAY_COUNT(Cases)); ++Index)
		{
			const Case& C = Cases[Index];
			std::vector<BYTE> Wire;
			FReplay::FInputEvent IE = { C.Key, C.State, C.Delta };
			SaveEvent(Wire, IE);
			if (ExpectBytes(Wire, C.Wire, C.WireCount, C.Name))
				continue;

			// Round-trip each literal case back through the same operator.
			FStreamArchive Loader = FStreamArchive::Loader(Wire);
			FReplay::FInputEvent Back = { IK_None, IST_None, 12345.f };
			EngineOperatorOracle(Loader, Back);
			if (!Loader.IsError() && !SameEvent(IE, Back))
				Fail("%s: round-trip changed {key=%i state=%i bits=0x%08X} into {key=%i state=%i bits=0x%08X}",
					C.Name, static_cast<int>(C.Key), static_cast<int>(C.State), BitsOf(C.Delta),
					static_cast<int>(Back.iKey), static_cast<int>(Back.State), BitsOf(Back.Delta));
		}

		// The tick sentinel written by SerializeFrameTick is just the operator
		// applied to {IK_Play, IST_Axis, tick}: assert its exact wire shape so
		// frame boundaries in recorded streams stay recognizable.
		{
			std::vector<BYTE> Wire;
			FReplay::FInputEvent Tick = { IK_Play, IST_Axis, 0.5f };
			SaveEvent(Wire, Tick);
			static const BYTE Expected[] = {0xFA, 0x84, 0x00, 0x00, 0x00, 0x3F};
			ExpectBytes(Wire, Expected, static_cast<INT>(sizeof(Expected)), "tick_sentinel");
		}

		// Canonical quiet NaN survives as an exact bit pattern (NaN != 0 sets
		// the delta flag; comparisons would never verify this).
		{
			std::vector<BYTE> Wire;
			FReplay::FInputEvent IE = { IK_Zoom, IST_Hold, FloatOf(0x7FC00000u) };
			SaveEvent(Wire, IE);
			static const BYTE Expected[] = {0xFB, 0x82, 0x00, 0x00, 0xC0, 0x7F};
			if (!ExpectBytes(Wire, Expected, static_cast<INT>(sizeof(Expected)), "nan_delta"))
				return;
			FStreamArchive Loader = FStreamArchive::Loader(Wire);
			FReplay::FInputEvent Back = { IK_None, IST_None, 0.f };
			EngineOperatorOracle(Loader, Back);
			Require(!Loader.IsError(), "nan_delta: load raised an unexpected error");
			Require(BitsOf(Back.Delta) == 0x7FC00000u, "nan_delta: NaN bit pattern did not survive the round trip");
		}

		// Negative zero compares equal to zero, so no delta flag is emitted
		// and the sign is flushed on load. Freeze this documented lossiness.
		{
			std::vector<BYTE> Wire;
			FReplay::FInputEvent IE = { IK_Period, IST_Axis, -0.0f };
			SaveEvent(Wire, IE);
			static const BYTE Expected[] = {0xBE, 0x04};
			if (!ExpectBytes(Wire, Expected, static_cast<INT>(sizeof(Expected)), "negative_zero"))
				return;
			FStreamArchive Loader = FStreamArchive::Loader(Wire);
			FReplay::FInputEvent Back = { IK_None, IST_None, -1.f };
			EngineOperatorOracle(Loader, Back);
			Require(!Loader.IsError(), "negative_zero: load raised an unexpected error");
			Require(BitsOf(Back.Delta) == 0x00000000u,
				"negative_zero: absent-delta load must normalize to positive zero");
		}

		// Serializing the same event twice is deterministic.
		{
			std::vector<BYTE> First, Second;
			FReplay::FInputEvent IE = { IK_MouseY, IST_Axis, -12.5f };
			SaveEvent(First, IE);
			SaveEvent(Second, IE);
			Require(First == Second, "repeated serialization was not byte-identical");
		}
	}

	//-------------------------------------------------------------------------
	// Multi-event sequence: structural equality plus byte-for-byte stream
	// equality after re-serializing what was loaded.
	//-------------------------------------------------------------------------
	void TestSequenceRoundTrip()
	{
		GTestName = "replay_sequence_roundtrip";

		static const FReplay::FInputEvent Sequence[] =
		{
			{IK_Space,  IST_Press,   0.f},
			{IK_D,      IST_Hold,    0.f},
			{IK_Space,  IST_Release, 0.f},
			{IK_E,      IST_Press,   0.f},
			{IK_E,      IST_Release, 0.f},
			{IK_MouseX, IST_Axis,    -12.5f},
			{IK_MouseY, IST_Axis,    0.f},                       // axis with zero delta omits its payload
			{IK_JoyV,   IST_Axis,    340282350000000000000000000000000000000.f},
			{IK_F17,    IST_Press,   0.f},                       // key byte lands on 0x80
			{IK_MAX,    IST_Press,   0.f},
			{IK_Joy1,   IST_None,    -340282350000000000000000000000000000000.f},
			{IK_Period, IST_Press,   0.125f},
			{IK_Play,   IST_Axis,    0.5f},                      // tick sentinel inside the stream
		};

		std::vector<BYTE> Wire;
		{
			FStreamArchive Ar = FStreamArchive::Saver(Wire);
			for (INT Index = 0; Index < static_cast<INT>(ARRAY_COUNT(Sequence)); ++Index)
			{
				FReplay::FInputEvent Copy = Sequence[Index];
				EngineOperatorOracle(Ar, Copy);
				Require(!Ar.IsError(), "sequence save raised an unexpected error");
			}
		}

		// Structural equality of every decoded event, in order.
		std::vector<FReplay::FInputEvent> Loaded;
		{
			FStreamArchive Ar = FStreamArchive::Loader(Wire);
			while (!Ar.AtEnd())
			{
				FReplay::FInputEvent IE = { IK_None, IST_None, 0.f };
				EngineOperatorOracle(Ar, IE);
				Require(!Ar.IsError(), "sequence load raised an unexpected error");
				Loaded.push_back(IE);
			}
		}
		if (static_cast<INT>(Loaded.size()) != static_cast<INT>(ARRAY_COUNT(Sequence)))
		{
			Fail("sequence decode count mismatch: expected %zu events, got %zu",
				ARRAY_COUNT(Sequence), Loaded.size());
			return;
		}
		for (INT Index = 0; Index < static_cast<INT>(ARRAY_COUNT(Sequence)); ++Index)
		{
			if (!SameEvent(Sequence[Index], Loaded[Index]))
			{
				Fail("event %i changed across the round trip: "
					"expected {key=%i state=%i bits=0x%08X}, got {key=%i state=%i bits=0x%08X}",
					Index, static_cast<int>(Sequence[Index].iKey), static_cast<int>(Sequence[Index].State),
					BitsOf(Sequence[Index].Delta),
					static_cast<int>(Loaded[Index].iKey), static_cast<int>(Loaded[Index].State),
					BitsOf(Loaded[Index].Delta));
			}
		}

		// Byte-for-byte stream equality: re-serializing the decoded sequence
		// must reproduce the original stream exactly.
		std::vector<BYTE> Resaved;
		{
			FStreamArchive Ar = FStreamArchive::Saver(Resaved);
			for (std::size_t Index = 0; Index < Loaded.size(); ++Index)
				EngineOperatorOracle(Ar, Loaded[Index]);
		}
		Require(Resaved == Wire, "re-serialized decoded stream diverged from the original bytes");

		// The tick sentinel remains findable after a round trip: the
		// SerializeFrameInput replay loop recognizes frames by
		// iKey==IK_Play && State==IST_Axis, which only survives when the
		// state byte's high-bit flag round-trips through the mask.
		bool FoundTick = false;
		for (std::size_t Index = 0; Index < Loaded.size(); ++Index)
		{
			if (Loaded[Index].iKey == IK_Play && Loaded[Index].State == IST_Axis)
				FoundTick = true;
		}
		Require(FoundTick, "tick sentinel lost its frame-terminator identity across the round trip");
	}

	//-------------------------------------------------------------------------
	// Corruption probes. These freeze current parser behavior; they are not
	// endorsements. No crash, silent garbage acceptance is documented exactly.
	//-------------------------------------------------------------------------
	void TestCorruptionBehavior()
	{
		GTestName = "replay_corruption";

		// Truncation mid-event: a complete press followed by the first three
		// bytes of a delta event. The delta short-read must raise the error
		// flag, zero-fill the payload, and clamp the position.
		{
			std::vector<BYTE> Wire;
			Wire.push_back(0x41); Wire.push_back(0x01);             // press A, complete
			Wire.push_back(0xE4); Wire.push_back(0x84); Wire.push_back(0x00); // axis header + 1 delta byte

			FStreamArchive Ar = FStreamArchive::Loader(Wire);
			FReplay::FInputEvent First = { IK_None, IST_None, 9.f };
			EngineOperatorOracle(Ar, First);
			Require(!Ar.IsError(), "complete leading event flagged an error");
			Require(First.iKey == IK_A && First.State == IST_Press && First.Delta == 0.f,
				"complete leading event decoded incorrectly before truncation");

			FReplay::FInputEvent Second = { IK_None, IST_None, 9.f };
			EngineOperatorOracle(Ar, Second);
			Require(Ar.IsError(), "truncated mid-event delta read failed to raise the error flag");
			Require(Ar.Tell() == 5 && Ar.AtEnd(), "position not clamped to end after truncated read");
			Require(Second.iKey == IK_MouseX, "truncated read corrupted already-received key byte");
			Require(static_cast<int>(Second.State) == IST_Axis, "truncated read corrupted already-received state nibble");
			Require(BitsOf(Second.Delta) == 0x00000000u, "truncated delta must surface as exact zero, not stale memory");

			// Reading past the error keeps reporting errors instead of looping
			// or crashing.
			FReplay::FInputEvent Third = { IK_None, IST_None, 9.f };
			EngineOperatorOracle(Ar, Third);
			Require(Ar.IsError(), "read past exhausted stream cleared the error flag");
			Require(Ar.AtEnd(), "exhausted stream no longer reports AtEnd");
		}

		// Truncation at the very first byte of an event.
		{
			std::vector<BYTE> Wire;
			Wire.push_back(0xC8);
			FStreamArchive Ar = FStreamArchive::Loader(Wire);
			FReplay::FInputEvent IE = { IK_None, IST_None, 9.f };
			EngineOperatorOracle(Ar, IE);
			Require(Ar.IsError(), "single-truncated-event read failed to raise the error flag");
			Require(IE.iKey == EInputKey(200) && IE.State == IST_None && IE.Delta == 0.f,
				"single-byte truncation did not degrade to zero-filled remainder");
		}

		// Trailing garbage that happens to parse: the operator consumes
		// exactly the bytes an event needs and silently accepts out-of-range
		// state values (high bit stripped). Freeze, do not fix.
		{
			std::vector<BYTE> Wire;
			Wire.push_back(0x41); Wire.push_back(0x01);             // press A, complete
			static const BYTE Garbage[] = {0xAB, 0x37, 0xDD, 0xCC, 0xBB, 0xAA};
			Wire.insert(Wire.end(), Garbage, Garbage + static_cast<INT>(sizeof(Garbage)));

			FStreamArchive Ar = FStreamArchive::Loader(Wire);
			FReplay::FInputEvent Good = { IK_None, IST_None, 9.f };
			EngineOperatorOracle(Ar, Good);
			Require(!Ar.IsError() && Good.iKey == IK_A && Good.State == IST_Press,
				"valid event before trailing garbage misdecoded");
			Require(Ar.Tell() == 2, "parser consumed more than the valid event's own bytes");

			FReplay::FInputEvent Junk = { IK_None, IST_None, 9.f };
			EngineOperatorOracle(Ar, Junk);
			Require(!Ar.IsError(), "in-bounds garbage event unexpectedly raised an error");
			Require(Junk.iKey == EInputKey(171) && static_cast<int>(Junk.State) == (0x37 & 0x7F)
				&& BitsOf(Junk.Delta) == 0x00000000u,
				"garbage event did not decode per the frozen no-validation rule");
			// The garbage event's state byte (0x37) carries no delta flag, so
			// the parser consumes exactly key+state and leaves the remaining
			// bytes for subsequent reads. Freeze, do not fix.
			Require(Ar.Tell() == 4 && !Ar.AtEnd(),
				"garbage event must consume exactly its framed key+state bytes");
		}

		// Trailing garbage shorter than any event: partial read errors out but
		// still yields the partially received key byte.
		{
			std::vector<BYTE> Wire;
			Wire.push_back(0x41); Wire.push_back(0x01);
			Wire.push_back(0xC8);
			FStreamArchive Ar = FStreamArchive::Loader(Wire);
			FReplay::FInputEvent Good = { IK_None, IST_None, 9.f };
			EngineOperatorOracle(Ar, Good);
			FReplay::FInputEvent Trailing = { IK_None, IST_None, 9.f };
			EngineOperatorOracle(Ar, Trailing);
			Require(Ar.IsError(), "one-byte trailing garbage failed to raise the error flag");
			Require(Trailing.iKey == EInputKey(200) && Trailing.State == IST_None && Trailing.Delta == 0.f,
				"one-byte trailing garbage did not decode key then zero-fill the rest");
		}

		// The wire's high bit alone decides whether a delta follows; declared
		// event semantics never validate against it. An IST_Axis event whose
		// state byte lacks the flag loads with an exact-zero delta, and an
		// out-of-range masked state value loads unvalidated.
		{
			static const BYTE NoFlag[] = {0x39, 0x04};              // claims IST_Axis, omits delta
			std::vector<BYTE> Wire(NoFlag, NoFlag + sizeof(NoFlag));
			FStreamArchive Ar = FStreamArchive::Loader(Wire);
			FReplay::FInputEvent IE = { IK_None, IST_None, 9.f };
			EngineOperatorOracle(Ar, IE);
			Require(!Ar.IsError() && static_cast<int>(IE.State) == IST_Axis && BitsOf(IE.Delta) == 0x00000000u,
				"flagless axis encoding must load IST_Axis with exact-zero delta");

			static const BYTE OutOfRange[] = {0x39, 0x85, 0x10, 0x20, 0x40, 0x60};
			std::vector<BYTE> Wire2(OutOfRange, OutOfRange + sizeof(OutOfRange));
			FStreamArchive Ar2 = FStreamArchive::Loader(Wire2);
			FReplay::FInputEvent IE2 = { IK_None, IST_None, 0.f };
			EngineOperatorOracle(Ar2, IE2);
			Require(!Ar2.IsError(), "out-of-range state load unexpectedly errored");
			Require(static_cast<int>(IE2.State) == 0x05 && BitsOf(IE2.Delta) == BitsOf(FloatOf(0x60402010u)),
				"out-of-range state/delta pair did not load verbatim under the frozen rule");
		}
	}

	std::string Sha256Hex(const std::vector<BYTE>& Bytes)
	{
		unsigned char Digest[CC_SHA256_DIGEST_LENGTH];
		CC_SHA256(Bytes.empty() ? NULL : &Bytes[0],
			static_cast<CC_LONG>(Bytes.size()), Digest);
		static const char Hex[] = "0123456789abcdef";
		std::string Result(CC_SHA256_DIGEST_LENGTH * 2, '0');
		for (INT Index = 0; Index < CC_SHA256_DIGEST_LENGTH; ++Index)
		{
			Result[Index * 2] = Hex[Digest[Index] >> 4];
			Result[Index * 2 + 1] = Hex[Digest[Index] & 15];
		}
		return Result;
	}

	void EmitProductionFixture(const char* ArtifactDir)
	{
		GTestName = "replay_fixture_emit";
		const char* FixtureName = "cxx-freplay-v1.rep";
		const char* MetadataName = "cxx-freplay-v1.json";
		const FString URL(TEXT("HP2Entry?Game=HP2Game.HP2Game"));
		static const FReplay::FInputEvent Records[] =
		{
			{IK_Play,   IST_Axis,    1.0f / 30.0f}, // frame zero delimiter
			{IK_Space,  IST_Press,   0.f},
			{IK_Space,  IST_Release, 0.f},
			{IK_MouseX, IST_Axis,    -3.25f},
			{IK_Play,   IST_Axis,    0.05f},        // frame one delimiter
			{IK_JoyX,   IST_Axis,    0.375f},
			{IK_LeftMouse, IST_Press, 0.f},
			{IK_LeftMouse, IST_Release, 0.f},
			{IK_Play,   IST_Axis,    0.125f},       // frame two delimiter
		};

		std::vector<BYTE> Wire;
		FStreamArchive Ar = FStreamArchive::Saver(Wire);
		FString URLCopy = URL;
		Ar << URLCopy;
		for (INT Index = 0; Index < static_cast<INT>(ARRAY_COUNT(Records)); ++Index)
		{
			FReplay::FInputEvent Record = Records[Index];
			Ar << Record;
		}
		Require(!Ar.IsError(), "production fixture serialization raised an archive error");

		const std::string FixturePath = std::string(ArtifactDir) + "/" + FixtureName;
		std::ofstream Fixture(FixturePath.c_str(), std::ios::binary | std::ios::trunc);
		if (!Fixture)
		{
			Fail("cannot create replay fixture %s", FixturePath.c_str());
			return;
		}
		if (!Wire.empty())
			Fixture.write(reinterpret_cast<const char*>(&Wire[0]), static_cast<std::streamsize>(Wire.size()));
		Fixture.close();
		if (!Fixture)
		{
			Fail("cannot finish replay fixture %s", FixturePath.c_str());
			return;
		}

		const std::string Hash = Sha256Hex(Wire);
		const std::string MetadataPath = std::string(ArtifactDir) + "/" + MetadataName;
		std::ofstream Metadata(MetadataPath.c_str(), std::ios::trunc);
		if (!Metadata)
		{
			Fail("cannot create replay fixture metadata %s", MetadataPath.c_str());
			return;
		}
		Metadata << "{\"schema_version\":1,\"kind\":\"legacy_cxx_freplay\","
			<< "\"fixture\":\"" << FixtureName << "\","
			<< "\"sha256\":\"" << Hash << "\",\"size\":" << Wire.size() << ","
			<< "\"url\":\"HP2Entry?Game=HP2Game.HP2Game\","
			<< "\"frame_count\":3,\"event_count\":6,"
			<< "\"provenance\":{\"writer\":\"FArchive operators\","
			<< "\"input_event_operator\":\"Engine/Src/UnReplayWire.cpp\","
			<< "\"url_operator\":\"Core FString operator<<\"}}\n";
		Require(static_cast<bool>(Metadata), "cannot finish replay fixture metadata");
	}
}

int main(int ArgC, char** ArgV)
{
	// Must happen before ANY engine allocation: the global operator new
	// overridden in UnFile.h routes through GMalloc, whose Core.cpp default
	// aborts on use. Without this, behavior is layout-dependent (observed as
	// an env-sensitive segfault inside the first vector insert).
	FMallocAnsi Allocator;
	FFileManagerUnix FileManager;
	GMalloc = &Allocator;
	GFileManager = &FileManager;

	TestStaticLayout();
	TestLiteralPacking();
	TestSequenceRoundTrip();
	TestCorruptionBehavior();
	if (const char* ArtifactDir = std::getenv("HP2_ARTIFACT_DIR"))
		EmitProductionFixture(ArtifactDir);

	std::string Command;
	for (int Index = 0; Index < ArgC; ++Index)
	{
		if (Index) Command += ' ';
		Command += ArgV[Index];
	}

	// Report schema v1 (see wave contract): additive structured summary for
	// the verification graph; stdout/stderr and the exit code stay canonical.
	if (const char* ArtifactDir = std::getenv("HP2_ARTIFACT_DIR"))
	{
		std::string Path = std::string(ArtifactDir) + "/replay_roundtrip.json";
		std::ofstream Report(Path.c_str());
		if (Report)
		{
			Report << "{\"schema\":1,\"name\":\"replay_roundtrip\","
				<< "\"status\":\"" << (GFailures ? "fail" : "pass") << "\","
				<< "\"invariant\":\"replay_input_event_stream_framing\""
				<< (GFailures ? ",\"reason_code\":\"replay.framing_mismatch\"" : "")
				<< ",\"data\":{\"profile\":\"data-none\"}"
				<< ",\"artifacts\":[\"cxx-freplay-v1.rep\",\"cxx-freplay-v1.json\"]"
				<< ",\"command\":\"" << Command << "\""
				<< ",\"exit_reason\":\"" << (GFailures ? "assertion_failed" : "assertions_passed") << "\""
				<< "}\n";
		}
	}

	if (GFailures)
	{
		std::fprintf(stderr, "%s: %i failure(s)\n", GTestName, GFailures);
		return 1;
	}
	std::printf("%s: all framing contracts hold\n", GTestName);
	return 0;
}

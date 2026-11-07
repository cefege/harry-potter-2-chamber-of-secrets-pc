#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <new>
#include <sstream>
#include <string>
#include <vector>

#include "Core.h"

static std::size_t GAllocationCount = 0;

class FTestMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* ) override
	{
		++GAllocationCount;
		return std::malloc(Count);
	}
	void* Realloc( void* Original, DWORD Count, const TCHAR* ) override
	{
		++GAllocationCount;
		return std::realloc(Original, Count);
	}
	void Free( void* Original ) override { std::free(Original); }
	void DumpAllocs() override {}
	void HeapCheck() override {}
	void Init() override {}
	void Exit() override {}
};

static FTestMalloc GTestMalloc;
FMalloc* GMalloc = &GTestMalloc;
FOutputDeviceError* GError = NULL;
void FOutputDevice::Logf( const TCHAR*, ... )
{
	std::abort();
}

namespace
{
	static int GFailures = 0;

	static void Require( bool Condition, const char* Message )
	{
		if( !Condition )
		{
			std::fprintf( stderr, "FAIL: %s\n", Message );
			++GFailures;
		}
	}

	static std::uint64_t HashSamples( const SWORD* Samples, INT Count )
	{
		std::uint64_t Hash = UINT64_C(14695981039346656037);
		for( INT Index=0; Index<Count; ++Index )
		{
			const std::uint16_t Value = static_cast<std::uint16_t>(Samples[Index]);
			Hash ^= Value & 0xffu;
			Hash *= UINT64_C(1099511628211);
			Hash ^= Value >> 8;
			Hash *= UINT64_C(1099511628211);
		}
		return Hash;
	}

	static void MakeBlock( BYTE* Block, INT Predictor, INT Range, const BYTE* Packed )
	{
		Block[0] = static_cast<BYTE>((Predictor << 4) | Range);
		std::memcpy( Block + 1, Packed, 14 );
	}

	static void TestPredictorsAndRanges()
	{
		static const BYTE Packed[14] =
		{
			0x81, 0x7f, 0x2e, 0xd3, 0x4c, 0xb5, 0x6a,
			0x99, 0x08, 0xf1, 0x27, 0xc4, 0x5b, 0xae
		};
		static const std::uint64_t PredictorHashes[16] =
		{
			UINT64_C(0xb4392c847559eab0), UINT64_C(0xcfcced6922942062),
			UINT64_C(0x8eb9430a10e4d7ed), UINT64_C(0x70d3f46bd18d4f9a),
			UINT64_C(0xb4392c847559eab0), UINT64_C(0x667cb725f52b5fa0),
			UINT64_C(0xeb71899b19bfd825), UINT64_C(0xebc235d01db9bebb),
			UINT64_C(0x60a3e99686d96303), UINT64_C(0x58fec4079a72a92d),
			UINT64_C(0x4754b6ea623af48c), UINT64_C(0x650ea94c65b964b7),
			UINT64_C(0xf0b9ebc3efa33bc7), UINT64_C(0xc25014e615b4b87b),
			UINT64_C(0x282dc598b6f70193), UINT64_C(0xcc5a4bb1cc87cf0e)
		};
		static const std::uint64_t RangeHashes[16] =
		{
			UINT64_C(0x40257dfe4ebf84af), UINT64_C(0xc97f3a1463e35e11),
			UINT64_C(0xc0b8598699ca9c53), UINT64_C(0x471bbd65322df58f),
			UINT64_C(0x959def15d408035c), UINT64_C(0x58ed65b451034be5),
			UINT64_C(0x4e66574eabc78c88), UINT64_C(0x8eb9430a10e4d7ed),
			UINT64_C(0x6d4cd8671cef73de), UINT64_C(0x5d928d427a5598d1),
			UINT64_C(0x941dfec8b7b31fb0), UINT64_C(0x5df22620fcf7a0a3),
			UINT64_C(0xda3df3b139ecf7cd), UINT64_C(0x0327ac44a6ac36b9),
			UINT64_C(0x38a8e187596c7596), UINT64_C(0x26d2d1209fc7e9b5)
		};

		for( INT Predictor=0; Predictor<16; ++Predictor )
		{
			BYTE Block[15];
			SWORD Output[28];
			MakeBlock( Block, Predictor, 7, Packed );
			FEAXABlockDecoder Decoder;
			Decoder.ResetState();
			const std::size_t Before = GAllocationCount;
			Require( Decoder.Feed(Block, 15, 28) != 0, "predictor Feed rejected" );
			Require( Decoder.Decode(Output, 28) == 28, "predictor sample count" );
			Require( GAllocationCount == Before, "Feed/Decode allocated memory" );
			Require( HashSamples(Output, 28) == PredictorHashes[Predictor], "predictor vector mismatch" );
		}

		for( INT Range=0; Range<16; ++Range )
		{
			BYTE Block[15];
			SWORD Output[28];
			MakeBlock( Block, 2, Range, Packed );
			FEAXABlockDecoder Decoder;
			Decoder.ResetState();
			Require( Decoder.Feed(Block, 15, 28) != 0, "range Feed rejected" );
			Require( Decoder.Decode(Output, 28) == 28, "range sample count" );
			Require( HashSamples(Output, 28) == RangeHashes[Range], "range vector mismatch" );
		}
	}

	static void TestSignedNibblesAndOrder()
	{
		static const BYTE Packed[14] =
		{
			0x81, 0x7f, 0x2e, 0xd3, 0x4c, 0xb5, 0x6a,
			0x99, 0x08, 0xf1, 0x27, 0xc4, 0x5b, 0xae
		};
		static const SWORD Expected[28] =
		{
			-8, 1, 7, -1, 2, -2, -3, 3, 4, -4, -5, 5, 6, -6,
			-7, -7, 0, -8, -1, 1, 2, 7, -4, 4, 5, -5, -6, -2
		};
		BYTE Block[15];
		SWORD Output[28];
		MakeBlock( Block, 0, 12, Packed );
		FEAXABlockDecoder Decoder;
		Require( Decoder.Feed(Block, 15, 28) != 0, "signed-nibble Feed rejected" );
		Require( Decoder.Decode(Output, 28) == 28, "signed-nibble sample count" );
		Require( std::memcmp(Output, Expected, sizeof(Expected)) == 0, "signed nibble or low/high order mismatch" );
	}

	static void TestClipping()
	{
		BYTE PositivePacked[14];
		BYTE NegativePacked[14];
		std::memset( PositivePacked, 0x77, sizeof(PositivePacked) );
		std::memset( NegativePacked, 0x88, sizeof(NegativePacked) );
		BYTE Block[15];
		SWORD Output[28];
		FEAXABlockDecoder Decoder;

		MakeBlock( Block, 2, 0, PositivePacked );
		Decoder.ResetState( 32767, 32767 );
		Require( Decoder.Feed(Block, 15, 28) != 0, "positive clipping Feed rejected" );
		Require( Decoder.Decode(Output, 28) == 28, "positive clipping sample count" );
		for( INT Index=0; Index<28; ++Index )
			Require( Output[Index] == 32767, "positive clipping mismatch" );

		MakeBlock( Block, 2, 0, NegativePacked );
		Decoder.ResetState( -32768, -32768 );
		Require( Decoder.Feed(Block, 15, 28) != 0, "negative clipping Feed rejected" );
		Require( Decoder.Decode(Output, 28) == 28, "negative clipping sample count" );
		for( INT Index=0; Index<28; ++Index )
			Require( Output[Index] == -32768, "negative clipping mismatch" );
	}

	static void TestPartialRequestsCapAndEof()
	{
		static const BYTE Packed[14] =
		{
			0x81, 0x7f, 0x2e, 0xd3, 0x4c, 0xb5, 0x6a,
			0x99, 0x08, 0xf1, 0x27, 0xc4, 0x5b, 0xae
		};
		static const SWORD Expected[28] =
		{
			-8, 1, 7, -1, 2, -2, -3, 3, 4, -4, -5, 5, 6, -6,
			-7, -7, 0, -8, -1, 1, 2, 7, -4, 4, 5, -5, -6, -2
		};
		BYTE Block[15];
		SWORD Output[30];
		std::memset( Output, 0x55, sizeof(Output) );
		MakeBlock( Block, 0, 12, Packed );
		FEAXABlockDecoder Decoder;
		Require( Decoder.Feed(Block, 15, 19) != 0, "partial Feed rejected" );
		Require( Decoder.Decode(Output, 3) == 3, "partial request 1 count" );
		Require( Decoder.Decode(Output + 3, 7) == 7, "partial request 2 count" );
		Require( Decoder.Decode(Output + 10, 100) == 9, "NumSamples cap count" );
		Require( Decoder.Decode(Output + 19, 1) == 0, "EOF did not return zero" );
		Require( std::memcmp(Output, Expected, 19 * sizeof(SWORD)) == 0, "partial output mismatch" );
		Require( Output[19] == static_cast<SWORD>(0x5555), "Decode wrote past sample cap" );
		Require( Decoder.Decode(NULL, 4) == 0, "null destination accepted" );
		Require( Decoder.Decode(Output, 0) == 0, "zero request accepted samples" );
		Require( Decoder.Decode(Output, -1) == 0, "negative request accepted samples" );
	}

	static void TestMalformedFeedIsTransactional()
	{
		BYTE Block[15] = { 0x0c, 0x21, 0x43, 0x65, 0x07, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 };
		SWORD Output[5];
		FEAXABlockDecoder Decoder;
		Require( Decoder.Feed(Block, 15, 5) != 0, "valid baseline Feed rejected" );
		Require( Decoder.Feed(Block, 14, 5) == 0, "partial block accepted" );
		Require( Decoder.Feed(NULL, 15, 1) == 0, "null encoded data accepted" );
		Require( Decoder.Feed(Block, -15, 0) == 0, "negative byte count accepted" );
		Require( Decoder.Feed(Block, 15, -1) == 0, "negative sample count accepted" );
		Require( Decoder.Feed(Block, 15, 29) == 0, "insufficient encoded data accepted" );
		Require( Decoder.Decode(Output, 5) == 5, "rejected Feed changed prior stream" );
		Require( Output[0] == 2 && Output[1] == 1 && Output[2] == 4 && Output[3] == 3 && Output[4] == 6,
			"transactional Feed output mismatch" );
		Require( Decoder.Feed(NULL, 0, 0) != 0, "empty stream rejected" );
		Require( Decoder.Decode(Output, 5) == 0, "empty stream produced samples" );
	}

	static void TestHistoryPreservedAcrossFeed()
	{
		static const SWORD Expected[56] =
		{
			1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792,
			1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792, 1792,
			1680, 1575, 1477, 1385, 1298, 1217, 1141, 1070, 1003, 940, 881, 826, 774, 726,
			681, 638, 598, 561, 526, 493, 462, 433, 406, 381, 357, 335, 314, 294
		};
		BYTE FirstBlock[15];
		std::memset( FirstBlock, 0x77, sizeof(FirstBlock) );
		FirstBlock[0] = 0x04;
		BYTE Block[15] = { 0x1c, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 };
		SWORD Output[56];
		FEAXABlockDecoder Decoder;
		Decoder.ResetState();
		Require( Decoder.Feed(FirstBlock, 15, 28) != 0, "history first Feed rejected" );
		Require( Decoder.Decode(Output, 28) == 28, "history first count" );
		Require( Decoder.Feed(Block, 15, 28) != 0, "history refeed rejected" );
		Require( Decoder.Decode(Output + 28, 28) == 28, "history refeed count" );
		Require( std::memcmp(Output, Expected, sizeof(Expected)) == 0, "history was not preserved across Feed" );
		Decoder.ResetState( 1792, 1792 );
		Require( Decoder.Feed(Block, 15, 28) != 0, "explicit history Feed rejected" );
		Require( Decoder.Decode(Output, 28) == 28, "explicit history sample count" );
		Require( std::memcmp(Output, Expected + 28, 28 * sizeof(SWORD)) == 0, "explicit history mismatch" );

		BYTE TwoBlocks[30];
		std::memcpy( TwoBlocks, FirstBlock, 15 );
		std::memcpy( TwoBlocks + 15, Block, 15 );
		Decoder.ResetState();
		Require( Decoder.Feed(TwoBlocks, 30, 56) != 0, "two-block Feed rejected" );
		Require( Decoder.Decode(Output, 56) == 56, "two-block sample count" );
		Require( std::memcmp(Output, Expected, sizeof(Expected)) == 0, "history was not preserved between blocks" );
	}

	class FSha256
	{
	public:
		FSha256() { Reset(); }

		void Update( const BYTE* Data, std::size_t Size )
		{
			for( std::size_t Index=0; Index<Size; ++Index )
			{
				Buffer[BufferSize++] = Data[Index];
				if( BufferSize == 64 )
				{
					Transform();
					BitCount += 512;
					BufferSize = 0;
				}
			}
		}

		std::string FinalHex()
		{
			BitCount += static_cast<std::uint64_t>(BufferSize) * 8;
			Buffer[BufferSize++] = 0x80;
			if( BufferSize > 56 )
			{
				while( BufferSize < 64 ) Buffer[BufferSize++] = 0;
				Transform();
				BufferSize = 0;
			}
			while( BufferSize < 56 ) Buffer[BufferSize++] = 0;
			for( INT Index=7; Index>=0; --Index )
				Buffer[BufferSize++] = static_cast<BYTE>(BitCount >> (Index * 8));
			Transform();

			static const char Hex[] = "0123456789abcdef";
			std::string Result(64, '0');
			for( INT Word=0; Word<8; ++Word )
				for( INT Nibble=0; Nibble<8; ++Nibble )
					Result[Word * 8 + Nibble] = Hex[(State[Word] >> ((7 - Nibble) * 4)) & 15];
			return Result;
		}

	private:
		static std::uint32_t Rotate( std::uint32_t Value, INT Bits )
		{
			return (Value >> Bits) | (Value << (32 - Bits));
		}

		void Reset()
		{
			State[0]=0x6a09e667u; State[1]=0xbb67ae85u; State[2]=0x3c6ef372u; State[3]=0xa54ff53au;
			State[4]=0x510e527fu; State[5]=0x9b05688cu; State[6]=0x1f83d9abu; State[7]=0x5be0cd19u;
			BitCount = 0;
			BufferSize = 0;
		}

		void Transform()
		{
			static const std::uint32_t K[64] =
			{
				0x428a2f98u,0x71374491u,0xb5c0fbcfu,0xe9b5dba5u,0x3956c25bu,0x59f111f1u,0x923f82a4u,0xab1c5ed5u,
				0xd807aa98u,0x12835b01u,0x243185beu,0x550c7dc3u,0x72be5d74u,0x80deb1feu,0x9bdc06a7u,0xc19bf174u,
				0xe49b69c1u,0xefbe4786u,0x0fc19dc6u,0x240ca1ccu,0x2de92c6fu,0x4a7484aau,0x5cb0a9dcu,0x76f988dau,
				0x983e5152u,0xa831c66du,0xb00327c8u,0xbf597fc7u,0xc6e00bf3u,0xd5a79147u,0x06ca6351u,0x14292967u,
				0x27b70a85u,0x2e1b2138u,0x4d2c6dfcu,0x53380d13u,0x650a7354u,0x766a0abbu,0x81c2c92eu,0x92722c85u,
				0xa2bfe8a1u,0xa81a664bu,0xc24b8b70u,0xc76c51a3u,0xd192e819u,0xd6990624u,0xf40e3585u,0x106aa070u,
				0x19a4c116u,0x1e376c08u,0x2748774cu,0x34b0bcb5u,0x391c0cb3u,0x4ed8aa4au,0x5b9cca4fu,0x682e6ff3u,
				0x748f82eeu,0x78a5636fu,0x84c87814u,0x8cc70208u,0x90befffau,0xa4506cebu,0xbef9a3f7u,0xc67178f2u
			};
			std::uint32_t W[64];
			for( INT Index=0; Index<16; ++Index )
				W[Index] = (static_cast<std::uint32_t>(Buffer[Index*4]) << 24)
					| (static_cast<std::uint32_t>(Buffer[Index*4+1]) << 16)
					| (static_cast<std::uint32_t>(Buffer[Index*4+2]) << 8)
					| Buffer[Index*4+3];
			for( INT Index=16; Index<64; ++Index )
			{
				const std::uint32_t S0 = Rotate(W[Index-15],7) ^ Rotate(W[Index-15],18) ^ (W[Index-15] >> 3);
				const std::uint32_t S1 = Rotate(W[Index-2],17) ^ Rotate(W[Index-2],19) ^ (W[Index-2] >> 10);
				W[Index] = W[Index-16] + S0 + W[Index-7] + S1;
			}
			std::uint32_t A=State[0], B=State[1], C=State[2], D=State[3];
			std::uint32_t E=State[4], F=State[5], G=State[6], H=State[7];
			for( INT Index=0; Index<64; ++Index )
			{
				const std::uint32_t S1 = Rotate(E,6) ^ Rotate(E,11) ^ Rotate(E,25);
				const std::uint32_t Choice = (E & F) ^ (~E & G);
				const std::uint32_t T1 = H + S1 + Choice + K[Index] + W[Index];
				const std::uint32_t S0 = Rotate(A,2) ^ Rotate(A,13) ^ Rotate(A,22);
				const std::uint32_t Majority = (A & B) ^ (A & C) ^ (B & C);
				const std::uint32_t T2 = S0 + Majority;
				H=G; G=F; F=E; E=D+T1; D=C; C=B; B=A; A=T1+T2;
			}
			State[0]+=A; State[1]+=B; State[2]+=C; State[3]+=D;
			State[4]+=E; State[5]+=F; State[6]+=G; State[7]+=H;
		}

		std::uint32_t State[8];
		std::uint64_t BitCount;
		BYTE Buffer[64];
		std::size_t BufferSize;
	};

	static bool IsJsonSpace( char Character )
	{
		return Character == ' ' || Character == '\t' || Character == '\r' || Character == '\n';
	}

	static bool FindUniqueValueStart( const std::string& Object, const char* Key,
		std::size_t& Position )
	{
		const std::string Token = std::string("\"") + Key + "\"";
		Position = Object.find(Token);
		if( Position == std::string::npos
			|| Object.find(Token, Position + Token.size()) != std::string::npos )
			return false;
		Position += Token.size();
		while( Position < Object.size() && IsJsonSpace(Object[Position]) )
			++Position;
		if( Position == Object.size() || Object[Position++] != ':' )
			return false;
		while( Position < Object.size() && IsJsonSpace(Object[Position]) )
			++Position;
		return Position < Object.size();
	}

	static bool HasJsonValueTerminator( const std::string& Object, std::size_t Position )
	{
		while( Position < Object.size() && IsJsonSpace(Object[Position]) )
			++Position;
		return Position < Object.size() && (Object[Position] == ',' || Object[Position] == '}');
	}

	static bool ExtractString( const std::string& Object, const char* Key, std::string& Value )
	{
		std::size_t Position = 0;
		if( !FindUniqueValueStart(Object, Key, Position) || Object[Position++] != '"' )
			return false;
		const std::size_t Start = Position;
		while( Position < Object.size() && Object[Position] != '"' )
		{
			if( Object[Position] == '\\' )
				return false;
			++Position;
		}
		if( Position == Object.size() )
			return false;
		Value = Object.substr(Start, Position - Start);
		return HasJsonValueTerminator(Object, Position + 1);
	}

	static bool ExtractInteger( const std::string& Object, const char* Key, long long& Value )
	{
		std::size_t Position = 0;
		if( !FindUniqueValueStart(Object, Key, Position)
			|| Object[Position] < '0' || Object[Position] > '9' )
			return false;
		const std::size_t Start = Position;
		while( Position < Object.size() && Object[Position] >= '0' && Object[Position] <= '9' )
			++Position;
		if( !HasJsonValueTerminator(Object, Position) )
			return false;
		try { Value = std::stoll(Object.substr(Start, Position - Start)); }
		catch( ... ) { return false; }
		return true;
	}
	static std::uint16_t ReadCorpusLe16( const BYTE* Data )
	{
		return static_cast<std::uint16_t>(Data[0])
			| static_cast<std::uint16_t>(Data[1]) << 8;
	}

	static std::uint32_t ReadCorpusLe32( const BYTE* Data )
	{
		return static_cast<std::uint32_t>(Data[0])
			| static_cast<std::uint32_t>(Data[1]) << 8
			| static_cast<std::uint32_t>(Data[2]) << 16
			| static_cast<std::uint32_t>(Data[3]) << 24;
	}

	static const long long SerializedSoundFlagStreaming = 4;

	static bool HasExactEaxaLayout( long long InputSize, long long NumSamples,
		long long Bits, long long Channels, long long SampleRate )
	{
		if( InputSize < 0 || InputSize > MAXINT || NumSamples <= 0 || NumSamples > MAXINT
			|| Bits != 16 || Channels != 1 || SampleRate <= 0 || SampleRate > MAXINT )
			return false;
		const std::uint64_t Blocks =
			(static_cast<std::uint64_t>(NumSamples) + 27u) / 28u;
		return Blocks * 15u == static_cast<std::uint64_t>(InputSize);
	}

	static bool IsPcmWavePayload( const std::vector<BYTE>& Data )
	{
		if( Data.size() < 12
			|| std::memcmp(Data.data(), "RIFF", 4) != 0
			|| std::memcmp(Data.data() + 8, "WAVE", 4) != 0
			|| static_cast<std::uint64_t>(ReadCorpusLe32(Data.data() + 4)) + 8u != Data.size() )
			return false;

		bool FoundFormat = false;
		bool FoundData = false;
		std::uint16_t BlockAlign = 0;
		std::size_t Offset = 12;
		while( Offset < Data.size() )
		{
			if( Data.size() - Offset < 8 )
				return false;
			const BYTE* Header = Data.data() + Offset;
			const std::size_t ChunkSize = ReadCorpusLe32(Header + 4);
			const std::size_t PayloadOffset = Offset + 8;
			if( ChunkSize > Data.size() - PayloadOffset )
				return false;

			if( std::memcmp(Header, "fmt ", 4) == 0 )
			{
				if( ChunkSize < 16 )
					return false;
				const BYTE* Format = Data.data() + PayloadOffset;
				const std::uint16_t Codec = ReadCorpusLe16(Format);
				const std::uint16_t Channels = ReadCorpusLe16(Format + 2);
				const std::uint32_t SampleRate = ReadCorpusLe32(Format + 4);
				const std::uint32_t ByteRate = ReadCorpusLe32(Format + 8);
				BlockAlign = ReadCorpusLe16(Format + 12);
				const std::uint16_t Bits = ReadCorpusLe16(Format + 14);
				const std::uint32_t ExpectedAlign =
					static_cast<std::uint32_t>(Channels) * static_cast<std::uint32_t>(Bits) / 8u;
				if( Codec != 1 || Channels == 0 || SampleRate == 0
					|| (Bits != 8 && Bits != 16) || ExpectedAlign == 0
					|| BlockAlign != ExpectedAlign
					|| static_cast<std::uint64_t>(ByteRate)
						!= static_cast<std::uint64_t>(SampleRate) * ExpectedAlign )
					return false;
				FoundFormat = true;
			}
			else if( std::memcmp(Header, "data", 4) == 0 )
			{
				if( BlockAlign == 0 || ChunkSize % BlockAlign != 0 )
					return false;
				FoundData = true;
			}

			Offset = PayloadOffset + ChunkSize;
			if( ChunkSize & 1u )
			{
				if( Offset == Data.size() )
					return false;
				++Offset;
			}
		}
		return Offset == Data.size() && FoundFormat && FoundData;
	}

	static bool HasSignature( const std::vector<BYTE>& Data, const char* Signature )
	{
		return Data.size() >= 4 && std::memcmp(Data.data(), Signature, 4) == 0;
	}


	static bool ReadFile( const std::string& Path, std::vector<BYTE>& Bytes )
	{
		std::ifstream Input(Path, std::ios::binary);
		if( !Input ) return false;
		Input.seekg(0, std::ios::end);
		const std::streamoff Size = Input.tellg();
		if( Size < 0 ) return false;
		Input.seekg(0, std::ios::beg);
		Bytes.resize(static_cast<std::size_t>(Size));
		if( Size > 0 ) Input.read(reinterpret_cast<char*>(Bytes.data()), Size);
		return Input.good() || Input.eof();
	}

	static bool ValidateCorpus( const std::string& Directory )
	{
		std::vector<BYTE> JsonBytes;
		if( !ReadFile(Directory + "/index.json", JsonBytes) )
		{
			std::fprintf(stderr, "FAIL: cannot read corpus index.json\n");
			return false;
		}
		const std::string Json(JsonBytes.begin(), JsonBytes.end());
		const std::size_t EntriesKey = Json.find("\"entries\"");
		const std::size_t ArrayStart = EntriesKey == std::string::npos ? std::string::npos : Json.find('[', EntriesKey);
		const std::size_t ArrayEnd = ArrayStart == std::string::npos ? std::string::npos : Json.find(']', ArrayStart);
		if( ArrayStart == std::string::npos || ArrayEnd == std::string::npos )
		{
			std::fprintf(stderr, "FAIL: malformed corpus entries array\n");
			return false;
		}

		const std::string Root = Json.substr(ArrayEnd + 1);
		std::string CorpusFormat;
		long long SchemaVersion = -1;
		long long DeclaredSoundCount = -1;
		if( !ExtractString(Root, "format", CorpusFormat)
			|| !ExtractInteger(Root, "schema_version", SchemaVersion)
			|| !ExtractInteger(Root, "sound_count", DeclaredSoundCount)
			|| CorpusFormat != "hp2-audio-corpus" || SchemaVersion != 2
			|| DeclaredSoundCount <= 0 || DeclaredSoundCount > MAXINT )
		{
			std::fprintf(stderr, "FAIL: invalid audio corpus schema\n");
			return false;
		}

		std::size_t Cursor = ArrayStart + 1;
		INT Entries = 0;
		INT EaxaEntries = 0;
		std::string PreviousObjectPath;
		while( Cursor < ArrayEnd )
		{
			while( Cursor < ArrayEnd && IsJsonSpace(Json[Cursor]) )
				++Cursor;
			if( Cursor == ArrayEnd )
				break;
			if( Json[Cursor] != '{' )
			{
				std::fprintf(stderr, "FAIL: non-object corpus entry\n");
				return false;
			}
			const std::size_t Begin = Cursor;
			const std::size_t End = Json.find('}', Begin);
			if( End == std::string::npos || End > ArrayEnd )
			{
				std::fprintf(stderr, "FAIL: malformed corpus entry\n");
				return false;
			}
			const std::string Object = Json.substr(Begin, End - Begin + 1);
			std::string Filename, ExpectedHash, FileType, Format, ObjectPath, Status;
			long long InputSize = -1, NumSamples = -1, Bits = -1, Channels = -1;
			long long CoreFlags = -1, SampleRate = -1;
			if( !ExtractString(Object, "file", Filename)
				|| !ExtractString(Object, "file_type", FileType)
				|| !ExtractString(Object, "format", Format)
				|| !ExtractString(Object, "input_sha256", ExpectedHash)
				|| !ExtractString(Object, "object_path", ObjectPath)
				|| !ExtractString(Object, "status", Status)
				|| !ExtractInteger(Object, "input_size", InputSize)
				|| !ExtractInteger(Object, "raw_NumSamples", NumSamples)
				|| !ExtractInteger(Object, "bits", Bits)
				|| !ExtractInteger(Object, "channels", Channels)
				|| !ExtractInteger(Object, "core_flags", CoreFlags)
				|| !ExtractInteger(Object, "sample_rate", SampleRate)
				|| InputSize < 0 || InputSize > MAXINT || NumSamples < 0 || NumSamples > MAXINT
				|| Bits < 0 || Bits > MAXINT || Channels < 0 || Channels > MAXINT
				|| CoreFlags < 0 || CoreFlags > MAXINT
				|| SampleRate < 0 || SampleRate > MAXINT || ObjectPath.empty()
				|| ExpectedHash.size() != 64
				|| ExpectedHash.find_first_not_of("0123456789abcdef") != std::string::npos
				|| Filename != ExpectedHash + ".bin"
				|| (!PreviousObjectPath.empty() && ObjectPath <= PreviousObjectPath) )
			{
				std::fprintf(stderr, "FAIL: invalid or unsorted corpus entry\n");
				return false;
			}
			PreviousObjectPath = ObjectPath;

			std::vector<BYTE> Encoded;
			if( !ReadFile(Directory + "/" + Filename, Encoded)
				|| static_cast<long long>(Encoded.size()) != InputSize )
			{
				std::fprintf(stderr, "FAIL: corpus size mismatch for %s\n", Filename.c_str());
				return false;
			}
			FSha256 Sha;
			Sha.Update(Encoded.data(), Encoded.size());
			if( Sha.FinalHex() != ExpectedHash )
			{
				std::fprintf(stderr, "FAIL: corpus SHA-256 mismatch for %s\n", Filename.c_str());
				return false;
			}

			if( Format == "eaxa" )
			{
				if( FileType != "XA" || Status != "valid"
					|| !(CoreFlags & SerializedSoundFlagStreaming)
					|| !HasExactEaxaLayout(InputSize, NumSamples, Bits, Channels, SampleRate) )
				{
					std::fprintf(stderr, "FAIL: malformed claimed EA-XA entry %s\n", ObjectPath.c_str());
					return false;
				}

				std::vector<SWORD> First(static_cast<std::size_t>(NumSamples));
				std::vector<SWORD> Second(static_cast<std::size_t>(NumSamples));
				for( INT Pass=0; Pass<2; ++Pass )
				{
					FEAXABlockDecoder Decoder;
					if( !Decoder.Feed(Encoded.data(), static_cast<INT>(Encoded.size()), static_cast<INT>(NumSamples)) )
					{
						std::fprintf(stderr, "FAIL: corpus Feed rejected %s\n", Filename.c_str());
						return false;
					}
					std::vector<SWORD>& Output = Pass == 0 ? First : Second;
					INT Produced = 0;
					while( Produced < NumSamples )
					{
						INT Request = static_cast<INT>(NumSamples) - Produced;
						if( Request > 257 )
							Request = 257;
						const INT Count = Decoder.Decode(Output.data() + Produced, Request);
						if( Count <= 0 )
							break;
						Produced += Count;
					}
					if( Produced != NumSamples || Decoder.Decode(Output.data(), 1) != 0 )
					{
						std::fprintf(stderr, "FAIL: corpus sample count mismatch for %s\n", Filename.c_str());
						return false;
					}
				}
				if( First != Second )
				{
					std::fprintf(stderr, "FAIL: nondeterministic corpus decode for %s\n", Filename.c_str());
					return false;
				}
				++EaxaEntries;
			}
			else if( Format == "pcm-wave" )
			{
				if( FileType != "wav" || Status != "valid"
					|| (CoreFlags & SerializedSoundFlagStreaming) || !IsPcmWavePayload(Encoded) )
				{
					std::fprintf(stderr, "FAIL: malformed PCM WAVE entry %s\n", ObjectPath.c_str());
					return false;
				}
			}
			else if( Format == "ogg-vorbis" )
			{
				if( FileType != "ogg" || Status != "valid"
					|| !(CoreFlags & SerializedSoundFlagStreaming) || !HasSignature(Encoded, "OggS") )
				{
					std::fprintf(stderr, "FAIL: malformed Ogg Vorbis entry %s\n", ObjectPath.c_str());
					return false;
				}
			}
			else if( Format == "empty" )
			{
				if( FileType != "None" || Status != "empty"
					|| (CoreFlags & SerializedSoundFlagStreaming) || !Encoded.empty()
					|| NumSamples != 0 || Bits != 0 || Channels != 0 || SampleRate != 0 )
				{
					std::fprintf(stderr, "FAIL: malformed empty sound entry %s\n", ObjectPath.c_str());
					return false;
				}
			}
			else
			{
				std::fprintf(stderr, "FAIL: unsupported or malformed sound format for %s\n", ObjectPath.c_str());
				return false;
			}

			++Entries;
			Cursor = End + 1;
			while( Cursor < ArrayEnd && IsJsonSpace(Json[Cursor]) )
				++Cursor;
			if( Cursor < ArrayEnd )
			{
				if( Json[Cursor++] != ',' )
				{
					std::fprintf(stderr, "FAIL: missing corpus entry separator\n");
					return false;
				}
				while( Cursor < ArrayEnd && IsJsonSpace(Json[Cursor]) )
					++Cursor;
				if( Cursor == ArrayEnd )
				{
					std::fprintf(stderr, "FAIL: trailing corpus entry separator\n");
					return false;
				}
			}
		}
		if( Entries != DeclaredSoundCount || EaxaEntries == 0 )
		{
			std::fprintf(stderr, "FAIL: corpus entry count mismatch or no EA-XA entries\n");
			return false;
		}
		std::printf("validated %d audio corpus entries (%d EA-XA decoded)\n", Entries, EaxaEntries);
		return true;
	}
}

int main( int ArgC, char** ArgV )
{
	TestPredictorsAndRanges();
	TestSignedNibblesAndOrder();
	TestClipping();
	TestPartialRequestsCapAndEof();
	TestMalformedFeedIsTransactional();
	TestHistoryPreservedAcrossFeed();

	if( ArgC == 2 && std::strncmp(ArgV[1], "--corpus=", 9) == 0 )
	{
		if( !ValidateCorpus(ArgV[1] + 9) ) ++GFailures;
	}
	else if( ArgC != 1 )
	{
		std::fprintf(stderr, "usage: %s [--corpus=<dir>]\n", ArgV[0]);
		++GFailures;
	}

	if( GFailures != 0 )
	{
		std::fprintf(stderr, "%d EAXA test failure(s)\n", GFailures);
		return 1;
	}
	std::printf("EAXA decoder tests passed\n");
	return 0;
}

/*=============================================================================
	PackageAudit.cpp: Deterministic package-79 runtime audit and audio corpus dump.
=============================================================================*/

#include "Engine.h"
#include "FConfigCacheIni.h"
#include "FFeedbackContextAnsi.h"
#include "FFileManagerUnix.h"
#include "FMallocAnsi.h"
#include "FOutputDeviceAnsiError.h"
#include "FOutputDeviceFile.h"
#include "HP2Paths.h"
#include "HP2StaticPackages.h"
#include "UnLinker.h"

#include <algorithm>
#include <array>
#include <cerrno>
#include <cstdint>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <limits>
#include <map>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#if !defined(_WIN32)
#include <unistd.h>
#endif

INT GFilesOpen = 0;
INT GFilesOpened = 0;
extern "C" { TCHAR GPackage[64] = TEXT("PackageAudit"); }

namespace
{

constexpr std::uint32_t PackageTag = 0x9e2a83c1u;
constexpr std::uint32_t PackageVersion = 79u;
constexpr std::uint32_t AuditMinimumPackageVersion = 60u;
constexpr std::uint32_t AuditMaximumPackageVersion = 79u;
constexpr std::uint32_t LicenseeVersion = 0u;
constexpr std::uint32_t FunctionFlagNet = 0x00000040u;
constexpr std::uint32_t FunctionFlagNetReliable = 0x00000080u;
constexpr std::uint32_t FunctionFlagNative = 0x00000400u;
constexpr std::uint32_t FunctionFlagMask = 0x0001ffffu;
constexpr std::uint32_t MaximumNativeIndex = 0x1000u;
constexpr std::size_t MaximumNameUnits = 128u;

const char* CurrentAuditStage = "appInit";

struct PackageSpec
{
	const char* ManifestPath;
	const char* PackageName;
	const char* LinkerPath;
};

constexpr PackageSpec PackageSpecs[] =
{
	{ "HarryPotter2/Unreal/System/Core.u", "Core", "Core.u" },
	{ "HarryPotter2/Unreal/System/Engine.u", "Engine", "Engine.u" },
	{ "HarryPotter2/Unreal/System/HGame.u", "HGame", "HGame.u" },
	{ "HarryPotter2/Unreal/Maps/startup.unr", "startup", "../Maps/startup.unr" },
	{ "HarryPotter2/Unreal/Textures/HP2_Master.utx", "HP2_Master", "../Textures/HP2_Master.utx" },
	{ "HarryPotter2/Unreal/Sounds/General.uax", "General", "../Sounds/General.uax" },
};

class AuditError : public std::runtime_error
{
public:
	explicit AuditError(const std::string& Message)
		: std::runtime_error(Message)
	{}
};

std::string HexByte(unsigned int Value)
{
	static constexpr char Digits[] = "0123456789abcdef";
	std::string Result(2, '0');
	Result[0] = Digits[(Value >> 4) & 15u];
	Result[1] = Digits[Value & 15u];
	return Result;
}

void AppendUtf8(std::string& Output, std::uint32_t Scalar)
{
	if (Scalar <= 0x7fu)
	{
		Output.push_back(static_cast<char>(Scalar));
	}
	else if (Scalar <= 0x7ffu)
	{
		Output.push_back(static_cast<char>(0xc0u | (Scalar >> 6)));
		Output.push_back(static_cast<char>(0x80u | (Scalar & 0x3fu)));
	}
	else if (Scalar <= 0xffffu)
	{
		Output.push_back(static_cast<char>(0xe0u | (Scalar >> 12)));
		Output.push_back(static_cast<char>(0x80u | ((Scalar >> 6) & 0x3fu)));
		Output.push_back(static_cast<char>(0x80u | (Scalar & 0x3fu)));
	}
	else
	{
		Output.push_back(static_cast<char>(0xf0u | (Scalar >> 18)));
		Output.push_back(static_cast<char>(0x80u | ((Scalar >> 12) & 0x3fu)));
		Output.push_back(static_cast<char>(0x80u | ((Scalar >> 6) & 0x3fu)));
		Output.push_back(static_cast<char>(0x80u | (Scalar & 0x3fu)));
	}
}

std::basic_string<TCHAR> Utf8ToTchar(const std::string& Input)
{
	std::basic_string<TCHAR> Result;
	Result.reserve(Input.size());
	for (std::size_t Index = 0; Index < Input.size();)
	{
		const std::uint8_t First = static_cast<std::uint8_t>(Input[Index++]);
		std::uint32_t Scalar = 0;
		unsigned int Continuations = 0;
		std::uint32_t Minimum = 0;
		if (First < 0x80u)
		{
			Scalar = First;
		}
		else if ((First & 0xe0u) == 0xc0u)
		{
			Scalar = First & 0x1fu;
			Continuations = 1;
			Minimum = 0x80u;
		}
		else if ((First & 0xf0u) == 0xe0u)
		{
			Scalar = First & 0x0fu;
			Continuations = 2;
			Minimum = 0x800u;
		}
		else if ((First & 0xf8u) == 0xf0u)
		{
			Scalar = First & 0x07u;
			Continuations = 3;
			Minimum = 0x10000u;
		}
		else
		{
			throw AuditError("invalid UTF-8 leading byte in path");
		}
		if (Index + Continuations > Input.size())
			throw AuditError("truncated UTF-8 path");
		for (unsigned int Count = 0; Count < Continuations; ++Count)
		{
			const std::uint8_t Byte = static_cast<std::uint8_t>(Input[Index++]);
			if ((Byte & 0xc0u) != 0x80u)
				throw AuditError("invalid UTF-8 continuation byte in path");
			Scalar = (Scalar << 6) | (Byte & 0x3fu);
		}
		if (Scalar < Minimum || Scalar > 0x10ffffu || (Scalar >= 0xd800u && Scalar <= 0xdfffu))
			throw AuditError("invalid Unicode scalar in path");
		Result.push_back(static_cast<TCHAR>(Scalar));
	}
	return Result;
}

std::string TcharToUtf8(const TCHAR* Input)
{
	std::string Result;
	if (!Input)
		return Result;
	for (; *Input; ++Input)
	{
		const std::uint32_t Scalar = static_cast<std::uint32_t>(*Input);
		if (Scalar > 0x10ffffu || (Scalar >= 0xd800u && Scalar <= 0xdfffu))
			throw AuditError("runtime produced an invalid host TCHAR");
		AppendUtf8(Result, Scalar);
	}
	return Result;
}

bool RuntimeTextEquals(const TCHAR* Runtime, const std::string& SerializedUtf8)
{
	const std::basic_string<TCHAR> Serialized = Utf8ToTchar(SerializedUtf8);
	return appStricmp(Runtime, Serialized.c_str()) == 0;
}

std::vector<std::uint8_t> ReadFile(const std::filesystem::path& Filename)
{
	std::ifstream Stream(Filename, std::ios::binary);
	if (!Stream)
		throw AuditError("cannot open " + Filename.string());
	Stream.seekg(0, std::ios::end);
	const std::streamoff End = Stream.tellg();
	if (End < 0 || static_cast<std::uintmax_t>(End) > std::numeric_limits<std::size_t>::max())
		throw AuditError("invalid file size for " + Filename.string());
	Stream.seekg(0, std::ios::beg);
	std::vector<std::uint8_t> Result(static_cast<std::size_t>(End));
	if (!Result.empty())
		Stream.read(reinterpret_cast<char*>(Result.data()), static_cast<std::streamsize>(Result.size()));
	if (!Stream || Stream.peek() != std::char_traits<char>::eof())
		throw AuditError("short or unstable read of " + Filename.string());
	return Result;
}

std::string ReadTextFile(const std::filesystem::path& Filename)
{
	const std::vector<std::uint8_t> Bytes = ReadFile(Filename);
	return std::string(reinterpret_cast<const char*>(Bytes.data()), Bytes.size());
}

class Sha256
{
public:
	Sha256()
		: State{ 0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
			0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u }
		, TotalBytes(0)
		, Buffered(0)
	{}

	void Update(const std::uint8_t* Data, std::size_t Size)
	{
		if (Size > std::numeric_limits<std::uint64_t>::max() - TotalBytes)
			throw AuditError("SHA-256 input is too large");
		TotalBytes += static_cast<std::uint64_t>(Size);
		while (Size)
		{
			const std::size_t Available = Block.size() - Buffered;
			const std::size_t Copy = Size < Available ? Size : Available;
			std::memcpy(Block.data() + Buffered, Data, Copy);
			Buffered += Copy;
			Data += Copy;
			Size -= Copy;
			if (Buffered == Block.size())
			{
				Transform(Block.data());
				Buffered = 0;
			}
		}
	}

	std::array<std::uint8_t, 32> Final()
	{
		const std::uint64_t Bits = TotalBytes * 8u;
		Block[Buffered++] = 0x80u;
		if (Buffered > 56u)
		{
			while (Buffered < 64u)
				Block[Buffered++] = 0;
			Transform(Block.data());
			Buffered = 0;
		}
		while (Buffered < 56u)
			Block[Buffered++] = 0;
		for (unsigned int Index = 0; Index < 8u; ++Index)
			Block[56u + Index] = static_cast<std::uint8_t>(Bits >> (56u - Index * 8u));
		Transform(Block.data());

		std::array<std::uint8_t, 32> Digest{};
		for (std::size_t Word = 0; Word < State.size(); ++Word)
			for (unsigned int Byte = 0; Byte < 4u; ++Byte)
				Digest[Word * 4u + Byte] = static_cast<std::uint8_t>(State[Word] >> (24u - Byte * 8u));
		return Digest;
	}

private:
	static std::uint32_t RotateRight(std::uint32_t Value, unsigned int Bits)
	{
		return (Value >> Bits) | (Value << (32u - Bits));
	}

	void Transform(const std::uint8_t* Data)
	{
		static constexpr std::uint32_t Constants[64] =
		{
			0x428a2f98u,0x71374491u,0xb5c0fbcfu,0xe9b5dba5u,0x3956c25bu,0x59f111f1u,0x923f82a4u,0xab1c5ed5u,
			0xd807aa98u,0x12835b01u,0x243185beu,0x550c7dc3u,0x72be5d74u,0x80deb1feu,0x9bdc06a7u,0xc19bf174u,
			0xe49b69c1u,0xefbe4786u,0x0fc19dc6u,0x240ca1ccu,0x2de92c6fu,0x4a7484aau,0x5cb0a9dcu,0x76f988dau,
			0x983e5152u,0xa831c66du,0xb00327c8u,0xbf597fc7u,0xc6e00bf3u,0xd5a79147u,0x06ca6351u,0x14292967u,
			0x27b70a85u,0x2e1b2138u,0x4d2c6dfcu,0x53380d13u,0x650a7354u,0x766a0abbu,0x81c2c92eu,0x92722c85u,
			0xa2bfe8a1u,0xa81a664bu,0xc24b8b70u,0xc76c51a3u,0xd192e819u,0xd6990624u,0xf40e3585u,0x106aa070u,
			0x19a4c116u,0x1e376c08u,0x2748774cu,0x34b0bcb5u,0x391c0cb3u,0x4ed8aa4au,0x5b9cca4fu,0x682e6ff3u,
			0x748f82eeu,0x78a5636fu,0x84c87814u,0x8cc70208u,0x90befffau,0xa4506cebu,0xbef9a3f7u,0xc67178f2u,
		};
		std::uint32_t Words[64];
		for (unsigned int Index = 0; Index < 16u; ++Index)
		{
			const unsigned int Offset = Index * 4u;
			Words[Index] = (static_cast<std::uint32_t>(Data[Offset]) << 24)
				| (static_cast<std::uint32_t>(Data[Offset + 1]) << 16)
				| (static_cast<std::uint32_t>(Data[Offset + 2]) << 8)
				| static_cast<std::uint32_t>(Data[Offset + 3]);
		}
		for (unsigned int Index = 16u; Index < 64u; ++Index)
		{
			const std::uint32_t S0 = RotateRight(Words[Index - 15u], 7u)
				^ RotateRight(Words[Index - 15u], 18u) ^ (Words[Index - 15u] >> 3u);
			const std::uint32_t S1 = RotateRight(Words[Index - 2u], 17u)
				^ RotateRight(Words[Index - 2u], 19u) ^ (Words[Index - 2u] >> 10u);
			Words[Index] = Words[Index - 16u] + S0 + Words[Index - 7u] + S1;
		}

		std::uint32_t A = State[0], B = State[1], C = State[2], D = State[3];
		std::uint32_t E = State[4], F = State[5], G = State[6], H = State[7];
		for (unsigned int Index = 0; Index < 64u; ++Index)
		{
			const std::uint32_t S1 = RotateRight(E, 6u) ^ RotateRight(E, 11u) ^ RotateRight(E, 25u);
			const std::uint32_t Choice = (E & F) ^ (~E & G);
			const std::uint32_t Temporary1 = H + S1 + Choice + Constants[Index] + Words[Index];
			const std::uint32_t S0 = RotateRight(A, 2u) ^ RotateRight(A, 13u) ^ RotateRight(A, 22u);
			const std::uint32_t Majority = (A & B) ^ (A & C) ^ (B & C);
			const std::uint32_t Temporary2 = S0 + Majority;
			H = G; G = F; F = E; E = D + Temporary1;
			D = C; C = B; B = A; A = Temporary1 + Temporary2;
		}
		State[0] += A; State[1] += B; State[2] += C; State[3] += D;
		State[4] += E; State[5] += F; State[6] += G; State[7] += H;
	}

	std::array<std::uint32_t, 8> State;
	std::uint64_t TotalBytes;
	std::array<std::uint8_t, 64> Block{};
	std::size_t Buffered;
};

std::string Sha256Hex(const std::uint8_t* Data, std::size_t Size)
{
	Sha256 Hash;
	Hash.Update(Data, Size);
	const std::array<std::uint8_t, 32> Digest = Hash.Final();
	std::string Result;
	Result.reserve(64);
	for (std::uint8_t Byte : Digest)
		Result += HexByte(Byte);
	return Result;
}

std::string Sha256Hex(const std::vector<std::uint8_t>& Data, std::size_t Offset, std::size_t Size)
{
	if (Offset > Data.size() || Size > Data.size() - Offset)
		throw AuditError("SHA-256 span is outside package data");
	return Sha256Hex(Data.data() + Offset, Size);
}

class Cursor
{
public:
	Cursor(const std::vector<std::uint8_t>& InData, std::string InPath, std::size_t InOffset, std::string InContext)
		: Data(InData), Path(std::move(InPath)), Position(InOffset), Context(std::move(InContext))
	{}

	std::size_t Tell() const { return Position; }

	std::uint8_t U8()
	{
		Require(1);
		return Data[Position++];
	}

	std::uint16_t U16()
	{
		Require(2);
		const std::uint16_t Result = static_cast<std::uint16_t>(Data[Position])
			| static_cast<std::uint16_t>(Data[Position + 1] << 8);
		Position += 2;
		return Result;
	}

	std::uint32_t U32()
	{
		Require(4);
		const std::uint32_t Result = static_cast<std::uint32_t>(Data[Position])
			| (static_cast<std::uint32_t>(Data[Position + 1]) << 8)
			| (static_cast<std::uint32_t>(Data[Position + 2]) << 16)
			| (static_cast<std::uint32_t>(Data[Position + 3]) << 24);
		Position += 4;
		return Result;
	}

	std::int32_t I32() { return static_cast<std::int32_t>(U32()); }

	std::int32_t CompactIndex()
	{
		const std::size_t Start = Position;
		const std::uint8_t First = U8();
		std::uint64_t Magnitude = First & 0x3fu;
		unsigned int Shift = 6u;
		std::uint8_t Byte = First;
		unsigned int Count = 1u;
		while (Byte & (Count == 1u ? 0x40u : 0x80u))
		{
			if (Count == 5u)
				Fail(Start, "compact index exceeds five bytes");
			Byte = U8();
			++Count;
			Magnitude |= static_cast<std::uint64_t>(Byte & 0x7fu) << Shift;
			Shift += 7u;
			if (Count == 5u)
				break;
		}
		const std::int64_t Signed = (First & 0x80u)
			? -static_cast<std::int64_t>(Magnitude) : static_cast<std::int64_t>(Magnitude);
		if (Signed < std::numeric_limits<std::int32_t>::min() || Signed > std::numeric_limits<std::int32_t>::max())
			Fail(Start, "compact index is outside int32");
		const std::int32_t Result = static_cast<std::int32_t>(Signed);
		const std::vector<std::uint8_t> Canonical = EncodeCompactIndex(Result);
		if (Canonical.size() != Position - Start
			|| !std::equal(Canonical.begin(), Canonical.end(), Data.begin() + static_cast<std::ptrdiff_t>(Start)))
			Fail(Start, "non-canonical compact index");
		return Result;
	}

	std::string NameString(std::string& Encoding, std::int32_t& SerializedCount)
	{
		const std::size_t Start = Position;
		SerializedCount = CompactIndex();
		const std::uint64_t Units64 = SerializedCount < 0
			? static_cast<std::uint64_t>(-static_cast<std::int64_t>(SerializedCount))
			: static_cast<std::uint64_t>(SerializedCount);
		if (!Units64)
			Fail(Start, "name FString has zero serialized characters");
		if (Units64 > MaximumNameUnits)
			Fail(Start, "name FString exceeds NAME_SIZE");
		const std::size_t Units = static_cast<std::size_t>(Units64);
		std::string Result;
		if (SerializedCount > 0)
		{
			Require(Units);
			if (Data[Position + Units - 1] != 0)
				Fail(Start, "ANSI name is not NUL-terminated");
			for (std::size_t Index = 0; Index + 1 < Units; ++Index)
			{
				if (Data[Position + Index] == 0)
					Fail(Start, "ANSI name contains an embedded NUL");
				AppendUtf8(Result, Data[Position + Index]);
			}
			Position += Units;
			Encoding = "ansi";
		}
		else
		{
			Require(Units * 2u);
			std::vector<std::uint16_t> CodeUnits;
			CodeUnits.reserve(Units);
			for (std::size_t Index = 0; Index < Units; ++Index)
			{
				const std::uint16_t Unit = static_cast<std::uint16_t>(Data[Position + Index * 2u])
					| static_cast<std::uint16_t>(Data[Position + Index * 2u + 1u] << 8);
				CodeUnits.push_back(Unit);
			}
			if (CodeUnits.back() != 0)
				Fail(Start, "UTF-16 name is not NUL-terminated");
			for (std::size_t Index = 0; Index + 1 < CodeUnits.size(); ++Index)
			{
				std::uint32_t Scalar = CodeUnits[Index];
				if (!Scalar)
					Fail(Start, "UTF-16 name contains an embedded NUL");
				if (Scalar >= 0xd800u && Scalar <= 0xdbffu)
				{
					if (Index + 2 >= CodeUnits.size())
						Fail(Start, "UTF-16 name has an unmatched high surrogate");
					const std::uint32_t Low = CodeUnits[++Index];
					if (Low < 0xdc00u || Low > 0xdfffu)
						Fail(Start, "UTF-16 name has an unmatched high surrogate");
					Scalar = 0x10000u + ((Scalar - 0xd800u) << 10) + (Low - 0xdc00u);
				}
				else if (Scalar >= 0xdc00u && Scalar <= 0xdfffu)
				{
					Fail(Start, "UTF-16 name has an unmatched low surrogate");
				}
				AppendUtf8(Result, Scalar);
			}
			Position += Units * 2u;
			Encoding = "utf-16le";
		}
		if (Result.empty())
			Fail(Start, "empty package name entry");
		return Result;
	}

	std::string AnsiCString()
	{
		// Pre-64 name entry: NUL-terminated ANSI, no length prefix (UnName.cpp era).
		const std::size_t Start = Position;
		for (;;)
		{
			Require(1);
			if (!Data[Position])
				break;
			++Position;
		}
		const std::size_t Units = Position - Start + 1u;
		if (Units > MaximumNameUnits)
			Fail(Start, "pre-64 ANSI name entry exceeds NAME_SIZE");
		std::string Result;
		Result.reserve(Units);
		for (std::size_t Index = Start; Index < Position; ++Index)
			AppendUtf8(Result, Data[Index]);
		if (Result.empty())
			Fail(Start, "empty package name entry");
		++Position;
		return Result;
	}

	[[noreturn]] void Fail(std::size_t Offset, const std::string& Message) const
	{
		throw AuditError(Path + ": offset " + std::to_string(Offset) + ": " + Message);
	}

private:
	void Require(std::size_t Size) const
	{
		if (Position > Data.size() || Size > Data.size() - Position)
			Fail(Position, "truncated " + Context);
	}

	static std::vector<std::uint8_t> EncodeCompactIndex(std::int32_t Value)
	{
		const bool Negative = Value < 0;
		std::uint64_t Magnitude = Negative
			? static_cast<std::uint64_t>(-static_cast<std::int64_t>(Value))
			: static_cast<std::uint64_t>(Value);
		std::vector<std::uint8_t> Result;
		std::uint8_t First = static_cast<std::uint8_t>(Magnitude & 0x3fu);
		Magnitude >>= 6;
		First |= Negative ? 0x80u : 0u;
		First |= Magnitude ? 0x40u : 0u;
		Result.push_back(First);
		while (Magnitude)
		{
			std::uint8_t Byte = static_cast<std::uint8_t>(Magnitude & 0x7fu);
			Magnitude >>= 7;
			if (Result.size() < 4u && Magnitude)
				Byte |= 0x80u;
			Result.push_back(Byte);
		}
		return Result;
	}

	const std::vector<std::uint8_t>& Data;
	std::string Path;
	std::size_t Position;
	std::string Context;
};

struct GenerationRecord
{
	std::int32_t ExportCount;
	std::int32_t NameCount;
};

struct SummaryRecord
{
	std::uint32_t Tag;
	std::uint32_t VersionWord;
	std::uint32_t PackageFlags;
	std::int32_t NameCount;
	std::int32_t NameOffset;
	std::int32_t ExportCount;
	std::int32_t ExportOffset;
	std::int32_t ImportCount;
	std::int32_t ImportOffset;
	std::uint32_t Guid[4];
	std::vector<GenerationRecord> Generations;
	std::size_t HeaderSize;
};

struct NameRecord
{
	std::string Text;
	std::string Encoding;
	std::uint32_t Flags;
	std::int32_t SerializedCount;
	std::size_t RecordOffset;
	std::size_t RecordSize;
};

struct ImportRecord
{
	std::int32_t ClassPackageIndex;
	std::int32_t ClassNameIndex;
	std::int32_t OuterRef;
	std::int32_t ObjectNameIndex;
	std::size_t RecordOffset;
	std::size_t RecordSize;
};

struct ExportRecord
{
	std::int32_t ClassRef;
	std::int32_t SuperRef;
	std::int32_t OuterRef;
	std::int32_t ObjectNameIndex;
	std::uint32_t ObjectFlags;
	std::int32_t SerialSize;
	std::optional<std::int32_t> SerialOffset;
	std::size_t RecordOffset;
	std::size_t RecordSize;
};

struct NativeRecord
{
	std::size_t ExportIndex;
	std::uint32_t FunctionFlags;
	std::uint16_t NativeIndex;
	std::string ObjectPath;
	std::uint8_t OperatorPrecedence;
	std::size_t PayloadEnd;
	std::size_t PayloadOffset;
	std::size_t PayloadSize;
	std::optional<std::uint16_t> RepOffset;
	std::size_t TrailerOffset;
};

class PackageData
{
public:
	PackageData(const PackageSpec& InSpec, std::vector<std::uint8_t> InBytes,
		std::uint32_t InMinimumPackageVersion = PackageVersion,
		std::uint32_t InMaximumPackageVersion = PackageVersion)
		: Spec(InSpec), Bytes(std::move(InBytes))
		, MinimumPackageVersion(InMinimumPackageVersion), MaximumPackageVersion(InMaximumPackageVersion)
	{}

	void Parse()
	{
		ParseSummary();
		ParseNames();
		ParseImports();
		ParseExports();
		ValidateAndResolve();
		ParseNativeFunctions();
	}

	void ValidateRuntimeLinker(ULinkerLoad* Linker) const
	{
		if (!Linker)
			throw AuditError(std::string(Spec.ManifestPath) + ": runtime linker is null");
		const FPackageFileSummary& Runtime = Linker->Summary;
		RequireRuntime(static_cast<std::uint32_t>(Runtime.Tag) == Summary.Tag, "summary tag");
		RequireRuntime(static_cast<std::uint32_t>(Runtime.GetFileVersion()) == (Summary.VersionWord & 0xffffu), "package version");
		RequireRuntime(static_cast<std::uint32_t>(Runtime.GetFileVersionLicensee()) == (Summary.VersionWord >> 16), "licensee version");
		RequireRuntime(static_cast<std::uint32_t>(Runtime.PackageFlags) == Summary.PackageFlags, "package flags");
		RequireRuntime(Runtime.NameCount == Summary.NameCount && Runtime.NameOffset == Summary.NameOffset, "name table summary");
		RequireRuntime(Runtime.ImportCount == Summary.ImportCount && Runtime.ImportOffset == Summary.ImportOffset, "import table summary");
		RequireRuntime(Runtime.ExportCount == Summary.ExportCount && Runtime.ExportOffset == Summary.ExportOffset, "export table summary");
		RequireRuntime(Runtime.Generations.Num() == static_cast<INT>(Summary.Generations.size()), "generation count");
		for (INT Index = 0; Index < Runtime.Generations.Num(); ++Index)
		{
			RequireRuntime(Runtime.Generations(Index).ExportCount == Summary.Generations[static_cast<std::size_t>(Index)].ExportCount,
				"generation export count");
			RequireRuntime(Runtime.Generations(Index).NameCount == Summary.Generations[static_cast<std::size_t>(Index)].NameCount,
				"generation name count");
		}
		RequireRuntime(Linker->NameMap.Num() == static_cast<INT>(Names.size()), "runtime name count");
		for (INT Index = 0; Index < Linker->NameMap.Num(); ++Index)
			RequireRuntime(RuntimeTextEquals(*Linker->NameMap(Index), Names[static_cast<std::size_t>(Index)].Text),
				"runtime name " + std::to_string(Index));
		RequireRuntime(Linker->ImportMap.Num() == static_cast<INT>(Imports.size()), "runtime import count");
		for (INT Index = 0; Index < Linker->ImportMap.Num(); ++Index)
		{
			const FObjectImport& RuntimeImport = Linker->ImportMap(Index);
			const ImportRecord& Parsed = Imports[static_cast<std::size_t>(Index)];
			RequireRuntime(RuntimeTextEquals(*RuntimeImport.ClassPackage, Names[static_cast<std::size_t>(Parsed.ClassPackageIndex)].Text),
				"runtime import class package " + std::to_string(Index));
			RequireRuntime(RuntimeTextEquals(*RuntimeImport.ClassName, Names[static_cast<std::size_t>(Parsed.ClassNameIndex)].Text),
				"runtime import class name " + std::to_string(Index));
			RequireRuntime(RuntimeImport.PackageIndex == Parsed.OuterRef, "runtime import outer " + std::to_string(Index));
			RequireRuntime(RuntimeTextEquals(*RuntimeImport.ObjectName, Names[static_cast<std::size_t>(Parsed.ObjectNameIndex)].Text),
				"runtime import object name " + std::to_string(Index));
		}
		RequireRuntime(Linker->ExportMap.Num() == static_cast<INT>(Exports.size()), "runtime export count");
		for (INT Index = 0; Index < Linker->ExportMap.Num(); ++Index)
		{
			const FObjectExport& RuntimeExport = Linker->ExportMap(Index);
			const ExportRecord& Parsed = Exports[static_cast<std::size_t>(Index)];
			RequireRuntime(RuntimeExport.ClassIndex == Parsed.ClassRef, "runtime export class " + std::to_string(Index));
			RequireRuntime(RuntimeExport.SuperIndex == Parsed.SuperRef, "runtime export super " + std::to_string(Index));
			RequireRuntime(RuntimeExport.PackageIndex == Parsed.OuterRef, "runtime export outer " + std::to_string(Index));
			RequireRuntime(RuntimeTextEquals(*RuntimeExport.ObjectName, Names[static_cast<std::size_t>(Parsed.ObjectNameIndex)].Text),
				"runtime export object name " + std::to_string(Index));
			RequireRuntime(static_cast<std::uint32_t>(RuntimeExport.ObjectFlags) == Parsed.ObjectFlags,
				"runtime export flags " + std::to_string(Index));
			RequireRuntime(RuntimeExport.SerialSize == Parsed.SerialSize,
				"runtime export serial size " + std::to_string(Index));
			RequireRuntime(RuntimeExport.SerialOffset == Parsed.SerialOffset.value_or(0),
				"runtime export serial offset " + std::to_string(Index));
			const std::string RuntimeClassPath = TcharToUtf8(*Linker->GetExportClassPackage(Index)) + "."
				+ TcharToUtf8(*Linker->GetExportClassName(Index));
			RequireRuntime(RuntimeTextEquals(Utf8ToTchar(RuntimeClassPath).c_str(), ClassPath(static_cast<std::size_t>(Index))),
				"resolved runtime export class path " + std::to_string(Index));
		}
	}

	const PackageSpec& GetSpec() const { return Spec; }
	const std::vector<std::uint8_t>& GetBytes() const { return Bytes; }
	const SummaryRecord& GetSummary() const { return Summary; }
	const std::vector<NameRecord>& GetNames() const { return Names; }
	const std::vector<ImportRecord>& GetImports() const { return Imports; }
	const std::vector<ExportRecord>& GetExports() const { return Exports; }
	const std::vector<NativeRecord>& GetNativeRecords() const { return NativeRecords; }
	std::size_t GetNamesEnd() const { return NamesEnd; }
	std::size_t GetImportsEnd() const { return ImportsEnd; }
	std::size_t GetExportsEnd() const { return ExportsEnd; }

	std::optional<std::string> RefPath(std::int32_t Ref) const
	{
		if (!Ref)
			return std::nullopt;
		if (Ref > 0)
			return ExportPath(static_cast<std::size_t>(Ref - 1));
		return ImportPath(static_cast<std::size_t>(-static_cast<std::int64_t>(Ref) - 1));
	}

	const std::string& ImportPath(std::size_t Index) const
	{
		ResolveImport(Index);
		return ImportPaths[Index];
	}

	const std::string& ExportPath(std::size_t Index) const
	{
		ResolveExport(Index);
		return ExportPaths[Index];
	}

	std::string ClassPath(std::size_t ExportIndex) const
	{
		const ExportRecord& Entry = Exports[ExportIndex];
		return Entry.ClassRef == 0 ? "Core.Class" : RefPath(Entry.ClassRef).value();
	}

	void ParseSummary()
	{
		Cursor Input(Bytes, Spec.ManifestPath, 0, "package summary");
		Summary.Tag = Input.U32();
		Summary.VersionWord = Input.U32();
		Summary.PackageFlags = Input.U32();
		Summary.NameCount = Input.I32();
		Summary.NameOffset = Input.I32();
		Summary.ExportCount = Input.I32();
		Summary.ExportOffset = Input.I32();
		Summary.ImportCount = Input.I32();
		Summary.ImportOffset = Input.I32();
		const std::uint32_t Version = Summary.VersionWord & 0xffffu;
		const bool HasGenerations = Version >= 68u;
		if (HasGenerations)
			for (std::uint32_t& Word : Summary.Guid)
				Word = Input.U32();
		const std::int32_t GenerationCount = HasGenerations ? Input.I32() : 0;
		const std::size_t GenerationCountOffset = Input.Tell() - 4u;
		std::int32_t HeritageCount = 0;
		std::int32_t HeritageOffset = 0;
		if (!HasGenerations)
		{
			HeritageCount = Input.I32();
			HeritageOffset = Input.I32();
		}
		if (Summary.Tag != PackageTag)
			Input.Fail(0, "bad package tag");
		if (Version < MinimumPackageVersion || Version > MaximumPackageVersion
			|| (Summary.VersionWord >> 16) != LicenseeVersion)
			Input.Fail(4, "unsupported package version/licensee");
		if (HasGenerations)
		{
			ValidateCount(Input, GenerationCount, GenerationCountOffset, "generation count");
			if (!GenerationCount)
				Input.Fail(GenerationCountOffset, "package has no generations");
			if (static_cast<std::uint64_t>(GenerationCount) > (Bytes.size() - Input.Tell()) / 8u)
				Input.Fail(GenerationCountOffset, "generation table is truncated");
		}
		else
		{
			ValidateCount(Input, HeritageCount, Input.Tell() - 8u, "heritage count");
			ValidateOffset(Input, HeritageOffset, Input.Tell() - 4u, "heritage offset");
		}
		ValidateCount(Input, Summary.NameCount, 12, "name count");
		ValidateCount(Input, Summary.ExportCount, 20, "export count");
		ValidateCount(Input, Summary.ImportCount, 28, "import count");
		for (std::int32_t Index = 0; Index < GenerationCount; ++Index)
		{
			GenerationRecord Record{ Input.I32(), Input.I32() };
			ValidateCount(Input, Record.ExportCount, Input.Tell() - 8u, "generation export count");
			ValidateCount(Input, Record.NameCount, Input.Tell() - 4u, "generation name count");
			Summary.Generations.push_back(Record);
		}
		Summary.HeaderSize = Input.Tell();
		if (HasGenerations
			&& (Summary.Generations.back().ExportCount != Summary.ExportCount
				|| Summary.Generations.back().NameCount != Summary.NameCount))
			Input.Fail(GenerationCountOffset, "latest generation counts disagree with summary");
		ValidateOffset(Input, Summary.NameOffset, 16, "name offset");
		ValidateOffset(Input, Summary.ExportOffset, 24, "export offset");
		ValidateOffset(Input, Summary.ImportOffset, 32, "import offset");
		if (HasGenerations && static_cast<std::size_t>(Summary.NameOffset) != Summary.HeaderSize)
			Input.Fail(16, "name table does not immediately follow summary");
		if (!HasGenerations && static_cast<std::size_t>(Summary.NameOffset) < Summary.HeaderSize)
			Input.Fail(16, "name table begins before end of heritage summary");
	}

	void ParseNames()
	{
		const bool Pre64Names = (Summary.VersionWord & 0xffffu) < 64u;
		Cursor Input(Bytes, Spec.ManifestPath, static_cast<std::size_t>(Summary.NameOffset), "name table");
		Names.reserve(static_cast<std::size_t>(Summary.NameCount));
		for (std::int32_t Index = 0; Index < Summary.NameCount; ++Index)
		{
			NameRecord Record;
			Record.RecordOffset = Input.Tell();
			if (Pre64Names)
			{
				Record.Text = Input.AnsiCString();
				Record.Encoding = "ansi";
				Record.SerializedCount = static_cast<std::int32_t>(Record.Text.size() + 1u);
			}
			else
			{
				Record.Text = Input.NameString(Record.Encoding, Record.SerializedCount);
			}
			Record.Flags = Input.U32();
			Record.RecordSize = Input.Tell() - Record.RecordOffset;
			Names.push_back(std::move(Record));
		}
		NamesEnd = Input.Tell();
	}

	void ParseImports()
	{
		Cursor Input(Bytes, Spec.ManifestPath, static_cast<std::size_t>(Summary.ImportOffset), "import table");
		Imports.reserve(static_cast<std::size_t>(Summary.ImportCount));
		for (std::int32_t Index = 0; Index < Summary.ImportCount; ++Index)
		{
			ImportRecord Record;
			Record.RecordOffset = Input.Tell();
			Record.ClassPackageIndex = Input.CompactIndex();
			Record.ClassNameIndex = Input.CompactIndex();
			Record.OuterRef = Input.I32();
			Record.ObjectNameIndex = Input.CompactIndex();
			Record.RecordSize = Input.Tell() - Record.RecordOffset;
			ValidateNameIndex(Input, Record.ClassPackageIndex, Record.RecordOffset, "import class package");
			ValidateNameIndex(Input, Record.ClassNameIndex, Record.RecordOffset, "import class name");
			ValidateNameIndex(Input, Record.ObjectNameIndex, Record.RecordOffset, "import object name");
			Imports.push_back(Record);
		}
		ImportsEnd = Input.Tell();
	}

	void ParseExports()
	{
		Cursor Input(Bytes, Spec.ManifestPath, static_cast<std::size_t>(Summary.ExportOffset), "export table");
		Exports.reserve(static_cast<std::size_t>(Summary.ExportCount));
		for (std::int32_t Index = 0; Index < Summary.ExportCount; ++Index)
		{
			ExportRecord Record;
			Record.RecordOffset = Input.Tell();
			Record.ClassRef = Input.CompactIndex();
			Record.SuperRef = Input.CompactIndex();
			Record.OuterRef = Input.I32();
			Record.ObjectNameIndex = Input.CompactIndex();
			Record.ObjectFlags = Input.U32();
			Record.SerialSize = Input.CompactIndex();
			if (Record.SerialSize < 0)
				Input.Fail(Record.RecordOffset, "negative export serial size");
			if (Record.SerialSize)
			{
				Record.SerialOffset = Input.CompactIndex();
				if (*Record.SerialOffset < 0)
					Input.Fail(Record.RecordOffset, "negative export serial offset");
			}
			Record.RecordSize = Input.Tell() - Record.RecordOffset;
			ValidateNameIndex(Input, Record.ObjectNameIndex, Record.RecordOffset, "export object name");
			Exports.push_back(Record);
		}
		ExportsEnd = Input.Tell();
	}

	void ValidateAndResolve()
	{
		Cursor Input(Bytes, Spec.ManifestPath, 0, "validation");
		for (const ImportRecord& Entry : Imports)
			ValidateObjectRef(Input, Entry.OuterRef, Entry.RecordOffset, "import outer");
		for (const ExportRecord& Entry : Exports)
		{
			ValidateObjectRef(Input, Entry.ClassRef, Entry.RecordOffset, "export class");
			ValidateObjectRef(Input, Entry.SuperRef, Entry.RecordOffset, "export super");
			ValidateObjectRef(Input, Entry.OuterRef, Entry.RecordOffset, "export outer");
		}
		if (NamesEnd > static_cast<std::size_t>(Summary.ImportOffset)
			|| Summary.ImportOffset > Summary.ExportOffset)
			Input.Fail(NamesEnd, "name/payload/import/export regions are out of order");
		if (ImportsEnd != static_cast<std::size_t>(Summary.ExportOffset))
			Input.Fail(ImportsEnd, "import table does not end at export offset");
		if (ExportsEnd != Bytes.size())
			Input.Fail(ExportsEnd, "export table does not end at file size");

		struct Span { std::size_t Start, End, Index; };
		std::vector<Span> Spans;
		for (std::size_t Index = 0; Index < Exports.size(); ++Index)
		{
			const ExportRecord& Entry = Exports[Index];
			if (!Entry.SerialSize)
			{
				if (Entry.SerialOffset)
					Input.Fail(Entry.RecordOffset, "zero-size export has an offset");
				continue;
			}
			const std::size_t Start = static_cast<std::size_t>(*Entry.SerialOffset);
			const std::size_t Size = static_cast<std::size_t>(Entry.SerialSize);
			if (Start < NamesEnd || Start > Bytes.size() || Size > Bytes.size() - Start
				|| Start + Size > static_cast<std::size_t>(Summary.ImportOffset))
				Input.Fail(Entry.RecordOffset, "export serial span is outside payload region");
			Spans.push_back({ Start, Start + Size, Index });
		}
		std::sort(Spans.begin(), Spans.end(), [](const Span& A, const Span& B)
		{
			if (A.Start != B.Start) return A.Start < B.Start;
			if (A.End != B.End) return A.End < B.End;
			return A.Index < B.Index;
		});
		std::size_t Expected = NamesEnd;
		for (const Span& Value : Spans)
		{
			if (Value.Start != Expected)
				Input.Fail(Value.Start, "export serial spans are not contiguous");
			Expected = Value.End;
		}
		if (Expected != static_cast<std::size_t>(Summary.ImportOffset))
			Input.Fail(Expected, "serial payloads do not end at import offset");

		ImportPaths.resize(Imports.size());
		ExportPaths.resize(Exports.size());
		ImportStates.assign(Imports.size(), 0);
		ExportStates.assign(Exports.size(), 0);
		for (std::size_t Index = 0; Index < Imports.size(); ++Index)
			ResolveImport(Index);
		for (std::size_t Index = 0; Index < Exports.size(); ++Index)
			ResolveExport(Index);
	}

	void ParseNativeFunctions()
	{
		for (std::size_t Index = 0; Index < Exports.size(); ++Index)
		{
			const ExportRecord& Entry = Exports[Index];
			const std::string Class = ClassPath(Index);
			if (Class != "Core.Function"
				&& (Class.size() < 9u || Class.compare(Class.size() - 9u, 9u, ".Function") != 0))
				continue;
			if (!Entry.SerialOffset || Entry.SerialSize < 7)
				throw AuditError(std::string(Spec.ManifestPath) + ": Function export is too small");
			const std::size_t PayloadOffset = static_cast<std::size_t>(*Entry.SerialOffset);
			const std::size_t PayloadSize = static_cast<std::size_t>(Entry.SerialSize);
			const std::size_t PayloadEnd = PayloadOffset + PayloadSize;
			std::vector<NativeRecord> Candidates;
			for (const bool IsNet : { false, true })
			{
				const std::size_t TrailerSize = IsNet ? 9u : 7u;
				if (TrailerSize > PayloadSize)
					continue;
				const std::size_t TrailerOffset = PayloadEnd - TrailerSize;
				const std::uint16_t NativeIndex = ReadU16(TrailerOffset);
				const std::uint8_t Precedence = Bytes[TrailerOffset + 2u];
				const std::uint32_t Flags = ReadU32(TrailerOffset + 3u);
				if (NativeIndex >= MaximumNativeIndex || (Flags & ~FunctionFlagMask))
					continue;
				if (((Flags & FunctionFlagNet) != 0) != IsNet)
					continue;
				if ((Flags & FunctionFlagNetReliable) && !IsNet)
					continue;
				if (NativeIndex && !(Flags & FunctionFlagNative))
					continue;
				NativeRecord Candidate{ Index, Flags, NativeIndex, ExportPath(Index), Precedence,
					PayloadEnd, PayloadOffset, PayloadSize, std::nullopt, TrailerOffset };
				if (IsNet)
					Candidate.RepOffset = ReadU16(TrailerOffset + 7u);
				Candidates.push_back(std::move(Candidate));
			}
			if (Candidates.size() != 1u)
				throw AuditError(std::string(Spec.ManifestPath) + ": Function has ambiguous terminal fields at export "
					+ std::to_string(Index));
			NativeRecords.push_back(std::move(Candidates.front()));
		}
	}

	void ResolveImport(std::size_t Index) const
	{
		if (Index >= Imports.size())
			throw AuditError(std::string(Spec.ManifestPath) + ": import path index is out of range");
		if (ImportStates[Index] == 2)
			return;
		if (ImportStates[Index] == 1)
			throw AuditError(std::string(Spec.ManifestPath) + ": cycle in import/export outer references");
		ImportStates[Index] = 1;
		const ImportRecord& Entry = Imports[Index];
		const std::optional<std::string> Outer = RefPath(Entry.OuterRef);
		ImportPaths[Index] = Outer ? *Outer + "." + Names[static_cast<std::size_t>(Entry.ObjectNameIndex)].Text
			: Names[static_cast<std::size_t>(Entry.ObjectNameIndex)].Text;
		ImportStates[Index] = 2;
	}

	void ResolveExport(std::size_t Index) const
	{
		if (Index >= Exports.size())
			throw AuditError(std::string(Spec.ManifestPath) + ": export path index is out of range");
		if (ExportStates[Index] == 2)
			return;
		if (ExportStates[Index] == 1)
			throw AuditError(std::string(Spec.ManifestPath) + ": cycle in import/export outer references");
		ExportStates[Index] = 1;
		const ExportRecord& Entry = Exports[Index];
		const std::optional<std::string> OuterRefPath = RefPath(Entry.OuterRef);
		const std::string& Outer = OuterRefPath ? *OuterRefPath : Spec.PackageName;
		ExportPaths[Index] = Outer + "." + Names[static_cast<std::size_t>(Entry.ObjectNameIndex)].Text;
		ExportStates[Index] = 2;
	}

	void ValidateCount(const Cursor& Input, std::int32_t Value, std::size_t Offset, const char* Label) const
	{
		if (Value < 0 || static_cast<std::uint64_t>(Value) > Bytes.size())
			Input.Fail(Offset, std::string("invalid ") + Label);
	}

	void ValidateOffset(const Cursor& Input, std::int32_t Value, std::size_t Offset, const char* Label) const
	{
		if (Value < 0 || static_cast<std::uint64_t>(Value) > Bytes.size())
			Input.Fail(Offset, std::string("invalid ") + Label);
	}

	void ValidateNameIndex(const Cursor& Input, std::int32_t Value, std::size_t Offset, const char* Label) const
	{
		if (Value < 0 || static_cast<std::size_t>(Value) >= Names.size())
			Input.Fail(Offset, std::string(Label) + " is out of range");
	}

	void ValidateObjectRef(const Cursor& Input, std::int32_t Ref, std::size_t Offset, const char* Label) const
	{
		if ((Ref > 0 && static_cast<std::size_t>(Ref) > Exports.size())
			|| (Ref < 0 && static_cast<std::uint64_t>(-static_cast<std::int64_t>(Ref)) > Imports.size()))
			Input.Fail(Offset, std::string(Label) + " is out of range");
	}

	std::uint16_t ReadU16(std::size_t Offset) const
	{
		if (Offset > Bytes.size() || 2u > Bytes.size() - Offset)
			throw AuditError(std::string(Spec.ManifestPath) + ": truncated uint16");
		return static_cast<std::uint16_t>(Bytes[Offset])
			| static_cast<std::uint16_t>(Bytes[Offset + 1u] << 8);
	}

	std::uint32_t ReadU32(std::size_t Offset) const
	{
		if (Offset > Bytes.size() || 4u > Bytes.size() - Offset)
			throw AuditError(std::string(Spec.ManifestPath) + ": truncated uint32");
		return static_cast<std::uint32_t>(Bytes[Offset])
			| (static_cast<std::uint32_t>(Bytes[Offset + 1u]) << 8)
			| (static_cast<std::uint32_t>(Bytes[Offset + 2u]) << 16)
			| (static_cast<std::uint32_t>(Bytes[Offset + 3u]) << 24);
	}

	void RequireRuntime(bool Condition, const std::string& Field) const
	{
		if (!Condition)
			throw AuditError(std::string(Spec.ManifestPath) + ": runtime linker mismatch: " + Field);
	}

	const PackageSpec& Spec;
	std::uint32_t MinimumPackageVersion;
	std::uint32_t MaximumPackageVersion;
	std::vector<std::uint8_t> Bytes;
	SummaryRecord Summary{};
	std::vector<NameRecord> Names;
	std::vector<ImportRecord> Imports;
	std::vector<ExportRecord> Exports;
	std::vector<NativeRecord> NativeRecords;
	std::size_t NamesEnd = 0;
	std::size_t ImportsEnd = 0;
	std::size_t ExportsEnd = 0;
	mutable std::vector<std::string> ImportPaths;
	mutable std::vector<std::string> ExportPaths;
	mutable std::vector<std::uint8_t> ImportStates;
	mutable std::vector<std::uint8_t> ExportStates;
};

class JsonWriter
{
public:
	void BeginObject() { BeforeValue(); Output += '{'; Stack.push_back({ Kind::Object, true, false }); }
	void EndObject()
	{
		Require(!Stack.empty() && Stack.back().Type == Kind::Object && !Stack.back().AwaitingValue, "invalid JSON object close");
		const bool Empty = Stack.back().First;
		Stack.pop_back();
		if (!Empty) { Output += '\n'; Indent(); }
		Output += '}';
	}
	void BeginArray() { BeforeValue(); Output += '['; Stack.push_back({ Kind::Array, true, false }); }
	void EndArray()
	{
		Require(!Stack.empty() && Stack.back().Type == Kind::Array, "invalid JSON array close");
		const bool Empty = Stack.back().First;
		Stack.pop_back();
		if (!Empty) { Output += '\n'; Indent(); }
		Output += ']';
	}
	void Key(const char* Name)
	{
		Require(!Stack.empty() && Stack.back().Type == Kind::Object && !Stack.back().AwaitingValue, "invalid JSON key");
		Context& Current = Stack.back();
		if (!Current.First) Output += ',';
		Output += '\n';
		Indent();
		AppendString(Name);
		Output += ": ";
		Current.First = false;
		Current.AwaitingValue = true;
	}
	void String(const std::string& Value) { BeforeValue(); AppendString(Value); }
	void String(const char* Value) { String(std::string(Value)); }
	void Number(std::int64_t Value) { BeforeValue(); Output += std::to_string(Value); }
	void Unsigned(std::uint64_t Value) { BeforeValue(); Output += std::to_string(Value); }
	void Null() { BeforeValue(); Output += "null"; }
	// Emits a pre-serialized JSON value produced by an independent JsonWriter at
	// depth zero. Structural newlines only (string escapes never emit raw '\n'),
	// so re-indenting every continuation line by the target depth is exact.
	void RawValue(const std::string& Fragment)
	{
		Require(!Stack.empty(), "raw JSON value outside a container");
		const std::size_t Depth = Stack.size();
		BeforeValue();
		std::size_t Start = 0;
		for (;;)
		{
			const std::size_t Line = Fragment.find('\n', Start);
			if (Line == std::string::npos)
			{
				Output.append(Fragment, Start, std::string::npos);
				break;
			}
			Output.append(Fragment, Start, Line - Start);
			Output += '\n';
			Output.append(Depth * 2u, ' ');
			Start = Line + 1u;
		}
	}
	std::string Finish()
	{
		Require(Stack.empty(), "unfinished JSON document");
		Output += '\n';
		return std::move(Output);
	}

private:
	enum class Kind { Object, Array };
	struct Context { Kind Type; bool First; bool AwaitingValue; };

	void BeforeValue()
	{
		if (Stack.empty())
		{
			Require(Output.empty(), "multiple JSON roots");
			return;
		}
		Context& Current = Stack.back();
		if (Current.Type == Kind::Object)
		{
			Require(Current.AwaitingValue, "JSON object value has no key");
			Current.AwaitingValue = false;
		}
		else
		{
			if (!Current.First) Output += ',';
			Output += '\n';
			Indent();
			Current.First = false;
		}
	}

	void Indent() { Output.append(Stack.size() * 2u, ' '); }

	void AppendString(const std::string& Value)
	{
		static constexpr char Digits[] = "0123456789abcdef";
		Output += '"';
		for (unsigned char Byte : Value)
		{
			switch (Byte)
			{
			case '"': Output += "\\\""; break;
			case '\\': Output += "\\\\"; break;
			case '\b': Output += "\\b"; break;
			case '\f': Output += "\\f"; break;
			case '\n': Output += "\\n"; break;
			case '\r': Output += "\\r"; break;
			case '\t': Output += "\\t"; break;
			default:
				if (Byte < 0x20u)
				{
					Output += "\\u00";
					Output += Digits[(Byte >> 4) & 15u];
					Output += Digits[Byte & 15u];
				}
				else
				{
					Output += static_cast<char>(Byte);
				}
			}
		}
		Output += '"';
	}

	void Require(bool Condition, const char* Message)
	{
		if (!Condition) throw AuditError(Message);
	}

	std::string Output;
	std::vector<Context> Stack;
};

void WriteNameReference(JsonWriter& Json, const PackageData& Package, std::int32_t NameIndex)
{
	Json.BeginObject();
	Json.Key("index"); Json.Number(NameIndex);
	Json.Key("text"); Json.String(Package.GetNames()[static_cast<std::size_t>(NameIndex)].Text);
	Json.EndObject();
}

void WriteRegion(JsonWriter& Json, const PackageData& Package, std::size_t Start, std::size_t End)
{
	Json.BeginObject();
	Json.Key("end"); Json.Unsigned(End);
	Json.Key("offset"); Json.Unsigned(Start);
	Json.Key("sha256"); Json.String(Sha256Hex(Package.GetBytes(), Start, End - Start));
	Json.Key("size"); Json.Unsigned(End - Start);
	Json.EndObject();
}

void WritePackage(JsonWriter& Json, const PackageData& Package)
{
	const SummaryRecord& Summary = Package.GetSummary();
	Json.BeginObject();
	Json.Key("exports"); Json.BeginArray();
	for (std::size_t Index = 0; Index < Package.GetExports().size(); ++Index)
	{
		const ExportRecord& Entry = Package.GetExports()[Index];
		Json.BeginObject();
		Json.Key("class_path"); Json.String(Package.ClassPath(Index));
		Json.Key("class_ref"); Json.Number(Entry.ClassRef);
		Json.Key("index"); Json.Unsigned(Index);
		Json.Key("object_flags"); Json.Unsigned(Entry.ObjectFlags);
		Json.Key("object_name"); WriteNameReference(Json, Package, Entry.ObjectNameIndex);
		Json.Key("object_path"); Json.String(Package.ExportPath(Index));
		Json.Key("outer_path"); Json.String(Package.RefPath(Entry.OuterRef).value_or(Package.GetSpec().PackageName));
		Json.Key("outer_ref"); Json.Number(Entry.OuterRef);
		Json.Key("record_offset"); Json.Unsigned(Entry.RecordOffset);
		Json.Key("record_size"); Json.Unsigned(Entry.RecordSize);
		Json.Key("serial");
		if (Entry.SerialSize)
		{
			const std::size_t Offset = static_cast<std::size_t>(*Entry.SerialOffset);
			const std::size_t Size = static_cast<std::size_t>(Entry.SerialSize);
			Json.BeginObject();
			Json.Key("end"); Json.Unsigned(Offset + Size);
			Json.Key("offset"); Json.Unsigned(Offset);
			Json.Key("sha256"); Json.String(Sha256Hex(Package.GetBytes(), Offset, Size));
			Json.Key("size"); Json.Unsigned(Size);
			Json.EndObject();
		}
		else Json.Null();
		Json.Key("super_path");
		if (const std::optional<std::string> Path = Package.RefPath(Entry.SuperRef)) Json.String(*Path); else Json.Null();
		Json.Key("super_ref"); Json.Number(Entry.SuperRef);
		Json.EndObject();
	}
	Json.EndArray();

	Json.Key("file_sha256"); Json.String(Sha256Hex(Package.GetBytes(), 0, Package.GetBytes().size()));
	Json.Key("file_size"); Json.Unsigned(Package.GetBytes().size());
	Json.Key("imports"); Json.BeginArray();
	for (std::size_t Index = 0; Index < Package.GetImports().size(); ++Index)
	{
		const ImportRecord& Entry = Package.GetImports()[Index];
		Json.BeginObject();
		Json.Key("class_name"); WriteNameReference(Json, Package, Entry.ClassNameIndex);
		Json.Key("class_package"); WriteNameReference(Json, Package, Entry.ClassPackageIndex);
		Json.Key("class_path"); Json.String(Package.GetNames()[static_cast<std::size_t>(Entry.ClassPackageIndex)].Text
			+ "." + Package.GetNames()[static_cast<std::size_t>(Entry.ClassNameIndex)].Text);
		Json.Key("index"); Json.Unsigned(Index);
		Json.Key("object_name"); WriteNameReference(Json, Package, Entry.ObjectNameIndex);
		Json.Key("object_path"); Json.String(Package.ImportPath(Index));
		Json.Key("outer_path");
		if (const std::optional<std::string> Path = Package.RefPath(Entry.OuterRef)) Json.String(*Path); else Json.Null();
		Json.Key("outer_ref"); Json.Number(Entry.OuterRef);
		Json.Key("record_offset"); Json.Unsigned(Entry.RecordOffset);
		Json.Key("record_size"); Json.Unsigned(Entry.RecordSize);
		Json.EndObject();
	}
	Json.EndArray();

	Json.Key("names"); Json.BeginArray();
	for (std::size_t Index = 0; Index < Package.GetNames().size(); ++Index)
	{
		const NameRecord& Entry = Package.GetNames()[Index];
		Json.BeginObject();
		Json.Key("encoding"); Json.String(Entry.Encoding);
		Json.Key("flags"); Json.Unsigned(Entry.Flags);
		Json.Key("index"); Json.Unsigned(Index);
		Json.Key("record_offset"); Json.Unsigned(Entry.RecordOffset);
		Json.Key("record_size"); Json.Unsigned(Entry.RecordSize);
		Json.Key("serialized_character_count"); Json.Number(Entry.SerializedCount);
		Json.Key("text"); Json.String(Entry.Text);
		Json.EndObject();
	}
	Json.EndArray();

	Json.Key("native_indices"); Json.BeginObject();
	Json.Key("entries"); Json.BeginArray();
	for (const NativeRecord& Entry : Package.GetNativeRecords())
	{
		Json.BeginObject();
		Json.Key("export_index"); Json.Unsigned(Entry.ExportIndex);
		Json.Key("function_flags"); Json.Unsigned(Entry.FunctionFlags);
		Json.Key("native_index"); Json.Unsigned(Entry.NativeIndex);
		Json.Key("object_path"); Json.String(Entry.ObjectPath);
		Json.Key("operator_precedence"); Json.Unsigned(Entry.OperatorPrecedence);
		Json.Key("payload_end"); Json.Unsigned(Entry.PayloadEnd);
		Json.Key("payload_offset"); Json.Unsigned(Entry.PayloadOffset);
		Json.Key("payload_size"); Json.Unsigned(Entry.PayloadSize);
		if (Entry.RepOffset) { Json.Key("rep_offset"); Json.Unsigned(*Entry.RepOffset); }
		Json.Key("trailer_offset"); Json.Unsigned(Entry.TrailerOffset);
		Json.EndObject();
	}
	Json.EndArray();
	Json.Key("status"); Json.String("derived-from-version-79-ufunction-terminal-fields");
	Json.EndObject();

	Json.Key("package_name"); Json.String(Package.GetSpec().PackageName);
	Json.Key("path"); Json.String(Package.GetSpec().ManifestPath);
	Json.Key("regions"); Json.BeginObject();
	Json.Key("export_table"); WriteRegion(Json, Package, static_cast<std::size_t>(Summary.ExportOffset), Package.GetExportsEnd());
	Json.Key("import_table"); WriteRegion(Json, Package, static_cast<std::size_t>(Summary.ImportOffset), Package.GetImportsEnd());
	Json.Key("name_table"); WriteRegion(Json, Package, static_cast<std::size_t>(Summary.NameOffset), Package.GetNamesEnd());
	Json.Key("serial_payloads"); WriteRegion(Json, Package, Package.GetNamesEnd(), static_cast<std::size_t>(Summary.ImportOffset));
	Json.Key("summary"); WriteRegion(Json, Package, 0, Summary.HeaderSize);
	Json.EndObject();

	char GuidText[36];
	std::snprintf(GuidText, sizeof(GuidText), "%08x-%08x-%08x-%08x",
		Summary.Guid[0], Summary.Guid[1], Summary.Guid[2], Summary.Guid[3]);
	char TagText[11];
	std::snprintf(TagText, sizeof(TagText), "0x%08x", Summary.Tag);
	Json.Key("summary"); Json.BeginObject();
	Json.Key("export_count"); Json.Number(Summary.ExportCount);
	Json.Key("export_offset"); Json.Number(Summary.ExportOffset);
	Json.Key("generations"); Json.BeginArray();
	for (std::size_t Index = 0; Index < Summary.Generations.size(); ++Index)
	{
		Json.BeginObject();
		Json.Key("export_count"); Json.Number(Summary.Generations[Index].ExportCount);
		Json.Key("index"); Json.Unsigned(Index);
		Json.Key("name_count"); Json.Number(Summary.Generations[Index].NameCount);
		Json.EndObject();
	}
	Json.EndArray();
	Json.Key("guid"); Json.BeginObject();
	Json.Key("a"); Json.Unsigned(Summary.Guid[0]);
	Json.Key("b"); Json.Unsigned(Summary.Guid[1]);
	Json.Key("c"); Json.Unsigned(Summary.Guid[2]);
	Json.Key("d"); Json.Unsigned(Summary.Guid[3]);
	Json.Key("text"); Json.String(GuidText);
	Json.EndObject();
	Json.Key("header_size"); Json.Unsigned(Summary.HeaderSize);
	Json.Key("import_count"); Json.Number(Summary.ImportCount);
	Json.Key("import_offset"); Json.Number(Summary.ImportOffset);
	Json.Key("licensee"); Json.Unsigned(Summary.VersionWord >> 16);
	Json.Key("name_count"); Json.Number(Summary.NameCount);
	Json.Key("name_offset"); Json.Number(Summary.NameOffset);
	Json.Key("package_flags"); Json.Unsigned(Summary.PackageFlags);
	Json.Key("tag"); Json.Unsigned(Summary.Tag);
	Json.Key("tag_hex"); Json.String(TagText);
	Json.Key("version"); Json.Unsigned(Summary.VersionWord & 0xffffu);
	Json.Key("version_word"); Json.Unsigned(Summary.VersionWord);
	Json.EndObject();
	Json.EndObject();
}

std::string BuildManifest(const std::vector<PackageData>& Packages)
{
	JsonWriter Json;
	Json.BeginObject();
	Json.Key("format"); Json.String("hp2-ue1-package-reference");
	Json.Key("licensee_version"); Json.Unsigned(LicenseeVersion);
	Json.Key("package_version"); Json.Unsigned(PackageVersion);
	Json.Key("packages"); Json.BeginArray();
	for (const PackageData& Package : Packages)
		WritePackage(Json, Package);
	Json.EndArray();
	Json.Key("schema_version"); Json.Unsigned(1);
	Json.EndObject();
	return Json.Finish();
}

std::string CompactJsonString(const std::string& Input)
{
	std::string Result = "\"";
	static constexpr char Digits[] = "0123456789abcdef";
	for (unsigned char Byte : Input)
	{
		switch (Byte)
		{
		case '"': Result += "\\\""; break;
		case '\\': Result += "\\\\"; break;
		case '\n': Result += "\\n"; break;
		case '\r': Result += "\\r"; break;
		case '\t': Result += "\\t"; break;
		default:
			if (Byte < 0x20u)
			{
				Result += "\\u00";
				Result += Digits[(Byte >> 4) & 15u];
				Result += Digits[Byte & 15u];
			}
			else Result += static_cast<char>(Byte);
		}
	}
	Result += '"';
	return Result;
}

std::string LineAt(const std::string& Text, std::size_t Offset)
{
	const std::size_t Start = Offset ? Text.rfind('\n', Offset - 1u) + 1u : 0u;
	const std::size_t End = Text.find('\n', Offset);
	return Text.substr(Start, End == std::string::npos ? std::string::npos : End - Start);
}

void ReportMismatch(const std::string& Expected, const std::string& Actual)
{
	std::size_t Offset = 0;
	while (Offset < Expected.size() && Offset < Actual.size() && Expected[Offset] == Actual[Offset])
		++Offset;
	std::size_t Line = 1, Column = 1;
	for (std::size_t Index = 0; Index < Offset && Index < Expected.size(); ++Index)
	{
		if (Expected[Index] == '\n') { ++Line; Column = 1; }
		else ++Column;
	}
	const std::string ExpectedLine = Offset < Expected.size() ? LineAt(Expected, Offset) : "<end-of-file>";
	const std::string ActualLine = Offset < Actual.size() ? LineAt(Actual, Offset) : "<end-of-file>";
	std::fprintf(stderr,
		"{\"status\":\"manifest_mismatch\",\"byte\":%zu,\"line\":%zu,\"column\":%zu,\"expected_line\":%s,\"actual_line\":%s}\n",
		Offset, Line, Column, CompactJsonString(ExpectedLine).c_str(), CompactJsonString(ActualLine).c_str());
}

std::filesystem::path ResolveOptionPath(const char* Value, const char* StartupDirectory)
{
	const std::filesystem::path Path(Value);
	return Path.is_absolute() ? Path : std::filesystem::path(StartupDirectory) / Path;
}

std::filesystem::path FindReference(const char* Explicit, const char* StartupDirectory)
{
	if (Explicit)
	{
		const std::filesystem::path Path = ResolveOptionPath(Explicit, StartupDirectory);
		if (!std::filesystem::is_regular_file(Path))
			throw AuditError("reference manifest does not exist: " + Path.string());
		return Path;
	}
	std::filesystem::path Current(StartupDirectory);
	for (;;)
	{
		const std::filesystem::path Candidate = Current / "Tests/Fixtures/package79-reference.json";
		if (std::filesystem::is_regular_file(Candidate))
			return Candidate;
		if (!Current.has_parent_path() || Current.parent_path() == Current)
			break;
		Current = Current.parent_path();
	}
	const std::filesystem::path Base = std::filesystem::path(TcharToUtf8(appBaseDir()));
	Current = Base;
	for (unsigned int Depth = 0; Depth < 6u && Current.has_parent_path(); ++Depth)
	{
		Current = Current.parent_path();
		const std::filesystem::path Candidate = Current / "Tests/Fixtures/package79-reference.json";
		if (std::filesystem::is_regular_file(Candidate))
			return Candidate;
	}
	throw AuditError("cannot locate Tests/Fixtures/package79-reference.json; pass --reference=<path>");
}

ULinkerLoad* LoadRuntimeLinker(const PackageSpec& Spec)
{
	const std::basic_string<TCHAR> Path = Utf8ToTchar(Spec.LinkerPath);
	UObject::BeginLoad();
	ULinkerLoad* Linker = nullptr;
	try
	{
		Linker = UObject::GetPackageLinker(nullptr, Path.c_str(),
			LOAD_Throw | LOAD_NoWarn | LOAD_Quiet | LOAD_NoVerify, nullptr, nullptr);
		UObject::EndLoad();
	}
	catch (...)
	{
		UObject::EndLoad();
		throw;
	}
	if (!Linker)
		throw AuditError(std::string(Spec.ManifestPath) + ": failed to create runtime linker");
	return Linker;
}

std::uint16_t ReadAudioLe16(const std::uint8_t* Data)
{
	return static_cast<std::uint16_t>(Data[0])
		| static_cast<std::uint16_t>(Data[1]) << 8;
}

std::uint32_t ReadAudioLe32(const std::uint8_t* Data)
{
	return static_cast<std::uint32_t>(Data[0])
		| static_cast<std::uint32_t>(Data[1]) << 8
		| static_cast<std::uint32_t>(Data[2]) << 16
		| static_cast<std::uint32_t>(Data[3]) << 24;
}

bool IsPcmWavePayload(const std::vector<std::uint8_t>& Data)
{
	if (Data.size() < 12
		|| std::memcmp(Data.data(), "RIFF", 4) != 0
		|| std::memcmp(Data.data() + 8, "WAVE", 4) != 0
		|| static_cast<std::uint64_t>(ReadAudioLe32(Data.data() + 4)) + 8u != Data.size())
		return false;

	bool FoundFormat = false;
	bool FoundData = false;
	std::uint16_t BlockAlign = 0;
	std::size_t Offset = 12;
	while (Offset < Data.size())
	{
		if (Data.size() - Offset < 8)
			return false;
		const std::uint8_t* Header = Data.data() + Offset;
		const std::size_t ChunkSize = ReadAudioLe32(Header + 4);
		const std::size_t PayloadOffset = Offset + 8;
		if (ChunkSize > Data.size() - PayloadOffset)
			return false;

		if (std::memcmp(Header, "fmt ", 4) == 0)
		{
			if (ChunkSize < 16)
				return false;
			const std::uint8_t* Format = Data.data() + PayloadOffset;
			const std::uint16_t Codec = ReadAudioLe16(Format);
			const std::uint16_t Channels = ReadAudioLe16(Format + 2);
			const std::uint32_t SampleRate = ReadAudioLe32(Format + 4);
			const std::uint32_t ByteRate = ReadAudioLe32(Format + 8);
			BlockAlign = ReadAudioLe16(Format + 12);
			const std::uint16_t Bits = ReadAudioLe16(Format + 14);
			const std::uint32_t ExpectedAlign =
				static_cast<std::uint32_t>(Channels) * static_cast<std::uint32_t>(Bits) / 8u;
			if (Codec != 1 || Channels == 0 || SampleRate == 0
				|| (Bits != 8 && Bits != 16) || ExpectedAlign == 0
				|| BlockAlign != ExpectedAlign
				|| static_cast<std::uint64_t>(ByteRate)
					!= static_cast<std::uint64_t>(SampleRate) * ExpectedAlign)
				return false;
			FoundFormat = true;
		}
		else if (std::memcmp(Header, "data", 4) == 0)
		{
			if (BlockAlign == 0 || ChunkSize % BlockAlign != 0)
				return false;
			FoundData = true;
		}

		Offset = PayloadOffset + ChunkSize;
		if (ChunkSize & 1u)
		{
			if (Offset == Data.size())
				return false;
			++Offset;
		}
	}
	return Offset == Data.size() && FoundFormat && FoundData;
}

bool IsEaxaPayload(std::size_t Size, std::int32_t NumSamples, std::int32_t Bits,
	std::int32_t Channels, std::int32_t SampleRate)
{
	if (NumSamples <= 0 || Bits != 16 || Channels != 1 || SampleRate <= 0)
		return false;
	const std::uint64_t Blocks =
		(static_cast<std::uint64_t>(NumSamples) + 27u) / 28u;
	return Blocks * 15u == Size;
}

bool HasAudioSignature(const std::vector<std::uint8_t>& Data, const char* Signature)
{
	return Data.size() >= 4 && std::memcmp(Data.data(), Signature, 4) == 0;
}

struct AudioEntry
{
	std::int32_t Bits;
	std::int32_t Channels;
	std::int32_t CoreFlags;
	std::string File;
	std::string FileType;
	std::string Format;
	std::string Hash;
	std::size_t Size;
	std::string ObjectPath;
	std::int32_t RawNumSamples;
	std::int32_t SampleRate;
	std::string Status;
	std::vector<std::uint8_t> Data;
};

void ClassifyAudioEntry(AudioEntry& Entry)
{
	if (Entry.FileType == "XA")
	{
		Entry.Format = "eaxa";
		Entry.Status = ((Entry.CoreFlags & SF_Streaming)
			&& IsEaxaPayload(Entry.Size, Entry.RawNumSamples, Entry.Bits,
				Entry.Channels, Entry.SampleRate)) ? "valid" : "malformed";
	}
	else if (Entry.FileType == "wav")
	{
		Entry.Format = "pcm-wave";
		Entry.Status = (!(Entry.CoreFlags & SF_Streaming)
			&& IsPcmWavePayload(Entry.Data)) ? "valid" : "malformed";
	}
	else if (Entry.FileType == "ogg")
	{
		Entry.Format = "ogg-vorbis";
		Entry.Status = ((Entry.CoreFlags & SF_Streaming)
			&& HasAudioSignature(Entry.Data, "OggS")) ? "valid" : "malformed";
	}
	else if (Entry.FileType == "None" && !(Entry.CoreFlags & SF_Streaming)
		&& Entry.Data.empty())
	{
		Entry.Format = "empty";
		Entry.Status = "empty";
	}
	else
	{
		Entry.Format = "unknown";
		Entry.Status = "malformed";
	}
}

void WriteExactFile(const std::filesystem::path& Filename, const std::vector<std::uint8_t>& Data)
{
	if (std::filesystem::exists(Filename))
	{
		const std::vector<std::uint8_t> Existing = ReadFile(Filename);
		if (Existing != Data)
			throw AuditError("collision or stale corpus payload at " + Filename.string());
		return;
	}
	std::ofstream Stream(Filename, std::ios::binary | std::ios::out);
	if (!Stream)
		throw AuditError("cannot create " + Filename.string());
	if (!Data.empty())
		Stream.write(reinterpret_cast<const char*>(Data.data()), static_cast<std::streamsize>(Data.size()));
	Stream.close();
	if (!Stream)
	{
		std::error_code Ignored;
		std::filesystem::remove(Filename, Ignored);
		throw AuditError("failed writing " + Filename.string());
	}
}

void DumpAudio(const std::filesystem::path& Directory, ULinkerLoad* GeneralLinker)
{
	UObject::BeginLoad();
	try
	{
		GeneralLinker->Verify();
		GeneralLinker->LoadAllObjects();
		UObject::EndLoad();
	}
	catch (...)
	{
		UObject::EndLoad();
		throw;
	}

	std::vector<AudioEntry> Entries;
	for (TObjectIterator<USound> It; It; ++It)
	{
		USound* Sound = *It;
		if (!Sound->IsIn(GeneralLinker->LinkerRoot))
			continue;
		Sound->Data.Load();
		if (Sound->Data.Num() < 0)
			throw AuditError("negative USound::Data size");
		AudioEntry Entry;
		Entry.Bits = Sound->raw_BitsPerSample;
		Entry.Channels = Sound->raw_NumChannels;
		Entry.CoreFlags = Sound->CoreFlags;
		Entry.FileType = TcharToUtf8(*Sound->FileType);
		Entry.Size = static_cast<std::size_t>(Sound->Data.Num());
		Entry.ObjectPath = TcharToUtf8(Sound->GetPathName());
		Entry.RawNumSamples = Sound->raw_NumSamples;
		Entry.SampleRate = Sound->raw_SampleRate;
		Entry.Data.resize(Entry.Size);
		if (Entry.Size)
			std::memcpy(Entry.Data.data(), Sound->Data.GetData(), Entry.Size);
		ClassifyAudioEntry(Entry);
		Entry.Hash = Sha256Hex(Entry.Data.data(), Entry.Data.size());
		Entry.File = Entry.Hash + ".bin";
		Entries.push_back(std::move(Entry));
	}
	std::sort(Entries.begin(), Entries.end(), [](const AudioEntry& A, const AudioEntry& B)
	{
		return A.ObjectPath < B.ObjectPath;
	});
	for (std::size_t Index = 1; Index < Entries.size(); ++Index)
		if (Entries[Index - 1].ObjectPath == Entries[Index].ObjectPath)
			throw AuditError("duplicate sound object path: " + Entries[Index].ObjectPath);

	std::error_code Error;
	std::filesystem::create_directories(Directory, Error);
	if (Error || !std::filesystem::is_directory(Directory))
		throw AuditError("cannot create audio corpus directory: " + Directory.string());
	for (const AudioEntry& Entry : Entries)
		WriteExactFile(Directory / Entry.File, Entry.Data);

	JsonWriter Json;
	Json.BeginObject();
	Json.Key("entries"); Json.BeginArray();
	for (const AudioEntry& Entry : Entries)
	{
		Json.BeginObject();
		Json.Key("bits"); Json.Number(Entry.Bits);
		Json.Key("channels"); Json.Number(Entry.Channels);
		Json.Key("core_flags"); Json.Number(Entry.CoreFlags);
		Json.Key("file"); Json.String(Entry.File);
		Json.Key("file_type"); Json.String(Entry.FileType);
		Json.Key("format"); Json.String(Entry.Format);
		Json.Key("input_sha256"); Json.String(Entry.Hash);
		Json.Key("input_size"); Json.Unsigned(Entry.Size);
		Json.Key("object_path"); Json.String(Entry.ObjectPath);
		Json.Key("raw_NumSamples"); Json.Number(Entry.RawNumSamples);
		Json.Key("sample_rate"); Json.Number(Entry.SampleRate);
		Json.Key("status"); Json.String(Entry.Status);
		Json.EndObject();
	}
	Json.EndArray();
	Json.Key("format"); Json.String("hp2-audio-corpus");
	Json.Key("schema_version"); Json.Unsigned(2);
	Json.Key("sound_count"); Json.Unsigned(Entries.size());
	Json.EndObject();
	const std::string Index = Json.Finish();
	WriteExactFile(Directory / "index.json",
		std::vector<std::uint8_t>(reinterpret_cast<const std::uint8_t*>(Index.data()),
			reinterpret_cast<const std::uint8_t*>(Index.data()) + Index.size()));
	std::printf("{\"status\":\"audio_dump_ok\",\"sounds\":%zu,\"directory\":%s}\n",
		Entries.size(), CompactJsonString(Directory.string()).c_str());
}

enum class DataProfile
{
	LegacyPrototype,
	RetailOnly,
};

std::string FoldProfileToken(std::string Value)
{
	for (char& Character : Value)
	{
		if (Character >= 'A' && Character <= 'Z')
			Character = static_cast<char>(Character - 'A' + 'a');
		else if (Character == '_')
			Character = '-';
	}
	return Value;
}

std::optional<std::string> ExtractManifestProfileField(const std::string& Text)
{
	static constexpr char Key[] = "\"profile\"";
	std::size_t Offset = Text.find(Key);
	while (Offset != std::string::npos)
	{
		std::size_t Scan = Offset + sizeof(Key) - 1u;
		while (Scan < Text.size() && (Text[Scan] == ' ' || Text[Scan] == '\t'
			|| Text[Scan] == '\n' || Text[Scan] == '\r'))
			++Scan;
		if (Scan < Text.size() && Text[Scan] == ':')
		{
			++Scan;
			while (Scan < Text.size() && (Text[Scan] == ' ' || Text[Scan] == '\t'
				|| Text[Scan] == '\n' || Text[Scan] == '\r'))
				++Scan;
			if (Scan < Text.size() && Text[Scan] == '"')
			{
				++Scan;
				std::string Value;
				bool Terminated = false;
				while (Scan < Text.size())
				{
					if (Text[Scan] == '"' && Text[Scan - 1u] != '\\')
					{
						Terminated = true;
						break;
					}
					if (Text[Scan] == '\\' && Scan + 1u < Text.size())
						Value += Text[++Scan];
					else
						Value += Text[Scan];
					++Scan;
				}
				if (Terminated)
					return Value;
			}
		}
		Offset = Text.find(Key, Offset + sizeof(Key) - 1u);
	}
	return std::nullopt;
}

// Tests/LocalizationTests.py::detect_overlay_profile convention: a data root
// carrying overlay-manifest.json with profile "retail-only" (case-insensitive,
// '_' folds to '-') audits as a retail import; anything else, including a
// missing or unreadable manifest, keeps the legacy prototype behavior.
DataProfile DetectDataProfile(const std::filesystem::path& DataRoot)
{
	try
	{
		const std::string Manifest = ReadTextFile(DataRoot / "overlay-manifest.json");
		const std::optional<std::string> Profile = ExtractManifestProfileField(Manifest);
		if (Profile && FoldProfileToken(*Profile) == "retail-only")
			return DataProfile::RetailOnly;
	}
	catch (const std::exception&)
	{
	}
	return DataProfile::LegacyPrototype;
}

struct AuditExportSummary
{
	std::string ClassPath;
	std::string ObjectPath;
};

struct AuditPackageEntry
{
	std::int32_t ExportCount = 0;
	std::vector<AuditExportSummary> Exports;
	std::int32_t ImportCount = 0;
	std::uint32_t Licensee = 0;
	std::int32_t NameCount = 0;
	std::vector<NativeRecord> NativeFunctions;
	std::uint32_t PackageFlags = 0;
	std::string Path;
	std::uint32_t Version = 0;
};

// Required payload surface for the retail-only profile, verified against the
// prepared overlay layout: every prototype-census package that also exists in
// the retail tree stays mandatory, and General.uax is replaced by the retail
// localization bundle. Category floors catch truncated imports loudly.
void ValidateRetailSurface(const std::vector<AuditPackageEntry>& Packages)
{
	static constexpr const char* RequiredPackages[] =
	{
		"Maps/startup.unr",
		"Sounds/AllDialog.USA_uax",
		"System/Core.u",
		"System/Engine.u",
		"System/HGame.u",
		"Textures/HP2_Master.utx",
	};
	static constexpr const char* RequiredCategories[] =
	{
		"Maps/", "Sounds/", "System/", "Textures/",
	};
	if (Packages.empty())
		throw AuditError("retail-only profile: data root contains no UE1 packages");
	for (const char* Required : RequiredPackages)
	{
		const bool Present = std::any_of(Packages.begin(), Packages.end(),
			[Required](const AuditPackageEntry& Entry) { return Entry.Path == Required; });
		if (!Present)
			throw AuditError(std::string("retail-only profile: required package is absent: ") + Required);
	}
	for (const char* Prefix : RequiredCategories)
	{
		const bool Any = std::any_of(Packages.begin(), Packages.end(),
			[Prefix](const AuditPackageEntry& Entry) { return Entry.Path.rfind(Prefix, 0) == 0; });
		if (!Any)
			throw AuditError(std::string("retail-only profile: no packages under ") + Prefix);
	}
}

AuditPackageEntry AuditSinglePackage(const std::string& RelativePath, const std::vector<std::uint8_t>& Bytes)
{
	// Mirror Python Path(relative_path).stem: strip one final suffix only.
	std::string PackageName = RelativePath.substr(RelativePath.find_last_of('/') + 1u);
	const std::size_t Dot = PackageName.find_last_of('.');
	if (Dot != std::string::npos)
		PackageName.resize(Dot);
	PackageSpec Spec{ RelativePath.c_str(), PackageName.c_str(), "" };
	PackageData Package(Spec, Bytes, AuditMinimumPackageVersion, AuditMaximumPackageVersion);
	Package.Parse();
	AuditPackageEntry Entry;
	Entry.Path = RelativePath;
	const SummaryRecord& Summary = Package.GetSummary();
	Entry.Version = Summary.VersionWord & 0xffffu;
	Entry.Licensee = Summary.VersionWord >> 16;
	Entry.PackageFlags = Summary.PackageFlags;
	Entry.ExportCount = Summary.ExportCount;
	Entry.ImportCount = Summary.ImportCount;
	Entry.NameCount = Summary.NameCount;
	Entry.Exports.reserve(Package.GetExports().size());
	for (std::size_t Index = 0; Index < Package.GetExports().size(); ++Index)
		Entry.Exports.push_back({ Package.ClassPath(Index), Package.ExportPath(Index) });
	for (const NativeRecord& Record : Package.GetNativeRecords())
		if (Record.FunctionFlags & FunctionFlagNative)
			Entry.NativeFunctions.push_back(Record);
	return Entry;
}

std::string WriteAuditPackageEntry(const AuditPackageEntry& Entry)
{
	JsonWriter Json;
	Json.BeginObject();
	Json.Key("export_count"); Json.Number(Entry.ExportCount);
	Json.Key("exports"); Json.BeginArray();
	for (const AuditExportSummary& Export : Entry.Exports)
	{
		Json.BeginObject();
		Json.Key("class_path"); Json.String(Export.ClassPath);
		Json.Key("object_path"); Json.String(Export.ObjectPath);
		Json.EndObject();
	}
	Json.EndArray();
	Json.Key("import_count"); Json.Number(Entry.ImportCount);
	Json.Key("licensee"); Json.Unsigned(Entry.Licensee);
	Json.Key("name_count"); Json.Number(Entry.NameCount);
	Json.Key("native_functions"); Json.BeginArray();
	for (const NativeRecord& Function : Entry.NativeFunctions)
	{
		Json.BeginObject();
		Json.Key("native_index"); Json.Unsigned(Function.NativeIndex);
		Json.Key("object_path"); Json.String(Function.ObjectPath);
		Json.EndObject();
	}
	Json.EndArray();
	Json.Key("package_flags"); Json.Unsigned(Entry.PackageFlags);
	Json.Key("path"); Json.String(Entry.Path);
	Json.Key("version"); Json.Unsigned(Entry.Version);
	Json.EndObject();
	std::string Fragment = Json.Finish();
	if (!Fragment.empty() && Fragment.back() == '\n')
		Fragment.pop_back();
	return Fragment;
}

// Byte-identical to Build/package79_reference.py --data-root output:
// json.dumps(..., ensure_ascii=False, indent=2, sort_keys=True) + "\n", keys
// emitted in sorted order.
std::string BuildAuditDocument(const std::filesystem::path& DataRoot,
	const std::vector<AuditPackageEntry>& Packages,
	const std::vector<std::string>& NonPackageFiles,
	const std::map<std::string, std::uint64_t>& FormatProfiles)
{
	JsonWriter Json;
	Json.BeginObject();
	Json.Key("accepted_versions"); Json.BeginObject();
	Json.Key("max"); Json.Unsigned(AuditMaximumPackageVersion);
	Json.Key("min"); Json.Unsigned(AuditMinimumPackageVersion);
	Json.EndObject();
	Json.Key("census"); Json.BeginObject();
	Json.Key("file_count"); Json.Unsigned(Packages.size() + NonPackageFiles.size());
	Json.Key("format_profiles"); Json.BeginObject();
	for (const std::pair<const std::string, std::uint64_t>& Profile : FormatProfiles)
	{
		Json.Key(Profile.first.c_str());
		Json.Unsigned(Profile.second);
	}
	Json.EndObject();
	Json.Key("non_package_count"); Json.Unsigned(NonPackageFiles.size());
	Json.Key("package_count"); Json.Unsigned(Packages.size());
	Json.EndObject();
	// The generator emits str(root.relative_to(repo_root)) when the root lives
	// inside --repo-root and the absolute path otherwise; retail imports live
	// outside the repository by construction.
	Json.Key("data_root"); Json.String(DataRoot.generic_string());
	Json.Key("format"); Json.String("hp1-ue1-package-audit");
	Json.Key("non_package_files"); Json.BeginArray();
	for (const std::string& File : NonPackageFiles)
		Json.String(File);
	Json.EndArray();
	Json.Key("packages"); Json.BeginArray();
	for (const AuditPackageEntry& Package : Packages)
		Json.RawValue(WriteAuditPackageEntry(Package));
	Json.EndArray();
	Json.Key("schema_version"); Json.Unsigned(1u);
	Json.EndObject();
	return Json.Finish();
}

struct Options
{
	const char* Reference = nullptr;
	const char* DumpAudioDirectory = nullptr;
	const char* StartupDirectory = nullptr;
};

bool StartsWithNoCase(const char* Value, const char* Prefix)
{
	while (*Prefix)
	{
		if (!*Value)
			return false;
		char A = *Value++, B = *Prefix++;
		if (A >= 'A' && A <= 'Z') A = static_cast<char>(A - 'A' + 'a');
		if (B >= 'A' && B <= 'Z') B = static_cast<char>(B - 'A' + 'a');
		if (A != B) return false;
	}
	return true;
}

bool ParseOptions(int ArgC, char* ArgV[], Options& Result, const char*& ErrorMessage)
{
	for (int Index = 1; Index < ArgC; ++Index)
	{
		const char* Argument = ArgV[Index];
		if (StartsWithNoCase(Argument, "-datadir="))
			continue;
		if (std::strncmp(Argument, "--reference=", 12) == 0)
		{
			if (Result.Reference || !Argument[12])
			{
				ErrorMessage = "--reference must be supplied exactly once with a nonempty path";
				return false;
			}
			Result.Reference = Argument + 12;
		}
		else if (std::strncmp(Argument, "--dump-audio=", 13) == 0)
		{
			if (Result.DumpAudioDirectory || !Argument[13])
			{
				ErrorMessage = "--dump-audio must be supplied exactly once with a nonempty directory";
				return false;
			}
			Result.DumpAudioDirectory = Argument + 13;
		}
		else
		{
			ErrorMessage = "unknown argument";
			return false;
		}
	}
	return true;
}

// byte-for-byte. Every file under the data root is enumerated (sorted by
// relative POSIX path; top-level provenance *.json and transient engine
// crash-report.json artifacts are excluded); UE1 packages are
// decoded structurally with the same v60-79 rules as the generator, everything
// else is censused as a non-package file. The golden comparison afterwards is
// unchanged and stays byte-exact.
int RunRetailOnlyAudit(const Options& OptionsValue, const std::filesystem::path& DataRoot)
{
	if (OptionsValue.DumpAudioDirectory)
		throw AuditError("--dump-audio requires the legacy prototype audio corpus;"
			" it is unsupported for the retail-only data profile");

	std::vector<std::string> RelativeFiles;
	for (std::filesystem::recursive_directory_iterator It(DataRoot, std::filesystem::directory_options::skip_permission_denied), End;
		It != End; ++It)
	{
		std::error_code Ignored;
		if (!It->is_regular_file(Ignored) || Ignored)
			continue;
		const std::string Key = It->path().lexically_relative(DataRoot).generic_string();
		if ((Key.find('/') == std::string::npos && It->path().extension() == ".json")
				|| It->path().filename() == "crash-report.json")
			continue;
		RelativeFiles.push_back(Key);
	}
	std::sort(RelativeFiles.begin(), RelativeFiles.end());

	std::vector<AuditPackageEntry> Packages;
	std::vector<std::string> NonPackageFiles;
	std::map<std::string, std::uint64_t> FormatProfiles;
	Packages.reserve(RelativeFiles.size());
	for (const std::string& Relative : RelativeFiles)
	{
		CurrentAuditStage = Relative.c_str();
		const std::vector<std::uint8_t> Bytes = ReadFile(DataRoot / Relative);
		if (Bytes.size() < 4u
			|| (static_cast<std::uint32_t>(Bytes[0])
				| (static_cast<std::uint32_t>(Bytes[1]) << 8)
				| (static_cast<std::uint32_t>(Bytes[2]) << 16)
				| (static_cast<std::uint32_t>(Bytes[3]) << 24)) != PackageTag)
		{
			NonPackageFiles.push_back(Relative);
			continue;
		}
		AuditPackageEntry Entry = AuditSinglePackage(Relative, Bytes);
		char Profile[48];
		std::snprintf(Profile, sizeof(Profile), "v%u/licensee%u/flags%u",
			Entry.Version, Entry.Licensee, Entry.PackageFlags);
		++FormatProfiles[Profile];
		Packages.push_back(std::move(Entry));
	}

	ValidateRetailSurface(Packages);

	CurrentAuditStage = "package79-reference.json";
	const std::filesystem::path ReferencePath = FindReference(
		OptionsValue.Reference, OptionsValue.StartupDirectory);
	const std::string Expected = ReadTextFile(ReferencePath);
	const std::string Actual = BuildAuditDocument(DataRoot, Packages, NonPackageFiles, FormatProfiles);
	if (Actual != Expected)
	{
		ReportMismatch(Expected, Actual);
		return 2;
	}
	std::printf("{\"status\":\"manifest_ok\",\"packages\":%zu}\n", Packages.size());
	return 0;
}

int RunAudit(const Options& OptionsValue)
{
	// appBaseDir() is the canonical "<data root>/System" directory (with a
	// trailing separator from PrepareHP2Paths); trim it before taking the parent.
	std::string BaseText = TcharToUtf8(appBaseDir());
	while (BaseText.size() > 1u && BaseText.back() == '/')
		BaseText.pop_back();
	const std::filesystem::path BaseDirectory(BaseText);
	if (DetectDataProfile(BaseDirectory.parent_path()) == DataProfile::RetailOnly)
		return RunRetailOnlyAudit(OptionsValue, BaseDirectory.parent_path());
	std::vector<PackageData> Packages;
	std::vector<ULinkerLoad*> Linkers;
	Packages.reserve(std::size(PackageSpecs));
	Linkers.reserve(std::size(PackageSpecs));
	for (const PackageSpec& Spec : PackageSpecs)
	{
		CurrentAuditStage = Spec.ManifestPath;
		PackageData Package(Spec, ReadFile(BaseDirectory / Spec.LinkerPath));
		Package.Parse();
		ULinkerLoad* Linker = LoadRuntimeLinker(Spec);
		Package.ValidateRuntimeLinker(Linker);
		Packages.push_back(std::move(Package));
		Linkers.push_back(Linker);
	}

	CurrentAuditStage = "package79-reference.json";
	const std::filesystem::path ReferencePath = FindReference(
		OptionsValue.Reference, OptionsValue.StartupDirectory);
	const std::string Expected = ReadTextFile(ReferencePath);
	const std::string Actual = BuildManifest(Packages);
	if (Actual != Expected)
	{
		ReportMismatch(Expected, Actual);
		return 2;
	}
	std::printf("{\"status\":\"manifest_ok\",\"packages\":%zu}\n", Packages.size());
	CurrentAuditStage = "audio-corpus";
	if (OptionsValue.DumpAudioDirectory)
		DumpAudio(ResolveOptionPath(OptionsValue.DumpAudioDirectory, OptionsValue.StartupDirectory),
			Linkers.back());
	return 0;
}

FOutputDeviceFile Log;
FOutputDeviceAnsiError Error;
FFeedbackContextAnsi Warn;
FFileManagerUnix FileManager;
FMallocAnsi Malloc;

} // namespace

int main(int argc, char* argv[])
{
	if (!PrepareHP2Paths(argc, argv))
		return 1;

	Options OptionsValue;
	const char* ArgumentError = nullptr;
	if (!ParseOptions(argc, argv, OptionsValue, ArgumentError))
	{
		std::fprintf(stderr, "{\"status\":\"argument_error\",\"message\":\"%s\"}\n", ArgumentError);
		return 1;
	}
	OptionsValue.StartupDirectory = getcwd(nullptr, 0);
	if (!OptionsValue.StartupDirectory)
	{
		std::fprintf(stderr, "{\"status\":\"argument_error\",\"message\":\"cannot read startup directory\"}\n");
		return 1;
	}

#if __STATIC_LINK
	InstallHP2NativeLookups();
#endif
#if !_MSC_VER
	__Context::StaticInit();
	std::strncpy(GModule, argv[0], sizeof(GModule) - 1u);
	GModule[sizeof(GModule) - 1u] = 0;
#endif

	int Result = 1;
	bool Initialized = false;
	GIsStarted = 1;
#ifndef _DEBUG
	try
#endif
	{
		GIsGuarded = 1;
		appInit(TEXT("PackageAudit"), TEXT(""), &Malloc, &Log, &Error, &Warn,
			&FileManager, FConfigCacheIni::Factory, 1);
		Initialized = true;
		CurrentAuditStage = "runtime-class-registration";
#if __STATIC_LINK
		RegisterHP2RuntimeClasses();
#endif
		UObject::SetLanguage(TEXT("int"));
		GIsClient = GIsServer = GIsEditor = GIsScriptable = 1;
		GLazyLoad = 0;
		CurrentAuditStage = "package-audit";
		Result = RunAudit(OptionsValue);
		appPreExit();
		Initialized = false;
		GIsGuarded = 0;
	}
#ifndef _DEBUG
	catch (const std::exception& Exception)
	{
		std::fprintf(stderr, "{\"status\":\"audit_error\",\"message\":%s}\n",
			CompactJsonString(Exception.what()).c_str());
		Result = 1;
		GIsGuarded = 0;
	}
	catch (const TCHAR* Exception)
	{
		std::fprintf(stderr, "{\"status\":\"runtime_error\",\"stage\":%s,\"message\":%s,\"history\":%s}\n",
			CompactJsonString(CurrentAuditStage).c_str(),
			CompactJsonString(TcharToUtf8(Exception)).c_str(),
			CompactJsonString(TcharToUtf8(GErrorHist)).c_str());
		Result = 1;
		GIsGuarded = 0;
	}
	catch (const char* Exception)
	{
		std::fprintf(stderr, "{\"status\":\"runtime_error\",\"stage\":%s,\"message\":%s,\"history\":%s}\n",
			CompactJsonString(CurrentAuditStage).c_str(),
			CompactJsonString(Exception ? Exception : "<null>").c_str(),
			CompactJsonString(TcharToUtf8(GErrorHist)).c_str());
		Result = 1;
		GIsGuarded = 0;
	}
	catch (...)
	{
		std::fprintf(stderr, "{\"status\":\"runtime_error\",\"stage\":%s,\"message\":\"unknown exception\",\"history\":%s}\n",
			CompactJsonString(CurrentAuditStage).c_str(),
			CompactJsonString(TcharToUtf8(GErrorHist)).c_str());
		Result = 1;
		GIsGuarded = 0;
	}
#endif
	if (Initialized)
		appPreExit();
	appExit();
	GIsStarted = 0;
	std::free(const_cast<char*>(OptionsValue.StartupDirectory));
	return Result;
}

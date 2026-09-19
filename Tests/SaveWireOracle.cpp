/*=============================================================================
	SaveWireOracle.cpp: deterministic legacy save/package writer fixture oracle.

	The oracle uses the production FPackageFileSummary and FGenerationInfo
	operators. It intentionally emits the smallest valid package-79 save wire
	envelope: one generation and empty name/import/export tables whose offsets
	all point immediately past the serialized summary.
=============================================================================*/

#include "HP2PortableSha256.h"
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <string>
#include <vector>

#include "Core.h"
#include "FMallocAnsi.h"
#include "FFileManagerUnix.h"
#include "UnLinker.h"

extern "C" { TCHAR GPackage[64] = TEXT("SaveWireOracle"); }

namespace
{
	class FMemoryWriter : public FArchive
	{
	public:
		explicit FMemoryWriter(std::vector<BYTE>& InBytes)
		: Bytes(InBytes), Position(0)
		{
			ArIsSaving = 1;
			ArIsPersistent = 1;
		}

		void Serialize(void* Data, INT Count) override
		{
			if (Count < 0 || Position != static_cast<INT>(Bytes.size()))
			{
				ArIsError = 1;
				return;
			}
			const BYTE* First = static_cast<const BYTE*>(Data);
			Bytes.insert(Bytes.end(), First, First + Count);
			Position += Count;
		}
		INT Tell() override { return Position; }
		INT TotalSize() override { return static_cast<INT>(Bytes.size()); }
		void Seek(INT NewPosition) override
		{
			if (NewPosition != Position)
				ArIsError = 1;
		}

	private:
		std::vector<BYTE>& Bytes;
		INT Position;
	};

	struct FDeterministicSummary : FPackageFileSummary
	{
		void SetVersionWord(INT Version) { FileVersion = Version; }
	};

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
}

int main()
{
	FMallocAnsi Allocator;
	FFileManagerUnix FileManager;
	GMalloc = &Allocator;
	GFileManager = &FileManager;

	const char* ArtifactDir = std::getenv("HP2_ARTIFACT_DIR");
	if (!ArtifactDir || !*ArtifactDir)
	{
		std::fprintf(stderr, "save_wire_oracle: HP2_ARTIFACT_DIR is required\n");
		return 2;
	}

	FDeterministicSummary Summary;
	Summary.Tag = PACKAGE_FILE_TAG;
	Summary.SetVersionWord(79);
	Summary.PackageFlags = 0;
	Summary.NameCount = Summary.ExportCount = Summary.ImportCount = 0;
	Summary.NameOffset = Summary.ExportOffset = Summary.ImportOffset = 64;
	Summary.Guid.A = 0x2144F1A6u;
	Summary.Guid.B = 0x8C7D4B2Eu;
	Summary.Guid.C = 0x9A51C03Du;
	Summary.Guid.D = 0x6E2F8B17u;
	new(Summary.Generations) FGenerationInfo(0, 0);

	std::vector<BYTE> Wire;
	FMemoryWriter Writer(Wire);
	Writer << static_cast<FPackageFileSummary&>(Summary);
	if (Writer.IsError() || Wire.size() != 64)
	{
		std::fprintf(stderr, "save_wire_oracle: production summary writer emitted %zu bytes\n", Wire.size());
		return 1;
	}

	const std::string FixturePath = std::string(ArtifactDir) + "/cxx-save-wire-v1.usa";
	const std::string MetadataPath = std::string(ArtifactDir) + "/cxx-save-wire-v1.json";
	if (std::ifstream(FixturePath.c_str(), std::ios::binary).good()
		|| std::ifstream(MetadataPath.c_str(), std::ios::binary).good())
	{
		std::fprintf(stderr, "save_wire_oracle: refusing to overwrite an existing oracle artifact\n");
		return 2;
	}

	std::ofstream Fixture(FixturePath.c_str(), std::ios::binary | std::ios::trunc);
	Fixture.write(reinterpret_cast<const char*>(&Wire[0]), static_cast<std::streamsize>(Wire.size()));
	Fixture.close();
	if (!Fixture)
	{
		std::fprintf(stderr, "save_wire_oracle: cannot write fixture\n");
		return 1;
	}

	const std::string Hash = Sha256Hex(Wire);
	std::ofstream Metadata(MetadataPath.c_str(), std::ios::trunc);
	Metadata << "{\"schema_version\":1,\"kind\":\"legacy_cxx_save_wire\","
		<< "\"fixture\":\"cxx-save-wire-v1.usa\",\"sha256\":\"" << Hash << "\","
		<< "\"size\":" << Wire.size() << ",\"package_version\":79,"
		<< "\"table_counts\":{\"names\":0,\"imports\":0,\"exports\":0},"
		<< "\"provenance\":{\"writer\":\"FPackageFileSummary::operator<<\","
		<< "\"generation_writer\":\"FGenerationInfo::operator<<\","
		<< "\"source\":\"Core/Inc/UnLinker.h\"}}\n";
	if (!Metadata)
	{
		std::fprintf(stderr, "save_wire_oracle: cannot write metadata\n");
		return 1;
	}
	std::printf("save_wire_oracle: emitted deterministic package-79 save wire\n");
	return 0;
}

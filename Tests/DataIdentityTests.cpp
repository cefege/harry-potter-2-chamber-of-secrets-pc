/*=============================================================================
	DataIdentityTests.cpp: Data-root bootstrap identity contracts.

	Builds synthetic data trees under $HP2_ARTIFACT_DIR/tmp and exercises the
	two-mode bootstrap rule in HP2Paths: legacy System/Default.ini acceptance
	and full overlay-manifest.json + overlay-checksums.txt verification with
	structured rejection reason codes.

	This executable intentionally links only HP2Paths.cpp; the three Core
	entry points it references are stubbed below.
=============================================================================*/

#include "Core.h"
#include "HP2Paths.h"

#include <CommonCrypto/CommonDigest.h>
#include <sys/stat.h>

#include <unistd.h>

#include <algorithm>
#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <utility>
#include <vector>

/*----------------------------------------------------------------------------
	Minimal Core link seams: HP2Paths.cpp references exactly these entry
	points, and the contracts under test never install engine paths.
----------------------------------------------------------------------------*/

class FTestMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* ) override { return std::malloc(Count); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* ) override { return std::realloc(Original, Count); }
	void Free( void* Original ) override { std::free(Original); }
	void DumpAllocs() override {}
	void HeapCheck() override {}
	void Init() override {}
	void Exit() override {}
};

static FTestMalloc GTestMalloc;
FMalloc* GMalloc = &GTestMalloc;
FOutputDeviceError* GError = nullptr;
void FOutputDevice::Logf( const TCHAR*, ... )
{
	std::abort();
}

void appSetBaseDir( const TCHAR* )
{
}

void appSetUserDir( const TCHAR* )
{
}
 UBOOL appFromUtf8InPlace( TCHAR* Dest, const ANSICHAR* Src, INT DestCapacity )
{
	if( !Dest || !Src || DestCapacity <= 0 )
		return 0;
	INT Index = 0;
	for( ; Index < DestCapacity - 1 && Src[Index]; ++Index )
	{
		const unsigned char Byte = static_cast<unsigned char>(Src[Index]);
		Dest[Index] = static_cast<TCHAR>(Byte);
	}
	Dest[Index] = 0;
	return 1;
}

namespace
{
	int GFailures = 0;

	void Expect(bool Condition, const char* Message)
	{
		if (!Condition)
		{
			std::fprintf(stderr, "FAIL: %s\n", Message);
			++GFailures;
		}
	}

	void ExpectReason(
		const HP2PathsBootstrapStatus& Status,
		const char* ReasonCode,
		const char* Message)
	{
		Expect(!Status.Accepted, Message);
		Expect(Status.ReasonCode != nullptr && std::strcmp(Status.ReasonCode, ReasonCode) == 0,
			Message);
		if (Status.ReasonCode != nullptr && std::strcmp(Status.ReasonCode, ReasonCode) != 0)
		{
			std::fprintf(stderr, "      got reason '%s'\n", Status.ReasonCode);
		}
	}

	// ------------------------------------------------------------------
	// Synthetic tree construction.
	// ------------------------------------------------------------------

	std::string Sha256Hex(const std::string& Content)
	{
		CC_SHA256_CTX Context;
		CC_SHA256_Init(&Context);
		if (!Content.empty())
		{
			CC_SHA256_Update(&Context, Content.data(),
				static_cast<CC_LONG>(Content.size()));
		}
		unsigned char Digest[CC_SHA256_DIGEST_LENGTH];
		CC_SHA256_Final(Digest, &Context);
		static const char HexDigits[] = "0123456789abcdef";
		std::string Hex;
		Hex.reserve(CC_SHA256_DIGEST_LENGTH * 2);
		for (const unsigned char Byte : Digest)
		{
			Hex.push_back(HexDigits[Byte >> 4]);
			Hex.push_back(HexDigits[Byte & 0x0f]);
		}
		return Hex;
	}

	std::string JoinPath(const std::string& Left, const char* Right)
	{
		std::string Result = Left;
		if (!Result.empty() && Result.back() != '/')
			Result.push_back('/');
		Result += Right;
		return Result;
	}

	bool EnsureParentDirectories(const std::string& Path)
	{
		const size_t LastSlash = Path.rfind('/');
		if (LastSlash == std::string::npos || LastSlash == 0)
			return true;
		for (size_t Index = 1; Index < LastSlash; ++Index)
		{
			if (Path[Index] != '/')
				continue;
			const std::string Partial = Path.substr(0, Index);
			if (mkdir(Partial.c_str(), 0755) == 0 || errno == EEXIST)
				continue;
			return false;
		}
		const std::string Parent = Path.substr(0, LastSlash);
		return mkdir(Parent.c_str(), 0755) == 0 || errno == EEXIST;
	}

	void WriteFile(const std::string& Path, const std::string& Content)
	{
		if (!EnsureParentDirectories(Path))
		{
			std::fprintf(stderr, "FAIL: cannot create directories for %s\n", Path.c_str());
			++GFailures;
			return;
		}
		FILE* Stream = std::fopen(Path.c_str(), "wb");
		if (!Stream)
		{
			std::fprintf(stderr, "FAIL: cannot write %s\n", Path.c_str());
			++GFailures;
			return;
		}
		if (!Content.empty())
			std::fwrite(Content.data(), 1, Content.size(), Stream);
		std::fclose(Stream);
	}

	using FileList = std::vector<std::pair<std::string, std::string>>;

	std::string ManifestJson(const FileList& Files, const char* Profile, int SchemaVersion)
	{
		std::string Json = "{\n  \"entries\": [\n";
		for (size_t Index = 0; Index < Files.size(); ++Index)
		{
			Json += "    {\"path\": \"" + Files[Index].first + "\", \"sha256\": \""
				+ Sha256Hex(Files[Index].second) + "\", \"size\": "
				+ std::to_string(Files[Index].second.size()) + "}";
			if (Index + 1 < Files.size())
				Json += ",";
			Json += "\n";
		}
		Json += "  ],\n  \"profile\": \"";
		Json += Profile;
		Json += "\",\n  \"schema_version\": ";
		Json += std::to_string(SchemaVersion);
		Json += "\n}\n";
		return Json;
	}

	// '<relative-path>\t<size>\t<sha256>' lines, LF-terminated, sorted by path
	// bytes with a trailing newline - the format prepare_retail_data.py emits.
	std::string ChecksumsText(const FileList& Files)
	{
		FileList Sorted = Files;
		std::sort(Sorted.begin(), Sorted.end(),
			[](const std::pair<std::string, std::string>& A,
				const std::pair<std::string, std::string>& B)
			{
				return A.first < B.first;
			});
		std::string Text;
		for (const auto& File : Sorted)
		{
			Text += File.first;
			Text += '\t';
			Text += std::to_string(File.second.size());
			Text += '\t';
			Text += Sha256Hex(File.second);
			Text += '\n';
		}
		return Text;
	}

	std::string WriteOverlayTree(const char* CaseName, const FileList& Files)
	{
		const char* ArtifactDir = std::getenv("HP2_ARTIFACT_DIR");
		const std::string Base = JoinPath(
			ArtifactDir ? ArtifactDir : ".", "tmp/data-identity");
		const std::string Root = JoinPath(Base, CaseName);
		for (const auto& File : Files)
			WriteFile(JoinPath(Root, File.first.c_str()), File.second);
		return Root;
	}

	FileList StandardFiles()
	{
		FileList Files;
		Files.emplace_back("System/Default.ini",
			std::string("[Engine.Engine]\r\nGameEngine=Engine.GameEngine\r\n"));
		Files.emplace_back("Maps/Entry.unr", std::string("fake map payload 1234567890\n"));
		return Files;
	}

	// Captures everything written to stderr while Fn runs.
	template <typename Fn>
	std::string CaptureStderr(Fn&& FnBody)
	{
		std::fflush(stderr);
		const int Saved = dup(fileno(stderr));
		char Template[] = "/tmp/hp2-data-identity-stderr-XXXXXX";
		const int Captured = mkstemp(Template);
		std::string Text;
		if (Captured >= 0 && Saved >= 0)
		{
			dup2(Captured, fileno(stderr));
			FnBody();
			std::fflush(stderr);
			dup2(Saved, fileno(stderr));
			close(Saved);
			lseek(Captured, 0, SEEK_SET);
			char Buffer[4096];
			ssize_t Read = 0;
			while ((Read = read(Captured, Buffer, sizeof(Buffer))) > 0)
				Text.append(Buffer, static_cast<size_t>(Read));
		}
		else if (Saved >= 0)
		{
			close(Saved);
			FnBody();
		}
		if (Captured >= 0)
		{
			close(Captured);
			unlink(Template);
		}
		return Text;
	}

}

int main()
{
	// ------------------------------------------------------------------
	// Legacy mode: no overlay bookkeeping files at all.
	// ------------------------------------------------------------------
	{
		const std::string Root = WriteOverlayTree("legacy",
			{{"System/Default.ini", "[Engine.Engine]\n"}});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"legacy root without overlay files is accepted");
		Expect(Status.Profile == HP2DataProfile_Development,
			"legacy root reports the development profile");
		Expect(Status.ReasonCode == nullptr,
			"accepted legacy root has no rejection reason code");

		std::string LegacyError;
		std::string LegacyRoot;
		Expect(ValidateHP2DataRoot(Root, LegacyRoot, LegacyError),
			"ValidateHP2DataRoot still accepts legacy roots");
		Expect(LegacyError.empty(),
			"ValidateHP2DataRoot sets no error for accepted roots");
	}

	{
		// A directory that is not a data root keeps refusing.
		const std::string Root = WriteOverlayTree("empty-dir", {});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(!ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"directory without System/Default.ini is refused");
		Expect(Status.ReasonCode == nullptr,
			"non-data directory carries no identity reason code");
	}

	// ------------------------------------------------------------------
	// Overlay mode: marker present but manifest missing.
	// ------------------------------------------------------------------
	{
		const FileList Files = StandardFiles();
		const std::string Root = WriteOverlayTree("manifest-missing", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Files[1].second},
			{"overlay-checksums.txt", ChecksumsText(Files)},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(!ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"stray checksums index without manifest is refused");
		ExpectReason(Status, "data.manifest_missing",
			"stray checksums index reports data.manifest_missing");
	}

	// ------------------------------------------------------------------
	// Overlay mode: manifest present but checksums missing.
	// ------------------------------------------------------------------
	{
		const FileList Files = StandardFiles();
		const std::string Root = WriteOverlayTree("checksums-missing", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Files[1].second},
			{"overlay-manifest.json", ManifestJson(Files, "safe", 3)},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(!ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"manifest without sibling checksums is refused");
		ExpectReason(Status, "data.checksums_missing",
			"manifest without checksums reports data.checksums_missing");
	}

	// ------------------------------------------------------------------
	// Overlay mode: malformed manifest.
	// ------------------------------------------------------------------
	{
		const FileList Files = StandardFiles();
		const std::string Root = WriteOverlayTree("manifest-invalid", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Files[1].second},
			{"overlay-checksums.txt", ChecksumsText(Files)},
			{"overlay-manifest.json", "{\"entries\": [ not json"},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(!ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"malformed manifest is refused");
		ExpectReason(Status, "data.manifest_invalid",
			"malformed manifest reports data.manifest_invalid");
	}

	{
		// Unsupported schema version is also a structural failure.
		const FileList Files = StandardFiles();
		const std::string Root = WriteOverlayTree("schema-version", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Files[1].second},
			{"overlay-checksums.txt", ChecksumsText(Files)},
			{"overlay-manifest.json", ManifestJson(Files, "safe", 99)},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(!ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"unsupported schema version is refused");
		ExpectReason(Status, "data.manifest_invalid",
			"unsupported schema version reports data.manifest_invalid");
	}

	// ------------------------------------------------------------------
	// Overlay mode: unknown profile.
	// ------------------------------------------------------------------
	{
		const FileList Files = StandardFiles();
		const std::string Root = WriteOverlayTree("profile-unknown", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Files[1].second},
			{"overlay-checksums.txt", ChecksumsText(Files)},
			{"overlay-manifest.json", ManifestJson(Files, "prototype", 3)},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(!ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"unknown profile is refused");
		ExpectReason(Status, "data.profile_unknown",
			"unknown profile reports data.profile_unknown");
	}

	// ------------------------------------------------------------------
	// Overlay mode: path set disagreement between the two indexes.
	// ------------------------------------------------------------------
	{
		const FileList Files = StandardFiles();
		FileList Reduced = Files;
		Reduced.pop_back(); // checksums index loses Maps/Entry.unr
		const std::string Root = WriteOverlayTree("path-set-mismatch", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Files[1].second},
			{"overlay-checksums.txt", ChecksumsText(Reduced)},
			{"overlay-manifest.json", ManifestJson(Files, "safe", 3)},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(!ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"path set mismatch between manifest and checksums is refused");
		ExpectReason(Status, "data.path_set_mismatch",
			"path set mismatch reports data.path_set_mismatch");
	}

	// ------------------------------------------------------------------
	// Overlay mode: on-disk size drift.
	// ------------------------------------------------------------------
	{
		const FileList Files = StandardFiles();
		std::string Truncated = Files[1].second.substr(0, Files[1].second.size() - 4);
		const std::string Root = WriteOverlayTree("size-mismatch", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Truncated},
			{"overlay-checksums.txt", ChecksumsText(Files)},
			{"overlay-manifest.json", ManifestJson(Files, "safe", 3)},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(!ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"truncated overlay file is refused");
		ExpectReason(Status, "data.size_mismatch",
			"size drift reports data.size_mismatch");
	}

	// ------------------------------------------------------------------
	// Overlay mode: same-length content corruption.
	// ------------------------------------------------------------------
	{
		const FileList Files = StandardFiles();
		std::string Corrupted = Files[1].second;
		Corrupted[8] = static_cast<char>(Corrupted[8] ^ 0x5a);
		const std::string RejectLine = CaptureStderr([&]()
		{
			std::string CanonicalRoot;
			HP2PathsBootstrapStatus Status;
			ValidateHP2DataRootStatus(
				WriteOverlayTree("hash-mismatch", {
					{"System/Default.ini", Files[0].second},
					{"Maps/Entry.unr", Corrupted},
					{"overlay-checksums.txt", ChecksumsText(Files)},
					{"overlay-manifest.json", ManifestJson(Files, "safe", 3)},
				}),
				CanonicalRoot,
				Status);
		});
		Expect(RejectLine.find("HP2_DATA_IDENTITY_REJECT data.hash_mismatch ") == 0,
			"stderr carries 'HP2_DATA_IDENTITY_REJECT data.hash_mismatch <detail>'");

		// Re-run outside capture to check the structured status too.
		const std::string Root = WriteOverlayTree("hash-mismatch-status", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Corrupted},
			{"overlay-checksums.txt", ChecksumsText(Files)},
			{"overlay-manifest.json", ManifestJson(Files, "safe", 3)},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(!ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"same-length corruption is refused");
		ExpectReason(Status, "data.hash_mismatch",
			"same-length corruption reports data.hash_mismatch");
	}

	// ------------------------------------------------------------------
	// Overlay mode: consistent trees are accepted per profile.
	// ------------------------------------------------------------------
	{
		const FileList Files = StandardFiles();
		const std::string Root = WriteOverlayTree("accept-safe", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Files[1].second},
			{"overlay-checksums.txt", ChecksumsText(Files)},
			{"overlay-manifest.json", ManifestJson(Files, "safe", 3)},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(ValidateHP2DataRootStatus(Root, CanonicalRoot, Status),
			"consistent safe overlay is accepted");
		Expect(Status.Profile == HP2DataProfile_Safe,
			"safe overlay reports the safe profile");
		Expect(Status.ReasonCode == nullptr,
			"accepted overlay has no rejection reason code");
	}

	{
		const FileList Files = StandardFiles();
		const std::string FullRoot = WriteOverlayTree("accept-full", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Files[1].second},
			{"overlay-checksums.txt", ChecksumsText(Files)},
			{"overlay-manifest.json", ManifestJson(Files, "full", 2)},
		});
		std::string CanonicalRoot;
		HP2PathsBootstrapStatus Status;
		Expect(ValidateHP2DataRootStatus(FullRoot, CanonicalRoot, Status)
			&& Status.Profile == HP2DataProfile_Full,
			"full overlay maps to the full profile");

		const std::string RetailRoot = WriteOverlayTree("accept-retail-only", {
			{"System/Default.ini", Files[0].second},
			{"Maps/Entry.unr", Files[1].second},
			{"overlay-checksums.txt", ChecksumsText(Files)},
			{"overlay-manifest.json", ManifestJson(Files, "retail-only", 3)},
		});
		Status = HP2PathsBootstrapStatus();
		Expect(ValidateHP2DataRootStatus(RetailRoot, CanonicalRoot, Status)
			&& Status.Profile == HP2DataProfile_RetailOnly,
			"retail-only overlay maps to the retail-only profile");
	}

	// ------------------------------------------------------------------
	// PrepareHP2Paths keeps its signature and accepts an explicit legacy
	// root end-to-end when HOME provides a writable user directory.
	// ------------------------------------------------------------------
	if (std::getenv("HOME"))
	{
		const std::string Root = WriteOverlayTree("prepare-legacy",
			{{"System/Default.ini", "[Engine.Engine]\n"}});
		std::string Argument = "-datadir=" + Root;
		char* Arguments[2] = {const_cast<char*>("hp2-data-identity-tests"), &Argument[0]};
		Expect(PrepareHP2Paths(2, Arguments),
			"PrepareHP2Paths accepts an explicit legacy data root");
	}

	if (GFailures == 0)
	{
		std::printf("data identity contracts passed\n");
		return 0;
	}
	std::printf("%d data identity contract failure(s)\n", GFailures);
	return 1;
}

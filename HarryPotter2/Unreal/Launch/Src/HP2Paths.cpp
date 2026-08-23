/*=============================================================================
	HP2Paths.cpp: Pre-appInit data and user path bootstrap.
=============================================================================*/

#include "Core.h"
#include "HP2Paths.h"

#include <CommonCrypto/CommonDigest.h>

#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <memory>
#include <sys/stat.h>
#include <unistd.h>
#include <utility>
#include <vector>

namespace
{
constexpr const char* DataDirPrefix = "-datadir=";
constexpr const char* ExternalDataSuffix =
	"Library/Application Support/Harry Potter 2/Data/Unreal";
constexpr const char* ConventionalRetailDataSuffix =
	"Library/Application Support/Harry Potter 2/Data/Retail";
constexpr const char* ConventionalPrototypeDataSuffix =
	"Library/Application Support/Harry Potter 2/Data/Prototype";
constexpr const char* UserSuffix =
	"Library/Application Support/Harry Potter 2/User";

bool EqualsAsciiNoCase(char A, char B)
{
	if (A >= 'A' && A <= 'Z')
		A = static_cast<char>(A - 'A' + 'a');
	if (B >= 'A' && B <= 'Z')
		B = static_cast<char>(B - 'A' + 'a');
	return A == B;
}

const char* DataDirArgument(const char* Argument)
{
	if (!Argument)
		return nullptr;

	const size_t PrefixLength = std::strlen(DataDirPrefix);
	for (size_t Index = 0; Index < PrefixLength; ++Index)
		if (!Argument[Index] || !EqualsAsciiNoCase(Argument[Index], DataDirPrefix[Index]))
			return nullptr;
	return Argument + PrefixLength;
}

char* JoinPath(const char* Left, const char* Right)
{
	if (!Left || !Right)
		return nullptr;
	const size_t LeftLength = std::strlen(Left);
	const size_t RightLength = std::strlen(Right);
	const bool NeedSlash = LeftLength > 0 && Left[LeftLength - 1] != '/';
	const size_t SeparatorLength = NeedSlash ? 1 : 0;
	if (LeftLength > static_cast<size_t>(-1) - RightLength - SeparatorLength - 1)
		return nullptr;

	char* Result = static_cast<char*>(std::malloc(
		LeftLength + SeparatorLength + RightLength + 1));
	if (!Result)
		return nullptr;
	std::memcpy(Result, Left, LeftLength);
	size_t Position = LeftLength;
	if (NeedSlash)
		Result[Position++] = '/';
	std::memcpy(Result + Position, Right, RightLength + 1);
	return Result;
}

char* CanonicalizeExistingDirectory(const char* Directory);

bool GetConventionalDataRoot(
	const char* Suffix,
	std::string& AbsoluteRoot)
{
	AbsoluteRoot.clear();
	char* Home = CanonicalizeExistingDirectory(std::getenv("HOME"));
	if (!Home)
		return false;

	char* Root = JoinPath(Home, Suffix);
	std::free(Home);
	if (!Root)
		return false;
	AbsoluteRoot = Root;
	std::free(Root);
	return true;
}

bool IsReadableFile(const char* Filename)
{
	struct stat Info;
	return Filename
		&& stat(Filename, &Info) == 0
		&& S_ISREG(Info.st_mode)
		&& access(Filename, R_OK) == 0;
}

bool IsDirectory(const char* Directory)
{
	struct stat Info;
	return Directory && stat(Directory, &Info) == 0 && S_ISDIR(Info.st_mode);
}

bool IsWritableDirectory(const char* Directory)
{
	return IsDirectory(Directory) && access(Directory, W_OK | X_OK) == 0;
}

char* CanonicalizeExistingDirectory(const char* Directory)
{
	char* Canonical = Directory ? realpath(Directory, nullptr) : nullptr;
	if (Canonical && !IsDirectory(Canonical))
	{
		std::free(Canonical);
		Canonical = nullptr;
	}
	return Canonical;
}

bool IsSameOrDescendant(const char* Path, const char* Parent)
{
	const size_t ParentLength = std::strlen(Parent);
	return std::strcmp(Path, Parent) == 0
		|| (std::strncmp(Path, Parent, ParentLength) == 0
			&& Path[ParentLength]
			&& (ParentLength == 1 || Path[ParentLength] == '/'));
}

constexpr const char* OverlayManifestName = "overlay-manifest.json";
constexpr const char* OverlayChecksumsName = "overlay-checksums.txt";

constexpr const char* ReasonManifestMissing = "data.manifest_missing";
constexpr const char* ReasonChecksumsMissing = "data.checksums_missing";
constexpr const char* ReasonManifestInvalid = "data.manifest_invalid";
constexpr const char* ReasonPathSetMismatch = "data.path_set_mismatch";
constexpr const char* ReasonSizeMismatch = "data.size_mismatch";
constexpr const char* ReasonHashMismatch = "data.hash_mismatch";
constexpr const char* ReasonProfileUnknown = "data.profile_unknown";

void SanitizeDetail(std::string& Text)
{
	for (char& Ch : Text)
	{
		const unsigned char Byte = static_cast<unsigned char>(Ch);
		if (Byte < 0x20 || Byte == 0x7f)
			Ch = ' ';
	}
}

void ReportIdentityReject(const char* ReasonCode, const std::string& Detail)
{
	std::string Safe = Detail;
	SanitizeDetail(Safe);
	std::fprintf(stderr, "HP2_DATA_IDENTITY_REJECT %s %s\n", ReasonCode, Safe.c_str());
}

bool ReadFileBytes(const char* Path, std::string& Contents)
{
	Contents.clear();
	FILE* Stream = std::fopen(Path, "rb");
	if (!Stream)
		return false;
	char Buffer[8192];
	size_t Read = 0;
	while ((Read = std::fread(Buffer, 1, sizeof(Buffer), Stream)) > 0)
		Contents.append(Buffer, Read);
	const bool Ok = std::ferror(Stream) == 0;
	std::fclose(Stream);
	return Ok;
}

bool HashFileSha256(const char* Path, unsigned long long& OutSize, std::string& OutDigest)
{
	OutSize = 0;
	OutDigest.clear();
	FILE* Stream = std::fopen(Path, "rb");
	if (!Stream)
		return false;
	CC_SHA256_CTX Context;
	CC_SHA256_Init(&Context);
	unsigned char Buffer[65536];
	size_t Read = 0;
	unsigned long long Total = 0;
	while ((Read = std::fread(Buffer, 1, sizeof(Buffer), Stream)) > 0)
	{
		CC_SHA256_Update(&Context, Buffer, static_cast<CC_LONG>(Read));
		Total += Read;
	}
	const bool Ok = std::ferror(Stream) == 0;
	if (Ok)
	{
		unsigned char Digest[CC_SHA256_DIGEST_LENGTH];
		CC_SHA256_Final(Digest, &Context);
		static const char HexDigits[] = "0123456789abcdef";
		for (const unsigned char Byte : Digest)
		{
			OutDigest.push_back(HexDigits[Byte >> 4]);
			OutDigest.push_back(HexDigits[Byte & 0x0f]);
		}
		OutSize = Total;
	}
	std::fclose(Stream);
	return Ok;
}

std::string ToLowerAscii(const std::string& Text)
{
	std::string Lower = Text;
	for (char& Ch : Lower)
	{
		if (Ch >= 'A' && Ch <= 'Z')
			Ch = static_cast<char>(Ch - 'A' + 'a');
	}
	return Lower;
}

bool IsHexDigest(const std::string& Text)
{
	if (Text.size() != 64)
		return false;
	for (const char Ch : Text)
	{
		const bool Hex = (Ch >= '0' && Ch <= '9')
			|| (Ch >= 'a' && Ch <= 'f')
			|| (Ch >= 'A' && Ch <= 'F');
		if (!Hex)
			return false;
	}
	return true;
}

bool IsSafeOverlayRelativePath(const std::string& Path)
{
	if (Path.empty() || Path.front() == '/' || Path.back() == '/')
		return false;
	if (Path.find('\\') != std::string::npos)
		return false;
	size_t Start = 0;
	for (;;)
	{
		const size_t Slash = Path.find('/', Start);
		const size_t End = Slash == std::string::npos ? Path.size() : Slash;
		const size_t ComponentLength = End - Start;
		if (ComponentLength == 0)
			return false;
		if (ComponentLength == 1 && Path[Start] == '.')
			return false;
		if (ComponentLength == 2 && Path[Start] == '.' && Path[Start + 1] == '.')
			return false;
		if (Slash == std::string::npos)
			return true;
		Start = Slash + 1;
	}
}

struct JsonNode;
using JsonNodePtr = std::unique_ptr<JsonNode>;

struct JsonNode
{
	enum class Kind
	{
		Null,
		Boolean,
		Number,
		String,
		Array,
		Object
	};

	Kind KindValue = Kind::Null;
	bool BooleanValue = false;
	bool HasInteger = false;
	long long IntegerValue = 0;
	std::string StringValue;
	std::vector<JsonNodePtr> Items;
	std::vector<std::pair<std::string, JsonNodePtr>> Members;

	const JsonNode* FindMember(const char* Name) const
	{
		for (const auto& Member : Members)
		{
			if (Member.first == Name)
				return Member.second.get();
		}
		return nullptr;
	}
};

// Strict single-document JSON reader covering the manifest subset: objects,
// arrays, strings with escapes, integers, floats, booleans, and null.
class JsonParser
{
public:
	static bool Parse(const char* Text, JsonNodePtr& Root, std::string& Error)
	{
		JsonParser Parser(Text);
		if (!Parser.ParseValue(Root))
		{
			Error = Parser.ErrorMessage;
			return false;
		}
		Parser.SkipWhitespace();
		if (*Parser.Cursor != '\0')
		{
			Parser.Fail("trailing characters after JSON document");
			Error = Parser.ErrorMessage;
			return false;
		}
		return true;
	}

private:
	explicit JsonParser(const char* Text)
		: Cursor(Text)
	{
	}

	const char* Cursor;
	std::string ErrorMessage;

	void SkipWhitespace()
	{
		while (*Cursor == ' ' || *Cursor == '\t' || *Cursor == '\n' || *Cursor == '\r')
			++Cursor;
	}

	bool Fail(const char* Message)
	{
		if (ErrorMessage.empty())
			ErrorMessage = Message;
		return false;
	}

	bool ParseValue(JsonNodePtr& Out)
	{
		Out.reset();
		SkipWhitespace();
		const char Next = *Cursor;
		if (Next == '{')
			return ParseObject(Out);
		if (Next == '[')
			return ParseArray(Out);
		if (Next == '"')
			return ParseStringNode(Out);
		if (Next == 't')
			return ParseLiteral("true", Out, JsonNode::Kind::Boolean, true);
		if (Next == 'f')
			return ParseLiteral("false", Out, JsonNode::Kind::Boolean, false);
		if (Next == 'n')
			return ParseLiteral("null", Out, JsonNode::Kind::Null, false);
		if (Next == '-' || (Next >= '0' && Next <= '9'))
			return ParseNumber(Out);
		return Fail("expected a JSON value");
	}

	bool ParseLiteral(const char* Literal, JsonNodePtr& Out, JsonNode::Kind Kind, bool BooleanValue)
	{
		const size_t Length = std::strlen(Literal);
		if (std::strncmp(Cursor, Literal, Length) != 0)
			return Fail("malformed JSON literal");
		Cursor += Length;
		Out = std::make_unique<JsonNode>();
		Out->KindValue = Kind;
		Out->BooleanValue = BooleanValue;
		return true;
	}

	bool ParseNumber(JsonNodePtr& Out)
	{
		char* End = nullptr;
		const long long Integer = std::strtoll(Cursor, &End, 10);
		if (End == Cursor)
			return Fail("malformed JSON number");
		const bool StrictlyInteger = *End != '.' && *End != 'e' && *End != 'E';
		if (!StrictlyInteger)
			std::strtod(Cursor, &End);
		Cursor = End;
		Out = std::make_unique<JsonNode>();
		Out->KindValue = JsonNode::Kind::Number;
		Out->HasInteger = StrictlyInteger;
		Out->IntegerValue = Integer;
		return true;
	}

	bool ParseHex4(unsigned& CodeUnit)
	{
		CodeUnit = 0;
		for (int Index = 0; Index < 4; ++Index)
		{
			const char Ch = *Cursor;
			unsigned Digit = 0;
			if (Ch >= '0' && Ch <= '9')
				Digit = static_cast<unsigned>(Ch - '0');
			else if (Ch >= 'a' && Ch <= 'f')
				Digit = static_cast<unsigned>(Ch - 'a' + 10);
			else if (Ch >= 'A' && Ch <= 'F')
				Digit = static_cast<unsigned>(Ch - 'A' + 10);
			else
				return false;
			CodeUnit = (CodeUnit << 4) | Digit;
			++Cursor;
		}
		return true;
	}

	void AppendUtf8(unsigned CodePoint, std::string& Out)
	{
		if (CodePoint < 0x80)
		{
			Out.push_back(static_cast<char>(CodePoint));
		}
		else if (CodePoint < 0x800)
		{
			Out.push_back(static_cast<char>(0xc0 | (CodePoint >> 6)));
			Out.push_back(static_cast<char>(0x80 | (CodePoint & 0x3f)));
		}
		else if (CodePoint < 0x10000)
		{
			Out.push_back(static_cast<char>(0xe0 | (CodePoint >> 12)));
			Out.push_back(static_cast<char>(0x80 | ((CodePoint >> 6) & 0x3f)));
			Out.push_back(static_cast<char>(0x80 | (CodePoint & 0x3f)));
		}
		else
		{
			Out.push_back(static_cast<char>(0xf0 | (CodePoint >> 18)));
			Out.push_back(static_cast<char>(0x80 | ((CodePoint >> 12) & 0x3f)));
			Out.push_back(static_cast<char>(0x80 | ((CodePoint >> 6) & 0x3f)));
			Out.push_back(static_cast<char>(0x80 | (CodePoint & 0x3f)));
		}
	}

	bool ParseStringRaw(std::string& Out)
	{
		if (*Cursor != '"')
			return Fail("expected a JSON string");
		++Cursor;
		Out.clear();
		for (;;)
		{
			const char Ch = *Cursor;
			if (Ch == '\0')
				return Fail("unterminated JSON string");
			if (Ch == '"')
			{
				++Cursor;
				return true;
			}
			if (Ch == '\\')
			{
				++Cursor;
				const char Escape = *Cursor;
				switch (Escape)
				{
				case '"': Out.push_back('"'); ++Cursor; break;
				case '\\': Out.push_back('\\'); ++Cursor; break;
				case '/': Out.push_back('/'); ++Cursor; break;
				case 'b': Out.push_back('\b'); ++Cursor; break;
				case 'f': Out.push_back('\f'); ++Cursor; break;
				case 'n': Out.push_back('\n'); ++Cursor; break;
				case 'r': Out.push_back('\r'); ++Cursor; break;
				case 't': Out.push_back('\t'); ++Cursor; break;
				case 'u':
				{
					++Cursor;
					unsigned CodeUnit = 0;
					if (!ParseHex4(CodeUnit))
						return Fail("malformed JSON unicode escape");
					unsigned CodePoint = CodeUnit;
					if (CodeUnit >= 0xd800 && CodeUnit <= 0xdbff
						&& Cursor[0] == '\\' && Cursor[1] == 'u')
					{
						Cursor += 2;
						unsigned LowSurrogate = 0;
						if (!ParseHex4(LowSurrogate)
							|| LowSurrogate < 0xdc00 || LowSurrogate > 0xdfff)
							return Fail("unpaired JSON surrogate escape");
						CodePoint = 0x10000
							+ ((CodeUnit - 0xd800) << 10)
							+ (LowSurrogate - 0xdc00);
					}
					AppendUtf8(CodePoint, Out);
					break;
				}
				default:
					return Fail("invalid JSON escape");
				}
				continue;
			}
			Out.push_back(Ch);
			++Cursor;
		}
	}

	bool ParseStringNode(JsonNodePtr& Out)
	{
		Out = std::make_unique<JsonNode>();
		Out->KindValue = JsonNode::Kind::String;
		return ParseStringRaw(Out->StringValue);
	}

	bool ParseObject(JsonNodePtr& Out)
	{
		++Cursor;
		Out = std::make_unique<JsonNode>();
		Out->KindValue = JsonNode::Kind::Object;
		SkipWhitespace();
		if (*Cursor == '}')
		{
			++Cursor;
			return true;
		}
		for (;;)
		{
			SkipWhitespace();
			std::string Name;
			if (!ParseStringRaw(Name))
				return false;
			SkipWhitespace();
			if (*Cursor != ':')
				return Fail("expected ':' in JSON object");
			++Cursor;
			JsonNodePtr Value;
			if (!ParseValue(Value))
				return false;
			Out->Members.emplace_back(std::move(Name), std::move(Value));
			SkipWhitespace();
			if (*Cursor == ',')
			{
				++Cursor;
				continue;
			}
			if (*Cursor == '}')
			{
				++Cursor;
				return true;
			}
			return Fail("expected ',' or '}' in JSON object");
		}
	}

	bool ParseArray(JsonNodePtr& Out)
	{
		++Cursor;
		Out = std::make_unique<JsonNode>();
		Out->KindValue = JsonNode::Kind::Array;
		SkipWhitespace();
		if (*Cursor == ']')
		{
			++Cursor;
			return true;
		}
		for (;;)
		{
			JsonNodePtr Value;
			if (!ParseValue(Value))
				return false;
			Out->Items.push_back(std::move(Value));
			SkipWhitespace();
			if (*Cursor == ',')
			{
				++Cursor;
				continue;
			}
			if (*Cursor == ']')
			{
				++Cursor;
				return true;
			}
			return Fail("expected ',' or ']' in JSON array");
		}
	}
};

struct OverlayEntryIdentity
{
	unsigned long long Size = 0;
	std::string Sha256;
};

using OverlayIdentityMap = std::map<std::string, OverlayEntryIdentity>;

// Structural failures of the checksums index (unreadable, malformed lines,
// bad digests) are refused with data.checksums_missing: without a usable
// index the overlay has no verifiable identity.
bool ParseChecksumsIndex(
	const std::string& Contents,
	OverlayIdentityMap& Out,
	std::string& Detail)
{
	if (Contents.empty())
	{
		Detail = "overlay-checksums.txt is empty";
		return false;
	}
	if (Contents.back() != '\n')
	{
		Detail = "overlay-checksums.txt does not end with a newline";
		return false;
	}

	size_t LineStart = 0;
	int LineNumber = 0;
	while (LineStart < Contents.size())
	{
		++LineNumber;
		const size_t LineEnd = Contents.find('\n', LineStart);
		const std::string Line = Contents.substr(LineStart, LineEnd - LineStart);
		LineStart = LineEnd + 1;
		if (Line.find('\r') != std::string::npos)
		{
			Detail = "checksum line " + std::to_string(LineNumber) + " contains a carriage return";
			return false;
		}
		const size_t FirstTab = Line.find('\t');
		const size_t SecondTab = FirstTab == std::string::npos
			? std::string::npos
			: Line.find('\t', FirstTab + 1);
		if (FirstTab == std::string::npos || SecondTab == std::string::npos
			|| Line.find('\t', SecondTab + 1) != std::string::npos)
		{
			Detail = "checksum line " + std::to_string(LineNumber)
				+ " is not '<path>\\t<size>\\t<sha256>'";
			return false;
		}

		const std::string Path = Line.substr(0, FirstTab);
		const std::string SizeText = Line.substr(FirstTab + 1, SecondTab - FirstTab - 1);
		const std::string Digest = Line.substr(SecondTab + 1);
		if (!IsSafeOverlayRelativePath(Path))
		{
			Detail = "checksum line " + std::to_string(LineNumber) + " has an unsafe path";
			return false;
		}
		if (SizeText.empty())
		{
			Detail = "checksum line " + std::to_string(LineNumber) + " has an empty size";
			return false;
		}
		for (const char Ch : SizeText)
		{
			if (Ch < '0' || Ch > '9')
			{
				Detail = "checksum line " + std::to_string(LineNumber) + " has a non-numeric size";
				return false;
			}
		}
		if (!IsHexDigest(Digest))
		{
			Detail = "checksum line " + std::to_string(LineNumber) + " has a malformed sha256";
			return false;
		}

		OverlayEntryIdentity Entry;
		Entry.Size = std::strtoull(SizeText.c_str(), nullptr, 10);
		Entry.Sha256 = ToLowerAscii(Digest);
		if (!Out.emplace(Path, std::move(Entry)).second)
		{
			Detail = "duplicate checksum line for path: " + Path;
			return false;
		}
	}
	return true;
}

bool ParseOverlayManifest(
	const std::string& Contents,
	HP2DataProfileKind& Profile,
	OverlayIdentityMap& Out,
	const char*& ReasonCode,
	std::string& Detail)
{
	ReasonCode = ReasonManifestInvalid;
	JsonNodePtr Root;
	std::string Error;
	if (!JsonParser::Parse(Contents.c_str(), Root, Error))
	{
		Detail = "overlay-manifest.json is not valid JSON: " + Error;
		return false;
	}

	const JsonNode* SchemaVersion = Root->FindMember("schema_version");
	if (!SchemaVersion || !SchemaVersion->HasInteger
		|| SchemaVersion->IntegerValue < 2 || SchemaVersion->IntegerValue > 3)
	{
		Detail = "overlay-manifest.json has a missing or unsupported schema_version";
		return false;
	}

	const JsonNode* ManifestProfile = Root->FindMember("profile");
	if (!ManifestProfile || ManifestProfile->KindValue != JsonNode::Kind::String)
	{
		ReasonCode = ReasonProfileUnknown;
		Detail = "overlay-manifest.json has no string profile";
		return false;
	}
	const std::string ProfileText = ManifestProfile->StringValue;
	if (ProfileText == "safe")
		Profile = HP2DataProfile_Safe;
	else if (ProfileText == "full")
		Profile = HP2DataProfile_Full;
	else if (ProfileText == "retail-only")
		Profile = HP2DataProfile_RetailOnly;
	else
	{
		ReasonCode = ReasonProfileUnknown;
		Detail = "unknown overlay profile: " + ProfileText;
		return false;
	}

	const JsonNode* Entries = Root->FindMember("entries");
	if (!Entries || Entries->KindValue != JsonNode::Kind::Array || Entries->Items.empty())
	{
		Detail = "overlay-manifest.json has no non-empty entries array";
		return false;
	}
	for (const JsonNodePtr& Entry : Entries->Items)
	{
		if (!Entry || Entry->KindValue != JsonNode::Kind::Object)
		{
			Detail = "overlay manifest entry is not an object";
			return false;
		}
		const JsonNode* Path = Entry->FindMember("path");
		if (!Path || Path->KindValue != JsonNode::Kind::String
			|| !IsSafeOverlayRelativePath(Path->StringValue))
		{
			Detail = "overlay manifest entry has a missing or unsafe path";
			return false;
		}
		const JsonNode* Size = Entry->FindMember("size");
		if (!Size || !Size->HasInteger || Size->IntegerValue < 0)
		{
			Detail = "overlay manifest entry " + Path->StringValue + " has no integer size";
			return false;
		}
		const JsonNode* Digest = Entry->FindMember("sha256");
		if (!Digest || Digest->KindValue != JsonNode::Kind::String || !IsHexDigest(Digest->StringValue))
		{
			Detail = "overlay manifest entry " + Path->StringValue + " has a malformed sha256";
			return false;
		}

		OverlayEntryIdentity Identity;
		Identity.Size = static_cast<unsigned long long>(Size->IntegerValue);
		Identity.Sha256 = ToLowerAscii(Digest->StringValue);
		if (!Out.emplace(Path->StringValue, std::move(Identity)).second)
		{
			Detail = "duplicate overlay manifest entry for path: " + Path->StringValue;
			return false;
		}
	}
	ReasonCode = nullptr;
	return true;
}

bool VerifyOverlayIdentity(
	const char* CanonicalRoot,
	const char*& ReasonCode,
	std::string& Detail,
	HP2DataProfileKind& Profile)
{
	Profile = HP2DataProfile_Unknown;

	char* ManifestPath = JoinPath(CanonicalRoot, OverlayManifestName);
	char* ChecksumsPath = JoinPath(CanonicalRoot, OverlayChecksumsName);
	const bool HasManifest = IsReadableFile(ManifestPath);
	const bool HasChecksums = IsReadableFile(ChecksumsPath);
	std::free(ManifestPath);
	std::free(ChecksumsPath);

	if (!HasManifest)
	{
		ReasonCode = ReasonManifestMissing;
		Detail = "overlay-checksums.txt present without overlay-manifest.json";
		return false;
	}
	if (!HasChecksums)
	{
		ReasonCode = ReasonChecksumsMissing;
		Detail = "overlay-manifest.json without sibling overlay-checksums.txt";
		return false;
	}

	std::string ManifestContents;
	std::string ChecksumsContents;
	ManifestPath = JoinPath(CanonicalRoot, OverlayManifestName);
	ChecksumsPath = JoinPath(CanonicalRoot, OverlayChecksumsName);
	const bool ReadManifest = ManifestPath && ReadFileBytes(ManifestPath, ManifestContents);
	const bool ReadChecksums = ChecksumsPath && ReadFileBytes(ChecksumsPath, ChecksumsContents);
	std::free(ManifestPath);
	std::free(ChecksumsPath);
	if (!ReadManifest)
	{
		ReasonCode = ReasonManifestInvalid;
		Detail = "cannot read overlay-manifest.json";
		return false;
	}
	if (!ReadChecksums)
	{
		ReasonCode = ReasonChecksumsMissing;
		Detail = "cannot read overlay-checksums.txt";
		return false;
	}

	HP2DataProfileKind ManifestProfile = HP2DataProfile_Unknown;
	OverlayIdentityMap ManifestEntries;
	if (!ParseOverlayManifest(ManifestContents, ManifestProfile, ManifestEntries, ReasonCode, Detail))
		return false;

	OverlayIdentityMap ChecksumsEntries;
	if (!ParseChecksumsIndex(ChecksumsContents, ChecksumsEntries, Detail))
	{
		ReasonCode = ReasonChecksumsMissing;
		return false;
	}

	std::vector<std::string> ManifestOnly;
	std::vector<std::string> ChecksumsOnly;
	auto ManifestIt = ManifestEntries.begin();
	auto ChecksumsIt = ChecksumsEntries.begin();
	while (ManifestIt != ManifestEntries.end() || ChecksumsIt != ChecksumsEntries.end())
	{
		if (ChecksumsIt == ChecksumsEntries.end()
			|| (ManifestIt != ManifestEntries.end() && ManifestIt->first < ChecksumsIt->first))
		{
			if (ManifestOnly.size() < 3)
				ManifestOnly.push_back(ManifestIt->first);
			++ManifestIt;
		}
		else if (ManifestIt == ManifestEntries.end() || ChecksumsIt->first < ManifestIt->first)
		{
			if (ChecksumsOnly.size() < 3)
				ChecksumsOnly.push_back(ChecksumsIt->first);
			++ChecksumsIt;
		}
		else
		{
			if (ManifestIt->second.Size != ChecksumsIt->second.Size)
			{
				ReasonCode = ReasonSizeMismatch;
				Detail = "index disagreement for " + ManifestIt->first + ": manifest size "
					+ std::to_string(ManifestIt->second.Size) + " vs checksums size "
					+ std::to_string(ChecksumsIt->second.Size);
				return false;
			}
			if (ManifestIt->second.Sha256 != ChecksumsIt->second.Sha256)
			{
				ReasonCode = ReasonHashMismatch;
				Detail = "index disagreement for " + ManifestIt->first
					+ ": manifest sha256 differs from checksums sha256";
				return false;
			}
			++ManifestIt;
			++ChecksumsIt;
		}
	}
	if (!ManifestOnly.empty() || !ChecksumsOnly.empty())
	{
		std::string ManifestOnlyText;
		for (const std::string& Path : ManifestOnly)
			ManifestOnlyText += (ManifestOnlyText.empty() ? "" : ", ") + Path;
		std::string ChecksumsOnlyText;
		for (const std::string& Path : ChecksumsOnly)
			ChecksumsOnlyText += (ChecksumsOnlyText.empty() ? "" : ", ") + Path;
		ReasonCode = ReasonPathSetMismatch;
		Detail = "manifest paths: [" + ManifestOnlyText + "]; checksums paths: ["
			+ ChecksumsOnlyText + "]";
		return false;
	}

	for (const auto& Entry : ManifestEntries)
	{
		char* EntryPath = JoinPath(CanonicalRoot, Entry.first.c_str());
		unsigned long long ActualSize = 0;
		std::string ActualDigest;
		const bool Hashed = EntryPath && HashFileSha256(EntryPath, ActualSize, ActualDigest);
		std::free(EntryPath);
		if (!Hashed)
		{
			ReasonCode = ReasonPathSetMismatch;
			Detail = "overlay entry missing from tree: " + Entry.first;
			return false;
		}
		if (ActualSize != Entry.second.Size)
		{
			ReasonCode = ReasonSizeMismatch;
			Detail = "size mismatch for " + Entry.first + ": expected "
				+ std::to_string(Entry.second.Size) + ", found " + std::to_string(ActualSize);
			return false;
		}
		if (ActualDigest != Entry.second.Sha256)
		{
			ReasonCode = ReasonHashMismatch;
			Detail = "sha256 mismatch for " + Entry.first + ": expected "
				+ Entry.second.Sha256 + ", found " + ActualDigest;
			return false;
		}
	}

	Profile = ManifestProfile;
	ReasonCode = nullptr;
	Detail = "overlay identity verified";
	return true;
}

char* ValidateDataRoot(const char* Candidate, HP2PathsBootstrapStatus* Status = nullptr)
{
	if (Status)
		*Status = HP2PathsBootstrapStatus();

	char* CanonicalRoot = CanonicalizeExistingDirectory(Candidate);
	if (!CanonicalRoot)
	{
		if (Status)
			Status->Detail = "candidate does not name an existing directory";
		return nullptr;
	}

	char* DefaultIni = JoinPath(CanonicalRoot, "System/Default.ini");
	const bool Valid = IsReadableFile(DefaultIni);
	std::free(DefaultIni);
	if (!Valid)
	{
		if (Status)
			Status->Detail = "expected a readable System/Default.ini";
		std::free(CanonicalRoot);
		return nullptr;
	}

	char* ManifestPath = JoinPath(CanonicalRoot, OverlayManifestName);
	char* ChecksumsPath = JoinPath(CanonicalRoot, OverlayChecksumsName);
	const bool OverlayMarked = IsReadableFile(ManifestPath) || IsReadableFile(ChecksumsPath);
	std::free(ManifestPath);
	std::free(ChecksumsPath);

	if (!OverlayMarked)
	{
		// Legacy mode: a readable System/Default.ini implies the development
		// profile; keep accepting so existing prototype checkouts keep working.
		if (Status)
		{
			Status->Accepted = true;
			Status->Profile = HP2DataProfile_Development;
			Status->Detail = "legacy development root accepted by System/Default.ini";
		}
		return CanonicalRoot;
	}

	const char* ReasonCode = nullptr;
	std::string Detail;
	HP2DataProfileKind Profile = HP2DataProfile_Unknown;
	if (!VerifyOverlayIdentity(CanonicalRoot, ReasonCode, Detail, Profile))
	{
		ReportIdentityReject(ReasonCode, Detail);
		if (Status)
		{
			Status->ReasonCode = ReasonCode;
			Status->Detail = Detail;
		}
		std::free(CanonicalRoot);
		return nullptr;
	}
	if (Status)
	{
		Status->Accepted = true;
		Status->Profile = Profile;
		Status->Detail = Detail;
	}
	return CanonicalRoot;
}


char* DiscoverDevelopmentDataRoot()
{
	char* Current = getcwd(nullptr, 0);
	if (!Current)
		return nullptr;

	for (;;)
	{
		static const char* CandidateSuffixes[] =
		{
			"out/retail-data",
			"HarryPotter2/Unreal"
		};
		for( size_t CandidateIndex=0; CandidateIndex<sizeof(CandidateSuffixes)/sizeof(CandidateSuffixes[0]); ++CandidateIndex )
		{
			char* Candidate = JoinPath(Current, CandidateSuffixes[CandidateIndex]);
			char* CanonicalRoot = ValidateDataRoot(Candidate);
			std::free(Candidate);
			if( CanonicalRoot )
			{
				std::free(Current);
				return CanonicalRoot;
			}
		}
		if (std::strcmp(Current, "/") == 0)
			break;

		char* Slash = std::strrchr(Current, '/');
		if (!Slash || Slash == Current)
			Current[1] = '\0';
		else
			*Slash = '\0';
	}
	std::free(Current);
	return nullptr;
}

char* DiscoverPrototypeDataRoot()
{
	char* Current = getcwd(nullptr, 0);
	if (!Current)
		return nullptr;

	for (;;)
	{
		char* Candidate = JoinPath(Current, "HarryPotter2/Unreal");
		char* CanonicalRoot = ValidateDataRoot(Candidate);
		std::free(Candidate);
		if (CanonicalRoot)
		{
			std::free(Current);
			return CanonicalRoot;
		}
		if (std::strcmp(Current, "/") == 0)
			break;

		char* Slash = std::strrchr(Current, '/');
		if (!Slash || Slash == Current)
			Current[1] = '\0';
		else
			*Slash = '\0';
	}
	std::free(Current);
	return nullptr;
}

bool EnsureDirectory(const char* Directory, mode_t Mode)
{
	if (Directory && mkdir(Directory, Mode) == 0)
		return true;
	return Directory && errno == EEXIST && IsDirectory(Directory);
}

bool EnsureDirectoryTree(const char* Directory, mode_t Mode)
{
	if (!Directory || Directory[0] != '/')
		return false;

	const size_t Length = std::strlen(Directory);
	char* Partial = static_cast<char*>(std::malloc(Length + 1));
	if (!Partial)
		return false;
	std::memcpy(Partial, Directory, Length + 1);

	bool Success = true;
	for (size_t Index = 1; Index <= Length; ++Index)
	{
		if (Partial[Index] != '/' && Partial[Index] != '\0')
			continue;
		const char Saved = Partial[Index];
		Partial[Index] = '\0';
		if (Index > 1 && !EnsureDirectory(Partial, Mode))
			Success = false;
		Partial[Index] = Saved;
		if (!Success)
			break;
	}
	std::free(Partial);
	return Success;
}

bool EnsureWritableChildDirectory(const char* Parent, const char* Child, mode_t Mode)
{
	char* Directory = JoinPath(Parent, Child);
	const bool Success = EnsureDirectory(Directory, Mode)
		&& IsWritableDirectory(Directory);
	std::free(Directory);
	return Success;
}

char* PrepareLauncherRoot(const char* Home)
{
	char* ApplicationSupport = JoinPath(
		Home, "Library/Application Support/Harry Potter 2");
	char* UserRoot = JoinPath(Home, UserSuffix);
	if (!ApplicationSupport || !UserRoot
		|| !EnsureDirectoryTree(ApplicationSupport, 0755)
		|| !EnsureDirectory(UserRoot, 0700))
	{
		std::free(ApplicationSupport);
		std::free(UserRoot);
		return nullptr;
	}
	std::free(ApplicationSupport);

	char* CanonicalUserRoot = CanonicalizeExistingDirectory(UserRoot);
	std::free(UserRoot);
	if (!CanonicalUserRoot || !IsWritableDirectory(CanonicalUserRoot))
	{
		std::free(CanonicalUserRoot);
		return nullptr;
	}
	return CanonicalUserRoot;
}

char* PrepareActiveUserRoot(const char* UserRoot)
{
	if (!UserRoot || UserRoot[0] != '/'
		|| !EnsureDirectoryTree(UserRoot, 0700))
		return nullptr;

	char* CanonicalUserRoot = CanonicalizeExistingDirectory(UserRoot);
	if (!CanonicalUserRoot || !IsWritableDirectory(CanonicalUserRoot))
	{
		std::free(CanonicalUserRoot);
		return nullptr;
	}

	const bool Success = EnsureWritableChildDirectory(CanonicalUserRoot, "Save", 0700)
		&& EnsureWritableChildDirectory(CanonicalUserRoot, "Save/cache", 0700)
		&& EnsureWritableChildDirectory(CanonicalUserRoot, "Cache", 0700);
	if (!Success)
	{
		std::free(CanonicalUserRoot);
		return nullptr;
	}
	return CanonicalUserRoot;
}

char* PrepareUserRoot(const char* Home)
{
	char* LauncherRoot = PrepareLauncherRoot(Home);
	if (!LauncherRoot)
		return nullptr;
	char* UserRoot = PrepareActiveUserRoot(LauncherRoot);
	std::free(LauncherRoot);
	return UserRoot;
}

bool Utf8ToTchar(const char* Input, TCHAR*& Output)
{
	static_assert(sizeof(TCHAR) == 4, "Native macOS paths require 32-bit host TCHAR");
	Output = nullptr;
	if (!Input)
		return false;

	const size_t InputLength = std::strlen(Input);
	if (InputLength >= static_cast<size_t>(MAXINT)
		|| InputLength > static_cast<size_t>(-1) / sizeof(TCHAR) - 1)
		return false;
	const INT Capacity = static_cast<INT>(InputLength + 1);
	TCHAR* Buffer = static_cast<TCHAR*>(
		std::malloc(static_cast<size_t>(Capacity) * sizeof(TCHAR)));
	if (!Buffer)
		return false;
	if (!appFromUtf8InPlace(Buffer, Input, Capacity))
	{
		std::free(Buffer);
		return false;
	}
	Output = Buffer;
	return true;
}
}
const char* HP2DataDirectoryArgumentValue(const char* Argument)
{
	return DataDirArgument(Argument);
}

bool IsHP2DataDirectoryArgument(const char* Argument)
{
	return HP2DataDirectoryArgumentValue(Argument) != nullptr;
}

bool GetHP2ConventionalRetailDataRoot(std::string& AbsoluteRoot)
{
	return GetConventionalDataRoot(ConventionalRetailDataSuffix, AbsoluteRoot);
}

bool GetHP2ConventionalPrototypeDataRoot(std::string& AbsoluteRoot)
{
	return GetConventionalDataRoot(ConventionalPrototypeDataSuffix, AbsoluteRoot);
}

bool DiscoverHP2RetailDataRoot(std::string& CanonicalRoot)
{
	CanonicalRoot.clear();
	char* Home = CanonicalizeExistingDirectory(std::getenv("HOME"));
	if (!Home)
		return false;
	char* Candidate = JoinPath(Home, ExternalDataSuffix);
	char* Root = ValidateDataRoot(Candidate);
	std::free(Candidate);
	std::free(Home);
	if (!Root)
		return false;
	CanonicalRoot = Root;
	std::free(Root);
	return true;
}

bool DiscoverHP2PrototypeDataRoot(std::string& CanonicalRoot)
{
	CanonicalRoot.clear();
	char* Root = DiscoverPrototypeDataRoot();
	if (!Root)
		return false;
	CanonicalRoot = Root;
	std::free(Root);
	return true;
}

bool PrepareHP2LauncherHome(std::string& LauncherRoot, std::string& Error)
{
	LauncherRoot.clear();
	Error.clear();
	char* Home = CanonicalizeExistingDirectory(std::getenv("HOME"));
	if (!Home)
	{
		Error = "HOME does not name an existing directory";
		return false;
	}

	char* Root = PrepareLauncherRoot(Home);
	std::free(Home);
	if (!Root)
	{
		Error = "could not create writable launcher directory";
		return false;
	}
	LauncherRoot = Root;
	std::free(Root);
	return true;
}

bool ValidateHP2DataRoot(
	const std::string& Candidate,
	std::string& CanonicalRoot,
	std::string& Error)
{
	CanonicalRoot.clear();
	Error.clear();
	HP2PathsBootstrapStatus Status;
	char* Root = ValidateDataRoot(Candidate.c_str(), &Status);
	if (!Root)
	{
		if (Status.ReasonCode)
			Error = std::string("data identity rejected: ") + Status.ReasonCode + ": " + Status.Detail;
		else if (!Status.Detail.empty())
			Error = Status.Detail;
		else
			Error = "expected a readable System/Default.ini";
		return false;
	}
	CanonicalRoot = Root;
	std::free(Root);
	return true;
}

bool ValidateHP2DataRootStatus(
	const std::string& Candidate,
	std::string& CanonicalRoot,
	HP2PathsBootstrapStatus& Status)
{
	CanonicalRoot.clear();
	Status = HP2PathsBootstrapStatus();
	char* Root = ValidateDataRoot(Candidate.c_str(), &Status);
	if (!Root)
	{
		if (Status.Detail.empty())
			Status.Detail = "expected a readable System/Default.ini";
		return false;
	}
	CanonicalRoot = Root;
	std::free(Root);
	return true;
}

bool InstallHP2Paths(
	const std::string& DataRoot,
	const std::string& UserRoot,
	std::string& Error)
{
	Error.clear();
	std::string CanonicalDataRoot;
	if (!ValidateHP2DataRoot(DataRoot, CanonicalDataRoot, Error))
		return false;
	if (UserRoot.empty() || UserRoot[0] != '/')
	{
		Error = "user directory must be an absolute path";
		return false;
	}

	char* CanonicalUserRoot = PrepareActiveUserRoot(UserRoot.c_str());
	if (!CanonicalUserRoot)
	{
		Error = "could not create writable user directory";
		return false;
	}
	if (IsSameOrDescendant(CanonicalDataRoot.c_str(), CanonicalUserRoot)
		|| IsSameOrDescendant(CanonicalUserRoot, CanonicalDataRoot.c_str()))
	{
		Error = "data and user directories must be separate";
		std::free(CanonicalUserRoot);
		return false;
	}

	char* SystemDirectory = JoinPath(CanonicalDataRoot.c_str(), "System");
	char* CanonicalSystem = CanonicalizeExistingDirectory(SystemDirectory);
	std::free(SystemDirectory);
	if (!CanonicalSystem)
	{
		Error = "could not canonicalize the validated System directory";
		std::free(CanonicalUserRoot);
		return false;
	}

	char* BaseDirectory = JoinPath(CanonicalSystem, "");
	char* UserDirectory = JoinPath(CanonicalUserRoot, "");
	std::free(CanonicalSystem);
	std::free(CanonicalUserRoot);
	TCHAR* BaseTchar = nullptr;
	TCHAR* UserTchar = nullptr;
	if (!BaseDirectory || !UserDirectory
		|| !Utf8ToTchar(BaseDirectory, BaseTchar)
		|| !Utf8ToTchar(UserDirectory, UserTchar))
	{
		Error = "filesystem paths are not valid UTF-8";
		std::free(BaseTchar);
		std::free(UserTchar);
		std::free(BaseDirectory);
		std::free(UserDirectory);
		return false;
	}
	std::free(BaseDirectory);
	std::free(UserDirectory);
	appSetBaseDir(BaseTchar);
	appSetUserDir(UserTchar);
	std::free(BaseTchar);
	std::free(UserTchar);
	return true;
}

bool PrepareHP2Paths(int ArgC, char* const ArgV[])
{
	const char* ExplicitDataDir = nullptr;
	for (int Index = 1; Index < ArgC; ++Index)
	{
		const char* Value = DataDirArgument(ArgV[Index]);
		if (Value)
		{
			ExplicitDataDir = Value;
			break;
		}
	}

	char* DataRoot = nullptr;
	if (ExplicitDataDir)
	{
		HP2PathsBootstrapStatus ExplicitStatus;
		DataRoot = ValidateDataRoot(ExplicitDataDir, &ExplicitStatus);
		if (!DataRoot)
		{
			if (ExplicitStatus.ReasonCode)
			{
				std::fprintf(stderr,
					"hp2: invalid -datadir '%s': refused by data identity verification\n",
					ExplicitDataDir);
			}
			else
			{
				std::fprintf(stderr,
					"hp2: invalid -datadir '%s': expected a readable System/Default.ini\n",
					ExplicitDataDir);
			}
			return false;
		}
	}

	const char* HomeEnvironment = std::getenv("HOME");
	char* Home = CanonicalizeExistingDirectory(HomeEnvironment);

	if (!ExplicitDataDir)
	{
		DataRoot = DiscoverDevelopmentDataRoot();
		if (!DataRoot && Home)
		{
			char* ExternalDataRoot = JoinPath(Home, ExternalDataSuffix);
			DataRoot = ValidateDataRoot(ExternalDataRoot);
			std::free(ExternalDataRoot);
		}
		if (!DataRoot)
		{
			char* ExpectedRoot = Home ? JoinPath(Home, ExternalDataSuffix) : nullptr;
			std::fprintf(stderr,
				"hp2: unable to locate game data; expected HarryPotter2/Unreal/System/Default.ini from the working directory or %s/System/Default.ini\n",
				ExpectedRoot ? ExpectedRoot : "$HOME/Library/Application Support/Harry Potter 2/Data/Unreal");
			std::free(ExpectedRoot);
			std::free(Home);
			return false;
		}
	}

	if (!Home)
	{
		std::fprintf(stderr, "hp2: HOME does not name an existing directory\n");
		std::free(DataRoot);
		return false;
	}

	char* UserRoot = PrepareUserRoot(Home);
	if (!UserRoot)
	{
		char* ExpectedUserRoot = JoinPath(Home, UserSuffix);
		std::fprintf(stderr, "hp2: could not create writable user directory '%s'\n",
			ExpectedUserRoot ? ExpectedUserRoot : UserSuffix);
		std::free(ExpectedUserRoot);
		std::free(DataRoot);
		std::free(Home);
		return false;
	}
	std::free(Home);

	if (IsSameOrDescendant(DataRoot, UserRoot)
		|| IsSameOrDescendant(UserRoot, DataRoot))
	{
		std::fprintf(stderr, "hp2: data and user directories must be separate\n");
		std::free(UserRoot);
		std::free(DataRoot);
		return false;
	}

	char* SystemDirectory = JoinPath(DataRoot, "System");
	char* CanonicalSystem = CanonicalizeExistingDirectory(SystemDirectory);
	std::free(SystemDirectory);
	std::free(DataRoot);
	if (!CanonicalSystem)
	{
		std::fprintf(stderr, "hp2: could not canonicalize the validated System directory\n");
		std::free(UserRoot);
		return false;
	}

	char* BaseDirectory = JoinPath(CanonicalSystem, "");
	char* UserDirectory = JoinPath(UserRoot, "");
	std::free(CanonicalSystem);
	std::free(UserRoot);
	TCHAR* BaseTchar = nullptr;
	TCHAR* UserTchar = nullptr;
	if (!BaseDirectory || !UserDirectory
		|| !Utf8ToTchar(BaseDirectory, BaseTchar)
		|| !Utf8ToTchar(UserDirectory, UserTchar))
	{
		std::fprintf(stderr, "hp2: filesystem paths are not valid UTF-8\n");
		std::free(BaseTchar);
		std::free(UserTchar);
		std::free(BaseDirectory);
		std::free(UserDirectory);
		return false;
	}
	std::free(BaseDirectory);
	std::free(UserDirectory);

	appSetBaseDir(BaseTchar);
	appSetUserDir(UserTchar);
	std::free(BaseTchar);
	std::free(UserTchar);
	return true;
}

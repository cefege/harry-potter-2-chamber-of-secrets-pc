/*=============================================================================
	HP2LauncherStore.cpp: Portable save and user-configuration persistence.
=============================================================================*/
#include "HP2LauncherStore.h"

#include "HP2LaunchPolicy.h"

#include <algorithm>
#include <cerrno>
#include <cctype>
#include <climits>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <dirent.h>
#include <fcntl.h>
#include <iomanip>
#include <locale>
#include <random>
#include <sstream>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

namespace HP2Launcher
{
namespace
{
constexpr off_t MaxThumbnailSize = 16 * 1024 * 1024;
enum class Encoding { Utf8, Utf8Bom, Utf16LE, Utf16BE };
struct Line { std::string text, ending; };
struct Document
{
	Encoding encoding = Encoding::Utf8;
	std::string newline = "\n";
	std::vector<Line> lines;
};
struct ParsedKey
{
	std::string key, value;
	std::size_t valueBegin = 0, valueEnd = 0;
};
struct StagedFile
{
	std::string destination, temporary, directory;
	bool active = false;
};
struct StagedDirectory
{
	std::string destination, temporary, parent;
	bool active = false;
};

#ifdef HP2_LAUNCHER_TESTING
int GFailAfterPublication = 0;
int GPublicationCount = 0;
#endif

std::string Join(const std::string& a, const std::string& b)
{
	if (a.empty()) return b;
	if (b.empty()) return a;
	return a.back() == '/' ? a + b : a + "/" + b;
}
std::string Parent(const std::string& path)
{
	const std::size_t slash = path.find_last_of('/');
	if (slash == std::string::npos) return ".";
	return slash == 0 ? "/" : path.substr(0, slash);
}
std::string Base(const std::string& path)
{
	const std::size_t slash = path.find_last_of('/');
	return slash == std::string::npos ? path : path.substr(slash + 1);
}
std::string ErrorAt(const char* operation, const std::string& path)
{
	return std::string(operation) + " '" + path + "': " + std::strerror(errno);
}
char Lower(char value)
{
	return static_cast<char>(std::tolower(static_cast<unsigned char>(value)));
}
std::string Lowercase(std::string value)
{
	for (char& c : value) c = Lower(c);
	return value;
}
bool Same(const std::string& a, const std::string& b)
{
	if (a.size() != b.size()) return false;
	for (std::size_t i = 0; i < a.size(); ++i) if (Lower(a[i]) != Lower(b[i])) return false;
	return true;
}
bool Space(char c)
{
	return c == ' ' || c == '\t' || c == '\v' || c == '\f' || c == '\r' || c == '\n';
}
std::string Trim(const std::string& value)
{
	std::size_t begin = 0, end = value.size();
	while (begin < end && Space(value[begin])) ++begin;
	while (end > begin && Space(value[end - 1])) --end;
	return value.substr(begin, end - begin);
}

bool ReadFile(const std::string& path, std::string& bytes, mode_t* mode, std::string& error)
{
	struct stat inspected;
	if (::lstat(path.c_str(), &inspected) != 0)
	{
		error = ErrorAt("Unable to inspect", path);
		return false;
	}
	if (!S_ISREG(inspected.st_mode))
	{
		error = "Expected a regular non-symlink file at '" + path + "'.";
		return false;
	}
	const int fd = ::open(path.c_str(), O_RDONLY | O_NOFOLLOW);
	if (fd < 0)
	{
		error = ErrorAt("Unable to open", path);
		return false;
	}
	struct stat opened;
	if (::fstat(fd, &opened) != 0 || !S_ISREG(opened.st_mode) || opened.st_size < 0)
	{
		::close(fd);
		error = "Unable to verify regular file '" + path + "'.";
		return false;
	}
	bytes.assign(static_cast<std::size_t>(opened.st_size), '\0');
	std::size_t done = 0;
	while (done < bytes.size())
	{
		const ssize_t count = ::read(fd, &bytes[done], bytes.size() - done);
		if (count < 0 && errno == EINTR) continue;
		if (count <= 0)
		{
			const int saved = count == 0 ? EIO : errno;
			::close(fd);
			errno = saved;
			error = ErrorAt("Unable to read", path);
			return false;
		}
		done += static_cast<std::size_t>(count);
	}
	if (::close(fd) != 0)
	{
		error = ErrorAt("Unable to close", path);
		return false;
	}
	if (mode) *mode = opened.st_mode & 07777;
	return true;
}

void PutUtf8(std::string& text, std::uint32_t cp)
{
	if (cp <= 0x7f) text.push_back(static_cast<char>(cp));
	else if (cp <= 0x7ff)
	{
		text.push_back(static_cast<char>(0xc0 | (cp >> 6)));
		text.push_back(static_cast<char>(0x80 | (cp & 0x3f)));
	}
	else if (cp <= 0xffff)
	{
		text.push_back(static_cast<char>(0xe0 | (cp >> 12)));
		text.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3f)));
		text.push_back(static_cast<char>(0x80 | (cp & 0x3f)));
	}
	else
	{
		text.push_back(static_cast<char>(0xf0 | (cp >> 18)));
		text.push_back(static_cast<char>(0x80 | ((cp >> 12) & 0x3f)));
		text.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3f)));
		text.push_back(static_cast<char>(0x80 | (cp & 0x3f)));
	}
}
bool Decode16(const std::string& bytes, bool little, std::string& text, std::string& error)
{
	if (bytes.size() % 2 != 0)
	{
		error = "UTF-16 configuration has an incomplete code unit.";
		return false;
	}
	auto unit = [&](std::size_t i)
	{
		const unsigned a = static_cast<unsigned char>(bytes[i]);
		const unsigned b = static_cast<unsigned char>(bytes[i + 1]);
		return static_cast<std::uint16_t>(little ? a | b << 8 : a << 8 | b);
	};
	text.clear();
	for (std::size_t i = 2; i < bytes.size(); i += 2)
	{
		std::uint32_t cp = unit(i);
		if (cp >= 0xd800 && cp <= 0xdbff)
		{
			if (i + 3 >= bytes.size()) { error = "UTF-16 configuration has an incomplete surrogate."; return false; }
			const std::uint16_t low = unit(i + 2);
			if (low < 0xdc00 || low > 0xdfff) { error = "UTF-16 configuration has an invalid surrogate."; return false; }
			cp = 0x10000 + ((cp - 0xd800) << 10) + low - 0xdc00;
			i += 2;
		}
		else if (cp >= 0xdc00 && cp <= 0xdfff)
		{
			error = "UTF-16 configuration has an unmatched surrogate.";
			return false;
		}
		PutUtf8(text, cp);
	}
	return true;
}
bool GetUtf8(const std::string& text, std::size_t& i, std::uint32_t& cp)
{
	const unsigned first = static_cast<unsigned char>(text[i++]);
	if (first < 0x80) { cp = first; return true; }
	int count;
	std::uint32_t minimum;
	if ((first & 0xe0) == 0xc0) { count = 1; cp = first & 0x1f; minimum = 0x80; }
	else if ((first & 0xf0) == 0xe0) { count = 2; cp = first & 0x0f; minimum = 0x800; }
	else if ((first & 0xf8) == 0xf0) { count = 3; cp = first & 7; minimum = 0x10000; }
	else return false;
	if (i + count > text.size()) return false;
	while (count--)
	{
		const unsigned next = static_cast<unsigned char>(text[i++]);
		if ((next & 0xc0) != 0x80) return false;
		cp = cp << 6 | (next & 0x3f);
	}
	return cp >= minimum && cp <= 0x10ffff && !(cp >= 0xd800 && cp <= 0xdfff);
}
bool Encode16(const std::string& text, bool little, std::string& bytes, std::string& error)
{
	bytes.clear();
	bytes.push_back(static_cast<char>(little ? 0xff : 0xfe));
	bytes.push_back(static_cast<char>(little ? 0xfe : 0xff));
	auto put = [&](std::uint16_t unit)
	{
		bytes.push_back(static_cast<char>(little ? unit & 255 : unit >> 8));
		bytes.push_back(static_cast<char>(little ? unit >> 8 : unit & 255));
	};
	for (std::size_t i = 0; i < text.size();)
	{
		std::uint32_t cp;
		if (!GetUtf8(text, i, cp)) { error = "Configuration contains invalid UTF-8 text."; return false; }
		if (cp <= 0xffff) put(static_cast<std::uint16_t>(cp));
		else
		{
			cp -= 0x10000;
			put(static_cast<std::uint16_t>(0xd800 | cp >> 10));
			put(static_cast<std::uint16_t>(0xdc00 | (cp & 0x3ff)));
		}
	}
	return true;
}
bool Decode(const std::string& bytes, Document& document, std::string& error)
{
	std::string text;
	if (bytes.size() >= 2 && static_cast<unsigned char>(bytes[0]) == 0xff && static_cast<unsigned char>(bytes[1]) == 0xfe)
	{
		document.encoding = Encoding::Utf16LE;
		if (!Decode16(bytes, true, text, error)) return false;
	}
	else if (bytes.size() >= 2 && static_cast<unsigned char>(bytes[0]) == 0xfe && static_cast<unsigned char>(bytes[1]) == 0xff)
	{
		document.encoding = Encoding::Utf16BE;
		if (!Decode16(bytes, false, text, error)) return false;
	}
	else if (bytes.size() >= 3 && bytes.compare(0, 3, "\xef\xbb\xbf") == 0)
	{
		document.encoding = Encoding::Utf8Bom;
		text = bytes.substr(3);
	}
	else { document.encoding = Encoding::Utf8; text = bytes; }
	for (std::size_t offset = 0; offset < text.size();)
	{
		std::uint32_t codePoint;
		if (!GetUtf8(text, offset, codePoint))
		{
			error = "Configuration contains invalid UTF-8 text.";
			return false;
		}
	}
	document.lines.clear();
	std::size_t begin = 0, crlf = 0, lf = 0;
	while (begin < text.size())
	{
		const std::size_t end = text.find('\n', begin);
		if (end == std::string::npos) { document.lines.push_back({text.substr(begin), ""}); break; }
		if (end > begin && text[end - 1] == '\r')
		{
			document.lines.push_back({text.substr(begin, end - begin - 1), "\r\n"});
			++crlf;
		}
		else { document.lines.push_back({text.substr(begin, end - begin), "\n"}); ++lf; }
		begin = end + 1;
	}
	document.newline = crlf > lf ? "\r\n" : "\n";
	return true;
}
bool Encode(const Document& document, std::string& bytes, std::string& error)
{
	std::string text;
	for (const Line& line : document.lines) text += line.text + line.ending;
	if (document.encoding == Encoding::Utf8) { bytes = text; return true; }
	if (document.encoding == Encoding::Utf8Bom) { bytes = "\xef\xbb\xbf" + text; return true; }
	return Encode16(text, document.encoding == Encoding::Utf16LE, bytes, error);
}

bool Section(const std::string& line, std::string& section)
{
	const std::string text = Trim(line);
	if (text.size() < 3 || text.front() != '[' || text.back() != ']') return false;
	section = Trim(text.substr(1, text.size() - 2));
	return !section.empty() && section.find('[') == std::string::npos && section.find(']') == std::string::npos;
}
bool Key(const std::string& line, ParsedKey& key)
{
	std::size_t begin = 0;
	while (begin < line.size() && Space(line[begin])) ++begin;
	if (begin == line.size() || line[begin] == ';' || line[begin] == '#') return false;
	const std::size_t equals = line.find('=', begin);
	if (equals == std::string::npos || (key.key = Trim(line.substr(begin, equals - begin))).empty()) return false;
	begin = equals + 1;
	while (begin < line.size() && Space(line[begin])) ++begin;
	std::size_t comment = line.size();
	for (std::size_t i = begin; i < line.size(); ++i)
	{
		if ((line[i] == ';' || line[i] == '#') && (i == begin || Space(line[i - 1]))) { comment = i; break; }
	}
	std::size_t end = comment;
	while (end > begin && Space(line[end - 1])) --end;
	key.valueBegin = begin;
	key.valueEnd = end;
	key.value = line.substr(begin, end - begin);
	return true;
}
bool Get(const Document& document, const std::string& wantedSection, const std::string& wantedKey, std::string& value)
{
	std::string section;
	bool found = false;
	for (const Line& line : document.lines)
	{
		std::string next;
		if (Section(line.text, next)) { section = next; continue; }
		ParsedKey key;
		if (Same(section, wantedSection) && Key(line.text, key) && Same(key.key, wantedKey))
		{
			value = Trim(key.value);
			found = true;
		}
	}
	return found;
}
void Set(Document& document, const std::string& wantedSection, const std::string& wantedKey, const std::string& value)
{
	std::string section;
	std::size_t final = document.lines.size();
	for (std::size_t i = 0; i < document.lines.size(); ++i)
	{
		std::string next;
		if (Section(document.lines[i].text, next)) { section = next; continue; }
		ParsedKey key;
		if (Same(section, wantedSection) && Key(document.lines[i].text, key) && Same(key.key, wantedKey)) final = i;
	}
	if (final != document.lines.size())
	{
		ParsedKey key;
		Key(document.lines[final].text, key);
		document.lines[final].text.replace(key.valueBegin, key.valueEnd - key.valueBegin, value);
		return;
	}
	std::size_t header = document.lines.size();
	for (std::size_t i = 0; i < document.lines.size(); ++i)
	{
		std::string next;
		if (Section(document.lines[i].text, next) && Same(next, wantedSection))
			header = i;
	}
	std::size_t insertion = document.lines.size();
	if (header != document.lines.size())
	{
		for (std::size_t i = header + 1; i < document.lines.size(); ++i)
		{
			std::string next;
			if (Section(document.lines[i].text, next))
			{
				insertion = i;
				break;
			}
		}
	}
	if (header != document.lines.size())
	{
		if (insertion && document.lines[insertion - 1].ending.empty()) document.lines[insertion - 1].ending = document.newline;
		document.lines.insert(document.lines.begin() + static_cast<std::ptrdiff_t>(insertion), {wantedKey + "=" + value, document.newline});
		return;
	}
	if (!document.lines.empty())
	{
		if (document.lines.back().ending.empty()) document.lines.back().ending = document.newline;
		if (!document.lines.back().text.empty()) document.lines.push_back({"", document.newline});
	}
	document.lines.push_back({"[" + wantedSection + "]", document.newline});
	document.lines.push_back({wantedKey + "=" + value, document.newline});
}

// Removes every key row matching section+key (case-insensitive), leaving the
// section header and surrounding formatting untouched. Used by selection
// commits so replaced generations cannot leave stale coordinate rows behind.
void Erase(Document& document, const std::string& wantedSection, const std::string& wantedKey)
{
	std::string section;
	std::vector<Line> kept;
	kept.reserve(document.lines.size());
	for (Line& line : document.lines)
	{
		std::string next;
		if (Section(line.text, next)) { section = next; kept.push_back(std::move(line)); continue; }
		ParsedKey key;
		if (Same(section, wantedSection) && Key(line.text, key) && Same(key.key, wantedKey))
			continue;
		kept.push_back(std::move(line));
	}
	document.lines = std::move(kept);
}

bool Integer(const std::string& text, int& value)
{
	const std::string source = Trim(text);
	if (source.empty()) return false;
	std::size_t i = 0;
	bool negative = false;
	if (source[i] == '+' || source[i] == '-') { negative = source[i++] == '-'; }
	if (i == source.size()) return false;
	const std::int64_t limit = static_cast<std::int64_t>(INT_MAX) + (negative ? 1 : 0);
	std::int64_t result = 0;
	for (; i < source.size(); ++i)
	{
		if (source[i] < '0' || source[i] > '9') return false;
		const int digit = source[i] - '0';
		if (result > (limit - digit) / 10) return false;
		result = result * 10 + digit;
	}
	value = static_cast<int>(negative ? -result : result);
	return true;
}
bool Index(const std::string& text, int& value)
{
	if (text.empty()) return false;
	std::uint64_t result = 0;
	for (char c : text)
	{
		if (c < '0' || c > '9') return false;
		const unsigned digit = static_cast<unsigned>(c - '0');
		if (result > (static_cast<std::uint64_t>(INT_MAX) - digit) / 10) return false;
		result = result * 10 + digit;
	}
	value = static_cast<int>(result);
	return true;
}
bool Number(const std::string& text, double& value)
{
	std::istringstream stream(Trim(text));
	stream.imbue(std::locale::classic());
	stream >> value;
	if (!stream || !std::isfinite(value))
		return false;
	char trailing;
	return !(stream >> trailing);
}
bool Boolean(const std::string& text, bool& value)
{
	const std::string lower = Lowercase(Trim(text));
	if (lower == "true" || lower == "on" || lower == "yes" || lower == "1") { value = true; return true; }
	if (lower == "false" || lower == "off" || lower == "no" || lower == "0") { value = false; return true; }
	return false;
}
std::string DoubleText(double value)
{
	std::ostringstream stream;
	stream.imbue(std::locale::classic());
	stream << std::setprecision(15) << value;
	return stream.str();
}
const char* BoolText(bool value) { return value ? "True" : "False"; }
bool ValidCap(int value)
{
	return std::find(
		FrameRateLimitValues.begin(),
		FrameRateLimitValues.end(),
		value) != FrameRateLimitValues.end();
}

bool LoadDocument(const std::string& path, Document& document, mode_t* mode, std::string& error)
{
	std::string bytes;
	if (!ReadFile(path, bytes, mode, error)) return false;
	if (!Decode(bytes, document, error)) { error = "Unable to decode '" + path + "': " + error; return false; }
	return true;
}
bool SelectDocument(const LauncherPaths& paths, const char* userName, const char* defaultName,
	Document& document, bool& exists, mode_t& mode, std::string& error)
{
	const std::string userPath = Join(paths.userRoot, userName);
	struct stat info;
	if (::lstat(userPath.c_str(), &info) == 0)
	{
		exists = true;
		std::string bytes;
		if (!ReadFile(userPath, bytes, &mode, error))
			return false;
		std::string decodeError;
		if (Decode(bytes, document, decodeError))
			return true;
		error.clear();
		return LoadDocument(Join(paths.systemRoot, defaultName), document, nullptr, error);
	}
	if (errno != ENOENT) { error = ErrorAt("Unable to inspect", userPath); return false; }
	exists = false;
	mode = 0600;
	return LoadDocument(Join(paths.systemRoot, defaultName), document, nullptr, error);
}

bool WriteAll(int fd, const std::string& bytes)
{
	std::size_t done = 0;
	while (done < bytes.size())
	{
		const ssize_t count = ::write(fd, bytes.data() + done, bytes.size() - done);
		if (count < 0 && errno == EINTR) continue;
		if (count <= 0) { if (count == 0) errno = EIO; return false; }
		done += static_cast<std::size_t>(count);
	}
	return true;
}
bool Stage(const std::string& destination, const std::string& bytes, mode_t mode, StagedFile& staged, std::string& error)
{
	staged.destination = destination;
	staged.directory = Parent(destination);
	std::random_device random;
	int fd = -1;
	for (int attempt = 0; attempt != 128; ++attempt)
	{
		const std::uint64_t nonce = static_cast<std::uint64_t>(random()) << 32 ^ random();
		std::ostringstream path;
		path << staged.directory << "/." << Base(destination) << ".tmp." << std::hex << nonce;
		staged.temporary = path.str();
		fd = ::open(staged.temporary.c_str(), O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0600);
		if (fd >= 0) break;
		if (errno != EEXIST) { error = ErrorAt("Unable to create temporary file", staged.temporary); return false; }
	}
	if (fd < 0) { error = "Unable to allocate an unpredictable temporary filename."; return false; }
	staged.active = true;
	bool okay = WriteAll(fd, bytes);
	if (okay && ::fchmod(fd, mode & 07777) != 0) okay = false;
	if (okay && ::fsync(fd) != 0) okay = false;
	const int saved = errno;
	if (::close(fd) != 0 && okay) okay = false;
	else if (!okay) errno = saved;
	if (!okay)
	{
		error = ErrorAt("Unable to stage", destination);
		::unlink(staged.temporary.c_str());
		staged.active = false;
	}
	return okay;
}
void Discard(StagedFile& staged)
{
	if (staged.active) ::unlink(staged.temporary.c_str());
	staged.active = false;
}
bool SyncParent(const std::string& directory, std::string& error)
{
	const int fd = ::open(directory.c_str(), O_RDONLY | O_DIRECTORY);
	if (fd < 0) { error = ErrorAt("Unable to open directory", directory); return false; }
	bool okay = ::fsync(fd) == 0;
	const int saved = errno;
	if (::close(fd) != 0 && okay) okay = false;
	else if (!okay) errno = saved;
	if (!okay) error = ErrorAt("Unable to synchronize directory", directory);
	return okay;
}
bool Publish(StagedFile& staged, std::string& error)
{
	if (::rename(staged.temporary.c_str(), staged.destination.c_str()) != 0)
	{
		error = ErrorAt("Unable to replace", staged.destination);
		Discard(staged);
		return false;
	}
	staged.active = false;
#ifdef HP2_LAUNCHER_TESTING
	++GPublicationCount;
	if (GFailAfterPublication == GPublicationCount)
	{
		errno = EIO;
		error = ErrorAt("Injected failure after replacing", staged.destination);
		return false;
	}
#endif
	return SyncParent(staged.directory, error);
}
bool RestorePublishedFile(const std::string& destination, bool existed, StagedFile& original, std::string& error)
{
	if (existed)
	{
		if (!original.active || ::rename(original.temporary.c_str(), destination.c_str()) != 0)
		{
			error = ErrorAt("Unable to restore", destination);
			return false;
		}
		original.active = false;
	}
	else if (::unlink(destination.c_str()) != 0 && errno != ENOENT)
	{
		error = ErrorAt("Unable to remove partially published file", destination);
		return false;
	}
	return SyncParent(Parent(destination), error);
}
bool Backup(const std::string& original, const std::string& bytes, mode_t mode, std::string& error)
{
	const std::string backup = original + ".bak";
	struct stat info;
	if (::lstat(backup.c_str(), &info) == 0)
	{
		if (S_ISREG(info.st_mode))
			return true;
		error = "Backup path is not a regular non-symlink file: '" + backup + "'.";
		return false;
	}
	if (errno != ENOENT) { error = ErrorAt("Unable to inspect backup", backup); return false; }
	StagedFile staged;
	if (!Stage(backup, bytes, mode, staged, error)) return false;
	if (::link(staged.temporary.c_str(), backup.c_str()) != 0)
	{
		if (errno == EEXIST)
		{
			Discard(staged);
			if (::lstat(backup.c_str(), &info) == 0 && S_ISREG(info.st_mode))
				return true;
			error = "Backup path is not a regular non-symlink file: '" + backup + "'.";
			return false;
		}
		error = ErrorAt("Unable to publish backup", backup);
		Discard(staged);
		return false;
	}
	if (::unlink(staged.temporary.c_str()) != 0)
	{
		error = ErrorAt("Unable to remove backup temporary file", staged.temporary);
		return false;
	}
	staged.active = false;
	return SyncParent(staged.directory, error);
}

bool Directory(const std::string& path)
{
	struct stat info;
	return ::lstat(path.c_str(), &info) == 0 && S_ISDIR(info.st_mode);
}
bool Names(const std::string& path, std::vector<std::string>& names, bool missingOkay, std::string& error)
{
	DIR* directory = ::opendir(path.c_str());
	if (!directory)
	{
		if (missingOkay && errno == ENOENT) { names.clear(); return true; }
		error = ErrorAt("Unable to open directory", path);
		return false;
	}
	names.clear();
	errno = 0;
	while (dirent* entry = ::readdir(directory))
	{
		const std::string name = entry->d_name;
		if (name != "." && name != "..") names.push_back(name);
		errno = 0;
	}
	const int readError = errno;
	if (::closedir(directory) != 0 && readError == 0) { error = ErrorAt("Unable to close directory", path); return false; }
	if (readError) { errno = readError; error = ErrorAt("Unable to read directory", path); return false; }
	std::sort(names.begin(), names.end());
	return true;
}
bool SaveName(const std::string& name, int& index, std::string& stem)
{
	const std::string lower = Lowercase(name);
	if (lower.size() <= 8 || lower.compare(0, 4, "save") != 0 || lower.compare(lower.size() - 4, 4, ".usa") != 0) return false;
	const std::string digits = lower.substr(4, lower.size() - 8);
	if (!Index(digits, index) || digits != std::to_string(index)) return false;
	stem = name.substr(0, name.size() - 4);
	return true;
}
bool SlotName(const std::string& name, int& slot)
{
	const std::string lower = Lowercase(name);
	if (lower.size() <= 4 || lower.compare(0, 4, "slot") != 0) return false;
	const std::string digits = lower.substr(4);
	return Index(digits, slot) && digits == std::to_string(slot);
}
std::string Thumbnail(const std::string& directory, const std::vector<std::string>& names, const std::string& stem)
{
	const std::string wanted = Lowercase(stem + ".bmp");
	for (const std::string& name : names)
	{
		if (Lowercase(name) != wanted) continue;
		const std::string path = Join(directory, name);
		struct stat info;
		if (::lstat(path.c_str(), &info) == 0 && S_ISREG(info.st_mode) && info.st_size >= 0 && info.st_size <= MaxThumbnailSize) return path;
	}
	return "";
}
void SavesIn(const std::string& directory, const std::vector<std::string>& names, int slot, bool slotted,
	std::vector<SaveRecord>& saves)
{
	for (const std::string& name : names)
	{
		int index;
		std::string stem;
		if (!SaveName(name, index, stem)) continue;
		const std::string path = Join(directory, name);
		struct stat info;
		if (::lstat(path.c_str(), &info) != 0 || !S_ISREG(info.st_mode) || info.st_size <= 0) continue;
		SaveRecord save;
		save.slot = slot;
		save.saveIndex = index;
		save.usesSlotDirectory = slotted;
		save.savePath = path;
		save.thumbnailPath = Thumbnail(directory, names, stem);
		save.displayName = slotted ? "Slot " + std::to_string(slot) + " - Save " + std::to_string(index)
			: "Save " + std::to_string(index);
		save.size = info.st_size;
		save.modifiedSeconds = info.st_mtime;
		saves.push_back(save);
	}
}

void LoadBool(const Document& doc, const char* section, const char* key, bool& target)
{
	std::string text;
	bool parsed;
	if (Get(doc, section, key, text) && Boolean(text, parsed)) target = parsed;
}
void LoadNumber(const Document& doc, const char* section, const char* key, double minimum, double maximum, double& target)
{
	std::string text;
	double parsed;
	if (Get(doc, section, key, text) && Number(text, parsed) && parsed >= minimum && parsed <= maximum) target = parsed;
}
template <std::size_t Count>
void LoadDiscreteNumber(
	const Document& doc,
	const char* section,
	const char* key,
	const std::array<double, Count>& accepted,
	double& target)
{
	std::string text;
	double parsed;
	if (Get(doc, section, key, text) && Number(text, parsed) &&
		std::find(accepted.begin(), accepted.end(), parsed) != accepted.end())
	{
		target = parsed;
	}
}
template <std::size_t Count>
void LoadDiscreteInteger(
	const Document& doc,
	const char* section,
	const char* key,
	const std::array<int, Count>& accepted,
	int& target)
{
	std::string text;
	double parsed;
	if (Get(doc, section, key, text) && Number(text, parsed) &&
		parsed == std::floor(parsed) && parsed >= INT_MIN && parsed <= INT_MAX)
	{
		const int discrete = static_cast<int>(parsed);
		if (std::find(accepted.begin(), accepted.end(), discrete) != accepted.end())
			target = discrete;
	}
}
void LoadAntiAliasing(const Document& doc, int& target)
{
	std::string text;
	bool enabled;
	if (!Get(doc, "XOpenGLDrv.XOpenGLRenderDevice", "UseAA", text) ||
		!Boolean(text, enabled))
	{
		return;
	}
	if (!enabled)
	{
		target = 0;
		return;
	}
	int samples;
	if (Get(doc, "XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", text) &&
		Integer(text, samples) && (samples == 2 || samples == 4))
	{
		target = samples;
	}
}
void LoadDimension(const Document& doc, const char* key, int& target)
{
	std::string text;
	int parsed;
	if (Get(doc, "SDLDrv.SDLClient", key, text) && Integer(text, parsed) && parsed >= 320 && parsed <= 16384) target = parsed;
}
void LoadSettings(const Document& game, const Document& user, LauncherSettings& settings)
{
	settings = LauncherSettings();
	bool fullscreen = false, borderless = false, desktop = false;
	LoadBool(game, "SDLDrv.SDLClient", "StartupFullscreen", fullscreen);
	LoadBool(game, "SDLDrv.SDLClient", "BorderlessWindow", borderless);
	LoadBool(game, "SDLDrv.SDLClient", "UseDesktopResolution", desktop);
	if (borderless || desktop) settings.screenMode = ScreenMode::BorderlessDesktop;
	else if (fullscreen) settings.screenMode = ScreenMode::Fullscreen;
	const bool fullSize = settings.screenMode != ScreenMode::Windowed;
	LoadDimension(game, fullSize ? "FullscreenViewportX" : "WindowedViewportX", settings.resolution.width);
	LoadDimension(game, fullSize ? "FullscreenViewportY" : "WindowedViewportY", settings.resolution.height);
	settings.resolution.label = std::to_string(settings.resolution.width) + " x " + std::to_string(settings.resolution.height);
	LoadBool(game, "XOpenGLDrv.XOpenGLRenderDevice", "UseVSync", settings.verticalSync);
	LoadDiscreteNumber(
		game,
		"XOpenGLDrv.XOpenGLRenderDevice",
		"RenderScale",
		RenderScaleValues,
		settings.renderScale);
	LoadDiscreteNumber(game, "SDLDrv.SDLClient", "UIScale", UIScaleValues, settings.uiScale);
	LoadBool(game, "SDLDrv.SDLClient", "ShowFPS", settings.showFPS);
	LoadBool(game, "SDLDrv.SDLClient", "MaintainVerticalFOV", settings.maintainVerticalFOV);
	LoadBool(game, "SDLDrv.SDLClient", "NativeText", settings.nativeText);
	LoadBool(game, "SDLDrv.SDLClient", "ScreenFlashes", settings.screenFlashes);
	LoadNumber(game, "SDLDrv.SDLClient", "Brightness", 0.1, 1.0, settings.brightness);
	LoadAntiAliasing(game, settings.antiAliasingSamples);
	LoadDiscreteInteger(
		game,
		"XOpenGLDrv.XOpenGLRenderDevice",
		"MaxAnisotropy",
		AnisotropyValues,
		settings.anisotropy);
	LoadBool(game, "SDLDrv.SDLClient", "UseJoystick", settings.joystickEnabled);
	LoadBool(game, "Engine.GameEngine", "UseSound", settings.soundEnabled);
	LoadNumber(game, "ALAudio.ALAudioSubsystem", "SoundVolume", 0.0, 1.0, settings.soundVolume);
	LoadNumber(game, "ALAudio.ALAudioSubsystem", "MusicVolume", 0.0, 1.0, settings.musicVolume);
	std::string value;
	double cap;
	if (Get(game, "Engine.GameEngine", "FrameRateLimit", value) && Number(value, cap) && cap == std::floor(cap) &&
		cap >= 0 && cap <= INT_MAX && ValidCap(static_cast<int>(cap))) settings.frameRateLimit = static_cast<int>(cap);
	if (Get(game, "SDLDrv.SDLClient", "TextureDetail", value))
	{
		if (Same(Trim(value), "Low")) settings.textureDetail = TextureDetail::Low;
		else if (Same(Trim(value), "Medium")) settings.textureDetail = TextureDetail::Medium;
		else if (Same(Trim(value), "High")) settings.textureDetail = TextureDetail::High;
	}
	LoadNumber(user, "Engine.PlayerPawn", "MouseSensitivity", 0.2, 10.0, settings.mouseSensitivity);
	LoadBool(user, "Engine.PlayerPawn", "bInvertMouse", settings.invertMouse);
	bool modernThirdPersonControls = false;
	LoadBool(user, "Engine.PlayerPawn", "bModernThirdPersonControls", modernThirdPersonControls);
	settings.controlMode = modernThirdPersonControls ? ControlMode::Modern : ControlMode::Classic;
	LoadBool(user, "HGame.Harry", "bAutoCenterCamera", settings.autoCenterCamera);
	LoadBool(user, "HGame.Harry", "bMoveWhileCasting", settings.moveWhileCasting);
	LoadBool(user, "HGame.Harry", "bAutoQuaff", settings.autoQuaff);
	if (Get(user, "Engine.PlayerPawn", "Difficulty", value))
	{
		if (Same(Trim(value), "DifficultyEasy")) settings.difficulty = Difficulty::Easy;
		else if (Same(Trim(value), "DifficultyMedium")) settings.difficulty = Difficulty::Medium;
		else if (Same(Trim(value), "DifficultyHard")) settings.difficulty = Difficulty::Hard;
	}
	if (Get(user, "Engine.PlayerPawn", "ObjectDetail", value))
	{
		value = Trim(value);
		if (Same(value, "ObjectDetailVeryLow")) settings.objectDetail = ObjectDetail::VeryLow;
		else if (Same(value, "ObjectDetailLow")) settings.objectDetail = ObjectDetail::Low;
		else if (Same(value, "ObjectDetailMedium")) settings.objectDetail = ObjectDetail::Medium;
		else if (Same(value, "ObjectDetailHigh")) settings.objectDetail = ObjectDetail::High;
		else if (Same(value, "ObjectDetailVeryHigh")) settings.objectDetail = ObjectDetail::VeryHigh;
	}
}
const char* Texture(TextureDetail value)
{
	switch (value) { case TextureDetail::Low: return "Low"; case TextureDetail::Medium: return "Medium"; case TextureDetail::High: return "High"; }
	return "High";
}
const char* Object(ObjectDetail value)
{
	switch (value)
	{
	case ObjectDetail::VeryLow: return "ObjectDetailVeryLow"; case ObjectDetail::Low: return "ObjectDetailLow";
	case ObjectDetail::Medium: return "ObjectDetailMedium"; case ObjectDetail::High: return "ObjectDetailHigh";
	case ObjectDetail::VeryHigh: return "ObjectDetailVeryHigh";
	}
	return "ObjectDetailMedium";
}
const char* DifficultyValue(Difficulty value)
{
	switch (value) { case Difficulty::Easy: return "DifficultyEasy"; case Difficulty::Medium: return "DifficultyMedium"; case Difficulty::Hard: return "DifficultyHard"; }
	return "DifficultyEasy";
}
void ApplyGame(Document& game, const LauncherSettings& settings)
{
	Set(game, "Engine.Engine", "ViewportManager", "SDLDrv.SDLClient");
	Set(game, "Engine.Engine", "GameRenderDevice", "XOpenGLDrv.XOpenGLRenderDevice");
	Set(game, "Engine.Engine", "WindowedRenderDevice", "XOpenGLDrv.XOpenGLRenderDevice");
	Set(game, "Engine.Engine", "RenderDevice", "XOpenGLDrv.XOpenGLRenderDevice");
	Set(game, "Engine.Engine", "AudioDevice", "ALAudio.ALAudioSubsystem");
	Set(game, "Engine.GameEngine", "UseSound", BoolText(settings.soundEnabled));
	Set(game, "Engine.GameEngine", "FrameRateLimit", std::to_string(settings.frameRateLimit));
	const std::string width = std::to_string(settings.resolution.width), height = std::to_string(settings.resolution.height);
	Set(game, "SDLDrv.SDLClient", "WindowedViewportX", width);
	Set(game, "SDLDrv.SDLClient", "WindowedViewportY", height);
	Set(game, "SDLDrv.SDLClient", "FullscreenViewportX", width);
	Set(game, "SDLDrv.SDLClient", "FullscreenViewportY", height);
	Set(game, "SDLDrv.SDLClient", "WindowedColorBits", "32");
	Set(game, "SDLDrv.SDLClient", "FullscreenColorBits", "32");
	Set(game, "SDLDrv.SDLClient", "StartupFullscreen", BoolText(settings.screenMode != ScreenMode::Windowed));
	Set(game, "SDLDrv.SDLClient", "BorderlessWindow", BoolText(settings.screenMode == ScreenMode::BorderlessDesktop));
	Set(game, "SDLDrv.SDLClient", "UseDesktopResolution", BoolText(settings.screenMode == ScreenMode::BorderlessDesktop));
	Set(game, "SDLDrv.SDLClient", "Brightness", DoubleText(settings.brightness));
	Set(game, "SDLDrv.SDLClient", "TextureDetail", Texture(settings.textureDetail));
	Set(game, "SDLDrv.SDLClient", "UseJoystick", BoolText(settings.joystickEnabled));
	Set(game, "XOpenGLDrv.XOpenGLRenderDevice", "UseVSync", settings.verticalSync ? "On" : "Off");
	Set(game, "XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", DoubleText(settings.renderScale));
	Set(game, "SDLDrv.SDLClient", "UIScale", DoubleText(settings.uiScale));
	Set(game, "SDLDrv.SDLClient", "ShowFPS", BoolText(settings.showFPS));
	Set(game, "SDLDrv.SDLClient", "MaintainVerticalFOV", BoolText(settings.maintainVerticalFOV));
	Set(game, "SDLDrv.SDLClient", "NativeText", BoolText(settings.nativeText));
	Set(game, "SDLDrv.SDLClient", "ScreenFlashes", BoolText(settings.screenFlashes));
	Set(game, "XOpenGLDrv.XOpenGLRenderDevice", "UseAA", BoolText(settings.antiAliasingSamples != 0));
	Set(game, "XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", std::to_string(settings.antiAliasingSamples));
	Set(game, "XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", std::to_string(settings.anisotropy));
	Set(game, "ALAudio.ALAudioSubsystem", "MusicVolume", DoubleText(settings.musicVolume));
	Set(game, "ALAudio.ALAudioSubsystem", "SoundVolume", DoubleText(settings.soundVolume));
}
void ApplyUser(Document& user, const LauncherSettings& settings)
{
	Set(user, "Engine.PlayerPawn", "MouseSensitivity", DoubleText(settings.mouseSensitivity));
	Set(user, "Engine.PlayerPawn", "bInvertMouse", BoolText(settings.invertMouse));
	Set(user, "Engine.PlayerPawn", "bModernThirdPersonControls", BoolText(settings.controlMode == ControlMode::Modern));
	Set(user, "Engine.PlayerPawn", "Difficulty", DifficultyValue(settings.difficulty));
	Set(user, "Engine.PlayerPawn", "ObjectDetail", Object(settings.objectDetail));
	Set(user, "HGame.Harry", "bAutoCenterCamera", BoolText(settings.autoCenterCamera));
	Set(user, "HGame.Harry", "bMoveWhileCasting", BoolText(settings.moveWhileCasting));
	Set(user, "HGame.Harry", "bAutoQuaff", BoolText(settings.autoQuaff));
}
bool IsValidUtf8(const std::string& text)
{
	for (std::size_t offset = 0; offset < text.size();)
	{
		std::uint32_t codePoint;
		if (!GetUtf8(text, offset, codePoint)) return false;
	}
	return true;
}
bool IsAbsolutePath(const std::string& path)
{
	return !path.empty() && path.front() == '/' && path.find('\0') == std::string::npos;
}
bool SourceText(DataSource source, const char*& text)
{
	switch (source)
	{
	case DataSource::Retail: text = "Retail"; return true;
	case DataSource::Prototype: text = "Prototype"; return true;
	}
	return false;
}
bool ParseSource(const std::string& text, DataSource& source)
{
	if (Same(text, "Retail")) { source = DataSource::Retail; return true; }
	if (Same(text, "Prototype")) { source = DataSource::Prototype; return true; }
	return false;
}
bool LoadLauncherDocument(
	const std::string& launcherRoot,
	Document& document,
	bool& exists,
	mode_t& mode,
	std::string& original,
	std::string& error)
{
	if (!Directory(launcherRoot))
	{
		error = "Launcher root is not a directory: '" + launcherRoot + "'.";
		return false;
	}
	const std::string path = Join(launcherRoot, "Launcher.ini");
	struct stat info;
	if (::lstat(path.c_str(), &info) != 0)
	{
		if (errno != ENOENT)
		{
			error = ErrorAt("Unable to inspect", path);
			return false;
		}
		document = Document();
		exists = false;
		mode = 0600;
		original.clear();
		return true;
	}
	exists = true;
	if (!ReadFile(path, original, &mode, error)) return false;
	if (!Decode(original, document, error))
	{
		error = "Unable to decode '" + path + "': " + error;
		return false;
	}
	return true;
}
bool PublishLauncherDocument(
	const std::string& launcherRoot,
	Document& document,
	bool existed,
	mode_t mode,
	const std::string& original,
	std::string& error)
{
	const std::string path = Join(launcherRoot, "Launcher.ini");
	std::string bytes;
	if (!Encode(document, bytes, error)) return false;
	if (existed && !Backup(path, original, mode, error)) return false;

	StagedFile staged, rollback;
	if (!Stage(path, bytes, existed ? mode : 0600, staged, error)) return false;
	if (existed && !Stage(path, original, mode, rollback, error))
	{
		Discard(staged);
		return false;
	}
	if (!Publish(staged, error))
	{
		const std::string publicationError = error;
		std::string restorationError;
		const bool restored = RestorePublishedFile(path, existed, rollback, restorationError);
		error = restored ? publicationError : publicationError + " Rollback failed: " + restorationError;
		return false;
	}
	Discard(rollback);
	return true;
}
bool OpenDirectoryNoFollow(const std::string& path, int& fd, mode_t& mode, std::string& error)
{
	fd = ::open(path.c_str(), O_RDONLY | O_DIRECTORY | O_NOFOLLOW);
	if (fd < 0)
	{
		error = ErrorAt("Unable to open directory", path);
		return false;
	}
	struct stat info;
	if (::fstat(fd, &info) != 0 || !S_ISDIR(info.st_mode))
	{
		::close(fd);
		error = "Unable to verify directory '" + path + "'.";
		return false;
	}
	mode = info.st_mode & 07777;
	return true;
}
bool OpenDirectoryAtNoFollow(
	int parentFd,
	const std::string& name,
	const std::string& displayPath,
	int& fd,
	mode_t& mode,
	std::string& error)
{
	fd = ::openat(parentFd, name.c_str(), O_RDONLY | O_DIRECTORY | O_NOFOLLOW);
	if (fd < 0)
	{
		error = ErrorAt("Unable to open directory", displayPath);
		return false;
	}
	struct stat info;
	if (::fstat(fd, &info) != 0 || !S_ISDIR(info.st_mode))
	{
		::close(fd);
		error = "Unable to verify directory '" + displayPath + "'.";
		return false;
	}
	mode = info.st_mode & 07777;
	return true;
}
bool DirectoryNamesAt(int fd, const std::string& displayPath, std::vector<std::string>& names, std::string& error)
{
	const int copy = ::dup(fd);
	if (copy < 0)
	{
		error = ErrorAt("Unable to duplicate directory descriptor", displayPath);
		return false;
	}
	DIR* directory = ::fdopendir(copy);
	if (!directory)
	{
		const int saved = errno;
		::close(copy);
		errno = saved;
		error = ErrorAt("Unable to open directory", displayPath);
		return false;
	}
	names.clear();
	errno = 0;
	while (dirent* entry = ::readdir(directory))
	{
		const std::string name = entry->d_name;
		if (name != "." && name != "..") names.push_back(name);
		errno = 0;
	}
	const int readError = errno;
	if (::closedir(directory) != 0 && readError == 0)
	{
		error = ErrorAt("Unable to close directory", displayPath);
		return false;
	}
	if (readError)
	{
		errno = readError;
		error = ErrorAt("Unable to read directory", displayPath);
		return false;
	}
	std::sort(names.begin(), names.end());
	return true;
}
bool SyncDirectoryFd(int fd, const std::string& path, std::string& error)
{
	if (::fsync(fd) == 0) return true;
	error = ErrorAt("Unable to synchronize directory", path);
	return false;
}
bool InspectLegacyTreeAt(
	int parentFd,
	const std::string& name,
	const std::string& path,
	bool missingOkay,
	std::string& error)
{
	struct stat info;
	if (::fstatat(parentFd, name.c_str(), &info, AT_SYMLINK_NOFOLLOW) != 0)
	{
		if (missingOkay && errno == ENOENT) return true;
		error = ErrorAt("Unable to inspect legacy profile item", path);
		return false;
	}
	if (S_ISREG(info.st_mode)) return true;
	if (!S_ISDIR(info.st_mode))
	{
		error = "Legacy profile item is not a regular non-symlink file or directory: '" + path + "'.";
		return false;
	}
	int directoryFd;
	mode_t mode;
	if (!OpenDirectoryAtNoFollow(parentFd, name, path, directoryFd, mode, error)) return false;
	std::vector<std::string> names;
	bool okay = DirectoryNamesAt(directoryFd, path, names, error);
	for (const std::string& child : names)
		if (okay && !InspectLegacyTreeAt(directoryFd, child, Join(path, child), false, error)) okay = false;
	if (::close(directoryFd) != 0 && okay)
	{
		error = ErrorAt("Unable to close directory", path);
		okay = false;
	}
	return okay;
}
bool CopyRegularFileAt(
	int sourceParentFd,
	const std::string& sourceName,
	int destinationParentFd,
	const std::string& destinationName,
	const std::string& sourcePath,
	const std::string& destinationPath,
	std::string& error)
{
	const int sourceFd = ::openat(sourceParentFd, sourceName.c_str(), O_RDONLY | O_NOFOLLOW);
	if (sourceFd < 0)
	{
		error = ErrorAt("Unable to open legacy profile file", sourcePath);
		return false;
	}
	struct stat sourceInfo;
	if (::fstat(sourceFd, &sourceInfo) != 0 || !S_ISREG(sourceInfo.st_mode))
	{
		::close(sourceFd);
		error = "Legacy profile item is not a regular non-symlink file: '" + sourcePath + "'.";
		return false;
	}
	const int destinationFd = ::openat(
		destinationParentFd,
		destinationName.c_str(),
		O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW,
		0600);
	if (destinationFd < 0)
	{
		const int saved = errno;
		::close(sourceFd);
		errno = saved;
		error = ErrorAt("Unable to create staged profile file", destinationPath);
		return false;
	}
	bool okay = true;
	char buffer[32768];
	while (okay)
	{
		const ssize_t count = ::read(sourceFd, buffer, sizeof(buffer));
		if (count < 0 && errno == EINTR) continue;
		if (count < 0) { okay = false; break; }
		if (count == 0) break;
		std::size_t written = 0;
		while (written < static_cast<std::size_t>(count))
		{
			const ssize_t result = ::write(
				destinationFd,
				buffer + written,
				static_cast<std::size_t>(count) - written);
			if (result < 0 && errno == EINTR) continue;
			if (result <= 0)
			{
				if (result == 0) errno = EIO;
				okay = false;
				break;
			}
			written += static_cast<std::size_t>(result);
		}
	}
	if (okay && ::fchmod(destinationFd, sourceInfo.st_mode & 07777) != 0) okay = false;
	if (okay && ::fsync(destinationFd) != 0) okay = false;
	int saved = errno;
	if (::close(destinationFd) != 0 && okay) { saved = errno; okay = false; }
	if (::close(sourceFd) != 0 && okay) { saved = errno; okay = false; }
	if (okay) return true;
	errno = saved;
	error = ErrorAt("Unable to copy legacy profile file", sourcePath);
	::unlinkat(destinationParentFd, destinationName.c_str(), 0);
	return false;
}
bool SynchronizeCopiedDirectory(int fd, mode_t mode, const std::string& path, std::string& error)
{
	if (::fchmod(fd, mode & 07777) != 0)
	{
		error = ErrorAt("Unable to set staged profile directory permissions", path);
		return false;
	}
	return SyncDirectoryFd(fd, path, error);
}
bool CopyLegacyTreeAt(
	int sourceParentFd,
	const std::string& sourceName,
	int destinationParentFd,
	const std::string& destinationName,
	const std::string& sourcePath,
	const std::string& destinationPath,
	std::string& error)
{
	struct stat info;
	if (::fstatat(sourceParentFd, sourceName.c_str(), &info, AT_SYMLINK_NOFOLLOW) != 0)
	{
		error = ErrorAt("Unable to inspect legacy profile item", sourcePath);
		return false;
	}
	if (S_ISREG(info.st_mode))
	{
		return CopyRegularFileAt(
			sourceParentFd,
			sourceName,
			destinationParentFd,
			destinationName,
			sourcePath,
			destinationPath,
			error);
	}
	if (!S_ISDIR(info.st_mode))
	{
		error = "Legacy profile item is not a regular non-symlink file or directory: '" + sourcePath + "'.";
		return false;
	}
	if (::mkdirat(destinationParentFd, destinationName.c_str(), 0700) != 0)
	{
		error = ErrorAt("Unable to create staged profile directory", destinationPath);
		return false;
	}
	int sourceFd = -1, destinationFd = -1;
	mode_t sourceMode, destinationMode;
	if (!OpenDirectoryAtNoFollow(sourceParentFd, sourceName, sourcePath, sourceFd, sourceMode, error) ||
		!OpenDirectoryAtNoFollow(destinationParentFd, destinationName, destinationPath, destinationFd, destinationMode, error))
	{
		if (sourceFd >= 0) ::close(sourceFd);
		return false;
	}
	std::vector<std::string> names;
	bool okay = DirectoryNamesAt(sourceFd, sourcePath, names, error);
	for (const std::string& child : names)
	{
		if (okay &&
			!CopyLegacyTreeAt(
				sourceFd,
				child,
				destinationFd,
				child,
				Join(sourcePath, child),
				Join(destinationPath, child),
				error))
		{
			okay = false;
		}
	}
	if (okay && !SynchronizeCopiedDirectory(destinationFd, sourceMode, destinationPath, error)) okay = false;
	int saved = errno;
	if (::close(destinationFd) != 0 && okay) { saved = errno; okay = false; }
	if (::close(sourceFd) != 0 && okay) { saved = errno; okay = false; }
	if (!okay) errno = saved;
	return okay;
}
bool RemoveTreeAt(int parentFd, const std::string& name, const std::string& path, std::string& error)
{
	struct stat info;
	if (::fstatat(parentFd, name.c_str(), &info, AT_SYMLINK_NOFOLLOW) != 0)
	{
		error = ErrorAt("Unable to inspect staged profile item", path);
		return false;
	}
	if (S_ISREG(info.st_mode))
	{
		if (::unlinkat(parentFd, name.c_str(), 0) == 0) return true;
		error = ErrorAt("Unable to remove staged profile file", path);
		return false;
	}
	if (!S_ISDIR(info.st_mode))
	{
		error = "Staged profile contains a symlink or irregular file: '" + path + "'.";
		return false;
	}
	int directoryFd;
	mode_t mode;
	if (!OpenDirectoryAtNoFollow(parentFd, name, path, directoryFd, mode, error)) return false;
	std::vector<std::string> names;
	bool okay = DirectoryNamesAt(directoryFd, path, names, error);
	for (const std::string& child : names)
		if (okay && !RemoveTreeAt(directoryFd, child, Join(path, child), error)) okay = false;
	int saved = errno;
	if (::close(directoryFd) != 0 && okay) { saved = errno; okay = false; }
	if (!okay)
	{
		errno = saved;
		return false;
	}
	if (::unlinkat(parentFd, name.c_str(), AT_REMOVEDIR) == 0) return true;
	error = ErrorAt("Unable to remove staged profile directory", path);
	return false;
}
bool RemoveTreeNoFollow(const std::string& path, std::string& error)
{
	int parentFd;
	mode_t mode;
	if (!OpenDirectoryNoFollow(Parent(path), parentFd, mode, error)) return false;
	const bool removed = RemoveTreeAt(parentFd, Base(path), path, error);
	const int saved = errno;
	if (::close(parentFd) != 0 && removed)
	{
		error = ErrorAt("Unable to close directory", Parent(path));
		return false;
	}
	errno = saved;
	return removed;
}
void DiscardStagedDirectory(StagedDirectory& staged)
{
	if (staged.active)
	{
		std::string ignored;
		RemoveTreeNoFollow(staged.temporary, ignored);
	}
	staged.active = false;
}
bool EnsureDirectoryNoFollow(const std::string& path, std::string& error)
{
	struct stat info;
	if (::lstat(path.c_str(), &info) == 0)
	{
		if (S_ISDIR(info.st_mode)) return true;
		error = "Expected a directory at '" + path + "'.";
		return false;
	}
	if (errno != ENOENT)
	{
		error = ErrorAt("Unable to inspect directory", path);
		return false;
	}
	if (::mkdir(path.c_str(), 0700) != 0)
	{
		error = ErrorAt("Unable to create directory", path);
		return false;
	}
	return SyncParent(Parent(path), error);
}
bool StageProfile(
	const std::string& launcherRoot,
	const std::string& profileRoot,
	bool copyLegacy,
	StagedDirectory& staged,
	std::string& error)
{
	staged.destination = profileRoot;
	staged.parent = Parent(profileRoot);
	int parentFd;
	mode_t parentMode;
	if (!OpenDirectoryNoFollow(staged.parent, parentFd, parentMode, error)) return false;
	const std::string temporaryNamePrefix = "." + Base(profileRoot) + ".tmp.";
	std::random_device random;
	bool created = false;
	for (int attempt = 0; attempt != 128; ++attempt)
	{
		const std::uint64_t nonce = static_cast<std::uint64_t>(random()) << 32 ^ random();
		std::ostringstream name;
		name << temporaryNamePrefix << std::hex << nonce;
		staged.temporary = Join(staged.parent, name.str());
		if (::mkdirat(parentFd, name.str().c_str(), 0700) == 0)
		{
			created = true;
			break;
		}
		if (errno != EEXIST)
		{
			const int saved = errno;
			::close(parentFd);
			errno = saved;
			error = ErrorAt("Unable to create staged profile", staged.temporary);
			return false;
		}
	}
	if (!created)
	{
		::close(parentFd);
		error = "Unable to allocate an unpredictable staged profile directory.";
		return false;
	}
	staged.active = true;
	int stagedFd;
	mode_t stagedMode;
	if (!OpenDirectoryAtNoFollow(parentFd, Base(staged.temporary), staged.temporary, stagedFd, stagedMode, error))
	{
		::close(parentFd);
		DiscardStagedDirectory(staged);
		return false;
	}
	bool okay = true;
	int launcherFd = -1;
	if (copyLegacy)
	{
		mode_t launcherMode;
		if (!OpenDirectoryNoFollow(launcherRoot, launcherFd, launcherMode, error)) okay = false;
		static const char* const LegacyItems[] = {"Game.ini", "User.ini", "Save", "Cache"};
		for (const char* name : LegacyItems)
		{
			struct stat info;
			if (okay && ::fstatat(launcherFd, name, &info, AT_SYMLINK_NOFOLLOW) != 0)
			{
				if (errno == ENOENT) continue;
				error = ErrorAt("Unable to inspect legacy profile item", Join(launcherRoot, name));
				okay = false;
			}
			if (okay &&
				!CopyLegacyTreeAt(
					launcherFd,
					name,
					stagedFd,
					name,
					Join(launcherRoot, name),
					Join(staged.temporary, name),
					error))
			{
				okay = false;
			}
		}
	}
	if (launcherFd >= 0 && ::close(launcherFd) != 0 && okay)
	{
		error = ErrorAt("Unable to close directory", launcherRoot);
		okay = false;
	}
	if (okay && !SyncDirectoryFd(stagedFd, staged.temporary, error)) okay = false;
	if (::close(stagedFd) != 0 && okay)
	{
		error = ErrorAt("Unable to close directory", staged.temporary);
		okay = false;
	}
	if (::close(parentFd) != 0 && okay)
	{
		error = ErrorAt("Unable to close directory", staged.parent);
		okay = false;
	}
	if (!okay)
	{
		DiscardStagedDirectory(staged);
		return false;
	}
	return true;
}
bool RemovePublishedProfile(const std::string& profileRoot, std::string& error);

bool PublishProfile(StagedDirectory& staged, std::string& error)
{
	int parentFd;
	mode_t parentMode;
	if (!OpenDirectoryNoFollow(staged.parent, parentFd, parentMode, error))
	{
		DiscardStagedDirectory(staged);
		return false;
	}
	struct stat info;
	if (::fstatat(parentFd, Base(staged.destination).c_str(), &info, AT_SYMLINK_NOFOLLOW) == 0)
	{
		::close(parentFd);
		error = "Profile already exists: '" + staged.destination + "'.";
		DiscardStagedDirectory(staged);
		return false;
	}
	if (errno != ENOENT)
	{
		const int saved = errno;
		::close(parentFd);
		errno = saved;
		error = ErrorAt("Unable to inspect profile", staged.destination);
		DiscardStagedDirectory(staged);
		return false;
	}
	if (::renameat(parentFd, Base(staged.temporary).c_str(), parentFd, Base(staged.destination).c_str()) != 0)
	{
		const int saved = errno;
		::close(parentFd);
		errno = saved;
		error = ErrorAt("Unable to publish profile", staged.destination);
		DiscardStagedDirectory(staged);
		return false;
	}
	staged.active = false;
	const bool synchronized = SyncDirectoryFd(parentFd, staged.parent, error);
	const int synchronizationError = errno;
	bool closed = ::close(parentFd) == 0;
	if (!closed && synchronized) error = ErrorAt("Unable to close directory", staged.parent);
	if (synchronized && closed) return true;
	if (!synchronized) errno = synchronizationError;
	const std::string publicationError = error;
	std::string removalError;
	const bool removed = RemovePublishedProfile(staged.destination, removalError);
	error = publicationError;
	if (!removed) error += " Profile rollback failed: " + removalError;
	return false;
}
bool RemovePublishedProfile(const std::string& profileRoot, std::string& error)
{
	int parentFd;
	mode_t parentMode;
	if (!OpenDirectoryNoFollow(Parent(profileRoot), parentFd, parentMode, error)) return false;
	const bool removed = RemoveTreeAt(parentFd, Base(profileRoot), profileRoot, error);
	if (removed && !SyncDirectoryFd(parentFd, Parent(profileRoot), error))
	{
		::close(parentFd);
		return false;
	}
	const int saved = errno;
	if (::close(parentFd) != 0 && removed)
	{
		error = ErrorAt("Unable to close directory", Parent(profileRoot));
		return false;
	}
	errno = saved;
	return removed;
}
}
#ifdef HP2_LAUNCHER_TESTING
void SetLauncherPublishFailureForTesting(int publicationIndex)
{
	GFailAfterPublication = publicationIndex;
	GPublicationCount = 0;
}
#endif


bool LoadDataSourceConfiguration(
	const std::string& launcherRoot,
	DataSourceConfiguration& configuration,
	std::string& error)
{
	configuration = DataSourceConfiguration();
	error.clear();
	Document document;
	bool exists;
	mode_t mode;
	std::string original;
	if (!LoadLauncherDocument(launcherRoot, document, exists, mode, original, error)) return false;

	std::string value;
	DataSource source;
	if (Get(document, "DataSources", "Selected", value) && ParseSource(value, source))
		configuration.selected = source;
	if (Get(document, "DataSources", "RetailRoot", value) &&
		IsAbsolutePath(value) &&
		IsValidUtf8(value) &&
		value.find_first_of("\r\n") == std::string::npos &&
		Trim(value) == value)
	{
		configuration.retailRoot = value;
	}
	if (Get(document, "DataSources", "PrototypeRoot", value) &&
		IsAbsolutePath(value) &&
		IsValidUtf8(value) &&
		value.find_first_of("\r\n") == std::string::npos &&
		Trim(value) == value)
	{
		configuration.prototypeRoot = value;
	}
	return true;
}

bool CommitDataSourceConfiguration(
	const std::string& launcherRoot,
	const DataSourceConfiguration& configuration,
	std::string& error)
{
	error.clear();
	const char* selected;
	if (!SourceText(configuration.selected, selected))
	{
		error = "Data source configuration contains an invalid selection.";
		return false;
	}
	auto validRoot = [](const std::string& root)
	{
		return root.empty() ||
			(IsAbsolutePath(root) &&
				IsValidUtf8(root) &&
				root.find_first_of("\r\n") == std::string::npos &&
				Trim(root) == root);
	};
	if (!validRoot(configuration.retailRoot) || !validRoot(configuration.prototypeRoot))
	{
		error = "Data source roots must be absolute, valid UTF-8 single-line paths.";
		return false;
	}

	Document document;
	bool exists;
	mode_t mode;
	std::string original;
	if (!LoadLauncherDocument(launcherRoot, document, exists, mode, original, error)) return false;
	Set(document, "DataSources", "Selected", selected);
	Set(document, "DataSources", "RetailRoot", configuration.retailRoot);
	Set(document, "DataSources", "PrototypeRoot", configuration.prototypeRoot);
	return PublishLauncherDocument(launcherRoot, document, exists, mode, original, error);
}

bool PrepareDataSourceProfile(
	const std::string& launcherRoot,
	DataSource source,
	std::string& profileRoot,
	std::string& error)
{
	profileRoot.clear();
	error.clear();
	const char* sourceName;
	if (!SourceText(source, sourceName))
	{
		error = "Data source configuration contains an invalid selection.";
		return false;
	}
	if (!Directory(launcherRoot))
	{
		error = "Launcher root is not a directory: '" + launcherRoot + "'.";
		return false;
	}

	const std::string profilesRoot = Join(launcherRoot, "Profiles");
	const std::string requestedProfile = Join(profilesRoot, sourceName);
	struct stat profilesInfo;
	if (::lstat(profilesRoot.c_str(), &profilesInfo) == 0)
	{
		if (!S_ISDIR(profilesInfo.st_mode))
		{
			error = "Profiles root is not a directory: '" + profilesRoot + "'.";
			return false;
		}
		struct stat profileInfo;
		if (::lstat(requestedProfile.c_str(), &profileInfo) == 0)
		{
			if (!S_ISDIR(profileInfo.st_mode))
			{
				error = "Profile root is not a directory: '" + requestedProfile + "'.";
				return false;
			}
			profileRoot = requestedProfile;
			return true;
		}
		if (errno != ENOENT)
		{
			error = ErrorAt("Unable to inspect profile", requestedProfile);
			return false;
		}
	}
	else if (errno != ENOENT)
	{
		error = ErrorAt("Unable to inspect profiles root", profilesRoot);
		return false;
	}

	Document document;
	bool documentExists;
	mode_t documentMode;
	std::string originalDocument;
	if (!LoadLauncherDocument(
			launcherRoot,
			document,
			documentExists,
			documentMode,
			originalDocument,
			error))
	{
		return false;
	}
	bool hasLegacyProfile = false;
	std::string legacyProfile;
	if (Get(document, "DataSources", "LegacyProfile", legacyProfile))
	{
		DataSource legacySource;
		if (!ParseSource(legacyProfile, legacySource))
		{
			error = "Launcher data source configuration has an invalid LegacyProfile marker.";
			return false;
		}
		hasLegacyProfile = true;
	}

	if (!hasLegacyProfile)
	{
		int legacyRootFd;
		mode_t legacyRootMode;
		if (!OpenDirectoryNoFollow(launcherRoot, legacyRootFd, legacyRootMode, error)) return false;
		static const char* const LegacyItems[] = {"Game.ini", "User.ini", "Save", "Cache"};
		bool inspected = true;
		for (const char* name : LegacyItems)
		{
			if (inspected &&
				!InspectLegacyTreeAt(legacyRootFd, name, Join(launcherRoot, name), true, error))
			{
				inspected = false;
			}
		}
		if (::close(legacyRootFd) != 0 && inspected)
		{
			error = ErrorAt("Unable to close directory", launcherRoot);
			inspected = false;
		}
		if (!inspected) return false;
	}

	if (!EnsureDirectoryNoFollow(profilesRoot, error)) return false;
	StagedDirectory staged;
	if (!StageProfile(launcherRoot, requestedProfile, !hasLegacyProfile, staged, error)) return false;
	if (!PublishProfile(staged, error)) return false;
	if (!hasLegacyProfile)
	{
		Set(document, "DataSources", "LegacyProfile", sourceName);
		if (!PublishLauncherDocument(
				launcherRoot,
				document,
				documentExists,
				documentMode,
				originalDocument,
				error))
		{
			const std::string publicationError = error;
			std::string removalError;
			const bool removed = RemovePublishedProfile(requestedProfile, removalError);
			error = publicationError;
			if (!removed) error += " Profile rollback failed: " + removalError;
			return false;
		}
	}
	profileRoot = requestedProfile;
	return true;
}

bool ValidateLauncherSettings(const LauncherSettings& settings, std::string& error)
{
	error.clear();
	if (settings.resolution.width < 320 || settings.resolution.width > 16384 ||
		settings.resolution.height < 320 || settings.resolution.height > 16384)
	{ error = "Resolution dimensions must each be between 320 and 16384."; return false; }
	if (!std::isfinite(settings.brightness) || settings.brightness < 0.1 || settings.brightness > 1.0)
	{ error = "Brightness must be between 0.1 and 1.0."; return false; }
	if (!std::isfinite(settings.renderScale) ||
		std::find(RenderScaleValues.begin(), RenderScaleValues.end(), settings.renderScale) == RenderScaleValues.end())
	{ error = "Render scale must be 0.50, 0.67, 0.75, 0.85, or 1.00."; return false; }
	if (!std::isfinite(settings.uiScale) ||
		std::find(UIScaleValues.begin(), UIScaleValues.end(), settings.uiScale) == UIScaleValues.end())
	{ error = "UI scale must be 0.75, 1.00, 1.25, 1.50, 1.75, or 2.00."; return false; }
	if (!std::isfinite(settings.mouseSensitivity) || settings.mouseSensitivity < 0.2 || settings.mouseSensitivity > 10.0)
	{ error = "Mouse sensitivity must be between 0.2 and 10.0."; return false; }
	if (!std::isfinite(settings.soundVolume) || settings.soundVolume < 0 || settings.soundVolume > 1 ||
		!std::isfinite(settings.musicVolume) || settings.musicVolume < 0 || settings.musicVolume > 1)
	{ error = "Sound and music volumes must be between 0.0 and 1.0."; return false; }
	if (!ValidCap(settings.frameRateLimit)) { error = "Frame rate limit must be unlimited, 30, 60, 120, or 144."; return false; }
	if (std::find(
			AntiAliasingSampleValues.begin(),
			AntiAliasingSampleValues.end(),
			settings.antiAliasingSamples) == AntiAliasingSampleValues.end())
	{ error = "Anti-aliasing must be off, 2x MSAA, or 4x MSAA."; return false; }
	if (std::find(AnisotropyValues.begin(), AnisotropyValues.end(), settings.anisotropy) == AnisotropyValues.end())
	{ error = "Anisotropy must be off, 4x, 8x, or 16x."; return false; }
	const int screen = static_cast<int>(settings.screenMode), texture = static_cast<int>(settings.textureDetail);
	const int object = static_cast<int>(settings.objectDetail), difficulty = static_cast<int>(settings.difficulty);
	const int control = static_cast<int>(settings.controlMode);
	if (screen < 0 || screen > 2 || texture < 0 || texture > 2 || object < 0 || object > 4 ||
		difficulty < 0 || difficulty > 2 || control < 0 || control > 1)
	{ error = "Launcher settings contain an invalid selection."; return false; }
	return true;
}

bool DiscoverSaves(const std::string& userRoot, std::vector<SaveRecord>& saves, std::string& error)
{
	saves.clear(); error.clear();
	const std::string root = Join(userRoot, "Save");
	struct stat rootInfo;
	if (::lstat(root.c_str(), &rootInfo) != 0)
	{
		if (errno == ENOENT)
			return true;
		error = ErrorAt("Unable to inspect save directory", root);
		return false;
	}
	if (!S_ISDIR(rootInfo.st_mode))
		return true;
	std::vector<std::string> rootNames;
	if (!Names(root, rootNames, false, error)) return false;
	SavesIn(root, rootNames, -1, false, saves);
	for (const std::string& name : rootNames)
	{
		int slot;
		if (!SlotName(name, slot)) continue;
		const std::string path = Join(root, name);
		struct stat info;
		if (::lstat(path.c_str(), &info) != 0 || !S_ISDIR(info.st_mode)) continue;
		std::vector<std::string> slotNames;
		if (!Names(path, slotNames, false, error)) return false;
		SavesIn(path, slotNames, slot, true, saves);
	}
	std::sort(saves.begin(), saves.end(), [](const SaveRecord& a, const SaveRecord& b)
	{
		if (a.usesSlotDirectory != b.usesSlotDirectory) return !a.usesSlotDirectory;
		if (!a.usesSlotDirectory && a.saveIndex != b.saveIndex) return a.saveIndex < b.saveIndex;
		if (a.usesSlotDirectory && a.slot != b.slot) return a.slot < b.slot;
		if (a.saveIndex != b.saveIndex) return a.saveIndex < b.saveIndex;
		return a.savePath < b.savePath;
	});
	return true;
}

bool LoadLauncherState(const LauncherPaths& paths, LauncherState& state, std::string& error)
{
	error.clear();
	Document game, user;
	bool gameExists, userExists;
	mode_t gameMode, userMode;
	if (!SelectDocument(paths, "Game.ini", "Default.ini", game, gameExists, gameMode, error) ||
		!SelectDocument(paths, "User.ini", "DefUser.ini", user, userExists, userMode, error)) return false;
	LoadSettings(game, user, state.settings);
	return DiscoverSaves(paths.userRoot, state.saves, error);
}

bool CommitLauncherSettings(const LauncherPaths& paths, const LauncherSettings& settings, std::string& error)
{
	if (!ValidateLauncherSettings(settings, error)) return false;
	if (!Directory(paths.userRoot)) { error = "Writable user root is not a directory: '" + paths.userRoot + "'."; return false; }
	Document game, user;
	bool gameExists, userExists;
	mode_t gameMode, userMode;
	if (!SelectDocument(paths, "Game.ini", "Default.ini", game, gameExists, gameMode, error) ||
		!SelectDocument(paths, "User.ini", "DefUser.ini", user, userExists, userMode, error)) return false;
	const std::string gamePath = Join(paths.userRoot, "Game.ini"), userPath = Join(paths.userRoot, "User.ini");
	std::string originalGame, originalUser;
	if (gameExists &&
		(!ReadFile(gamePath, originalGame, nullptr, error) || !Backup(gamePath, originalGame, gameMode, error)))
		return false;
	if (userExists &&
		(!ReadFile(userPath, originalUser, nullptr, error) || !Backup(userPath, originalUser, userMode, error)))
		return false;

	ApplyGame(game, settings);
	ApplyUser(user, settings);
	std::string gameBytes, userBytes;
	if (!Encode(game, gameBytes, error) || !Encode(user, userBytes, error)) return false;

	StagedFile stagedGame, stagedUser, rollbackGame, rollbackUser;
	if (!Stage(gamePath, gameBytes, gameExists ? gameMode : 0600, stagedGame, error)) return false;
	if (!Stage(userPath, userBytes, userExists ? userMode : 0600, stagedUser, error))
	{
		Discard(stagedGame);
		return false;
	}
	if (gameExists && !Stage(gamePath, originalGame, gameMode, rollbackGame, error))
	{
		Discard(stagedGame);
		Discard(stagedUser);
		return false;
	}
	if (userExists && !Stage(userPath, originalUser, userMode, rollbackUser, error))
	{
		Discard(stagedGame);
		Discard(stagedUser);
		Discard(rollbackGame);
		return false;
	}

	if (!Publish(stagedGame, error))
	{
		const std::string publicationError = error;
		std::string restorationError;
		const bool restored = RestorePublishedFile(gamePath, gameExists, rollbackGame, restorationError);
		Discard(stagedUser);
		Discard(rollbackUser);
		error = restored ? publicationError : publicationError + " Rollback failed: " + restorationError;
		return false;
	}
	if (!Publish(stagedUser, error))
	{
		const std::string publicationError = error;
		std::string userRestorationError, gameRestorationError;
		const bool userRestored = RestorePublishedFile(userPath, userExists, rollbackUser, userRestorationError);
		const bool gameRestored = RestorePublishedFile(gamePath, gameExists, rollbackGame, gameRestorationError);
		error = publicationError;
		if (!userRestored) error += " User.ini rollback failed: " + userRestorationError;
		if (!gameRestored) error += " Game.ini rollback failed: " + gameRestorationError;
		return false;
	}

	Discard(rollbackGame);
	Discard(rollbackUser);
	return true;
}

namespace
{
bool MigrationFinalValue(
	const std::vector<LegacySettingValue>& values,
	const char* section,
	const char* key,
	std::string& text)
{
	bool found = false;
	for (const LegacySettingValue& entry : values)
	{
		if (Same(entry.section, section) && Same(entry.key, key))
		{
			text = entry.value;
			found = true;
		}
	}
	return found;
}

void MigrationRewrite(
	std::vector<LegacySettingValue>& values,
	const char* section,
	const char* key,
	const std::string& next,
	const char* reason,
	std::vector<SettingsMigrationChange>& changes)
{
	std::string previous;
	const bool existed = MigrationFinalValue(values, section, key, previous);
	std::size_t last = values.size();
	for (std::size_t i = 0; i < values.size(); ++i)
	{
		if (Same(values[i].section, section) && Same(values[i].key, key)) last = i;
	}
	if (last != values.size())
		values[last].value = next;
	else
		values.push_back({section, key, next});
	if (!existed || previous != next)
		changes.push_back({section, key, existed ? previous : "", next, reason});
}
}

std::vector<LegacySettingValue> MigrateLegacySettings(
	const std::vector<LegacySettingValue>& legacyValues,
	std::vector<SettingsMigrationChange>& changes)
{
	changes.clear();
	std::vector<LegacySettingValue> values = legacyValues;

	// Owned booleans canonicalize to True/False; invalid spellings fall back
	// to the model default the loader would have used.
	struct OwnedBoolean { const char* section; const char* key; bool fallback; };
	static const OwnedBoolean OwnedBooleans[] = {
		{"SDLDrv.SDLClient", "StartupFullscreen", false},
		{"SDLDrv.SDLClient", "BorderlessWindow", false},
		{"SDLDrv.SDLClient", "UseDesktopResolution", false},
		{"SDLDrv.SDLClient", "ShowFPS", false},
		{"SDLDrv.SDLClient", "MaintainVerticalFOV", true},
		{"SDLDrv.SDLClient", "NativeText", true},
		{"SDLDrv.SDLClient", "ScreenFlashes", true},
		{"SDLDrv.SDLClient", "UseJoystick", true},
		{"XOpenGLDrv.XOpenGLRenderDevice", "UseVSync", false},
		{"XOpenGLDrv.XOpenGLRenderDevice", "UseAA", false},
		{"Engine.GameEngine", "UseSound", true},
		{"Engine.PlayerPawn", "bInvertMouse", false},
		{"Engine.PlayerPawn", "bModernThirdPersonControls", false},
		{"HGame.Harry", "bAutoCenterCamera", true},
		{"HGame.Harry", "bMoveWhileCasting", true},
		{"HGame.Harry", "bAutoQuaff", true},
	};
	for (const OwnedBoolean& owned : OwnedBooleans)
	{
		std::string text;
		if (!MigrationFinalValue(values, owned.section, owned.key, text)) continue;
		bool parsed = owned.fallback;
		const bool recognized = Boolean(text, parsed);
		MigrationRewrite(
			values,
			owned.section,
			owned.key,
			BoolText(parsed),
			recognized ? "settings.boolean_spelling" : "settings.boolean_invalid_defaulted",
			changes);
	}

	// Screen mode: borderless/desktop overrides fullscreen, exactly as the
	// loader resolves the three legacy switches.
	std::string text;
	bool startupFullscreen = false, borderless = false, desktopResolution = false;
	const bool hasStartupFullscreen =
		MigrationFinalValue(values, "SDLDrv.SDLClient", "StartupFullscreen", text) &&
		Boolean(text, startupFullscreen);
	const bool hasBorderless =
		MigrationFinalValue(values, "SDLDrv.SDLClient", "BorderlessWindow", text) &&
		Boolean(text, borderless);
	const bool hasDesktopResolution =
		MigrationFinalValue(values, "SDLDrv.SDLClient", "UseDesktopResolution", text) &&
		Boolean(text, desktopResolution);
	if (hasStartupFullscreen || hasBorderless || hasDesktopResolution)
	{
		const bool borderlessDesktop = borderless || desktopResolution;
		if (hasStartupFullscreen)
		{
			MigrationRewrite(
				values,
				"SDLDrv.SDLClient",
				"StartupFullscreen",
				BoolText(startupFullscreen && !borderlessDesktop),
				"settings.screen_mode_consolidated",
				changes);
		}
		if (hasBorderless)
		{
			MigrationRewrite(
				values,
				"SDLDrv.SDLClient",
				"BorderlessWindow",
				BoolText(borderlessDesktop),
				"settings.screen_mode_consolidated",
				changes);
		}

		// Viewport quartet collapses onto the resolution the launcher would
		// actually run for the consolidated mode: the loader reads the
		// fullscreen pair only when the mode is not windowed.
		const bool fullSize = borderlessDesktop || startupFullscreen;
		const char* widthKey = fullSize ? "FullscreenViewportX" : "WindowedViewportX";
		const char* heightKey = fullSize ? "FullscreenViewportY" : "WindowedViewportY";
		int width = 800, height = 600;
		int parsedDimension;
		if (MigrationFinalValue(values, "SDLDrv.SDLClient", widthKey, text) &&
			Integer(text, parsedDimension) && parsedDimension >= 320 && parsedDimension <= 16384)
		{
			width = parsedDimension;
		}
		if (MigrationFinalValue(values, "SDLDrv.SDLClient", heightKey, text) &&
			Integer(text, parsedDimension) && parsedDimension >= 320 && parsedDimension <= 16384)
		{
			height = parsedDimension;
		}
		static const char* const ViewportKeys[] = {
			"WindowedViewportX", "WindowedViewportY", "FullscreenViewportX", "FullscreenViewportY"
		};
		for (const char* viewportKey : ViewportKeys)
		{
			if (!MigrationFinalValue(values, "SDLDrv.SDLClient", viewportKey, text)) continue;
			const bool isHeight = Same(viewportKey, "WindowedViewportY") ||
				Same(viewportKey, "FullscreenViewportY");
			MigrationRewrite(
				values,
				"SDLDrv.SDLClient",
				viewportKey,
				std::to_string(isHeight ? height : width),
				"settings.viewport_consolidated",
				changes);
		}
	}

	// Frame rate cap: renderer floats like "60.000000" become plain integers.
	if (MigrationFinalValue(values, "Engine.GameEngine", "FrameRateLimit", text))
	{
		double cap;
		int canonical = 60;
		const char* reason = "settings.frame_rate_limit_invalid_defaulted";
		if (Number(text, cap) && cap == std::floor(cap) && cap >= 0 && cap <= INT_MAX &&
			ValidCap(static_cast<int>(cap)))
		{
			canonical = static_cast<int>(cap);
			reason = "settings.frame_rate_limit_normalized";
		}
		MigrationRewrite(values, "Engine.GameEngine", "FrameRateLimit", std::to_string(canonical), reason, changes);
	}

	auto migrateDiscreteScale = [&](const char* section, const char* key,
		const double* accepted, std::size_t count, double fallback)
	{
		std::string raw;
		if (!MigrationFinalValue(values, section, key, raw)) return;
		double parsed;
		double canonical = fallback;
		bool acceptedValue = false;
		if (Number(raw, parsed))
		{
			for (std::size_t i = 0; i < count; ++i)
			{
				if (parsed == accepted[i]) { canonical = accepted[i]; acceptedValue = true; break; }
			}
		}
		MigrationRewrite(
			values,
			section,
			key,
			DoubleText(canonical),
			acceptedValue ? "settings.scale_normalized" : "settings.scale_invalid_defaulted",
			changes);
	};
	migrateDiscreteScale("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale",
		RenderScaleValues.data(), RenderScaleValues.size(), 1.0);
	migrateDiscreteScale("SDLDrv.SDLClient", "UIScale",
		UIScaleValues.data(), UIScaleValues.size(), 1.0);

	auto migrateRangedNumber = [&](const char* section, const char* key,
		double minimum, double maximum, double fallback)
	{
		std::string raw;
		if (!MigrationFinalValue(values, section, key, raw)) return;
		double parsed;
		double canonical = fallback;
		const bool valid = Number(raw, parsed) && parsed >= minimum && parsed <= maximum;
		if (valid) canonical = parsed;
		MigrationRewrite(
			values,
			section,
			key,
			DoubleText(canonical),
			valid ? "settings.range_normalized" : "settings.range_invalid_defaulted",
			changes);
	};
	migrateRangedNumber("SDLDrv.SDLClient", "Brightness", 0.1, 1.0, 0.4);
	migrateRangedNumber("ALAudio.ALAudioSubsystem", "SoundVolume", 0.0, 1.0, 0.9);
	migrateRangedNumber("ALAudio.ALAudioSubsystem", "MusicVolume", 0.0, 1.0, 0.53);
	migrateRangedNumber("Engine.PlayerPawn", "MouseSensitivity", 0.2, 10.0, 3.0);

	// Anti-aliasing: a disabled or unrecognized UseAA clears any stale sample
	// count, mirroring LoadAntiAliasing.
	{
		std::string aaRaw, samplesRaw;
		const bool hasUseAA =
			MigrationFinalValue(values, "XOpenGLDrv.XOpenGLRenderDevice", "UseAA", aaRaw);
		const bool hasSamples =
			MigrationFinalValue(values, "XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", samplesRaw);
		if (hasUseAA || hasSamples)
		{
			bool enabled = false;
			if (hasUseAA) Boolean(aaRaw, enabled);
			int samples = 0;
			int parsedSamples;
			if (enabled && hasSamples && Integer(samplesRaw, parsedSamples) &&
				(parsedSamples == 2 || parsedSamples == 4))
			{
				samples = parsedSamples;
			}
			enabled = samples != 0;
			if (hasUseAA)
			{
				MigrationRewrite(values, "XOpenGLDrv.XOpenGLRenderDevice", "UseAA",
					BoolText(enabled), "settings.antialiasing_normalized", changes);
			}
			if (hasSamples)
			{
				MigrationRewrite(values, "XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples",
					std::to_string(samples), "settings.antialiasing_normalized", changes);
			}
		}
	}

	if (MigrationFinalValue(values, "XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", text))
	{
		double parsed;
		int canonical = 4;
		const char* reason = "settings.anisotropy_invalid_defaulted";
		if (Number(text, parsed) && parsed == std::floor(parsed) && parsed >= INT_MIN && parsed <= INT_MAX)
		{
			const int discrete = static_cast<int>(parsed);
			if (std::find(AnisotropyValues.begin(), AnisotropyValues.end(), discrete) != AnisotropyValues.end())
			{
				canonical = discrete;
				reason = "settings.anisotropy_normalized";
			}
		}
		MigrationRewrite(values, "XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", std::to_string(canonical), reason, changes);
	}

	auto migrateEnum = [&](const char* section, const char* key,
		const char* const* spellings, std::size_t count, std::size_t fallbackIndex)
	{
		std::string raw;
		if (!MigrationFinalValue(values, section, key, raw)) return;
		std::size_t selected = fallbackIndex;
		bool recognized = false;
		const std::string trimmed = Trim(raw);
		for (std::size_t i = 0; i < count; ++i)
		{
			if (Same(trimmed, spellings[i])) { selected = i; recognized = true; break; }
		}
		MigrationRewrite(
			values,
			section,
			key,
			spellings[selected],
			recognized ? "settings.enum_normalized" : "settings.enum_invalid_defaulted",
			changes);
	};
	static const char* const TextureSpellings[] = {"Low", "Medium", "High"};
	static const char* const ObjectSpellings[] = {
		"ObjectDetailVeryLow", "ObjectDetailLow", "ObjectDetailMedium",
		"ObjectDetailHigh", "ObjectDetailVeryHigh"
	};
	static const char* const DifficultySpellings[] = {"DifficultyEasy", "DifficultyMedium", "DifficultyHard"};
	migrateEnum("SDLDrv.SDLClient", "TextureDetail", TextureSpellings, 3, 2);
	migrateEnum("Engine.PlayerPawn", "ObjectDetail", ObjectSpellings, 5, 2);
	migrateEnum("Engine.PlayerPawn", "Difficulty", DifficultySpellings, 3, 0);

	return values;
}

bool CommitLaunchSelection(
	const std::string& launcherRoot,
	const LaunchSelection& selection,
	std::string& error)
{
	error.clear();
	std::vector<LaunchSelectionField> fields;
	if (!SerializeLaunchSelection(selection, fields, error)) return false;
	Document document;
	bool exists;
	mode_t mode;
	std::string original;
	if (!LoadLauncherDocument(launcherRoot, document, exists, mode, original, error)) return false;
	// A commit replaces the WHOLE selection, not individual rows: clear every
	// persisted selection key first so a non-Continue selection (or a Continue
	// without a slot) can never inherit stale save-coordinate rows from the
	// previous generation. Last-writer-wins and backup semantics are handled
	// by PublishLauncherDocument below and stay untouched.
	static const char* const SelectionKeys[] = {"Action", "HasSave", "SaveIndex", "SaveSlot"};
	for (const char* selectionKey : SelectionKeys)
		Erase(document, "LastLaunch", selectionKey);
	for (const LaunchSelectionField& field : fields)
		Set(document, "LastLaunch", field.key.c_str(), field.value);
	return PublishLauncherDocument(launcherRoot, document, exists, mode, original, error);
}

bool LoadLaunchSelection(const std::string& launcherRoot, LaunchSelection& selection, std::string& error)
{
	selection = LaunchSelection();
	error.clear();
	Document document;
	bool exists;
	mode_t mode;
	std::string original;
	if (!LoadLauncherDocument(launcherRoot, document, exists, mode, original, error)) return false;
	std::vector<LaunchSelectionField> fields;
	static const char* const SelectionKeys[] = {"Action", "HasSave", "SaveIndex", "SaveSlot"};
	for (const char* selectionKey : SelectionKeys)
	{
		std::string value;
		if (Get(document, "LastLaunch", selectionKey, value))
			fields.push_back({selectionKey, value});
	}
	return DeserializeLaunchSelection(fields, selection, error);
}
}

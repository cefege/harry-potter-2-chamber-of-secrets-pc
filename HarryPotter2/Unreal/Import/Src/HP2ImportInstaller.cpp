#include "HP2ImportInternal.h"
#include <libunshield.h>

// Installer discs: the game CD keeps every game file inside InstallShield
// cabinets (setup/data1.hdr + dataN.cab), split into numbered file groups
// whose install folders live in the compiled setup script, not the cabinet.
// Files are therefore placed by what they are: map, music and texture
// packages by type, the help pages, cutscene scripts and "Old textures" by
// the cabinet's own directory names, and the US-English language files by
// their .usa suffix. Loose packages beside the cabinets (setup/hgame.u) are
// copied on top, the way the installer does after unpacking.

namespace HP2Import::Detail {
namespace {
std::string Extension(const std::string& lowerName) {
    const auto dot = lowerName.rfind('.');
    return dot == std::string::npos ? std::string() : lowerName.substr(dot);
}

std::string NormalizedDirectory(std::string directory) {
    std::replace(directory.begin(), directory.end(), '\\', '/');
    while (!directory.empty() && directory.front() == '/') directory.erase(directory.begin());
    while (!directory.empty() && directory.back() == '/') directory.pop_back();
    return Lower(directory);
}

struct Cabinet {
    Unshield* handle = nullptr;
    explicit Cabinet(const fs::path& header) {
        unshield_set_log_level(0);
        handle = unshield_open(Utf8(header).c_str());
    }
    ~Cabinet() { if (handle) unshield_close(handle); }
    Cabinet(const Cabinet&) = delete;
    Cabinet& operator=(const Cabinet&) = delete;
};

struct Job {
    int index = -1;
    std::string destination;
};

bool SameBytes(const fs::path& a, const fs::path& b) {
    return Hash(a) == Hash(b);
}
}

bool IsInstallerDisc(const fs::path& root) {
    std::error_code ec;
    for (fs::directory_iterator it(root, ec), end; it != end && !ec; it.increment(ec)) {
        if (Lower(Utf8(it->path().filename())) != "setup" || !it->is_directory()) continue;
        for (fs::directory_iterator inner(it->path(), ec), innerEnd; inner != innerEnd && !ec; inner.increment(ec))
            if (Lower(Utf8(inner->path().filename())) == "data1.hdr" && inner->is_regular_file()) return true;
    }
    return false;
}

namespace {
constexpr std::uint64_t SectorSize = 2048;

// The files the expansion reads from setup/: the cabinet header, the
// cabinets, and loose packages the installer copies over the unpacked ones.
bool IsInstallerFile(const std::string& lowerName) {
    if (lowerName == "data1.hdr") return true;
    if (Ends(lowerName, ".u") && lowerName.size() > 2) return true;
    if (!Starts(lowerName, "data") || !Ends(lowerName, ".cab") || lowerName.size() <= 8) return false;
    const auto digits = lowerName.substr(4, lowerName.size() - 8);
    return std::all_of(digits.begin(), digits.end(), [](char c) { return c >= '0' && c <= '9'; });
}

struct DiscEntry {
    std::string name;
    std::uint64_t extent = 0, size = 0;
    bool directory = false;
};

std::uint32_t Le32(const unsigned char* p) {
    return static_cast<std::uint32_t>(p[0]) | static_cast<std::uint32_t>(p[1]) << 8 |
        static_cast<std::uint32_t>(p[2]) << 16 | static_cast<std::uint32_t>(p[3]) << 24;
}

void ReadAt(std::ifstream& in, std::uint64_t offset, unsigned char* data, std::size_t size, std::uint64_t imageSize) {
    if (offset > imageSize || size > imageSize - offset) Fail("the disc image is truncated or damaged");
    in.seekg(static_cast<std::streamoff>(offset));
    in.read(reinterpret_cast<char*>(data), static_cast<std::streamsize>(size));
    if (!in) Fail("cannot read the disc image");
}

std::vector<DiscEntry> ReadDirectory(std::ifstream& in, const DiscEntry& directory, std::uint64_t imageSize) {
    if (directory.size > 64ull * 1024 * 1024) Fail("the disc image has an implausible directory");
    std::vector<unsigned char> data(static_cast<std::size_t>(directory.size));
    ReadAt(in, directory.extent * SectorSize, data.data(), data.size(), imageSize);
    std::vector<DiscEntry> entries;
    std::size_t at = 0;
    while (at < data.size()) {
        const std::size_t length = data[at];
        if (length == 0) { at = (at / SectorSize + 1) * SectorSize; continue; }
        if (length < 34 || at + length > data.size()) Fail("the disc image has a damaged directory");
        const unsigned char* record = data.data() + at;
        const std::size_t nameLength = record[32];
        if (33 + nameLength > length) Fail("the disc image has a damaged directory");
        std::string name(reinterpret_cast<const char*>(record + 33), nameLength);
        at += length;
        if (nameLength == 1 && (name[0] == '\0' || name[0] == '\1')) continue;
        if (const auto semicolon = name.find(';'); semicolon != std::string::npos) name.erase(semicolon);
        if (!name.empty() && name.back() == '.') name.pop_back();
        entries.push_back({Lower(name), Le32(record + 2), Le32(record + 10), (record[25] & 0x02) != 0});
    }
    return entries;
}
}

fs::path ExtractDiscImage(const fs::path& image, const fs::path& destination) {
    std::ifstream in(image, std::ios::binary);
    if (!in) Fail("cannot open the disc image " + Utf8(image));
    const auto imageSize = static_cast<std::uint64_t>(fs::file_size(image));
    // The first primary volume descriptor; copy-protected discs carry more.
    DiscEntry root;
    for (std::uint64_t sector = 16;; ++sector) {
        unsigned char descriptor[SectorSize];
        ReadAt(in, sector * SectorSize, descriptor, sizeof descriptor, imageSize);
        if (std::string(reinterpret_cast<const char*>(descriptor + 1), 5) != "CD001")
            Fail("this file is not a CD image (ISO 9660)");
        if (descriptor[0] == 255) Fail("the disc image has no primary volume descriptor");
        if (descriptor[0] != 1) continue;
        root = {"", Le32(descriptor + 156 + 2), Le32(descriptor + 156 + 10), true};
        break;
    }
    DiscEntry setup;
    for (const auto& entry : ReadDirectory(in, root, imageSize))
        if (entry.directory && entry.name == "setup") setup = entry;
    if (!setup.directory)
        Fail("this disc image is not the game's installer CD (no setup folder)");
    const fs::path target = destination / "setup";
    std::error_code ec;
    fs::create_directories(target, ec);
    if (ec) Fail("cannot prepare " + Utf8(target) + ": " + ec.message());
    std::vector<char> buffer(ChunkSize);
    for (const auto& entry : ReadDirectory(in, setup, imageSize)) {
        if (entry.directory || !IsInstallerFile(entry.name)) continue;
        if (entry.extent * SectorSize > imageSize || entry.size > imageSize - entry.extent * SectorSize)
            Fail("the disc image is truncated or damaged");
        std::ofstream out(target / Path(entry.name), std::ios::binary | std::ios::trunc);
        if (!out) Fail("cannot write " + entry.name + " from the disc image");
        in.seekg(static_cast<std::streamoff>(entry.extent * SectorSize));
        for (std::uint64_t left = entry.size; left > 0;) {
            const auto chunk = static_cast<std::size_t>(std::min<std::uint64_t>(left, buffer.size()));
            in.read(buffer.data(), static_cast<std::streamsize>(chunk));
            if (!in) Fail("cannot read " + entry.name + " from the disc image");
            out.write(buffer.data(), static_cast<std::streamsize>(chunk));
            if (!out) Fail("cannot write " + entry.name + " from the disc image");
            left -= chunk;
        }
    }
    if (!IsInstallerDisc(destination))
        Fail("this disc image is not the game's installer CD (no setup/data1.hdr)");
    return destination;
}

std::string RouteInstallerFile(const std::string& directory, const std::string& name, bool systemGroup,
    const std::set<std::string>& cutscenes) {
    if (name.empty() || name.find('/') != std::string::npos || name.find('\\') != std::string::npos ||
        name == "." || name == "..")
        return {};
    if (Debris(Path(name))) return {};
    const auto dir = NormalizedDirectory(directory);
    const auto lower = Lower(name);
    const auto ext = Extension(lower);
    if (dir == "help") return Prohibited("Help/" + name) ? std::string() : "Help/" + name;
    if (dir == "old textures" || dir == "old_textures") return ext == ".utx" ? "Textures/Old textures/" + name : std::string();
    if (dir == "cutscenes") return ext == ".int" ? "System/Cutscenes/" + name : std::string();
    if (!dir.empty()) return {};
    if (ext == ".unr") return "Maps/" + name;
    if (ext == ".ogg") return "Music/" + name;
    if (ext == ".utx") return "Textures/" + name;
    // Dialogue banks carry their language in the name (AllDialog.USA_uax);
    // only the US-English one is installed.
    if (ext == ".uax" || Ends(lower, ".usa_uax")) return "Sounds/" + name;
    if (ext == ".int") return cutscenes.count(lower) ? "System/Cutscenes/" + name : "System/" + name;
    if (ext == ".u" || ext == ".usa") return "System/" + name;
    // Each language group ships its own default.ini; the base one is the
    // template (the importer sets the language itself).
    if (systemGroup && (lower == "default.ini" || lower == "defuser.ini")) return "System/" + name;
    return {};
}

fs::path ExpandInstaller(const fs::path& disc, const fs::path& destination, Progress* progress, bool consumeSource) {
    const auto setup = Child(disc, "setup");
    fs::path header;
    for (const auto& entry : fs::directory_iterator(setup))
        if (Lower(Utf8(entry.path().filename())) == "data1.hdr") header = entry.path();
    if (header.empty()) Fail("missing setup/data1.hdr on the game disc");
    Cabinet cabinet(header);
    if (!cabinet.handle) Fail("cannot read the installer cabinets in " + Utf8(setup));
    Unshield* u = cabinet.handle;

    auto directoryOf = [&](int index) {
        const char* name = unshield_directory_name(u, unshield_file_directory(u, index));
        return std::string(name ? name : "");
    };
    auto nameOf = [&](int index) {
        const char* name = unshield_file_name(u, index);
        return std::string(name ? name : "");
    };

    const int groups = unshield_file_group_count(u);
    int systemGroup = -1;
    std::set<std::string> cutscenes;
    for (int g = 0; g < groups; ++g) {
        const auto* group = unshield_file_group_get(u, g);
        if (!group) continue;
        for (int i = group->first_file; i <= group->last_file; ++i) {
            if (!unshield_file_is_valid(u, i)) continue;
            const auto dir = NormalizedDirectory(directoryOf(i));
            const auto lower = Lower(nameOf(i));
            if (dir.empty() && lower == "core.u") systemGroup = g;
            if (dir == "cutscenes") cutscenes.insert(lower);
        }
    }
    if (systemGroup < 0) Fail("the installer cabinets do not contain the game (no System/Core.u)");

    std::vector<Job> jobs;
    for (int g = 0; g < groups; ++g) {
        const auto* group = unshield_file_group_get(u, g);
        if (!group) continue;
        for (int i = group->first_file; i <= group->last_file; ++i) {
            if (!unshield_file_is_valid(u, i)) continue;
            auto target = RouteInstallerFile(directoryOf(i), nameOf(i), g == systemGroup, cutscenes);
            if (!target.empty()) jobs.push_back({i, std::move(target)});
        }
    }
    std::vector<fs::path> loose;
    for (const auto& entry : fs::directory_iterator(setup))
        if (entry.is_regular_file() && Lower(Utf8(entry.path().extension())) == ".u") loose.push_back(entry.path());

    if (progress) {
        progress->filesTotal = static_cast<std::int64_t>(jobs.size() + loose.size());
        progress->filesDone = 0;
    }
    const fs::path root = destination / "Harry Potter and the Chamber of Secrets";
    std::error_code ec;
    fs::create_directories(root, ec);
    if (ec) Fail("cannot prepare " + Utf8(root) + ": " + ec.message());
    auto advance = [&] {
        if (!progress) return;
        ++progress->filesDone;
        if (progress->cancelRequested.load()) Fail("Import cancelled.");
    };
    for (const auto& job : jobs) {
        const fs::path target = root / Path(job.destination);
        fs::create_directories(target.parent_path(), ec);
        if (ec) Fail("cannot prepare " + Utf8(target.parent_path()) + ": " + ec.message());
        fs::path part = target;
        part += ".part";
        if (!unshield_file_save(u, job.index, Utf8(part).c_str()))
            Fail("cannot extract " + job.destination + " from the installer cabinets");
        if (fs::exists(target)) {
            const bool same = SameBytes(part, target);
            fs::remove(part, ec);
            if (!same) Fail("the installer holds two different copies of " + job.destination);
        } else {
            fs::rename(part, target, ec);
            if (ec) Fail("cannot write " + job.destination + ": " + ec.message());
        }
        advance();
    }
    const fs::path system = root / "System";
    for (const auto& file : loose) {
        const auto name = Utf8(file.filename());
        for (const auto& existing : fs::directory_iterator(system))
            if (Lower(Utf8(existing.path().filename())) == Lower(name)) fs::remove(existing.path(), ec);
        fs::copy_file(file, system / file.filename(), ec);
        if (ec) Fail("cannot copy " + name + " from the game disc: " + ec.message());
        advance();
    }
    if (consumeSource) fs::remove_all(disc, ec);
    return root;
}
}

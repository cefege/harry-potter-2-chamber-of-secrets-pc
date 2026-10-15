#include "HP2ImportInternal.h"
#include <archive.h>
#include <archive_entry.h>

namespace HP2Import::Detail {
namespace {
std::string MemberPath(std::string name) {
    std::replace(name.begin(), name.end(), '\\', '/');
    if (name.empty() || name.front() == '/') Fail("unsafe archive member path: '" + name + "'");
    std::stringstream parts(name);
    std::string part, normalized;
    bool first = true;
    while (std::getline(parts, part, '/')) {
        if (part.empty() || part == "." || part == "..")
            Fail("unsafe archive member path: '" + name + "'");
        if (first && part.find(':') != std::string::npos)
            Fail("drive-qualified archive member path: '" + name + "'");
        if (part.find('\0') != std::string::npos) Fail("unsafe path component: '" + part + "'");
        if (!first) normalized += '/';
        normalized += part;
        first = false;
    }
    return normalized;
}
std::string EntryName(archive_entry* entry) {
    const char* utf8 = archive_entry_pathname_utf8(entry);
    const char* pathname = utf8 ? utf8 : archive_entry_pathname(entry);
    return pathname ? pathname : "";
}
std::string ArchiveError(archive* reader) {
    const char* error = archive_error_string(reader);
    return error ? error : "unknown archive error";
}
}

fs::path Extract(const fs::path& file, const fs::path& destination) {
    const auto extension = Lower(Utf8(file.filename()));
    if (Ends(extension, ".iso")) return ExtractDiscImage(file, destination);
    if (!Ends(extension, ".7z") && !Ends(extension, ".zip") &&
        !Ends(extension, ".tar") && !Ends(extension, ".tar.gz") &&
        !Ends(extension, ".tgz") && !Ends(extension, ".tar.bz2") &&
        !Ends(extension, ".tbz2") && !Ends(extension, ".tar.xz") &&
        !Ends(extension, ".txz"))
        Fail("unsupported archive format (use ZIP, TAR, 7z or the game CD's ISO): " + Utf8(file));
    archive* reader = archive_read_new();
    if (!reader) Fail("cannot read archive " + Utf8(file) + ": cannot allocate archive reader");
    archive_read_support_format_zip(reader);
    archive_read_support_format_tar(reader);
    archive_read_support_format_7zip(reader);
    archive_read_support_filter_none(reader);
    archive_read_support_filter_gzip(reader);
    archive_read_support_filter_bzip2(reader);
    archive_read_support_filter_xz(reader);
    const auto filename = Utf8(file);
    if (archive_read_open_filename(reader, filename.c_str(), ChunkSize) != ARCHIVE_OK) {
        const auto reason = ArchiveError(reader);
        archive_read_free(reader);
        Fail("cannot read archive " + filename + ": " + reason);
    }
    std::set<std::string> seen;
    archive_entry* entry = nullptr;
    int result = ARCHIVE_OK;
    std::vector<char> buffer(ChunkSize);
    while ((result = archive_read_next_header(reader, &entry)) == ARCHIVE_OK) {
        std::string raw = EntryName(entry);
        const bool directory = archive_entry_filetype(entry) == AE_IFDIR;
        if (directory) while (!raw.empty() && (raw.back() == '/' || raw.back() == '\\')) raw.pop_back();
        const auto relative = MemberPath(raw);
        if (!seen.insert(Lower(relative)).second) {
            archive_read_free(reader);
            Fail("case-insensitive duplicate archive member: " + raw);
        }
        const auto type = archive_entry_filetype(entry);
        if (archive_entry_hardlink(entry) || archive_entry_symlink(entry) ||
            (type != AE_IFDIR && type != AE_IFREG)) {
            archive_read_free(reader);
            Fail("archive link/device entry is forbidden: " + raw);
        }
        const fs::path target = destination / Path(relative);
        std::error_code ec;
        if (directory) {
            fs::create_directories(target, ec);
            if (ec) {
                archive_read_free(reader);
                Fail("cannot extract archive member " + raw + ": " + ec.message());
            }
            archive_read_data_skip(reader);
            continue;
        }
        fs::create_directories(target.parent_path(), ec);
        if (ec) {
            archive_read_free(reader);
            Fail("cannot extract archive member " + raw + ": " + ec.message());
        }
        if (fs::exists(target, ec) || ec) {
            archive_read_free(reader);
            Fail("cannot extract archive member " + raw + ": exclusive create failed");
        }
        std::ofstream output(target, std::ios::binary | std::ios::out);
        if (!output) {
            archive_read_free(reader);
            Fail("cannot extract archive member " + raw + ": exclusive create failed");
        }
        for (;;) {
            const auto amount = archive_read_data(reader, buffer.data(), buffer.size());
            if (amount == 0) break;
            if (amount < 0) {
                const auto reason = ArchiveError(reader);
                archive_read_free(reader);
                Fail("cannot read archive member: " + reason);
            }
            output.write(buffer.data(), static_cast<std::streamsize>(amount));
            if (!output) {
                archive_read_free(reader);
                Fail("cannot extract archive member " + raw + ": write failed");
            }
        }
        output.close();
        if (!output) {
            archive_read_free(reader);
            Fail("cannot extract archive member " + raw + ": write failed");
        }
    }
    if (result != ARCHIVE_EOF) {
        const auto reason = ArchiveError(reader);
        archive_read_free(reader);
        Fail("cannot read archive " + filename + ": " + reason);
    }
    archive_read_free(reader);

    std::vector<fs::path> candidates;
    std::vector<fs::path> directories{destination};
    while (!directories.empty()) {
        const auto current = directories.back();
        directories.pop_back();
        std::set<std::string> children;
        for (fs::directory_iterator iterator(current), end; iterator != end; ++iterator) {
            std::error_code ec;
            const auto status = iterator->symlink_status(ec);
            if (ec) Fail("cannot inspect extracted archive: " + Utf8(iterator->path()) + ": " + ec.message());
            if (fs::is_symlink(status)) Fail("symbolic-link directory in extracted archive: " + Utf8(iterator->path()));
            if (fs::is_directory(status)) {
                const auto name = Lower(Utf8(iterator->path().filename()));
                if (!children.insert(name).second) Fail("case-insensitive duplicate archive member: " + Utf8(iterator->path().filename()));
                directories.push_back(iterator->path());
            }
        }
        if (children.count("system") && children.count("maps") && children.count("textures") &&
            children.count("sounds") && children.count("music") && children.count("help"))
            candidates.push_back(current);
    }
    if (candidates.size() != 1)
        Fail("expected one retail tree in archive, found " + std::to_string(candidates.size()));
    return candidates.front();
}
}

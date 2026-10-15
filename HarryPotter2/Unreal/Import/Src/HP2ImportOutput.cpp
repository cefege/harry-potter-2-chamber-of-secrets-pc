#include "HP2ImportInternal.h"
#include <random>

namespace HP2Import {
using namespace Detail;
namespace {
std::string ProfileName(Profile profile) {
    switch (profile) {
    case Profile::Safe: return "safe";
    case Profile::Full: return "full";
    case Profile::RetailOnly: return "retail-only";
    }
    return "safe";
}
std::string RandomHex() {
    std::random_device random;
    static constexpr char digits[] = "0123456789abcdef";
    std::string value(32, '0');
    for (char& c : value) c = digits[random() & 15];
    return value;
}
fs::path SafeDirectory(const fs::path& path, const char* label) {
    std::error_code ec;
    const auto status = fs::symlink_status(path, ec);
    if (!ec && fs::is_symlink(status)) Fail(std::string(label) + " must not be a symbolic link: " + Utf8(path));
    auto resolved = fs::canonical(path, ec);
    if (ec) Fail("cannot resolve " + std::string(label) + " " + Utf8(path) + ": " + ec.message());
    if (!fs::is_directory(resolved)) Fail(std::string(label) + " is not a directory: " + Utf8(resolved));
    return resolved;
}
bool Overlap(const fs::path& a, const fs::path& b) {
    auto relative = a.lexically_relative(b);
    if (!relative.empty() && *relative.begin() != "..") return true;
    relative = b.lexically_relative(a);
    return !relative.empty() && *relative.begin() != "..";
}
std::map<std::string, fs::path> WalkOutput(const fs::path& output) {
    std::error_code ec;
    auto status = fs::symlink_status(output, ec);
    if (ec || fs::is_symlink(status) || !fs::is_directory(status))
        Fail("output is not a real directory: " + Utf8(output));
    std::map<std::string, fs::path> found;
    fs::recursive_directory_iterator it(output, fs::directory_options::none, ec), end;
    if (ec) Fail("cannot inspect output " + Utf8(output) + ": " + ec.message());
    for (; it != end; it.increment(ec)) {
        if (ec) Fail("cannot inspect output " + Utf8(it->path()) + ": " + ec.message());
        const fs::path path = it->path();
        // Keys are POSIX-style on every platform, matching asset paths; native separators
        // on Windows would make the output path set compare as unequal.
        const auto relative = GenericUtf8(path.lexically_relative(output));
        const auto entryStatus = it->symlink_status(ec);
        if (ec) Fail("cannot stat output " + Utf8(path) + ": " + ec.message());
        if (fs::is_symlink(entryStatus))
            Fail("symbolic link in output (dangling links are forbidden): " + Utf8(path));
        if (fs::is_directory(entryStatus)) continue;
        if (!fs::is_regular_file(entryStatus)) Fail("non-regular output entry: " + Utf8(path));
        const std::string key = Lower(relative);
        auto inserted = found.emplace(key, path);
        if (!inserted.second)
            Fail("case-insensitive output collision: " + Utf8(inserted.first->second) + " and " + Utf8(path));
        if (Prohibited(relative)) Fail("prohibited output payload: " + relative);
        std::ifstream stream(path, std::ios::binary);
        if (!stream) Fail("cannot inspect output " + Utf8(path) + ": cannot open file");
        char signature[2]{};
        stream.read(signature, 2);
        if (stream.gcount() == 2 && signature[0] == 'M' && signature[1] == 'Z')
            Fail("Windows PE payload disguised in output: " + relative);
        if (stream.bad()) Fail("cannot inspect output " + Utf8(path) + ": read error");
    }
    return found;
}
void ValidateOutput(const fs::path& output, const std::vector<Asset>& assets,
                    const std::string& manifest) {
    const auto found = WalkOutput(output);
    std::map<std::string, const Asset*> expected;
    for (const auto& asset : assets) expected.emplace(Lower(asset.path), &asset);
    expected.emplace("overlay-manifest.json", nullptr);
    expected.emplace("overlay-checksums.txt", nullptr);
    std::vector<std::string> missing, extra;
    for (const auto& item : expected) if (!found.count(item.first)) missing.push_back(item.first);
    for (const auto& item : found) if (!expected.count(item.first)) extra.push_back(item.first);
    if (!missing.empty() || !extra.empty()) {
        auto list = [](const std::vector<std::string>& values) {
            std::string result = "[";
            for (std::size_t i = 0; i < values.size(); ++i) {
                if (i) result += ", ";
                result += "'" + values[i] + "'";
            }
            return result + "]";
        };
        Fail("output path set mismatch: missing=" + list(missing) + ", extra=" + list(extra));
    }
    auto canonicalName = [&](const std::string& key, const std::string& expected,
                             const char* description) -> const fs::path& {
        const auto& path = found.at(key);
        const auto relative = GenericUtf8(path.lexically_relative(output));
        if (relative != expected)
            Fail(std::string(description) + " has incorrect canonical case: " + relative + ", expected " + expected);
        return path;
    };
    const auto& manifestPath = canonicalName("overlay-manifest.json", "overlay-manifest.json", "manifest");
    const auto& checksumsPath = canonicalName("overlay-checksums.txt", "overlay-checksums.txt", "checksums index");
    for (const auto& asset : assets) canonicalName(Lower(asset.path), asset.path, "output");
    auto readFile = [](const fs::path& path, const char* label) {
        std::ifstream stream(path, std::ios::binary);
        if (!stream) Fail("cannot read " + std::string(label) + " " + Utf8(path) + ": cannot open file");
        std::ostringstream bytes;
        bytes << stream.rdbuf();
        if (stream.bad()) Fail("cannot read " + std::string(label) + " " + Utf8(path) + ": read error");
        return bytes.str();
    };
    if (readFile(manifestPath, "manifest") != manifest)
        Fail("manifest is out of date or non-deterministic: " + Utf8(manifestPath));
    if (readFile(checksumsPath, "checksums index") != Checksums(assets))
        Fail("checksums index is out of date or non-deterministic: " + Utf8(checksumsPath));
    for (const auto& asset : assets) {
        const auto& path = found.at(Lower(asset.path));
        const auto [size, hash] = Hash(path);
        if (size != asset.size || hash != asset.hash)
            Fail("output hash mismatch for " + asset.path + ": expected " +
                 std::to_string(asset.size) + "/" + asset.hash + ", found " +
                 std::to_string(size) + "/" + hash);
    }
}
void Materialize(const fs::path& stage, const std::vector<Asset>& assets,
                 const std::string& manifest, const Options& options, Progress* progress) {
    for (const char* root : {"System", "Maps", "Textures", "Sounds", "Music", "Help"}) {
        std::error_code ec;
        fs::create_directory(stage / root, ec);
        if (ec) Fail("cannot create staging directory " + Utf8(stage / root) + ": " + ec.message());
    }
    std::error_code ec;
    for (const auto& asset : assets) {
        if (progress && progress->cancelRequested.load()) Fail("Import cancelled.");
        const fs::path destination = stage / Path(asset.path);
        fs::create_directories(destination.parent_path(), ec);
        if (ec) Fail("cannot materialize " + asset.path + ": " + ec.message());
        if (asset.generated) {
            std::ofstream stream(destination, std::ios::binary | std::ios::out | std::ios::trunc);
            stream.write(asset.content.data(), static_cast<std::streamsize>(asset.content.size()));
            if (!stream) Fail("cannot materialize " + asset.path + ": write failed");
        } else if (options.linkMode != LinkMode::Copy) {
            fs::create_hard_link(asset.disk, destination, ec);
            if (ec && options.linkMode == LinkMode::Hardlink)
                Fail("cannot materialize " + asset.path + ": " + ec.message());
            if (ec) {
                ec.clear();
                fs::copy_file(asset.disk, destination, fs::copy_options::none, ec);
            }
            if (ec) Fail("cannot materialize " + asset.path + ": " + ec.message());
        } else {
            fs::copy_file(asset.disk, destination, fs::copy_options::none, ec);
            if (ec) Fail("cannot materialize " + asset.path + ": " + ec.message());
        }
        if (progress) ++progress->filesDone;
    }
    std::ofstream manifestFile(stage / "overlay-manifest.json", std::ios::binary | std::ios::out | std::ios::trunc);
    std::ofstream checksumsFile(stage / "overlay-checksums.txt", std::ios::binary | std::ios::out | std::ios::trunc);
    const auto checksums = Checksums(assets);
    manifestFile.write(manifest.data(), static_cast<std::streamsize>(manifest.size()));
    checksumsFile.write(checksums.data(), static_cast<std::streamsize>(checksums.size()));
    if (!manifestFile || !checksumsFile) Fail("cannot write overlay identity files");
}
void CommitStage(const fs::path& stage, const fs::path& output) {
    std::error_code ec;
    const auto status = fs::symlink_status(output, ec);
    const bool exists = !ec && status.type() != fs::file_type::not_found;
    if (exists) {
        if (fs::is_symlink(status) || !fs::is_directory(status))
            Fail("refusing to replace non-directory output: " + Utf8(output));
        const fs::path backup = output.parent_path() / ("." + Utf8(output.filename()) + ".old-" + RandomHex());
        fs::rename(output, backup, ec);
        if (ec) Fail("cannot atomically replace " + Utf8(output) + ": " + ec.message());
        fs::rename(stage, output, ec);
        if (ec) {
            const auto reason = ec.message();
            std::error_code restore;
            fs::rename(backup, output, restore);
            Fail("cannot atomically replace " + Utf8(output) + ": " + reason);
        }
        fs::remove_all(backup, ec);
        if (ec) Fail("cannot remove replaced output backup " + Utf8(backup) + ": " + ec.message());
    } else {
        fs::rename(stage, output, ec);
        if (ec) Fail("cannot atomically install " + Utf8(output) + ": " + ec.message());
    }
}
}

bool RunImport(const Options& options, Progress* progress, std::string& Log, std::string& Error) {
    // Scratch space for an extracted archive and/or an expanded installer disc.
    fs::path scratch;
    try {
        if (options.retailRoot.empty() == options.archive.empty())
            Fail("exactly one of --retail-root and --archive is required");
        if (options.output.empty()) Fail("--output is required");
        if (options.profile != Profile::RetailOnly && options.prototypeRoot.empty())
            Fail("--prototype-root is required for safe and full profiles");
        fs::path output = Path(options.output);
        if (!output.is_absolute()) output = fs::absolute(output);
        output = output.lexically_normal();
        std::error_code ec;
        const fs::path parent = output.parent_path();
        if (options.checkOnly) {
            const auto status = fs::symlink_status(parent, ec);
            if (ec || fs::is_symlink(status) || !fs::is_directory(status))
                Fail("output parent does not exist for --check: " + Utf8(parent));
        } else {
            fs::create_directories(parent, ec);
            if (ec) Fail("cannot prepare output parent " + Utf8(parent) + ": " + ec.message());
        }
        const auto resolvedParent = fs::canonical(parent, ec);
        if (ec) Fail("cannot prepare output parent " + Utf8(parent) + ": " + ec.message());
        output = resolvedParent / output.filename();
        fs::path retail;
#ifdef __EMSCRIPTEN__
        if (!options.archive.empty())
            Fail("Archives are not supported in the browser; extract the archive and choose the folder.");
#else
        if (!options.archive.empty()) {
            const auto archive = Path(options.archive);
            if (fs::is_symlink(archive) || !fs::is_regular_file(archive))
                Fail("retail archive is not a regular file: " + Utf8(archive));
            scratch = resolvedParent / (".hp2-import-archive-" + RandomHex());
            fs::create_directory(scratch, ec);
            if (ec) Fail("cannot create archive staging directory " + Utf8(scratch) + ": " + ec.message());
            retail = Detail::Extract(archive, scratch);
        }
#endif
        if (retail.empty()) retail = SafeDirectory(Path(options.retailRoot), "retail root");
        if (Detail::IsInstallerDisc(retail)) {
            if (scratch.empty()) {
                scratch = resolvedParent / (".hp2-import-installer-" + RandomHex());
                fs::create_directory(scratch, ec);
                if (ec) Fail("cannot create installer staging directory " + Utf8(scratch) + ": " + ec.message());
            }
            retail = Detail::ExpandInstaller(retail, scratch / "installed", progress, options.consumeInstallerSource);
        }
        fs::path prototype;
        if (!options.prototypeRoot.empty()) prototype = SafeDirectory(Path(options.prototypeRoot), "prototype root");
        if ((!prototype.empty() && Overlap(output, prototype)) || Overlap(output, retail))
            Fail("output and source trees must not overlap");
        Options normalized = options;
        normalized.prototypeRoot = prototype.empty() ? "" : Utf8(prototype);
        const auto assets = Detail::Plan(normalized, retail, resolvedParent);
        const auto manifest = Detail::Manifest(assets, ProfileName(options.profile));
        if (progress) {
            progress->filesTotal = static_cast<std::int64_t>(assets.size());
            progress->filesDone = 0;
        }
        if (options.checkOnly) {
            ValidateOutput(output, assets, manifest);
            if (!scratch.empty()) fs::remove_all(scratch, ec);
            Log = "Import completed successfully.\n";
            Error.clear();
            return true;
        }
        if (fs::exists(output) || fs::is_symlink(output)) {
            try {
                ValidateOutput(output, assets, manifest);
                if (!scratch.empty()) fs::remove_all(scratch, ec);
                Log = "Import completed successfully.\n";
                Error.clear();
                return true;
            } catch (const Failure&) { }
        }
        const fs::path stage = resolvedParent / ("." + Utf8(output.filename()) + ".tmp-" + RandomHex());
        fs::create_directory(stage, ec);
        if (ec) Fail("cannot create staging directory " + Utf8(stage) + ": " + ec.message());
        try {
            Materialize(stage, assets, manifest, options, progress);
            ValidateOutput(stage, assets, manifest);
            if (progress && progress->cancelRequested.load())
                Fail("Import cancelled.");
            CommitStage(stage, output);
        } catch (...) {
            fs::remove_all(stage, ec);
            throw;
        }
        if (!scratch.empty()) fs::remove_all(scratch, ec);
        Log = "Import completed successfully.\n";
        Error.clear();
        return true;
    } catch (const Failure& failure) {
        std::error_code ignored;
        if (!scratch.empty()) fs::remove_all(scratch, ignored);
        Error = failure.message;
        Log.clear();
        return false;
    } catch (const std::exception& exception) {
        std::error_code ignored;
        if (!scratch.empty()) fs::remove_all(scratch, ignored);
        Error = exception.what();
        Log.clear();
        return false;
    }
}
}

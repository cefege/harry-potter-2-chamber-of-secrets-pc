#include "HP2ImportInternal.h"

namespace HP2Import::Detail {
namespace {
void Insert(std::map<std::string, Asset>& assets, Asset asset) {
    const auto key = Lower(asset.path);
    const auto it = assets.find(key);
    if (it != assets.end())
        Fail("case-insensitive output collision for " + asset.path + ": " + it->second.sourcePath + " and " + asset.sourcePath);
    assets.emplace(key, std::move(asset));
}
std::string Posix(std::string path) { std::replace(path.begin(), path.end(), '\\', '/'); return path; }

std::vector<fs::path> Walk(const fs::path& directory, bool recursive) {
    std::vector<fs::path> result;
    std::vector<fs::path> pending{directory};
    while (!pending.empty()) {
        const auto current = pending.back();
        pending.pop_back();
        std::error_code ec;
        std::vector<fs::path> entries;
        for (fs::directory_iterator it(current, ec), end; it != end && !ec; it.increment(ec))
            entries.push_back(it->path());
        if (ec) Fail("cannot inspect " + Utf8(current) + ": " + ec.message());
        std::sort(entries.begin(), entries.end(), [](const fs::path& a, const fs::path& b) { const auto x = Lower(Utf8(a.filename())), y = Lower(Utf8(b.filename())); return x == y ? Utf8(a.filename()) < Utf8(b.filename()) : x < y; });
        for (const auto& path : entries) {
            const auto status = fs::symlink_status(path, ec);
            if (ec) Fail("cannot inspect " + Utf8(path) + ": " + ec.message());
            if (fs::is_symlink(status)) Fail("symbolic link in input tree: " + Utf8(path));
            if (fs::is_directory(status)) {
                if (recursive) pending.push_back(path);
            } else if (fs::is_regular_file(status)) result.push_back(path);
            else Fail("unsupported input entry: " + Utf8(path));
        }
    }
    std::sort(result.begin(), result.end(), [&](const fs::path& a, const fs::path& b) { const auto x = Lower(Utf8(a.lexically_relative(directory))), y = Lower(Utf8(b.lexically_relative(directory))); return x == y ? Utf8(a.lexically_relative(directory)) < Utf8(b.lexically_relative(directory)) : x < y; });
    return result;
}

Asset SourceAsset(const std::string& source, const fs::path& root, const fs::path& file, const std::string& destination) {
    auto asset = MakeAsset(source, root, file, destination);
    asset.sourcePath = Posix(std::move(asset.sourcePath));
    return asset;
}
}

std::map<std::string, Asset> CollectSide(const fs::path& root, const std::string& source) {
    std::map<std::string, Asset> assets;
    const auto system = Child(root, "System");
    const auto maps = Child(root, "Maps");
    const auto textures = Child(root, "Textures");
    const auto sounds = Child(root, "Sounds");
    const auto music = Child(root, "Music");
    const auto help = Child(root, "Help", false);
    // Destinations are canonical roots, not the spelling found on disk.
    auto collectTo = [&](const fs::path& base, const std::string& destinationRoot, bool recursive, auto accept) {
        for (const auto& file : Walk(base, recursive)) {
            if (!accept(file)) continue;
            Insert(assets, SourceAsset(source, root, file, destinationRoot + "/" + Posix(Utf8(file.lexically_relative(base)))));
        }
    };
    // A Windows install keeps Game.exe, its DLLs and the SECDRV driver beside
    // the packages in System. They are never selected (only .u/.int/.usa
    // below), so they cannot reach the output; the output check also refuses
    // any executable payload.
    collectTo(system, "System", false, [](const fs::path& p) { const auto e = Lower(Utf8(p.extension())); return e == ".u" || e == ".int" || e == ".usa"; });
    collectTo(maps, "Maps", true, [](const fs::path& p) { return Lower(Utf8(p.extension())) == ".unr"; });
    collectTo(textures, "Textures", false, [](const fs::path& p) { return Lower(Utf8(p.extension())) == ".utx"; });
    collectTo(sounds, "Sounds", true, [](const fs::path& p) { const auto n = Lower(Utf8(p.filename())); return Ends(n, ".uax") || Ends(n, "_uax"); });
    collectTo(music, "Music", true, [](const fs::path& p) { return Lower(Utf8(p.extension())) == ".ogg"; });
    if (!help.empty()) collectTo(help, "Help", false, [](const fs::path& p) { return !Debris(p) && !Prohibited(Utf8(p.filename())); });
    return assets;
}

void CollectRetailCutscenes(const fs::path& root, std::map<std::string, Asset>& assets) {
    const auto cutscenes = Child(Child(root, "System"), "cutscenes");
    for (const auto& file : Walk(cutscenes, true)) {
        if (Lower(Utf8(file.extension())) != ".int") {
            if (!Debris(file)) Fail("unexpected retail cutscene payload: " + Utf8(file));
            continue;
        }
        Insert(assets, SourceAsset("retail", root, file, "System/Cutscenes/" + Posix(Utf8(file.lexically_relative(cutscenes)))));
    }
}

void CollectRetailRequiredNestedTextures(const fs::path& root, std::map<std::string, Asset>& assets) {
    const auto oldTextures = Child(Child(root, "Textures"), "Old textures");
    const auto menu = ChildFile(oldTextures, "HP_Menu.utx");
    Insert(assets, SourceAsset("retail", root, menu, "Textures/HP_Menu.utx"));
}

std::map<std::string, Asset> CollectPrototypeRuntimeCutscenes(const fs::path& root) {
    std::map<std::string, Asset> assets;
    const auto cutscenes = Child(Child(root, "System"), "cutscenes");
    for (const auto& file : Walk(cutscenes, true)) {
        if (Lower(Utf8(file.extension())) != ".int") {
            if (!Debris(file)) Fail("unexpected prototype runtime cutscene payload: " + Utf8(file));
            continue;
        }
        Insert(assets, SourceAsset("prototype", root, file, "System/Cutscenes/" + Posix(Utf8(file.lexically_relative(cutscenes)))));
    }
    return assets;
}

std::map<std::string, Asset> CollectPrototypeCutsceneReferences(const fs::path& root) {
    std::map<std::string, Asset> assets;
    const auto cutscenes = Child(root, "CutScenes", false);
    if (cutscenes.empty()) return assets;
    for (const auto& file : Walk(cutscenes, true)) {
        if (Lower(Utf8(file.extension())) != ".txt") continue;
        auto relative = file.lexically_relative(cutscenes);
        relative.replace_extension(".int");
        Insert(assets, SourceAsset("prototype", root, file, "System/Cutscenes/" + Posix(Utf8(relative))));
    }
    return assets;
}
}

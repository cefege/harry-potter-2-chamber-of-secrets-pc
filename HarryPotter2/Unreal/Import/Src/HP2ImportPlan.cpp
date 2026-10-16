#include "HP2ImportInternal.h"
#include "hp2_rs.h"
#include <cstdint>
#include <random>

namespace HP2Import::Detail {
std::map<std::string, Asset> CollectSide(const fs::path&, const std::string&);
void CollectRetailCutscenes(const fs::path&, std::map<std::string, Asset>&);
void CollectRetailRequiredNestedTextures(const fs::path&, std::map<std::string, Asset>&);
std::map<std::string, Asset> CollectPrototypeRuntimeCutscenes(const fs::path&);
std::map<std::string, Asset> CollectPrototypeCutsceneReferences(const fs::path&);

namespace {
void IniResultSink(void* ctx, const uint8_t* text, size_t len) {
    static_cast<std::string*>(ctx)->assign(reinterpret_cast<const char*>(text), len);
}
}

// Replaces HP2ImportIni.cpp (deleted); the ini parse/write logic is now
// crates/hp2-ini, called through the hp2-ffi bridge.
std::string BuildDefaultIni(const fs::path& templatePath, const fs::path& retailPath, Profile profile) {
    const std::string tpl = Utf8(templatePath), ret = Utf8(retailPath);
    std::string out;
    const bool ok = hp2_rs_ini_build_default(
        reinterpret_cast<const uint8_t*>(tpl.data()), tpl.size(),
        reinterpret_cast<const uint8_t*>(ret.data()), ret.size(),
        static_cast<uint32_t>(profile), &IniResultSink, &out);
    if (!ok) Fail(out);
    return out;
}

std::string BuildDefUserIni(const fs::path& templatePath, const fs::path& retailPath, Profile profile) {
    const std::string tpl = Utf8(templatePath), ret = Utf8(retailPath);
    std::string out;
    const bool ok = hp2_rs_ini_build_def_user(
        reinterpret_cast<const uint8_t*>(tpl.data()), tpl.size(),
        reinterpret_cast<const uint8_t*>(ret.data()), ret.size(),
        static_cast<uint32_t>(profile), &IniResultSink, &out);
    if (!ok) Fail(out);
    return out;
}

namespace {
constexpr const char* CriticalRetailPaths[] = {
    "System/Core.u", "System/Editor.u", "System/Engine.u", "System/Fire.u", "System/HGame.u", "System/HPModels.u",
    "System/HPParticle.u", "System/HProps.u", "System/HPSounds.u", "System/UnrealShare.u", "System/UWindow.u",
    "Textures/HP2_Master.utx", "Textures/HP2_Menu.utx", "Sounds/AllDialog.USA_uax"
};
constexpr const char* RetailOnlyRequiredPaths[] = {"Maps/Entry.unr", "Textures/HP_Menu.utx"};
constexpr const char* RetailFinalLocalization[] = {
    "System/BumpDialog.int", "System/BumpSet.int", "System/Credits.int", "System/Duelset.int", "System/hpcredits.int",
    "System/hpdialog.int", "System/HPMenu.int", "System/QuidSet.int", "System/BumpDialog.usa", "System/hpcredits.usa",
    "System/HpDialog.usa", "System/HpMenu.usa"
};
std::string Key(const std::string& s) { return Lower(s); }
Asset Generated(const std::string& path, const std::string& content, std::vector<std::string> sources, const std::string& classification) {
    Asset a;
    a.source = "generated";
    a.path = path;
    a.classification = classification;
    a.content = content;
    a.generated = true;
    a.size = content.size();
    CC_SHA256_CTX ctx;
    unsigned char digest[32];
    CC_SHA256_Init(&ctx);
    CC_SHA256_Update(&ctx, content.data(), static_cast<CC_LONG>(content.size()));
    CC_SHA256_Final(digest, &ctx);
    a.hash = Hex(digest, sizeof(digest));
    a.sources = std::move(sources);
    a.link = "generated";
    return a;
}
std::string ResolveLinkMode(const Asset& asset, const fs::path& outputParent, LinkMode requested) {
    if (asset.generated) return "generated";
    if (requested == LinkMode::Copy) return "copy";
    static constexpr char digits[] = "0123456789abcdef";
    std::random_device random;
    std::string probeName = ".hp2-link-probe-";
    for (unsigned i = 0; i < 32; ++i) probeName += digits[random() & 15];
    const fs::path probe = outputParent / Path(probeName);
    std::error_code ec;
    fs::create_hard_link(asset.disk, probe, ec);
    if (!ec) {
        fs::remove(probe, ec);
        if (ec) Fail("cannot remove link-mode probe " + Utf8(probe) + ": " + ec.message());
        return "hardlink";
    }
    if (ec == std::errc::cross_device_link) {
        if (requested == LinkMode::Hardlink)
            Fail("cannot hardlink across filesystems: " + Utf8(asset.disk) + " -> " + Utf8(outputParent));
        return "copy";
    }
    return "hardlink";
}
void AddRequired(const std::map<std::string, Asset>& exact, const std::string& path, const std::string& source) {
    auto it = exact.find(path);
    if (it == exact.end()) {
        for (const auto& item : exact) if (Key(item.first) == Key(path)) Fail("payload has incorrect canonical case: " + item.first + ", expected " + path);
        Fail("missing required payload: " + path);
    }
    if (it->second.source != source) Fail(path + " must be " + source + "-owned, found " + it->second.source);
}
void ValidatePlan(const std::vector<Asset>& plan, Profile profile) {
    std::map<std::string, Asset> exact;
    std::map<std::string, Asset> folded;
    for (const auto& asset : plan) {
        auto foldedKey = Key(asset.path);
        auto prior = folded.find(foldedKey);
        if (prior != folded.end()) Fail("case-insensitive planned output collision: " + prior->second.path + " and " + asset.path);
        folded.emplace(foldedKey, asset);
        const auto slash = asset.path.find('/');
        const auto root = slash == std::string::npos ? asset.path : asset.path.substr(0, slash);
        if ((root != "System" && root != "Maps" && root != "Textures" && root != "Sounds" && root != "Music" && root != "Help") || Prohibited(asset.path))
            Fail("invalid planned output path: " + asset.path);
        exact.emplace(asset.path, asset);
    }
    auto count = [&](const std::string& source, const std::string& prefix, const std::string& suffix) {
        std::size_t n = 0;
        for (const auto& a : plan) if (a.source == source && Starts(Key(a.path), Key(prefix)) && Ends(Key(a.path), Key(suffix))) ++n;
        return n;
    };
    const auto retailMaps = count("retail", "Maps/", ".unr");
    const auto prototypeMaps = count("prototype", "Maps/", ".unr");
    const auto retailOggs = count("retail", "Music/", ".ogg");
    if (retailOggs != 141) Fail("retail Ogg contract mismatch: expected 141, found " + std::to_string(retailOggs));
    if (profile == Profile::Full) {
        const auto cutscenes = count("retail", "System/Cutscenes/", ".int");
        if (retailMaps != 42 || prototypeMaps != 58) Fail("full map contract mismatch: retail=" + std::to_string(retailMaps) + ", prototype-only=" + std::to_string(prototypeMaps));
        for (const auto& a : plan) if (a.source == "prototype" && Starts(Key(a.path), "maps/") && Ends(Key(a.path), ".unr") && a.classification != "prototype_only") Fail("a full-profile prototype fallback map is not classified prototype_only");
        if (cutscenes != 214) Fail("full retail cutscene contract mismatch: expected 214, found " + std::to_string(cutscenes));
        for (const auto& path : CriticalRetailPaths) AddRequired(exact, path, "retail");
        for (const auto& path : RetailFinalLocalization) AddRequired(exact, path, "retail");
    } else if (profile == Profile::RetailOnly) {
        const auto cutscenes = count("retail", "System/Cutscenes/", ".int");
        if (retailMaps != 42 || prototypeMaps != 0) Fail("retail-only map contract mismatch: retail=" + std::to_string(retailMaps) + ", prototype=" + std::to_string(prototypeMaps));
        if (cutscenes != 213 && cutscenes != 214) Fail("retail-only cutscene contract mismatch: expected 213 or 214, found " + std::to_string(cutscenes));
        for (const auto& a : plan) {
            if (a.source != "retail" && a.source != "generated") Fail("retail-only payload has forbidden source " + a.source + ": " + a.path);
            if (a.source == "retail" && a.classification != "retail_asset") Fail("retail-only asset has invalid classification: " + a.path);
            if (a.source == "generated") {
                if (a.path != "System/Default.ini" && a.path != "System/DefUser.ini") Fail("unexpected generated retail-only payload: " + a.path);
                if (a.classification != "generated_retail_native") Fail("generated config is not marked retail-native: " + a.path);
                for (const auto& source : a.sources) if (!Starts(source, "retail:")) Fail("generated config has a non-retail template: " + a.path);
            }
        }
        for (const auto& path : CriticalRetailPaths) AddRequired(exact, path, "retail");
        for (const auto& path : RetailFinalLocalization) AddRequired(exact, path, "retail");
        for (const auto& path : RetailOnlyRequiredPaths) AddRequired(exact, path, "retail");
        AddRequired(exact, "System/Default.ini", "generated");
        AddRequired(exact, "System/DefUser.ini", "generated");
        // Config values are validated by the builder; check non-empty runtime category payloads here.
        for (const auto& root : {"System", "Maps", "Textures", "Sounds", "Music"}) {
            bool found = false;
            for (const auto& a : plan) if (a.source == "retail" && Starts(a.path, std::string(root) + "/")) { found = true; break; }
            if (!found) Fail("configured retail-only Paths category has no payload: " + std::string(root));
        }
        if (!retailMaps || !retailOggs) Fail("retail-only requires at least one map and one music file");
    } else {
        const auto cutscenes = count("prototype", "System/Cutscenes/", ".int");
        if (retailMaps != 0 || prototypeMaps != 100) Fail("safe map contract mismatch: retail=" + std::to_string(retailMaps) + ", prototype=" + std::to_string(prototypeMaps));
        for (const auto& a : plan) if (a.source == "prototype" && Starts(Key(a.path), "maps/") && Ends(Key(a.path), ".unr") && a.classification != "prototype_baseline") Fail("a safe-profile map is not classified prototype_baseline");
        if (cutscenes != 185) Fail("safe prototype cutscene contract mismatch: expected 185, found " + std::to_string(cutscenes));
        for (const auto& a : plan) if (a.source == "retail" && (Starts(Key(a.path), "system/") || Starts(Key(a.path), "maps/") || Starts(Key(a.path), "sounds/") || Starts(Key(a.path), "help/"))) Fail("safe profile contains retail runtime packages, maps, sounds, localization, or Help");
        std::size_t prototypeTextures = 0;
        std::vector<std::string> retailTextures;
        for (const auto& a : plan) {
            if (a.source == "prototype" && Starts(Key(a.path), "textures/")) ++prototypeTextures;
            if (a.source == "retail" && Starts(Key(a.path), "textures/")) retailTextures.push_back(a.path);
        }
        if (prototypeTextures != 52 || retailTextures != std::vector<std::string>{"Textures/HP2_Menu.utx"})
            Fail("safe texture contract mismatch: prototype=" + std::to_string(prototypeTextures) + ", retail=[" + (retailTextures.empty() ? "" : retailTextures.front()) + "]");
        for (const auto& path : {"System/Core.u", "System/Editor.u", "System/Engine.u", "System/Fire.u", "System/HGame.u", "System/HPModels.u", "System/HPParticle.u", "System/HProps.u", "System/HPSounds.u", "System/UnrealShare.u", "System/UWindow.u", "Textures/HP2_Master.utx", "Sounds/AllDialog.uax"}) AddRequired(exact, path, "prototype");
        AddRequired(exact, "Textures/HP2_Menu.utx", "retail");
        for (const auto& item : exact) if (item.first == "Sounds/AllDialog.USA_uax" || Ends(Key(item.first), ".usa")) Fail("safe profile must use prototype INT/AllDialog localization, not retail USA");
    }
}
}

std::vector<Asset> Plan(const Options& options, const fs::path& retailRoot, const fs::path& outputParent) {
    (void)outputParent;
    auto retail = CollectSide(retailRoot, "retail");
    std::vector<Asset> plan;
    if (options.profile == Profile::RetailOnly) {
        CollectRetailCutscenes(retailRoot, retail);
        CollectRetailRequiredNestedTextures(retailRoot, retail);
        std::map<std::string, std::string> canonicalPaths;
        for (const auto& path : CriticalRetailPaths) canonicalPaths.emplace(Key(path), path);
        for (const auto& path : RetailFinalLocalization) canonicalPaths.emplace(Key(path), path);
        for (const auto& path : RetailOnlyRequiredPaths) canonicalPaths.emplace(Key(path), path);
        for (auto& item : retail) {
            auto asset = std::move(item.second);
            const auto canonical = canonicalPaths.find(Key(asset.path));
            if (canonical != canonicalPaths.end()) asset.path = canonical->second;
            asset.classification = "retail_asset";
            plan.push_back(std::move(asset));
        }
    } else {
        if (options.prototypeRoot.empty()) Fail("--prototype-root is required for safe and full profiles");
        const auto prototypeRoot = Path(options.prototypeRoot);
        auto prototype = CollectSide(prototypeRoot, "prototype");
        auto refs = CollectPrototypeCutsceneReferences(prototypeRoot);
        auto runtime = CollectPrototypeRuntimeCutscenes(prototypeRoot);
        if (options.profile == Profile::Full) {
            CollectRetailCutscenes(retailRoot, retail);
            auto comparison = prototype;
            for (const auto& item : refs) if (comparison.find(item.first) == comparison.end()) comparison.emplace(item.first, item.second);
            for (const auto& key : [&] { std::set<std::string> keys; for (const auto& x : prototype) keys.insert(x.first); for (const auto& x : retail) keys.insert(x.first); return keys; }()) {
                const auto r = retail.find(key);
                const auto p = prototype.find(key);
                if (r != retail.end()) {
                    const auto c = comparison.find(key);
                    Asset asset = r->second;
                    asset.path = c == comparison.end() ? r->second.path : c->second.path;
                    asset.classification = c == comparison.end() ? "retail_only" : c->second.hash == r->second.hash ? "identical" : "changed";
                    plan.push_back(std::move(asset));
                } else if (p != prototype.end()) {
                    Asset asset = p->second;
                    asset.classification = "prototype_only";
                    plan.push_back(std::move(asset));
                }
            }
        } else {
            for (auto& item : prototype) if (!Starts(Key(item.second.path), "music/")) { item.second.classification = "prototype_baseline"; plan.push_back(item.second); }
            for (auto& item : runtime) { item.second.classification = "prototype_baseline"; plan.push_back(std::move(item.second)); }
            for (auto& item : retail) {
                auto& asset = item.second;
                if (!Starts(Key(asset.path), "music/")) continue;
                const auto comparison = prototype.find(item.first);
                asset.classification = comparison == prototype.end() ? "retail_only" : comparison->second.hash == asset.hash ? "identical" : "changed";
                if (comparison != prototype.end()) asset.path = comparison->second.path;
                plan.push_back(std::move(asset));
            }
            auto menu = retail.find("textures/hp2_menu.utx");
            if (menu == retail.end()) Fail("safe profile requires retail Textures/HP2_Menu.utx");
            menu->second.path = "Textures/HP2_Menu.utx";
            menu->second.classification = "retail_only";
            plan.push_back(menu->second);
        }
    }

    const auto retailSystem = Child(retailRoot, "System");
    const auto retailDefault = ChildFile(retailSystem, "Default.ini");
    const auto retailDefuser = ChildFile(retailSystem, "DefUser.ini");
    fs::path defaultTemplate = retailDefault, defuserTemplate = retailDefuser;
    std::string configClass;
    std::vector<std::string> defaultSources, defuserSources;
    if (options.profile == Profile::RetailOnly) {
        configClass = "generated_retail_native";
        defaultSources = {"retail:" + Utf8(retailDefault.lexically_relative(retailRoot))};
        defuserSources = {"retail:" + Utf8(retailDefuser.lexically_relative(retailRoot))};
    } else {
        const auto prototypeSystem = Child(Path(options.prototypeRoot), "System");
        defaultTemplate = ChildFile(prototypeSystem, "Default.ini");
        defuserTemplate = ChildFile(prototypeSystem, "DefUser.ini");
        configClass = options.profile == Profile::Full ? "generated_full_merge" : "generated_safe_native";
        defaultSources = options.profile == Profile::Full ? std::vector<std::string>{"prototype:System/Default.ini", "retail:System/default.ini"} : std::vector<std::string>{"prototype:System/Default.ini"};
        defuserSources = options.profile == Profile::Full ? std::vector<std::string>{"prototype:System/DefUser.ini", "retail:System/DefUser.ini"} : std::vector<std::string>{"prototype:System/DefUser.ini"};
    }
    plan.push_back(Generated("System/Default.ini", BuildDefaultIni(defaultTemplate, retailDefault, options.profile), std::move(defaultSources), configClass));
    plan.push_back(Generated("System/DefUser.ini", BuildDefUserIni(defuserTemplate, retailDefuser, options.profile), std::move(defuserSources), configClass));
    for (auto& asset : plan) {
        asset.link = ResolveLinkMode(asset, outputParent, options.linkMode);
        if (asset.classification.empty()) asset.classification = asset.source == "retail" ? "retail_asset" : "prototype_baseline";
        if (Prohibited(asset.path)) Fail("invalid planned output path: " + asset.path);
    }
    std::sort(plan.begin(), plan.end(), [](const Asset& a, const Asset& b) { const auto x = Key(a.path), y = Key(b.path); return x == y ? a.path < b.path : x < y; });
    ValidatePlan(plan, options.profile);
    return plan;
}
}

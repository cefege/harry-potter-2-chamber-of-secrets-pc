#include "HP2ImportInternal.h"
#include <variant>

namespace HP2Import::Detail {
namespace {
struct Json {
    using Array = std::vector<Json>;
    using Object = std::map<std::string, Json>;
    std::variant<std::nullptr_t, bool, std::int64_t, std::string, Array, Object> value;
    Json(const char* v) : value(std::string(v)) {}
    Json(std::string v) : value(std::move(v)) {}
    Json(std::int64_t v) : value(v) {}
    Json(Array v) : value(std::move(v)) {}
    Json(Object v) : value(std::move(v)) {}
};
void Quote(std::string& out, const std::string& value) {
    static constexpr char hex[] = "0123456789abcdef";
    out += '"';
    for (unsigned char c : value) {
        switch (c) {
        case '"': out += "\\\""; break;
        case '\\': out += "\\\\"; break;
        case '\n': out += "\\n"; break;
        case '\r': out += "\\r"; break;
        case '\t': out += "\\t"; break;
        case '\b': out += "\\b"; break;
        case '\f': out += "\\f"; break;
        default:
            if (c < 0x20) {
                out += "\\u00";
                out += hex[c >> 4];
                out += hex[c & 15];
            } else out += static_cast<char>(c);
        }
    }
    out += '"';
}
void WriteJson(std::string& out, const Json& json, unsigned depth) {
    const auto indent = [&](unsigned n) { out.append(n * 2, ' '); };
    if (auto s = std::get_if<std::string>(&json.value)) { Quote(out, *s); return; }
    if (auto n = std::get_if<std::int64_t>(&json.value)) { out += std::to_string(*n); return; }
    if (auto b = std::get_if<bool>(&json.value)) { out += *b ? "true" : "false"; return; }
    if (std::holds_alternative<std::nullptr_t>(json.value)) { out += "null"; return; }
    if (auto array = std::get_if<Json::Array>(&json.value)) {
        if (array->empty()) { out += "[]"; return; }
        out += "[\n";
        for (std::size_t i = 0; i < array->size(); ++i) {
            indent(depth + 1); WriteJson(out, (*array)[i], depth + 1);
            out += i + 1 == array->size() ? "\n" : ",\n";
        }
        indent(depth); out += ']'; return;
    }
    const auto& object = std::get<Json::Object>(json.value);
    if (object.empty()) { out += "{}"; return; }
    out += "{\n";
    std::size_t i = 0;
    for (const auto& [key, value] : object) {
        indent(depth + 1); Quote(out, key); out += ": ";
        WriteJson(out, value, depth + 1);
        out += ++i == object.size() ? "\n" : ",\n";
    }
    indent(depth); out += '}';
}
using Strings = std::vector<std::string>;
Json StringArray(const Strings& values) {
    Json::Array array;
    for (const auto& value : values) array.emplace_back(value);
    return Json(std::move(array));
}
Json Counts(const std::vector<Asset>& assets, const std::string Asset::*field) {
    std::map<std::string, std::int64_t> counts;
    for (const auto& asset : assets) ++counts[asset.*field];
    Json::Object object;
    for (const auto& [key, count] : counts) object.emplace(key, Json(count));
    return Json(std::move(object));
}
}

std::string Manifest(const std::vector<Asset>& assets, const std::string& profile) {
    static const Strings safeDefaults{
        "base: prototype System/Default.ini",
        "Engine.Engine: WgpuDrv, SDLDrv, OpenAL, prototype Language=int",
        "Engine.GameEngine: FrameRateLimit=144, UseAA/NumAASamples/MaxAnisotropy maxed, and no unavailable ServerActors",
        "Core.System: forward-slash portable relative data paths",
        "SDLDrv.SDLClient: native portable client defaults",
        "WgpuDrv.WgpuRenderDevice: wgpu feature set"};
    static const Strings safeDefuser{
        "base: prototype System/DefUser.ini with Modern controls disabled; prototype bindings retained",
        "Engine.PlayerPawn: forced ObjectDetailVeryHigh (max-quality first-run default)"};
    static const Strings fullDefaults{
        "base: prototype System/Default.ini",
        "FrontEnd: retail EASplashWaitTime, WBSplashWaitTime, LaunchCode, UseSaveSlot",
        "Engine.Engine: WgpuDrv, SDLDrv, OpenAL, Language=usa",
        "Engine.GameEngine: FrameRateLimit=144, UseAA/NumAASamples/MaxAnisotropy maxed, and no unavailable ServerActors",
        "Core.System: forward-slash portable relative data paths",
        "SDLDrv.SDLClient: native portable client defaults",
        "WgpuDrv.WgpuRenderDevice: wgpu feature set",
        "ALAudio.ALAudioSubsystem: retail final MusicVolume=0.53",
        "HGame console: retail final debug mode disabled"};
    static const Strings fullDefuser{
        "base: prototype System/DefUser.ini, retaining prototype controller fallback",
        "Engine.Input: retail final keyboard/mouse broom, map, cutscene, potion, and duel bindings",
        "Engine.PlayerPawn: ObjectDetailVeryHigh (max-quality first-run default, overriding retail's own Medium) and Modern controls disabled by default",
        "HGame.Harry: retail final difficulty damage multipliers"};
    static const Strings retailDefaults{
        "base: retail System/default.ini; no prototype bytes",
        "FrontEnd: retail final splash, launch, and save-slot values",
        "Engine.Engine: WgpuDrv, SDLDrv, OpenAL, Language=usa",
        "Engine.GameEngine: FrameRateLimit=144, UseAA/NumAASamples/MaxAnisotropy maxed, and no unavailable ServerActors",
        "Core.System: forward-slash portable relative data paths",
        "SDLDrv.SDLClient: native portable client defaults",
        "WgpuDrv.WgpuRenderDevice: wgpu feature set",
        "ALAudio.ALAudioSubsystem: retail final MusicVolume=0.53",
        "HGame console: retail final debug mode disabled"};
    static const Strings retailDefuser{
        "base: retail System/DefUser.ini; no prototype bytes",
        "Engine.Input: retail final keyboard/mouse broom, map, cutscene, potion, and duel bindings",
        "Engine.PlayerPawn: ObjectDetailVeryHigh (max-quality first-run default, overriding retail's own Medium) and Modern controls disabled by default",
        "HGame.Harry: retail final difficulty damage multipliers"};
    const bool retailOnly = profile == "retail-only";
    const bool full = profile == "full";
    const Strings& defaults = retailOnly ? retailDefaults : full ? fullDefaults : safeDefaults;
    const Strings& defuser = retailOnly ? retailDefuser : full ? fullDefuser : safeDefuser;
    Json::Array entries;
    std::map<std::string, std::int64_t> roots;
    for (const auto& asset : assets) {
        Json::Object entry{{"classification", Json(asset.classification)},
                           {"link_mode", Json(asset.link)},
                           {"path", Json(asset.path)},
                           {"sha256", Json(asset.hash)},
                           {"size", Json(static_cast<std::int64_t>(asset.size))},
                           {"source", Json(asset.source)}};
        if (!asset.sourcePath.empty()) entry.emplace("source_path", Json(asset.sourcePath));
        if (!asset.sources.empty()) entry.emplace("source_paths", StringArray(asset.sources));
        entries.emplace_back(std::move(entry));
        const auto slash = asset.path.find('/');
        ++roots[asset.path.substr(0, slash)];
    }
    Json::Object rootCounts;
    for (const auto& [key, count] : roots) rootCounts.emplace(key, Json(count));
    Json::Object summary{{"classification", Counts(assets, &Asset::classification)},
                         {"entry_count", Json(static_cast<std::int64_t>(assets.size()))},
                         {"link_mode", Counts(assets, &Asset::link)},
                         {"root", Json(std::move(rootCounts))},
                         {"source", Counts(assets, &Asset::source)}};
    Json::Object policy;
    if (full) {
        policy = {{"compatibility", Json("candidate retail class/package surface; requires complete retail native parity")},
                  {"cutscenes", Json("complete retail System/Cutscenes bundle; no prototype raw TXT mixing")},
                  {"help", Json("canonical outer Help only; nested help/help excluded")},
                  {"localization", Json("retail coherent USA bundle with prototype-only INT fallback")},
                  {"precedence", Json("retail changed/identical/retail-only runtime data, then prototype-only fallback")}};
    } else if (retailOnly) {
        policy = {{"compatibility", Json("final retail package/map surface with no prototype fallback")},
                  {"cutscenes", Json("complete retail System/Cutscenes bundle")},
                  {"help", Json("canonical outer retail Help only; nested help/help excluded")},
                  {"localization", Json("coherent retail Language=usa and AllDialog.USA_uax bundle")},
                  {"precedence", Json("retail files only; portable configs are generated from retail templates")}};
    } else {
        policy = {{"compatibility", Json("native-smokeable prototype class/package baseline")},
                  {"cutscenes", Json("compiled prototype System/Cutscenes INT runtime bundle")},
                  {"help", Json("prototype canonical outer Help only")},
                  {"localization", Json("coherent prototype Language=int and AllDialog.uax bundle")},
                  {"precedence", Json("prototype runtime data; retail Music Oggs and retail-only HP2_Menu.utx only")}};
    }
    policy.emplace("prohibited", Json("Windows binaries/installers/scripts, source-control metadata, and debris"));
    Json::Object manifest{{"config_merges", Json(Json::Object{{"System/Default.ini", StringArray(defaults)},
                                                                {"System/DefUser.ini", StringArray(defuser)}})},
                          {"entries", Json(std::move(entries))},
                          {"format", Json("hp2-retail-data-overlay")},
                          {"policy", Json(std::move(policy))},
                          {"profile", Json(profile)},
                          {"schema_version", Json(static_cast<std::int64_t>(retailOnly ? 3 : 2))},
                          {"summary", Json(std::move(summary))}};
    if (retailOnly) {
        Strings generated;
        std::int64_t prototypeAssetCount = 0, prototypeTemplateCount = 0;
        for (const auto& asset : assets) {
            if (asset.source == "generated") generated.push_back(asset.path);
            if (asset.source == "prototype") ++prototypeAssetCount;
            for (const auto& path : asset.sources)
                if (Starts(path, "prototype:")) { ++prototypeTemplateCount; break; }
        }
        manifest.emplace("provenance", Json(Json::Object{
            {"asset_byte_sources", StringArray({"retail"})},
            {"generated_entries", StringArray(generated)},
            {"manifest", Json("generated deterministically from the validated retail-only plan")},
            {"prototype_asset_entry_count", Json(prototypeAssetCount)},
            {"prototype_template_input_count", Json(prototypeTemplateCount)}}));
    }
    std::string output;
    WriteJson(output, Json(std::move(manifest)), 0);
    output += '\n';
    return output;
}

std::string Checksums(const std::vector<Asset>& assets) {
    std::vector<const Asset*> rows;
    rows.reserve(assets.size());
    for (const auto& asset : assets) rows.push_back(&asset);
    std::sort(rows.begin(), rows.end(), [](const Asset* a, const Asset* b) { return a->path < b->path; });
    std::string output;
    for (const auto* asset : rows)
        output += asset->path + "\t" + std::to_string(asset->size) + "\t" + asset->hash + "\n";
    return output;
}
}

#pragma once

#include <atomic>
#include <cstdint>
#include <string>

namespace HP2Import {

enum class Profile { Safe, Full, RetailOnly };
enum class LinkMode { Auto, Copy, Hardlink };
struct Options {
    std::string retailRoot;
    std::string archive;
    std::string prototypeRoot;
    std::string output;
    Profile profile = Profile::Safe;
    LinkMode linkMode = LinkMode::Auto;
    bool checkOnly = false;
    // retailRoot may be a game installer disc (setup/data1.hdr); it is
    // expanded first. Set when retailRoot is disposable scratch (the browser's
    // copy of the disc) so its cabinets are freed as soon as they are unpacked.
    bool consumeInstallerSource = false;
};
struct Progress {
    std::atomic<std::int64_t> filesDone{0}, filesTotal{0};
    std::atomic<bool> cancelRequested{false};
};
bool RunImport(const Options& options, Progress* progress, std::string& Log, std::string& Error);

}

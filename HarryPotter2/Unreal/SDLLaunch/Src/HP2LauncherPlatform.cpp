#include "HP2LauncherPlatform.h"

#include <SDL.h>
#if !defined(__EMSCRIPTEN__)
#include <nfd.h>
#endif

#include <filesystem>
#include <string>

#if defined(_WIN32)
#include <windows.h>
#include <shellapi.h>
#elif defined(__linux__)
#include <spawn.h>
extern char** environ;
#endif

namespace HP2LauncherPlatform
{
#if !defined(__EMSCRIPTEN__)
namespace
{
class FNfdSession
{
public:
	FNfdSession() : Ready(NFD_Init() == NFD_OKAY)
	{
		if (!Ready) SDL_Log("HP2Launcher: native file dialog initialization failed: %s", NFD_GetError());
	}
	~FNfdSession() { if (Ready) NFD_Quit(); }
	bool IsReady() const { return Ready; }
private:
	bool Ready;
};
}
#endif

#if !MACOSX
bool IsLauncherShortcutHeld() { return false; }
#endif

HP2Launcher::LauncherCapabilities CurrentCapabilities()
{
	HP2Launcher::LauncherCapabilities result;
#if defined(__EMSCRIPTEN__)
	result.screenModes = false;
	result.resolutions = false;
	result.rendererChoice = false;
	result.revealPaths = false;
	result.nativeFolderPicker = false;
#endif
	return result;
}

bool PickFolder(std::string& Path)
{
#if defined(__EMSCRIPTEN__)
	(void)Path;
	return false;
#else
	FNfdSession session;
	if (!session.IsReady()) return false;
	nfdu8char_t* selected = nullptr;
	const nfdresult_t result = NFD_PickFolderU8(&selected, nullptr);
	if (result == NFD_OKAY && selected)
	{
		Path.assign(selected);
		NFD_FreePathU8(selected);
		return true;
	}
	if (result == NFD_ERROR) SDL_Log("HP2Launcher: folder picker failed: %s", NFD_GetError());
	return false;
#endif
}

bool PickFile(std::string& Path, const std::vector<std::string>& Extensions)
{
#if defined(__EMSCRIPTEN__)
	(void)Path;
	(void)Extensions;
	return false;
#else
	std::string specification;
	for (const std::string& extension : Extensions)
	{
		if (extension.empty()) continue;
		if (!specification.empty()) specification.push_back(',');
		specification += extension;
	}
	const nfdu8filteritem_t filter = {"Retail game archives", specification.c_str()};
	FNfdSession session;
	if (!session.IsReady()) return false;
	nfdu8char_t* selected = nullptr;
	const nfdresult_t result = NFD_OpenDialogU8(&selected,
		specification.empty() ? nullptr : &filter,
		specification.empty() ? 0U : 1U, nullptr);
	if (result == NFD_OKAY && selected)
	{
		Path.assign(selected);
		NFD_FreePathU8(selected);
		return true;
	}
	if (result == NFD_ERROR) SDL_Log("HP2Launcher: file picker failed: %s", NFD_GetError());
	return false;
#endif
}

#if !defined(__APPLE__)

void RevealPath(const std::string& Path)
{
	if (Path.empty()) return;
#if defined(__EMSCRIPTEN__)
	(void)Path;
#elif defined(_WIN32)
	const std::filesystem::path native = std::filesystem::u8path(Path);
	ShellExecuteW(nullptr, L"open", native.c_str(), nullptr, nullptr, SW_SHOWNORMAL);
#elif defined(__linux__)
	pid_t child = 0;
	char* const arguments[] = {const_cast<char*>("xdg-open"), const_cast<char*>(Path.c_str()), nullptr};
	const int status = posix_spawnp(&child, "xdg-open", nullptr, nullptr, arguments, environ);
	if (status != 0) SDL_Log("HP2Launcher: could not start xdg-open for %s (error %d)", Path.c_str(), status);
#else
	(void)Path;
#endif
}

#endif
}

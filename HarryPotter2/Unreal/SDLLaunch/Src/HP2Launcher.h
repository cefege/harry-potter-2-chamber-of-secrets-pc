/*=============================================================================
	HP2Launcher.h: Platform-neutral in-engine launcher session.
=============================================================================*/
#ifndef HP2LAUNCHER_H
#define HP2LAUNCHER_H

#include "HP2LauncherModel.h"

#include <memory>
#include <string>

namespace HP2Launcher
{
struct LauncherCapabilities
{
	bool screenModes = true;
	bool resolutions = true;
	bool revealPaths = true;
	bool nativeFolderPicker = true;
};

class FLauncherSession
{
public:
	FLauncherSession(const LauncherRequest& request, const LauncherCapabilities& capabilities);
	~FLauncherSession();
	FLauncherSession(const FLauncherSession&) = delete;
	FLauncherSession& operator=(const FLauncherSession&) = delete;

	bool Open(std::string& Error);
	bool Frame();
	const LauncherResult& Result() const;

private:
	struct Impl;
	std::unique_ptr<Impl> State;
};

bool RunLauncher(const LauncherRequest& Request, LauncherResult& Result, std::string& Error);
}

#endif

#pragma once

#include "HP2LauncherModel.h"

#include <string>

namespace HP2Launcher
{
	// Run the Quickshell-based launcher on Linux, mirroring the macOS Cocoa launcher's
	// contract. Runs synchronously; returns the action taken and updates result with
	// parsed launcher settings and save selection.
	LaunchAction RunHP2ShellLauncher(
		const LauncherRequest& request,
		LauncherResult& result,
		std::string& error);

	// Records argv[0] so the launcher can locate assets installed next to the
	// binary (the Quickshell QML and the bundled importer). Called once from
	// main() before the chooser runs.
	void SetHP2ShellLauncherExecutablePath(const char* Invoked);

	// Platform-neutral dispatcher forwarder
	inline LaunchAction RunHP2NativeLauncher(
		const LauncherRequest& request,
		LauncherResult& result,
		std::string& error)
	{
		return RunHP2ShellLauncher(request, result, error);
	}
}

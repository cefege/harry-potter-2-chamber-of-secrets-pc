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

	// Platform-neutral dispatcher forwarder
	inline LaunchAction RunHP2NativeLauncher(
		const LauncherRequest& request,
		LauncherResult& result,
		std::string& error)
	{
		return RunHP2ShellLauncher(request, result, error);
	}
}

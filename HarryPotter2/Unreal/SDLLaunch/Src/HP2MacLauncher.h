/*=============================================================================
	HP2MacLauncher.h: Native macOS launcher bridge.
=============================================================================*/
#ifndef HP2MACLAUNCHER_H
#define HP2MACLAUNCHER_H

#include "HP2LauncherModel.h"

#include <string>

namespace HP2Launcher
{
// Must run synchronously on the process main thread after path preparation and
// before appInit or SDL video initialization. The window is released on return.
LaunchAction RunHP2MacLauncher(
	const LauncherRequest& request,
	LauncherResult& result,
	std::string& error);
}

#endif

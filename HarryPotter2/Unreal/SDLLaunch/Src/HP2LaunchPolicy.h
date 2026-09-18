/*=============================================================================
	HP2LaunchPolicy.h: Pure command-line launcher policy.
=============================================================================*/
#ifndef HP2LAUNCHPOLICY_H
#define HP2LAUNCHPOLICY_H

#include "HP2LauncherModel.h"

#include <string>

namespace HP2Launcher
{
bool ShouldRunNativeLauncher(int argc, char* const argv[]);
bool BuildSelectedCommand(
	const LaunchSelection& selection,
	std::string& utf8Prefix,
	std::string& error);
}

#endif

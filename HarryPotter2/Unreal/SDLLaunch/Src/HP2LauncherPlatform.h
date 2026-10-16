#ifndef HP2LAUNCHERPLATFORM_H
#define HP2LAUNCHERPLATFORM_H

#include "HP2Launcher.h"

#include <string>
#include <vector>

namespace HP2LauncherPlatform
{
HP2Launcher::LauncherCapabilities CurrentCapabilities();
bool IsLauncherShortcutHeld();
bool PickFolder(std::string& Path);
bool PickDiscImage(std::string& Path);
void RevealPath(const std::string& Path);
}

#endif

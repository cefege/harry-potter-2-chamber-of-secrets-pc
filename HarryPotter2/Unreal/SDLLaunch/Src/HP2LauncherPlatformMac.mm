#import <AppKit/AppKit.h>

#include "HP2LauncherPlatform.h"

namespace HP2LauncherPlatform
{
bool IsLauncherShortcutHeld()
{
	return ([NSEvent modifierFlags] & NSEventModifierFlagOption) != 0;
}

void RevealPath(const std::string& Path)
{
	if (Path.empty()) return;
	@autoreleasepool
	{
		NSString* utf8Path = [NSString stringWithUTF8String:Path.c_str()];
		if (!utf8Path) return;
		NSURL* url = [NSURL fileURLWithPath:utf8Path];
		if (![[NSWorkspace sharedWorkspace] openURL:url])
			NSLog(@"HP2Launcher: could not reveal path %@", utf8Path);
	}
}
}

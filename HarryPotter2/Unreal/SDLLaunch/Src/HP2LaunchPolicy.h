/*=============================================================================
	HP2LaunchPolicy.h: Pure command-line launcher policy.
=============================================================================*/
#ifndef HP2LAUNCHPOLICY_H
#define HP2LAUNCHPOLICY_H

#include "HP2LauncherModel.h"

#include <cstdint>
#include <string>
#include <vector>

namespace HP2Launcher
{
enum LauncherSettingField : std::uint32_t
{
	LauncherSettingScreenMode = 1u << 0,
	LauncherSettingResolution = 1u << 1,
	LauncherSettingVerticalSync = 1u << 2,
	LauncherSettingRenderScale = 1u << 3,
	LauncherSettingUIScale = 1u << 4,
	LauncherSettingFrameRateLimit = 1u << 5,
	LauncherSettingMaintainVerticalFOV = 1u << 6,
	LauncherSettingNativeText = 1u << 7,
	LauncherSettingAntiAliasingSamples = 1u << 8,
	LauncherSettingAnisotropy = 1u << 9,
	LauncherSettingBrightness = 1u << 10,
	LauncherSettingTextureDetail = 1u << 11,
	LauncherSettingObjectDetail = 1u << 12,
	LauncherSettingSoundEnabled = 1u << 13,
	LauncherSettingSoundVolume = 1u << 14,
	LauncherSettingMusicVolume = 1u << 15,
	LauncherSettingMouseSensitivity = 1u << 16,
	LauncherSettingInvertMouse = 1u << 17,
	LauncherSettingAutoCenterCamera = 1u << 18,
	LauncherSettingMoveWhileCasting = 1u << 19,
	LauncherSettingAutoQuaff = 1u << 20,
	LauncherSettingScreenFlashes = 1u << 21,
	LauncherSettingDifficulty = 1u << 22,
	LauncherSettingJoystickEnabled = 1u << 23
};

struct LauncherAutomationRequest
{
	LaunchAction action = LaunchAction::Error;
	LauncherSettings settings;
	DataSource source = DataSource::Retail;
	std::uint32_t settingFields = 0;
	int saveIndex = -1;
	int saveSlot = -1;
	bool hasSource = false;
	bool hasSaveIndex = false;
	bool hasSaveSlot = false;
};

enum class LauncherAutomationParseResult
{
	NotRequested,
	Valid,
	Invalid
};

// Parses the non-interactive -launchaction=... request. All launcher-specific
// options use the -launch prefix so they are not forwarded as engine options.
LauncherAutomationParseResult ParseLauncherAutomationRequest(
	int argc,
	char* const argv[],
	LauncherAutomationRequest& request,
	std::string& error);

// Merges a parsed request into the same model result produced by the native
// launcher. Continue resolves its save against request.saves.
bool BuildLauncherAutomationResult(
	const LauncherAutomationRequest& automation,
	const LauncherRequest& request,
	LauncherResult& result,
	std::string& error);

bool BuildSelectedCommand(
	const LaunchSelection& selection,
	std::string& utf8Prefix,
	std::string& error);

struct LaunchSelectionField
{
	std::string key;
	std::string value;
};

// Pure launch-selection persistence contract. Serialization emits the
// canonical [LastLaunch] field rows: Action, plus the minimal save
// coordinates for Continue. Runtime-derived save metadata (paths, display
// label, size, timestamp) is deliberately not part of the contract.
bool SerializeLaunchSelection(
	const LaunchSelection& selection,
	std::vector<LaunchSelectionField>& fields,
	std::string& error);

// Parses persisted field rows. Row order is free, the final duplicated row
// wins (matching INI lookup semantics), and unrelated rows are ignored.
// Malformed or inconsistent stores fall back as a whole to the safe Quit
// selection: the returned bool is false and error carries the reason, while
// selection is still populated with the fallback.
bool DeserializeLaunchSelection(
	const std::vector<LaunchSelectionField>& fields,
	LaunchSelection& selection,
	std::string& error);

bool ShouldRunNativeLauncher(int argc, char* const argv[]);
}

#endif

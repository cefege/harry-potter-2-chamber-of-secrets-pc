/*=============================================================================
	HP2LauncherStore.h: Portable save and user-configuration persistence.
=============================================================================*/
#ifndef HP2LAUNCHERSTORE_H
#define HP2LAUNCHERSTORE_H

#include "HP2LauncherModel.h"

#include <string>
#include <vector>

namespace HP2Launcher
{
bool LoadLauncherState(
	const LauncherPaths& paths,
	LauncherState& state,
	std::string& error);

bool ValidateLauncherSettings(
	const LauncherSettings& settings,
	std::string& error);

bool CommitLauncherSettings(
	const LauncherPaths& paths,
	const LauncherSettings& settings,
	std::string& error);

bool LoadDataSourceConfiguration(
	const std::string& launcherRoot,
	DataSourceConfiguration& configuration,
	std::string& error);

bool CommitDataSourceConfiguration(
	const std::string& launcherRoot,
	const DataSourceConfiguration& configuration,
	std::string& error);

// One parsed INI row of legacy launcher configuration. Duplicate section/key
// rows are allowed; lookup semantics mirror the INI reader where the final
// occurrence wins.
struct LegacySettingValue
{
	std::string section;
	std::string key;
	std::string value;
};

// One observable normalization performed by MigrateLegacySettings. An empty
// previousValue means the key was absent and is being materialized.
struct SettingsMigrationChange
{
	std::string section;
	std::string key;
	std::string previousValue;
	std::string newValue;
	std::string reason;
};

// Pure legacy-schema settings migration: consumes parsed INI rows and returns
// the canonical modern-schema rows plus a journal entry per rewritten row
// (dotted lowercase reason codes). It performs no filesystem, locale, or
// global state access, so it is fully table-testable.
std::vector<LegacySettingValue> MigrateLegacySettings(
	const std::vector<LegacySettingValue>& legacyValues,
	std::vector<SettingsMigrationChange>& changes);

// Persists the launch selection in the launcher-owned catalog under
// [LastLaunch] using the pure serialization contract from HP2LaunchPolicy.h.
bool CommitLaunchSelection(
	const std::string& launcherRoot,
	const LaunchSelection& selection,
	std::string& error);

// Reloads the persisted launch selection. Returns false when the catalog is
// missing or malformed; selection still receives the safe Quit fallback.
bool LoadLaunchSelection(
	const std::string& launcherRoot,
	LaunchSelection& selection,
	std::string& error);

bool PrepareDataSourceProfile(
	const std::string& launcherRoot,
	DataSource source,
	std::string& profileRoot,
	std::string& error);

#ifdef HP2_LAUNCHER_TESTING
void SetLauncherPublishFailureForTesting(int publicationIndex);
#endif

bool DiscoverSaves(
	const std::string& userRoot,
	std::vector<SaveRecord>& saves,
	std::string& error);
}

#endif

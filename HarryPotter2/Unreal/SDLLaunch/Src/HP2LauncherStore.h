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

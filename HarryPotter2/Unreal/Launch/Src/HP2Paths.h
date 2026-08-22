/*=============================================================================
	HP2Paths.h: Pre-appInit data and user path bootstrap.
=============================================================================*/

#ifndef HP2PATHS_H
#define HP2PATHS_H

#include <string>

// Returns true for the case-insensitive -datadir=<path> option recognized by
// PrepareHP2Paths. Parsing never consumes, filters, or mutates argv.
bool IsHP2DataDirectoryArgument(const char* Argument);

// Returns the path value for a recognized -datadir=<path> option, or null.
const char* HP2DataDirectoryArgumentValue(const char* Argument);

// Establishes the validated read-only data root and the separate writable user
// root. This function only uses the UTF-8/POSIX boundary and is safe before
// GMalloc and appInit. A failure is reported to stderr.
bool PrepareHP2Paths(int ArgC, char* const ArgV[]);

// Establishes the common launcher-owned root at
// $HOME/Library/Application Support/Harry Potter 2/User. It does not create
// a game profile or install an engine path.
bool PrepareHP2LauncherHome(std::string& LauncherRoot, std::string& Error);

// Canonicalizes an existing data root that contains a readable
// System/Default.ini.
bool ValidateHP2DataRoot(
	const std::string& Candidate,
	std::string& CanonicalRoot,
	std::string& Error);

// Installs one validated data root and one separate writable user/profile
// root before appInit.
bool InstallHP2Paths(
	const std::string& DataRoot,
	const std::string& UserRoot,
	std::string& Error);

// Returns the absolute conventional launcher folders without requiring them
// to exist or contain game data.
bool GetHP2ConventionalRetailDataRoot(std::string& AbsoluteRoot);
bool GetHP2ConventionalPrototypeDataRoot(std::string& AbsoluteRoot);

// Probes the documented retail import and the repository checkout prototype
// root independently. Neither probe selects or installs a root.
bool DiscoverHP2RetailDataRoot(std::string& CanonicalRoot);
bool DiscoverHP2PrototypeDataRoot(std::string& CanonicalRoot);

#endif

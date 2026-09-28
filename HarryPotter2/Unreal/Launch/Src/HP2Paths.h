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

// Result of a bootstrap that found no playable data root.
//
// A player with no game data installed is a normal, expected first-run state,
// not a fatal error: the launcher has to open so it can offer the import
// flow. PrepareHP2Paths therefore reports this outcome instead of exiting,
// and the engine is only started once a root has been imported or chosen.
struct HP2PathsBootstrap
{
	// True when a validated data root was found and installed.
	bool Installed = false;
	// Conventional location the importer should publish into, always set on
	// a failed bootstrap so the launcher knows where an import would land.
	std::string RetailDataRoot;
	// Conventional external root that was searched, for the error text.
	std::string SearchedRoot;
	// Conventional prototype root, probed so a prototype-only install is not
	// reported as having no data at all.
	std::string PrototypeDataRoot;
};

// Establishes the validated read-only data root and the separate writable
// user root, or reports a recoverable "no game data yet" outcome. Safe before
// GMalloc and appInit, exactly like PrepareHP2Paths.
HP2PathsBootstrap BootstrapHP2Paths(int ArgC, char* const ArgV[]);

// Data-root bootstrap runs in one of two modes:
//
//   Legacy mode - the candidate contains no retail overlay bookkeeping files
//     (overlay-manifest.json or overlay-checksums.txt). A readable
//     System/Default.ini implies the prototype "development" profile and the
//     root is accepted unchanged.
//
//   Overlay mode - the candidate contains overlay-manifest.json (or a stray
//     overlay-checksums.txt left by a partial import). The root must carry a
//     complete, self-consistent identity: a sibling overlay-checksums.txt, a
//     parseable manifest with a known profile, matching manifest/checksums
//     path sets, and per-file sizes plus SHA-256 digests verified by stream
//     hashing. Any violation refuses the root and reports
//     'HP2_DATA_IDENTITY_REJECT <reason_code> <detail>' on stderr.
//
// Rejection reason codes: data.manifest_missing, data.checksums_missing,
// data.manifest_invalid, data.path_set_mismatch, data.size_mismatch,
// data.hash_mismatch, data.profile_unknown.
enum HP2DataProfileKind
{
	HP2DataProfile_Unknown = 0,
	HP2DataProfile_Development,
	HP2DataProfile_Safe,
	HP2DataProfile_Full,
	HP2DataProfile_RetailOnly
};

// Structured outcome of validating one candidate data root.
struct HP2PathsBootstrapStatus
{
	bool Accepted = false;
	HP2DataProfileKind Profile = HP2DataProfile_Unknown;
	const char* ReasonCode = nullptr; // dotted lowercase; null when accepted
	std::string Detail;
};

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

// Validates one candidate data root and reports the structured bootstrap
// outcome without installing anything. Acceptance matches PrepareHP2Paths:
// legacy roots need a readable System/Default.ini; overlay roots must pass
// the full identity verification described above.
bool ValidateHP2DataRootStatus(
	const std::string& Candidate,
	std::string& CanonicalRoot,
	HP2PathsBootstrapStatus& Status);

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

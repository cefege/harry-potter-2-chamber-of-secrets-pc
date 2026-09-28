/*=============================================================================
	HP2DataImport.h: Portable game-data import service.

	Runs the bundled, already-validated Build/prepare_retail_data.py importer
	over a source folder or archive the player picked, and publishes the result
	into the conventional retail data root. This is the only supported way for
	a player to add game data, and it is reachable from the launcher on both
	platforms.
=============================================================================*/
#ifndef HP2DATAIMPORT_H
#define HP2DATAIMPORT_H

#include <string>

namespace HP2DataImport
{

// Where the bundled importer and the publisher's own scripts are found, in
// preference order. Empty when nothing is installed (a bare build tree with
// no packaged resources).
struct Toolchain
{
	// Absolute path to prepare_retail_data.py, or empty when not installed.
	std::string importerScript;
	// Absolute path to python3, or empty when python3 is not on PATH.
	std::string python;
};

// Resolves the importer script and the Python interpreter for the running
// process. `ExecutablePath` is argv[0] as invoked (used to locate the app
// bundle next to the executable); the current working directory is used as a
// fallback for a source checkout. Never fails: a missing component is
// reported as an empty field so the caller can explain it.
Toolchain ResolveToolchain(const std::string& ExecutablePath);

// Human-readable reason the importer cannot run right now, or an empty string
// when it can. Covers "no bundled script" and "no python3 on PATH".
std::string UnavailableReason(const Toolchain& Tool);

// Absolute conventional retail import destination
// (~/Library/Application Support/Harry Potter 2/Data/Retail on macOS,
// $XDG_DATA_HOME/harry-potter-2/Data/Retail on Linux).
bool GetConventionalRetailDestination(std::string& Destination);

// Runs the importer on `SourcePath`, which may be an extracted retail
// installation directory or a ZIP/TAR/7z archive of one. Returns true on a
// completed import. `Output` receives the process's combined stdout/stderr on
// completion, and `Error` a short actionable message on failure.
//
// The import is fully synchronous: it copies and hashes hundreds of
// megabytes, so callers must keep a UI responsive while this blocks.
bool RunRetailImport(
	const Toolchain& Tool,
	const std::string& SourcePath,
	const std::string& Destination,
	std::string& Output,
	std::string& Error);

}

#endif

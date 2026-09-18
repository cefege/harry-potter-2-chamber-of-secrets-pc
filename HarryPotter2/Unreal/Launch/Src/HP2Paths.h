/*=============================================================================
	HP2Paths.h: Pre-appInit data and user path bootstrap.
=============================================================================*/

#ifndef HP2PATHS_H
#define HP2PATHS_H

// Returns true for the case-insensitive -datadir=<path> option consumed by
// PrepareHP2Paths. Launchers omit this option from the engine command line.
bool IsHP2DataDirectoryArgument(const char* Argument);

// Establishes the validated read-only data root and the separate writable user
// root. This function only uses the UTF-8/POSIX boundary and is safe before
// GMalloc and appInit. A failure is reported to stderr.
bool PrepareHP2Paths(int ArgC, char* const ArgV[]);

#endif

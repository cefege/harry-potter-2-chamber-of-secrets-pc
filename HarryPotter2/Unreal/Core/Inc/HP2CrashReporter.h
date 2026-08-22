/*=============================================================================
	HP2CrashReporter.h: Async-signal-safe crash reporting (macOS/arm64).

	Installs POSIX signal handlers (SIGSEGV/BUS/ILL/FPE/ABORT) plus an
	optional wall-clock watchdog. On a fatal signal or watchdog expiry the
	reporter writes a small JSON report and an "HP2_CRASH <code> <path>"
	line to stderr using only async-signal-safe syscalls (open/write), then
	re-raises for normal core/exit behavior.

	Manual self-test:
	  clang++ -std=c++17 -D__UNIX__=1 -DMACOSX=1 -DCORE_API= \
	    -I Core/Inc Core/Src/HP2CrashReporter.cpp selftest.cpp
	  # raise(SIGSEGV) -> expect crash-report.json + HP2_CRASH on stderr
=============================================================================*/

#ifndef HP2CRASHREPORTER_H
#define HP2CRASHREPORTER_H

#include "Core.h"

struct HP2CrashReporterConfig
{
	// Directory receiving crash-report.json. Empty = current working dir.
	const char* ArtifactDir;
};

// Install handlers once at startup. Safe to call twice (second call is a
// no-op). Never throws; never allocates after install.
void HP2InstallCrashReporter( const struct HP2CrashReporterConfig* Config );

// Optional wall-clock deadline in seconds measured from install time.
// 0 disables (default). On expiry the reporter writes exit_reason
// "watchdog", reason_code crash.watchdog.timeout, then raises SIGABRT.
void HP2SetCrashReporterWatchdogSeconds( double Seconds );

// Override engine-log discovery with an explicit UTF-8 path (log tail is
// embedded into the report). Call once the launcher knows the real log
// location. Not signal-safe: setup path only.
void HP2SetCrashReporterLogHint( const char* Utf8Path );

#endif // HP2CRASHREPORTER_H

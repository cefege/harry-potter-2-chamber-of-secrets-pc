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
	// Human-readable process label embedded in reports ("HarryPotter2", "UCC").
	const char* ProcessName;
	// Engine log filename under the user root (e.g. "HarryPotter2.log");
	// its tail is embedded into every report.
	const char* LogFileBase;
	// Optional wall-clock deadline from install time. 0 disables.
	double WatchdogSeconds;
};

// Install handlers once at startup. Safe to call twice (second call is a
// no-op). Never throws; never allocates after install. When
// Config->WatchdogSeconds > 0 a monitor thread raises SIGABRT with a
// structured report on expiry.
void HP2InstallCrashReporter( const struct HP2CrashReporterConfig* Config );

// Override engine-log discovery with an explicit UTF-8 path. Call after
// install once the launcher knows the real location. Setup-path only.
void HP2SetCrashReporterLogHint( const char* Utf8Path );
#endif // HP2CRASHREPORTER_H

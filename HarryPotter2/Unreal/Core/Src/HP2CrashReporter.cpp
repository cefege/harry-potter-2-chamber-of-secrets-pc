/*=============================================================================
	HP2CrashReporter.cpp: Async-signal-safe crash reporting implementation.

	Design constraints: after a fatal signal only async-signal-safe calls
	are used (open/write/close, sigaction, _exit). All buffers are
	preallocated at install time; no malloc, no stdio, no locks. Backtraces
	are captured with backtrace() into static storage but NOT symbolicated
	in-signal (backtrace_symbols is not signal-safe); frame addresses are
	written as hex for offline symbolication with atos.
=============================================================================*/

#include "HP2CrashReporter.h"

#include <signal.h>
#include <fcntl.h>
#include <unistd.h>
#include <execinfo.h>
#include <pthread.h>
#include <time.h>
#include <stdlib.h>
#include <string.h>

namespace
{
	const int kMaxSignals = 5;
	const size_t kMaxFrames = 64;
	const size_t kLogTailBytes = 16384;

	volatile sig_atomic_t g_Installed = 0;
	volatile sig_atomic_t g_ReportWritten = 0;
	volatile double g_WatchdogSeconds = 0.0;
	volatile double g_InstallTime = 0.0;

	char g_ArtifactDir[1024] = {0};
	char g_ProcessName[128] = {0};
	char g_LogHint[1024] = {0};

	const int kWatchedSignals[kMaxSignals] = {SIGSEGV, SIGBUS, SIGILL, SIGFPE, SIGABRT};

	char g_ReportPath[1152] = {0};

	// Preallocated report buffer (largest plausible report).
	char g_Report[262144];
	size_t g_ReportLen = 0;

	double NowMonotonic()
	{
		struct timespec Ts;
		clock_gettime( CLOCK_MONOTONIC, &Ts );
		return (double)Ts.tv_sec + (double)Ts.tv_nsec * 1e-9;
	}

	size_t StrLen( const char* S )
	{
		size_t N = 0;
		if( !S ) return 0;
		while( S[N] && N < 4096 ) ++N;
		return N;
	}

	void MemCopy( char* Dst, const char* Src, size_t N )
	{
		for( size_t I = 0; I < N; ++I ) Dst[I] = Src[I];
	}

	// Appends into g_Report; silently truncates at capacity.
	void Append( const char* Text, size_t N )
	{
		if( g_ReportLen >= sizeof( g_Report ) - 1 ) return;
		if( N > sizeof( g_Report ) - 1 - g_ReportLen )
			N = sizeof( g_Report ) - 1 - g_ReportLen;
		MemCopy( g_Report + g_ReportLen, Text, N );
		g_ReportLen += N;
		g_Report[g_ReportLen] = 0;
	}

	void AppendC( const char* Text ) { Append( Text, StrLen( Text ) ); }

	void AppendU64( unsigned long long Value )
	{
		char Buf[24];
		int P = (int)( sizeof( Buf ) ) - 1;
		Buf[P] = 0;
		do { Buf[--P] = (char)('0' + (Value % 10)); Value /= 10; } while( Value );
		AppendC( &Buf[P] );
	}

	void AppendHex( uintptr_t Value )
	{
		char Buf[20];
		int P = (int)( sizeof( Buf ) ) - 1;
		Buf[P] = 0;
		const char* Digits = "0123456789abcdef";
		do { Buf[--P] = Digits[Value & 0xF]; Value >>= 4; } while( Value );
		AppendC( "0x" );
		Append( &Buf[P], (size_t)( (int)sizeof( Buf ) - 1 - P ) );
	}

	void AppendSignalName( int Signal )
	{
		switch( Signal )
		{
			case SIGSEGV: AppendC( "SIGSEGV" ); break;
			case SIGBUS:  AppendC( "SIGBUS" ); break;
			case SIGILL:  AppendC( "SIGILL" ); break;
			case SIGFPE:  AppendC( "SIGFPE" ); break;
			case SIGABRT: AppendC( "SIGABRT" ); break;
			default:      AppendC( "SIG?" ); AppendU64( (unsigned long long)Signal ); break;
		}
	}

	// Reads up to kLogTailBytes from the end of the hinted log file.
	void AppendEngineLogTail()
	{
		const char* Path = g_LogHint[0] ? g_LogHint : NULL;
		if( !Path ) return;

		int Fd = open( Path, O_RDONLY );
		if( Fd < 0 ) return;

		static volatile char Tail[kLogTailBytes];
		char* Buf = (char*)Tail;
		off_t Size = lseek( Fd, 0, SEEK_END );
		if( Size < 0 ) { close( Fd ); return; }

		off_t Start = Size > (off_t)kLogTailBytes ? Size - (off_t)kLogTailBytes : 0;
		lseek( Fd, Start, SEEK_SET );

		ssize_t Total = 0;
		while( Total < (ssize_t)kLogTailBytes )
		{
			ssize_t Got = read( Fd, Buf + Total, kLogTailBytes - (size_t)Total );
			if( Got <= 0 ) break;
			Total += Got;
		}
		close( Fd );

		AppendC( ",\"log_tail\":\"" );
		for( ssize_t I = 0; I < Total; ++I )
		{
			unsigned char C = (unsigned char)Buf[I];
			if( C == '"' ) AppendC( "\\\"" );
			else if( C == '\\' ) AppendC( "\\\\" );
			else if( C >= 0x20 && C != 0x7F ) Append( (const char*)&Buf[I], 1 );
			else if( C == '\n' || C == '\r' || C == '\t' ) AppendC( " " );
		}
		AppendC( "\"" );
	}

	void WriteReportAndStderr( const char* ExitReason, const char* ReasonCode, int Signal )
	{
		if( g_ReportWritten ) return;
		g_ReportWritten = 1;

		g_ReportLen = 0;
		g_Report[0] = 0;

		AppendC( "{\"format\":\"hp2-crash\",\"schema_version\":1,\"exit_reason\":\"" );
		AppendC( ExitReason );
		AppendC( "\",\"reason_code\":\"" );
		AppendC( ReasonCode );
		AppendC( "\",\"process\":\"" );
		AppendC( g_ProcessName[0] ? g_ProcessName : "unknown" );
		AppendC( "\"" );
		if( Signal > 0 )
		{
			AppendC( ",\"signal\":" );
			AppendU64( (unsigned long long)Signal );
			AppendC( ",\"signal_name\":\"" );
			AppendSignalName( Signal );
			AppendC( "\"" );
		}
		AppendEngineLogTail();

		// Frame addresses for offline symbolication (atos).
		if( Signal == SIGSEGV || Signal == SIGBUS || Signal == SIGILL || Signal == SIGFPE || Signal == SIGABRT )
		{
			static void* Frames[kMaxFrames];
			int Count = backtrace( Frames, (int)kMaxFrames );
			AppendC( ",\"frames\":[" );
			for( int I = 0; I < Count; ++I )
			{
				if( I ) AppendC( "," );
				AppendHex( (uintptr_t)Frames[I] );
			}
			AppendC( "]" );
		}
		AppendC( "}\n" );

		if( g_ArtifactDir[0] && g_ReportPath[0] == 0 )
		{
			// Build <artifactdir>/crash-report.json once.
			size_t Base = StrLen( g_ArtifactDir );
			MemCopy( g_ReportPath, g_ArtifactDir, Base );
			if( Base && g_ReportPath[Base-1] != '/' ) g_ReportPath[Base++] = '/';
			MemCopy( g_ReportPath + Base, "crash-report.json", 18 );
		}
		else if( g_ReportPath[0] == 0 )
		{
			MemCopy( g_ReportPath, "crash-report.json", 18 );
		}

		int Fd = open( g_ReportPath, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0644 );
		if( Fd >= 0 )
		{
			size_t Off = 0;
			while( Off < g_ReportLen )
			{
				ssize_t Wrote = write( Fd, g_Report + Off, g_ReportLen - Off );
				if( Wrote <= 0 ) break;
				Off += (size_t)Wrote;
			}
			close( Fd );
		}

		static const char Prefix[] = "HP2_CRASH ";
		write( 2, Prefix, sizeof(Prefix) - 1 );
		write( 2, ReasonCode, StrLen( ReasonCode ) );
		write( 2, " ", 1 );
		write( 2, g_ReportPath, StrLen( g_ReportPath ) );
		write( 2, "\n", 1 );
	}

	void Handler( int Signal, siginfo_t*, void* )
	{
		WriteReportAndStderr( "signal", "crash.signal", Signal );

		// Restore default and re-raise so core dumps/exit status stay honest.
		struct sigaction Sa;
		memset( &Sa, 0, sizeof( Sa ) );
		Sa.sa_handler = SIG_DFL;
		sigaction( Signal, &Sa, NULL );
		raise( Signal );
		_exit( 128 + Signal );
	}

	void* WatchdogThread( void* )
	{
		double Deadline = g_InstallTime + g_WatchdogSeconds;
		while( true )
		{
			struct timespec Req = { 0, 100 * 1000 * 1000 }; // 100 ms poll.
			nanosleep( &Req, NULL );
			double Now = NowMonotonic();
			if( g_WatchdogSeconds <= 0.0 ) continue;
			if( Now >= Deadline ) break;
		}
		WriteReportAndStderr( "watchdog", "crash.watchdog.timeout", 0 );
		raise( SIGABRT );
		return NULL;
}

} // namespace

//-----------------------------------------------------------------------------
void HP2InstallCrashReporter( const HP2CrashReporterConfig* Config )
{
	// Configuration, install time, and the watchdog thread are set up once;
	// signal DISPOSITIONS are refreshed on every call. The launcher installs
	// the reporter early, then __Context::StaticInit overwrites these
	// signals with its guard longjmp handlers; the post-StaticInit reinstall
	// must actually restore them or fatal signals bypass the reporter and
	// cascade through HandleSignal's exit-from-signal-handler path.
	const bool FirstInstall = !g_Installed;

	if( FirstInstall )
	{
		g_Installed = 1;

		if( Config )
		{
			size_t N = StrLen( Config->ProcessName );
			if( N >= sizeof( g_ProcessName ) ) N = sizeof( g_ProcessName ) - 1;
			MemCopy( g_ProcessName, Config->ProcessName ? Config->ProcessName : "", N );
			g_ProcessName[N] = 0;

			double Watchdog = Config->WatchdogSeconds;
			if( Watchdog < 0.0 ) Watchdog = 0.0;
			if( Watchdog > 86400.0 ) Watchdog = 86400.0;
			g_WatchdogSeconds = Watchdog;

			// Default engine-log discovery: <user root>/<LogFileBase>. The user
			// root layout matches HP2Paths (Application Support/Harry Potter 2).
			if( !g_LogHint[0] && Config->LogFileBase )
			{
				const char* Home = getenv( "HOME" );
				if( Home )
				{
					static const char UserRoot[] =
						"/Library/Application Support/Harry Potter 2/User/";
					size_t HN = StrLen( Home );
					size_t BN = StrLen( Config->LogFileBase );
					size_t Total = HN + sizeof(UserRoot) - 1 + BN;
					if( Total < sizeof( g_LogHint ) - 1 )
					{
						MemCopy( g_LogHint, Home, HN );
						MemCopy( g_LogHint + HN, UserRoot, sizeof(UserRoot) - 1 );
						MemCopy( g_LogHint + HN + sizeof(UserRoot) - 1,
							Config->LogFileBase, BN );
						g_LogHint[Total] = 0;
					}
				}
			}
		}

		g_InstallTime = NowMonotonic();
	}

	struct sigaction Sa;
	memset( &Sa, 0, sizeof( Sa ) );
	Sa.sa_sigaction = Handler;
	Sa.sa_flags = SA_SIGINFO | SA_RESETHAND;
	for( int I = 0; I < kMaxSignals; ++I )
		sigaction( kWatchedSignals[I], &Sa, NULL );

	if( g_WatchdogSeconds > 0.0 && FirstInstall )
	{
		static pthread_t WatchdogThreadHandle;
		pthread_create( &WatchdogThreadHandle, NULL, WatchdogThread, NULL );
	}

	static const char Installed[] = "HP2_CRASH installed\n";
	if( FirstInstall )
		write( 2, Installed, sizeof(Installed) - 1 );
}
void HP2SetCrashReporterLogHint( const char* Utf8Path )
{
	if( !Utf8Path ) return;
	size_t N = StrLen( Utf8Path );
	if( N >= sizeof( g_LogHint ) ) N = sizeof( g_LogHint ) - 1;
	MemCopy( g_LogHint, Utf8Path, N );
	g_LogHint[N] = 0;
}

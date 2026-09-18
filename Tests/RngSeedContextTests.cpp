/*=============================================================================
	RngSeedContextTests.cpp: Exact Unix Context seed bootstrap contract.
=============================================================================*/
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "Core.h"

extern void appFlushRandTrace();

namespace
{
	void Require( int Condition, const char* Message )
	{
		if( !Condition )
		{
			fprintf( stderr, "RngSeedContextTests: %s\n", Message );
			exit(1);
		}
	}

	void RequireNextRandAfterContext( unsigned Seed )
	{
		srand( Seed );
		const int Expected = rand();
		srand( Seed );
		__Context::StaticInit();
		Require( rand()==Expected, "Context consumed or changed the seeded stream" );
	}

	void RequireInvalidSeedFailsBeforeAppInit()
	{
		int Pipe[2];
		Require( pipe(Pipe)==0, "could not create stderr pipe" );
		const pid_t Child = fork();
		Require( Child>=0, "could not fork invalid seed case" );
		if( Child==0 )
		{
			close( Pipe[0] );
			Require( dup2(Pipe[1],STDERR_FILENO)>=0, "could not capture child stderr" );
			close( Pipe[1] );
			setenv( "HP2_RNG_SEED", "invalid", 1 );
			__Context::StaticInit();
			_exit(0);
		}

		close( Pipe[1] );
		char Error[256];
		const ssize_t Length = read( Pipe[0], Error, sizeof(Error)-1 );
		Require( Length>=0, "could not read child stderr" );
		Error[Length] = 0;
		close( Pipe[0] );
		int Status = 0;
		Require( waitpid(Child,&Status,0)==Child, "could not wait for invalid seed case" );
		Require( WIFEXITED(Status) && WEXITSTATUS(Status)==1, "invalid seed did not exit one" );

		char Expected[160];
		snprintf(
			Expected,
			sizeof(Expected),
			"Invalid HP2_RNG_SEED: expected an unsigned decimal value from 0 to %u\n",
			(unsigned)RAND_MAX
		);
		Require( !strcmp(Error,Expected), "invalid seed stderr changed" );
	}

	void RequireTraceCanArmAfterContext()
	{
		const char* ArtifactDirectory = getenv( "HP2_ARTIFACT_DIR" );
		Require( ArtifactDirectory && *ArtifactDirectory, "HP2_ARTIFACT_DIR is required" );
		char TracePath[512];
		const int PathLength = snprintf(
			TracePath,
			sizeof(TracePath),
			"%s/context-post-seam-rng-trace.json",
			ArtifactDirectory
		);
		Require( PathLength>0 && (size_t)PathLength<sizeof(TracePath), "trace path buffer too small" );
		unsetenv( "HP2_RNG_TRACE" );
		setenv( "HP2_RNG_SEED", "42", 1 );
		__Context::StaticInit();
		Require( setenv("HP2_RNG_TRACE",TracePath,1)==0, "could not arm post-context trace" );
		appRand();
		appFlushRandTrace();
		Require( access(TracePath,F_OK)==0, "Context initialized tracing before trace environment existed" );
	}
}

int main( int ArgC, char* ArgV[] )
{
	Require( ArgC==2, "expected default, diagnostic, invalid, or late_trace mode" );
	if( !strcmp(ArgV[1],"default") )
	{
		unsetenv( "HP2_RNG_SEED" );
		RequireNextRandAfterContext( 42 );
	}
	else if( !strcmp(ArgV[1],"diagnostic") )
	{
		setenv( "HP2_RNG_SEED", "0", 1 );
		RequireNextRandAfterContext( 1 );
	}
	else if( !strcmp(ArgV[1],"invalid") )
		RequireInvalidSeedFailsBeforeAppInit();
	else if( !strcmp(ArgV[1],"late_trace") )
		RequireTraceCanArmAfterContext();
	else
		Require( 0, "unknown mode" );

	puts("RngSeedContextTests: PASS");
	return 0;
}

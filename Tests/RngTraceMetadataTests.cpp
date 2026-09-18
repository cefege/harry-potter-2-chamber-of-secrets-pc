/*=============================================================================
	RngTraceMetadataTests.cpp: Startup-seed trace metadata contract.
=============================================================================*/
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "Core.h"

extern void appFlushRandTrace();

namespace
{
	void Require( int Condition, const char* Message )
	{
		if( !Condition )
		{
			fprintf( stderr, "RngTraceMetadataTests: %s\n", Message );
			exit(1);
		}
	}

	void PrepareTracePath( char* Path, size_t PathSize )
	{
		const char* ArtifactDirectory = getenv( "HP2_ARTIFACT_DIR" );
		Require( ArtifactDirectory && *ArtifactDirectory, "HP2_ARTIFACT_DIR is required" );
		const int Length = snprintf(
			Path,
			PathSize,
			"%s/rng-trace-metadata.json",
			ArtifactDirectory
		);
		Require( Length>0 && (size_t)Length<PathSize, "trace path buffer too small" );
		Require( setenv("HP2_RNG_TRACE",Path,1)==0, "could not arm RNG trace" );
		Require( setenv("HP2_RNG_TRACE_LIMIT","8",1)==0, "could not set RNG trace limit" );
	}

	char* ReadTrace( const char* Path )
	{
		FILE* Trace = fopen( Path, "rb" );
		Require( Trace!=NULL, "trace was not written" );
		Require( fseek(Trace,0,SEEK_END)==0, "could not seek trace" );
		const long Length = ftell( Trace );
		Require( Length>=0, "could not measure trace" );
		Require( fseek(Trace,0,SEEK_SET)==0, "could not rewind trace" );
		char* Contents = static_cast<char*>(malloc((size_t)Length+1));
		Require( Contents!=NULL, "could not allocate trace buffer" );
		Require( fread(Contents,1,(size_t)Length,Trace)==(size_t)Length, "could not read trace" );
		Contents[Length] = 0;
		Require( fclose(Trace)==0, "could not close trace" );
		return Contents;
	}

	void RequireDiagnosticMetadata( const char* Trace )
	{
		char Expected[128];
		snprintf(
			Expected,
			sizeof(Expected),
			"\"seed\":1,\"requested_seed\":%u,\"start_reason\":\"diagnostic\"",
			(unsigned)RAND_MAX
		);
		Require( strstr(Trace,Expected)!=NULL, "diagnostic effective/requested metadata missing" );
	}

	void RequireReplayMetadata( const char* Trace )
	{
		Require(
			strstr(Trace,"\"seed\":1,\"start_reason\":\"replay\"")!=NULL,
			"replay metadata missing"
		);
		Require( strstr(Trace,"\"requested_seed\"")==NULL, "replay retained diagnostic requested seed" );
	}
}

int main( int ArgC, char* ArgV[] )
{
	Require( ArgC==2, "expected diagnostic or replay mode" );
	char TracePath[128];
	PrepareTracePath( TracePath, sizeof(TracePath) );

	if( !strcmp(ArgV[1],"diagnostic") )
	{
		appSetRandTraceDiagnosticSeed( (unsigned)RAND_MAX, 1 );
		appRand();
	}
	else if( !strcmp(ArgV[1],"replay") )
	{
		appSetRandTraceDiagnosticSeed( 42, 42 );
		srand(1);
		appResetRandTraceForReplay();
		appRand();
	}
	else
		Require( 0, "unknown mode" );

	appFlushRandTrace();
	char* Trace = ReadTrace( TracePath );
	if( !strcmp(ArgV[1],"diagnostic") )
		RequireDiagnosticMetadata( Trace );
	else
		RequireReplayMetadata( Trace );
	free( Trace );

	puts("RngTraceMetadataTests: PASS");
	return 0;
}

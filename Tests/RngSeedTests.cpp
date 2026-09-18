/*=============================================================================
	RngSeedTests.cpp: HP2_RNG_SEED pre-appInit parser/bootstrap contract.
=============================================================================*/
#include <stdio.h>
#include <stdlib.h>

#include "HP2RngSeed.h"

namespace
{
	void Require( int Condition, const char* Message )
	{
		if( !Condition )
		{
			fprintf( stderr, "RngSeedTests: %s\n", Message );
			exit(1);
		}
	}

	void RequireValid( const char* Text, unsigned Expected )
	{
		unsigned Parsed = 0;
		Require( HP2ParseDiagnosticRngSeed(Text,&Parsed), Text );
		Require( Parsed==Expected, Text );
	}

	void RequireInvalid( const char* Text )
	{
		unsigned Parsed = 99;
		Require( !HP2ParseDiagnosticRngSeed(Text,&Parsed), Text ? Text : "NULL" );
		Require( Parsed==99, "invalid parse modified output" );
	}

	void RequireEndpointStreamMatchesOne( unsigned Seed )
	{
		srand( HP2DiagnosticRngSeedForSrand(Seed) );
		int Actual[8];
		for( int Index=0; Index<8; ++Index )
			Actual[Index] = rand();
		srand(1);
		for( int Index=0; Index<8; ++Index )
			Require( Actual[Index]==rand(), "endpoint seed stream diverges from Rust normalization" );
	}

	void RequireBootstrapAbsent( const char* Text )
	{
		unsigned Requested = 71;
		unsigned Effective = 73;
		Require(
			HP2PrepareDiagnosticRngSeed(Text,&Requested,&Effective)==HP2DiagnosticRngSeedAbsent,
			"expected absent diagnostic seed"
		);
		Require( Requested==71 && Effective==73, "absent bootstrap modified outputs" );
	}

	void RequireBootstrapValid( const char* Text, unsigned ExpectedRequested, unsigned ExpectedEffective )
	{
		unsigned Requested = 0;
		unsigned Effective = 0;
		Require(
			HP2PrepareDiagnosticRngSeed(Text,&Requested,&Effective)==HP2DiagnosticRngSeedAccepted,
			Text
		);
		Require( Requested==ExpectedRequested, "bootstrap requested seed changed" );
		Require( Effective==ExpectedEffective, "bootstrap effective seed changed" );
	}

	void RequireBootstrapInvalid( const char* Text )
	{
		unsigned Requested = 71;
		unsigned Effective = 73;
		Require(
			HP2PrepareDiagnosticRngSeed(Text,&Requested,&Effective)==HP2DiagnosticRngSeedInvalid,
			Text
		);
		Require( Requested==71 && Effective==73, "invalid bootstrap modified outputs" );
	}
}

int main()
{
	char Maximum[32];
	char TooLarge[32];
	snprintf( Maximum, sizeof(Maximum), "%u", (unsigned)RAND_MAX );
	snprintf( TooLarge, sizeof(TooLarge), "%llu", (unsigned long long)RAND_MAX+1ull );

	RequireValid( "0", 0 );
	RequireValid( "00042", 42 );
	RequireValid( Maximum, (unsigned)RAND_MAX );
	RequireInvalid( NULL );
	RequireInvalid( "" );
	RequireInvalid( "-1" );
	RequireInvalid( "+1" );
	RequireInvalid( " 1" );
	RequireInvalid( "1 " );
	RequireInvalid( "0x1" );
	RequireInvalid( "1.0" );
	RequireInvalid( TooLarge );
	RequireInvalid( "999999999999999999999999999999999999" );

	RequireBootstrapAbsent( NULL );
	RequireBootstrapAbsent( "" );
	RequireBootstrapValid( "0", 0, 1 );
	RequireBootstrapValid( "00042", 42, 42 );
	RequireBootstrapValid( Maximum, (unsigned)RAND_MAX, 1 );
	RequireBootstrapInvalid( "-1" );
	RequireBootstrapInvalid( "+1" );
	RequireBootstrapInvalid( " 1" );
	RequireBootstrapInvalid( "1 " );
	RequireBootstrapInvalid( "0x1" );
	RequireBootstrapInvalid( "1.0" );
	RequireBootstrapInvalid( TooLarge );
	RequireBootstrapInvalid( "999999999999999999999999999999999999" );
	Require(
		HP2PrepareDiagnosticRngSeed("1",NULL,NULL)==HP2DiagnosticRngSeedInvalid,
		"bootstrap accepted null outputs"
	);
	Require( HP2DiagnosticRngSeedForSrand(0)==1, "zero must normalize to one" );
	Require( HP2DiagnosticRngSeedForSrand((unsigned)RAND_MAX)==1, "RAND_MAX must normalize to one" );
	Require( HP2DiagnosticRngSeedForSrand(42)==42, "ordinary seed changed" );
	RequireEndpointStreamMatchesOne(0);
	RequireEndpointStreamMatchesOne((unsigned)RAND_MAX);

	puts("RngSeedTests: PASS");
	return 0;
}

/*=============================================================================
	HP2RngSeed.h: Strict pre-appInit diagnostic RNG seed parsing and normalization.
=============================================================================*/
#ifndef HP2RNGSEED_H
#define HP2RNGSEED_H

#include <stdlib.h>

// Accept only an ASCII unsigned decimal in the C rand() seed domain.  Parsing
// directly against RAND_MAX avoids accepting a host-width value that srand()
// would silently truncate or reinterpret.
static inline int HP2ParseDiagnosticRngSeed( const char* Text, unsigned* OutSeed )
{
	if( !Text || !*Text || !OutSeed )
		return 0;

	unsigned Value = 0;
	for( const char* It=Text; *It; ++It )
	{
		if( *It<'0' || *It>'9' )
			return 0;
		const unsigned Digit = (unsigned)(*It-'0');
		if( Value>((unsigned)RAND_MAX-Digit)/10u )
			return 0;
		Value = Value*10u + Digit;
	}
	*OutSeed = Value;
	return 1;
}

// The calibrated Rust stream normalizes a zero Park-Miller state to one.
// Preserve the inclusive diagnostic input domain while giving both engines
// the same stream for the two values that have a zero modulus.
static inline unsigned HP2DiagnosticRngSeedForSrand( unsigned Seed )
{
	return Seed==0 || Seed==(unsigned)RAND_MAX ? 1u : Seed;
}

enum EHP2DiagnosticRngSeedBootstrap
{
	HP2DiagnosticRngSeedAbsent,
	HP2DiagnosticRngSeedAccepted,
	HP2DiagnosticRngSeedInvalid
};

// An unset or empty environment value preserves ordinary startup. A nonempty
// value must parse strictly before StaticInit can seed libc rand().
static inline EHP2DiagnosticRngSeedBootstrap HP2PrepareDiagnosticRngSeed(
	const char* Text,
	unsigned* OutRequestedSeed,
	unsigned* OutEffectiveSeed
)
{
	if( !Text || !*Text )
		return HP2DiagnosticRngSeedAbsent;
	if( !OutRequestedSeed || !OutEffectiveSeed )
		return HP2DiagnosticRngSeedInvalid;

	unsigned RequestedSeed = 0;
	if( !HP2ParseDiagnosticRngSeed(Text,&RequestedSeed) )
		return HP2DiagnosticRngSeedInvalid;

	*OutRequestedSeed = RequestedSeed;
	*OutEffectiveSeed = HP2DiagnosticRngSeedForSrand(RequestedSeed);
	return HP2DiagnosticRngSeedAccepted;
}

#endif

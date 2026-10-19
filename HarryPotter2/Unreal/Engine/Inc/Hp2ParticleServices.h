/*=============================================================================
	Hp2ParticleServices.h: C++ services for the Rust particle port.

	Consumed by crates/hp2-engine/src/particle.rs (`Hp2ParticleServices`, #[repr(C)]).
	Field order must match the Rust side exactly. The Rust exports take every
	pointer as void*, so this header is only needed by the C++ translation units
	that pass GHp2ParticleServices to them.
=============================================================================*/

#ifndef _INC_HP2PARTICLESERVICES_H
#define _INC_HP2PARTICLESERVICES_H

class FVector;
class FPlane;

struct Hp2ParticleServices
{
	float (*AppFrand)();
	float (*SinTab)( int I );
	float (*CosTab)( int I );
	void  (*TotalWind)( void* Level, const FVector* Loc, FVector* Out );
	int   (*FastLineCheck)( void* Level, const FVector* End, const FVector* Start );
	int   (*SystemTraceLevelHit)( void* System, const FVector* End, const FVector* Start, FVector* Normal, FVector* Location );
	void  (*PalettePlane)( void* ColorPalette, float Age, float Lifetime, float ColorDelay, FPlane* Out );
};

extern const Hp2ParticleServices GHp2ParticleServices;

#endif

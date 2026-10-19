/*=============================================================================
	Hp2PaletteServices.cpp: C++ thunks for the Rust Heckbert quantiser port.

	Table consumed by crates/hp2-engine/src/palette.rs via palette.cpp.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2PaletteServices.h"

static void* Hp2PaletteMalloc( size_t Size, unsigned int Site )
{
	const TCHAR* Tag = TEXT("Histogram888");
	if( Site == 0 )
		Tag = TEXT("Histogram8888");
	else if( Site == 1 )
		Tag = TEXT("HistogramX888");
	return appMalloc( appCheckedAllocSize( static_cast<SIZE_T>(Size) ), Tag );
}

static void Hp2PaletteFree( void* Ptr )
{
	appFree( Ptr );
}

static void Hp2PaletteUnpopulatedBox( void )
{
	debugf( TEXT("Unpopulated box!\n") );
}

const Hp2PaletteServices GHp2PaletteServices =
{
	Hp2PaletteMalloc,
	Hp2PaletteFree,
	Hp2PaletteUnpopulatedBox,
};

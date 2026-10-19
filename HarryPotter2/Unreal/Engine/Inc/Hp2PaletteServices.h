/*=============================================================================
	Hp2PaletteServices.h: C++ services for the Rust Heckbert quantiser port.

	Consumed by crates/hp2-engine/src/palette.rs (`PaletteServices`, #[repr(C)]).
	Field order must match the Rust struct exactly. Thunks live in
	Hp2PaletteServices.cpp.
=============================================================================*/

#ifndef _INC_HP2PALETTESERVICES_H
#define _INC_HP2PALETTESERVICES_H

#include <stddef.h>

struct Hp2PaletteServices
{
	/* appMalloc(appCheckedAllocSize(size), tag_for(site)) */
	void* (*malloc)( size_t Size, unsigned int Site );
	/* appFree(Ptr) */
	void (*free)( void* Ptr );
	/* debugf(TEXT("Unpopulated box!\n")) */
	void (*unpopulated_box)( void );
};

extern const Hp2PaletteServices GHp2PaletteServices;

#endif

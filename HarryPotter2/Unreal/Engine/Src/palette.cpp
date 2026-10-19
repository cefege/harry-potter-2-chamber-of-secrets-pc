//
// Fast Heckbert quantiser for creating paletted textures from
// high/true colour input textures
//
// Alpha weighting is going to be a tricky thing - unlike the RGB weightings
// which are well defined, the importance of the alpha channel probably has to
// have the highest weighting of all. However, because it affects all the other
// components its correct weighting is almost certainly some product of the alpha
// value itself and the colours it encompasses, rather than a fixed value...
//
// Some code based on source from the Independent JPEG group
// Copyright (C) 1991-1998, Thomas G. Lane.
//
//
// A. Pomianowski
//

#include "EnginePrivate.h"
#include <stdlib.h>

#include "Palette.h"

#pragma warning( disable : 4706 )

#include "hp2_rs.h"
#include "Hp2PaletteServices.h"

// Heckbert quantiser body lives in crates/hp2-engine/src/palette.rs.
int HeckbertQuantize(void* srcdata, unsigned char* dstdata, unsigned short* paldata, int srcformat, int nentries, int width, int height, int has_colourkey, DWORD keyval, int generate_mipmaps)
{
	return hp2_rs_engine_palette_heckbert_quantize(&GHp2PaletteServices, srcdata, dstdata, paldata, srcformat, nentries, width, height, has_colourkey, keyval, generate_mipmaps);
}

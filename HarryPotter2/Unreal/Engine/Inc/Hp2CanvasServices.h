/*=============================================================================
	Hp2CanvasServices.h: Engine callbacks for the Rust UCanvas/UFont text and
	tile port (crates/hp2-engine/src/canvas_text.rs, crates/hp2-ffi/src/canvas_text.rs).
	The table is defined in Hp2CanvasServices.cpp.
=============================================================================*/

#ifndef HP2CANVASSERVICES_H
#define HP2CANVASSERVICES_H

#include "hp2_rs.h"

// Texture slots used by the DrawString path (FTextureInfo Infos[5]); matches TEXT_SLOTS in canvas_text.rs.
enum { HP2_CANVAS_TEXT_SLOTS = 5 };


extern const Hp2CanvasServices GHp2CanvasServices;

#endif

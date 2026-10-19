/*=============================================================================
	Hp2PawnNativeFilesServices.h: Cache, file, URL, console, and viewport
	services called by the Rust engine port
	(crates/hp2-engine/src/pawn_native_files.rs). The table is defined in
	Hp2PawnNativeFilesServices.cpp.

	Include after EnginePrivate.h. The text helpers convert between engine
	TCHAR strings and the NUL-terminated UNICHAR text the Rust exports take.
=============================================================================*/

#ifndef HP2PAWNNATIVEFILESSERVICES_H
#define HP2PAWNNATIVEFILESSERVICES_H

#include "hp2_rs.h"

extern const Hp2PawnNativeFilesServices GHp2PawnNativeFilesServices;

/* Engine text to a NUL-terminated UNICHAR array for the Rust exports. */
TArray<UNICHAR> Hp2PawnNativeFilesUnits( const TCHAR* Text );

/* NUL-terminated UNICHAR text from Rust to an engine string. */
FString Hp2PawnNativeFilesString( const UNICHAR* Units );

#endif

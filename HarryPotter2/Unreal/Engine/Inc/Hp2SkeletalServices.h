/*=============================================================================
	Hp2SkeletalServices.h: Services table for the Rust USkeletalMesh and
	UAnimation port (crates/hp2-engine/src/skeletal.rs, FFI in
	crates/hp2-ffi/src/engine_skeletal.rs). The table is defined in
	Hp2SkeletalServices.cpp. Layout is checked by the static_assert below.
	Include after EnginePrivate.h.
=============================================================================*/

#ifndef HP2SKELETALSERVICES_H
#define HP2SKELETALSERVICES_H

#include <stddef.h>
#include "hp2_rs.h"

// Mirrors Rust Hp2SkeletalServices: the GMath and GCache entry points and the
// host hooks the skeletal core calls through. Cache calls take the owning
// actor; the host computes MakeCacheID(CID_SkeletalData, Owner, NULL).
struct Hp2SkeletalServices
{
	float    (*SinTab)( int32_t I );
	float    (*CosTab)( int32_t I );
	uint8_t* (*CacheGet)( const void* Owner, void** Item );
	uint8_t* (*CacheCreate)( const void* Owner, size_t Len, void** Item );
	void     (*CacheFlush)( const void* Owner );
	void     (*CacheUnlock)( void* Item );
	void     (*WarnMissingLink)( const void* Mesh, int32_t Bone );
	void     (*RemoveAux)( void* Owner, int32_t Index );
	void     (*DebugPivots)( void* Mesh, const void* Bases, int32_t Num );
	void     (*DebugInfluences)( void* Mesh );
	void     (*FailCheck)( const char* Expr );
};

extern const Hp2SkeletalServices GHp2SkeletalServices;

static_assert( sizeof(Hp2SkeletalServices) == 88, "Hp2SkeletalServices layout" );

#endif

/*=============================================================================
	Hp2MeshServices.h: Services and argument mirrors for the Rust UMesh and
	ULodMesh port (crates/hp2-engine/src/mesh.rs and lodmesh.rs, FFI in
	crates/hp2-ffi/src/mesh.rs). The table is defined in Hp2MeshServices.cpp.
	Layouts are checked by the static_asserts below. Include after
	EnginePrivate.h.
=============================================================================*/

#ifndef HP2MESHSERVICES_H
#define HP2MESHSERVICES_H

#include <stddef.h>
#include "hp2_rs.h"

// Mirrors Rust Hp2MeshServices: the GCache and GMath entry points the mesh
// core calls through. Fields are the same C types as the Rust fn pointers.
struct Hp2MeshServices
{
	uint8_t* (*CacheGet)( uint64_t Id, void** Item );
	uint8_t* (*CacheCreate)( uint64_t Id, int32_t Len, void** Item );
	void     (*CacheFlush)( uint64_t Id );
	void     (*CacheUnlock)( void* Item );
	float    (*SinTab)( int32_t I );
	float    (*CosTab)( int32_t I );
};

// Mirrors Rust Hp2MeshArg: the UMesh fields the bounds and frame code reads.
// Pointers are the data of Verts, AnimSeqs and BoundingBoxes after Load().
struct Hp2MeshArg
{
	INT         FrameVerts;
	const void* Verts;
	INT         NumVerts;
	const void* AnimSeqs;
	INT         NumAnimSeqs;
	const void* BoundingBoxes;
	INT         NumBoundingBoxes;
	FBox        BoundingBox;
	FVector     Scale;
	FVector     Origin;
	FRotator    RotOrigin;
	const void* Mesh;
};

// Mirrors Rust Hp2LodArg: a Hp2MeshArg plus the ULodMesh fields.
struct Hp2LodArg
{
	Hp2MeshArg Mesh;
	INT        SpecialVerts;
	INT        LODRequest;
};

// Mirrors Rust Hp2OwnerArg: the AActor fields the mesh code reads.
struct Hp2OwnerArg
{
	FVector  Location;
	FVector  PrePivot;
	FRotator Rotation;
	FLOAT    DrawScale;
	INT      AnimSequence;
	FLOAT    AnimFrame;
	INT      HasParent;
	INT      ParentSequence;
	FLOAT    ParentFrame;
	INT      AnimByOwner;
	QWORD    CacheID;
};

extern const Hp2MeshServices GHp2MeshServices;

class AActor;
class UMesh;

// Fill the argument blocks from the live objects. Call after Verts is loaded.
void Hp2FillMeshArg( Hp2MeshArg& Arg, const UMesh& Mesh );
// Fills every field except CacheID, which the GetFrame caller sets.
void Hp2FillOwnerArg( Hp2OwnerArg& Arg, const AActor& Owner );

static_assert( sizeof(Hp2MeshServices) == 48, "Hp2MeshServices layout" );
static_assert( sizeof(FBox) == 28, "Hp2MeshArg layout" );
static_assert( sizeof(FRotator) == 12, "Hp2MeshArg layout" );
static_assert( offsetof(Hp2MeshArg, Verts) == 8, "Hp2MeshArg layout" );
static_assert( offsetof(Hp2MeshArg, AnimSeqs) == 24, "Hp2MeshArg layout" );
static_assert( offsetof(Hp2MeshArg, BoundingBoxes) == 40, "Hp2MeshArg layout" );
static_assert( offsetof(Hp2MeshArg, BoundingBox) == 52, "Hp2MeshArg layout" );
static_assert( offsetof(Hp2MeshArg, Scale) == 80, "Hp2MeshArg layout" );
static_assert( offsetof(Hp2MeshArg, RotOrigin) == 104, "Hp2MeshArg layout" );
static_assert( sizeof(Hp2MeshArg) == 128, "Hp2MeshArg layout" );
static_assert( offsetof(Hp2LodArg, SpecialVerts) == 128, "Hp2LodArg layout" );
static_assert( sizeof(Hp2LodArg) == 136, "Hp2LodArg layout" );
static_assert( offsetof(Hp2OwnerArg, DrawScale) == 36, "Hp2OwnerArg layout" );
static_assert( offsetof(Hp2OwnerArg, AnimFrame) == 44, "Hp2OwnerArg layout" );
static_assert( offsetof(Hp2OwnerArg, ParentFrame) == 56, "Hp2OwnerArg layout" );
static_assert( offsetof(Hp2OwnerArg, CacheID) == 64, "Hp2OwnerArg layout" );
static_assert( sizeof(Hp2OwnerArg) == 72, "Hp2OwnerArg layout" );

#endif

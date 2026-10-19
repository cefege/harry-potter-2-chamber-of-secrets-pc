/*=============================================================================
	Hp2MeshServices.cpp: GCache and GMath entry points for the Rust UMesh and
	ULodMesh port (crates/hp2-engine/src/mesh.rs). The wrappers call the same
	two- and three-argument overloads the original GetFrame code used, so the
	default alignment and safety pad are unchanged.
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnMesh.h"
#include "Hp2MeshServices.h"

// Leaf layouts that Rust reads with fixed strides (crates/hp2-engine/src/mesh.rs).
static_assert( sizeof(FMeshAnimSeq) == 40, "FMeshAnimSeq layout" );
static_assert( sizeof(FMeshVert) == 4, "FMeshVert layout" );
static_assert( sizeof(FCoords) == 48, "FCoords layout" );
static_assert( sizeof(FName) == 4, "FName layout" );

static uint8_t* MeshCacheGet( uint64_t Id, void** Item )
{
	FCacheItem* CacheItem = NULL;
	BYTE* Mem = GCache.Get( Id, CacheItem );
	*Item = CacheItem;
	return Mem;
}

static uint8_t* MeshCacheCreate( uint64_t Id, int32_t Len, void** Item )
{
	FCacheItem* CacheItem = NULL;
	BYTE* Mem = GCache.Create( Id, CacheItem, Len );
	*Item = CacheItem;
	return Mem;
}

static void MeshCacheFlush( uint64_t Id )
{
	GCache.Flush( Id );
}

static void MeshCacheUnlock( void* Item )
{
	((FCacheItem*)Item)->Unlock();
}

static float MeshSinTab( int32_t I )
{
	return GMath.SinTab( I );
}

static float MeshCosTab( int32_t I )
{
	return GMath.CosTab( I );
}

const Hp2MeshServices GHp2MeshServices =
{
	MeshCacheGet,
	MeshCacheCreate,
	MeshCacheFlush,
	MeshCacheUnlock,
	MeshSinTab,
	MeshCosTab,
};

void Hp2FillMeshArg( Hp2MeshArg& Arg, const UMesh& Mesh )
{
	Arg.FrameVerts        = Mesh.FrameVerts;
	Arg.Verts             = Mesh.Verts.GetData();
	Arg.NumVerts          = Mesh.Verts.Num();
	Arg.AnimSeqs          = Mesh.AnimSeqs.GetData();
	Arg.NumAnimSeqs       = Mesh.AnimSeqs.Num();
	Arg.BoundingBoxes     = Mesh.BoundingBoxes.GetData();
	Arg.NumBoundingBoxes  = Mesh.BoundingBoxes.Num();
	Arg.BoundingBox       = Mesh.BoundingBox;
	Arg.Scale             = Mesh.Scale;
	Arg.Origin            = Mesh.Origin;
	Arg.RotOrigin         = Mesh.RotOrigin;
	Arg.Mesh              = &Mesh;
}

void Hp2FillOwnerArg( Hp2OwnerArg& Arg, const AActor& Owner )
{
	Arg.Location         = Owner.Location;
	Arg.PrePivot         = Owner.PrePivot;
	Arg.Rotation         = Owner.Rotation;
	Arg.DrawScale        = Owner.DrawScale;
	Arg.AnimSequence     = Owner.AnimSequence.GetIndex();
	Arg.AnimFrame        = Owner.AnimFrame;
	Arg.HasParent        = Owner.Owner != NULL;
	// The parent fields are read only when HasParent is set.
	Arg.ParentSequence   = Owner.Owner ? Owner.Owner->AnimSequence.GetIndex() : 0;
	Arg.ParentFrame      = Owner.Owner ? Owner.Owner->AnimFrame : 0.0f;
	Arg.AnimByOwner      = Owner.bAnimByOwner;
	Arg.CacheID          = 0;
}

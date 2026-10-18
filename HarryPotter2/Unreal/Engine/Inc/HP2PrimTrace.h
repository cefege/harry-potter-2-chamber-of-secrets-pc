/*=============================================================================
	HP2PrimTrace.h: Argument mirrors and fill helpers for the Rust collision
	and BSP trace ports (crates/hp2-engine/src/prim.rs and trace.rs, FFI in
	crates/hp2-ffi/src/engine_prim_trace.rs). Layouts are checked by the
	static_asserts below. Include after EnginePrivate.h.
=============================================================================*/

#ifndef HP2PRIMTRACE_H
#define HP2PRIMTRACE_H

#include <stddef.h>
#include "hp2_rs.h"

// Mirrors Rust Hp2PrimOwner. Rust never dereferences Actor; it only hands the
// pointer back through FCheckResult::Actor.
struct Hp2PrimOwnerArg
{
	void*   Actor;
	FVector Location;
	FLOAT   CollisionRadius;
	FLOAT   CollisionWidth;
	FLOAT   CollisionHeight;
	FCoords ToLocal;
	FCoords ToWorld;
	FVector CylExtent;
	FVector CylExtentZeroWidth;
};

// Mirrors Rust Hp2BspArgs: the UModel BSP arrays a query reads.
struct Hp2BspArgs
{
	const void* Nodes;
	INT         NumNodes;
	const void* Points;
	INT         NumPoints;
	const void* Verts;
	INT         NumVerts;
	const void* Surfs;
	INT         NumSurfs;
};

// Mirrors Rust Hp2TraceModel: the UModel fields the BSP traces read.
struct Hp2TraceModelArg
{
	Hp2BspArgs bsp;
	const void* LeafHulls;
	INT         NumLeafHulls;
	FBox        BoundingBox;
	INT         RootOutside;
	INT         NumZones;
	void*       Primitive;
};

static_assert( sizeof(Hp2PrimOwnerArg) == 152, "Hp2PrimOwnerArg layout" );
static_assert( offsetof(Hp2PrimOwnerArg, Location) == 8, "Hp2PrimOwnerArg layout" );
static_assert( offsetof(Hp2PrimOwnerArg, CollisionRadius) == 20, "Hp2PrimOwnerArg layout" );
static_assert( offsetof(Hp2PrimOwnerArg, CollisionWidth) == 24, "Hp2PrimOwnerArg layout" );
static_assert( offsetof(Hp2PrimOwnerArg, CollisionHeight) == 28, "Hp2PrimOwnerArg layout" );
static_assert( offsetof(Hp2PrimOwnerArg, ToLocal) == 32, "Hp2PrimOwnerArg layout" );
static_assert( offsetof(Hp2PrimOwnerArg, ToWorld) == 80, "Hp2PrimOwnerArg layout" );
static_assert( offsetof(Hp2PrimOwnerArg, CylExtent) == 128, "Hp2PrimOwnerArg layout" );
static_assert( offsetof(Hp2PrimOwnerArg, CylExtentZeroWidth) == 140, "Hp2PrimOwnerArg layout" );
static_assert( sizeof(Hp2BspArgs) == 64, "Hp2BspArgs layout" );
static_assert( sizeof(Hp2TraceModelArg) == 120, "Hp2TraceModelArg layout" );
static_assert( offsetof(Hp2TraceModelArg, LeafHulls) == 64, "Hp2TraceModelArg layout" );
static_assert( offsetof(Hp2TraceModelArg, BoundingBox) == 76, "Hp2TraceModelArg layout" );
static_assert( offsetof(Hp2TraceModelArg, RootOutside) == 104, "Hp2TraceModelArg layout" );
static_assert( offsetof(Hp2TraceModelArg, Primitive) == 112, "Hp2TraceModelArg layout" );

// Which derived owner fields a call needs (Rust reads only the requested ones).
enum
{
	HP2_OWNER_FRAMES        = 1, // ToLocal, ToWorld
	HP2_OWNER_CYL_EXTENT    = 2, // CylExtent = Owner->GetCylinderExtent()
	HP2_OWNER_CYL_ZERO_WIDTH = 4 // CylExtentZeroWidth (CollisionWidth forced to 0)
};

// Cylinder extent with CollisionWidth temporarily zeroed. Restores the width.
inline FVector Hp2CylExtentZeroWidth( const AActor* Owner )
{
	AActor* A = const_cast<AActor*>(Owner);
	FLOAT Saved = A->CollisionWidth;
	A->CollisionWidth = 0.0f;
	FVector Ext = A->GetCylinderExtent();
	A->CollisionWidth = Saved;
	return Ext;
}

// Fills Arg for Owner and returns its pointer, or NULL for a NULL owner (the
// Rust side treats NULL as "no owner").
inline const void* Hp2PrimOwnerFor( const AActor* Owner, DWORD Fields, Hp2PrimOwnerArg& Arg )
{
	if( !Owner )
		return NULL;
	appMemzero( &Arg, sizeof(Arg) );
	Arg.Actor            = const_cast<AActor*>(Owner);
	Arg.Location         = Owner->Location;
	Arg.CollisionRadius  = Owner->CollisionRadius;
	Arg.CollisionWidth   = Owner->CollisionWidth;
	Arg.CollisionHeight  = Owner->CollisionHeight;
	if( Fields & HP2_OWNER_FRAMES )
	{
		Arg.ToLocal = Owner->ToLocal();
		Arg.ToWorld = Owner->ToWorld();
	}
	if( Fields & HP2_OWNER_CYL_EXTENT )
		Arg.CylExtent = Owner->GetCylinderExtent();
	if( Fields & HP2_OWNER_CYL_ZERO_WIDTH )
		Arg.CylExtentZeroWidth = Hp2CylExtentZeroWidth( Owner );
	return &Arg;
}

// Array data pointers for the Rust side (NULL when empty).
template<class T> inline const void* Hp2ArrayPtr( const TArray<T>& A )
{
	return A.Num() ? A.GetData() : NULL;
}
template<class T> inline const void* Hp2ArrayPtr( const TTransArray<T>& A )
{
	return A.Num() ? &A(0) : NULL;
}

inline void Hp2BspArgsFor( const UModel* Model, Hp2BspArgs& A )
{
	A.Nodes     = Hp2ArrayPtr( Model->Nodes );
	A.NumNodes  = Model->Nodes.Num();
	A.Points    = Hp2ArrayPtr( Model->Points );
	A.NumPoints = Model->Points.Num();
	A.Verts     = Hp2ArrayPtr( Model->Verts );
	A.NumVerts  = Model->Verts.Num();
	A.Surfs     = Hp2ArrayPtr( Model->Surfs );
	A.NumSurfs  = Model->Surfs.Num();
}

inline void Hp2TraceModelFor( const UModel* Model, Hp2TraceModelArg& A )
{
	Hp2BspArgsFor( Model, A.bsp );
	A.LeafHulls     = Hp2ArrayPtr( Model->LeafHulls );
	A.NumLeafHulls  = Model->LeafHulls.Num();
	A.BoundingBox   = Model->BoundingBox;
	A.RootOutside   = Model->RootOutside ? 1 : 0;
	A.NumZones      = Model->NumZones;
	A.Primitive     = static_cast<UPrimitive*>( const_cast<UModel*>(Model) );
}

#endif

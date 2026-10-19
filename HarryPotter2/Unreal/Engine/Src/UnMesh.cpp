/*=============================================================================
	UnMesh.cpp: Unreal mesh animation functions
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnMesh.h"
#include "Hp2MeshServices.h"

/*-----------------------------------------------------------------------------
	UMesh object implementation.
-----------------------------------------------------------------------------*/

UMesh::UMesh()
{
	guard(UMesh::UMesh);

	// Scaling.
	Scale			 = FVector(1.f,1.f,1.f);
	Origin			 = FVector(0.f,0.f,0.f);
	RotOrigin		 = FRotator(0.f,0.f,0.f);

	// Flags.
	AndFlags		 = ~(DWORD)0;
	OrFlags			 = 0;

	unguardobj;
}
void UMesh::Serialize( FArchive& Ar )
{
	guard(UMesh::Serialize);
	Super::Serialize(Ar);

	// Serialize this.
	Ar << Verts;
	Ar << Tris;
	if( Tris.Num() )
		check(Tris(0).iVertex[0]<Verts.Num());
	Ar << AnimSeqs;
	Ar << Connects << BoundingBox << BoundingSphere << VertLinks << Textures;
	Ar << BoundingBoxes << BoundingSpheres;
	Ar << FrameVerts << AnimFrames;
	Ar << AndFlags << OrFlags;
	Ar << Scale << Origin << RotOrigin;
	Ar << CurPoly << CurVertex;
	if( Ar.Ver()==65 )
		{FLOAT F; Ar << F;}
	if( Ar.Ver()>=66 )
		Ar << TextureLOD;

	unguardobj;
}
void UMesh::SetScale( FVector NewScale )
{
	Scale = NewScale;
}
IMPLEMENT_CLASS(UMesh);

/*-----------------------------------------------------------------------------
	UMesh collision interface.
-----------------------------------------------------------------------------*/

//
// Get the rendering bounding box for this primitive, as owned by Owner.
//
FCoords UMesh::GetRenderBoundingBox( const AActor* Owner, UBOOL Exact )
{
	guard(UMesh::GetRenderBoundingBox);
	Hp2MeshArg MeshArg;
	Hp2FillMeshArg( MeshArg, *this );
	Hp2OwnerArg OwnerArg;
	Hp2FillOwnerArg( OwnerArg, *Owner );
	FCoords Coords;
	hp2_rs_engine_mesh_render_bounding_box( &GHp2MeshServices, &MeshArg, &OwnerArg, &Coords );
	return Coords;
	unguardobj;
}

// Override UBoxPrim behaviour to incorporate scale and origin.
FBox UMesh::GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const
{
	Hp2MeshArg MeshArg;
	Hp2FillMeshArg( MeshArg, *this );
	Hp2OwnerArg OwnerArg;
	Hp2FillOwnerArg( OwnerArg, *Owner );
	FBox Bound;
	hp2_rs_engine_mesh_collision_bounding_box( &GHp2MeshServices, &MeshArg, &OwnerArg, bWorld ? 1 : 0, &Bound );
	return Bound;
}
FVector UMesh::GetCollisionExtent( const AActor* Owner ) const
{
	Hp2MeshArg MeshArg;
	Hp2FillMeshArg( MeshArg, *this );
	Hp2OwnerArg OwnerArg;
	Hp2FillOwnerArg( OwnerArg, *Owner );
	FVector Extent;
	hp2_rs_engine_mesh_collision_extent( &GHp2MeshServices, &MeshArg, &OwnerArg, &Extent );
	return Extent;
}

/*-----------------------------------------------------------------------------
	UMesh animation interface.
-----------------------------------------------------------------------------*/

const FMeshAnimSeq* UMesh::GetAnimSeq( FName SeqName ) const
{
	guardSlow(UMesh::GetAnimSeq);
	INT Index = hp2_rs_engine_mesh_anim_seq_index( AnimSeqs.GetData(), AnimSeqs.Num(), SeqName.GetIndex() );
	return Index >= 0 ? AnimSeqs.GetData() + Index : NULL;
	unguardSlow;
}
FMeshAnimSeq* UMesh::GetAnimSeq( FName SeqName )
{
	guardSlow(UMesh::GetAnimSeq);
	INT Index = hp2_rs_engine_mesh_anim_seq_index( AnimSeqs.GetData(), AnimSeqs.Num(), SeqName.GetIndex() );
	return Index >= 0 ? AnimSeqs.GetData() + Index : NULL;
	unguardSlow;
}

//
// Get the transformed point set corresponding to the animation frame
// of this primitive owned by Owner. Returns the total outcode of the points.
//
void UMesh::GetFrame
(
	FVector*	ResultVerts,
	INT			Size,
	FCoords		Coords,
	AActor*		Owner
)
{
	guard(UMesh::GetFrame);

	// Make sure lazy-loadable arrays are ready.
	Verts.Load();
	Tris.Load();
	Connects.Load();
	VertLinks.Load();

	// The tween cache and frame math live in Rust (crates/hp2-engine/src/mesh.rs).
	Hp2MeshArg MeshArg;
	Hp2FillMeshArg( MeshArg, *this );
	Hp2OwnerArg OwnerArg;
	Hp2FillOwnerArg( OwnerArg, *Owner );
	OwnerArg.CacheID = MakeCacheID( CID_TweenAnim, Owner, NULL );
	hp2_rs_engine_mesh_get_frame( &GHp2MeshServices, &MeshArg, &OwnerArg, &Coords, ResultVerts, Size );
	unguardobj;
}

FCoords UMesh::GetBoneCoords( AActor* Owner, int Bone ) const
{
	Hp2MeshArg MeshArg;
	Hp2FillMeshArg( MeshArg, *this );
	Hp2OwnerArg OwnerArg;
	Hp2FillOwnerArg( OwnerArg, *Owner );
	FCoords Coords;
	hp2_rs_engine_mesh_bone_coords( &GHp2MeshServices, &MeshArg, &OwnerArg, &Coords );
	return Coords;
}

/*-----------------------------------------------------------------------------
	UMesh constructor.
-----------------------------------------------------------------------------*/

//
// UMesh constructor.
//
UMesh::UMesh( INT NumPolys, INT NumVerts, INT NumFrames )
{
	guard(UMesh::UMesh);

	// Set counts.
	FrameVerts	= NumVerts;
	AnimFrames	= NumFrames;

	// Allocate all stuff.
	Tris			.Add(NumPolys);
	Verts			.Add(NumVerts * NumFrames);
	Connects		.Add(NumVerts);
	BoundingBoxes	.Add(NumFrames);
	BoundingSpheres .Add(NumFrames);

	// Init textures.
	for( INT i=0; i<Textures.Num(); i++ )
		Textures(i) = NULL;

	unguardobj;
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

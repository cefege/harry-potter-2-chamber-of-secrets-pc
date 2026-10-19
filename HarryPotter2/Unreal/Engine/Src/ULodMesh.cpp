/*=============================================================================
	UnLodMesh.cpp: Unreal mesh animation functions
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
		* Subclassed for level-of-detail meshes by Erik de Neve
		 
	    * Remarks
		  - New methods: the explicit ctor, and GetFrame()
		  - Distinguishing Mesh from LodMesh : if( Mesh->IsA(ULodMesh::StaticClass()) )
		  - No specific amd3d support.

=============================================================================*/ 

#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnMesh.h"
#include "Hp2MeshServices.h"

/*-----------------------------------------------------------------------------
	ULodMesh object implementation.
-----------------------------------------------------------------------------*/

void ULodMesh::Serialize( FArchive& Ar )
{
	guard(ULodMesh::Serialize);

	// Empty those structures not needed for LOD mesh rendering.
	// Vertlinks and Connects already empty for LOD meshes.
	if( Ar.IsSaving() )
	{
		Tris.Empty();
	}

	// Serialize parent's variables
	Super::Serialize(Ar);

	// Serialize the additional LodMesh variables.
	Ar << CollapsePointThus;
	Ar << FaceLevel;
	Ar << Faces;
	Ar << CollapseWedgeThus;
	Ar << Wedges;
	Ar << Materials;
	Ar << SpecialFaces;
	Ar << ModelVerts << SpecialVerts;
	Ar << MeshScaleMax;
	Ar << LODHysteresis << LODStrength << LODMinVerts << LODMorph << LODZDisplace;
	Ar << RemapAnimVerts << OldFrameVerts;

	// Quickly remap animation vertices at loading time.
	if( Ar.IsLoading() )
	{
		LODMinVerts = Min(LODMinVerts, FrameVerts);
		if ( RemapAnimVerts.Num() )
		{
			// Make sure lazy arrays got loaded.
			Verts.Load();

			// Bigass new array.

			TArray<FMeshVert> NewVerts;
			NewVerts.Add( FrameVerts * AnimFrames);
			for( INT f=0; f<AnimFrames; f++ )
			{
				INT FrameStart = f * OldFrameVerts;
				INT FullFrameStart = f * FrameVerts;
				// Copy permutated.
				for( INT v=0; v<FrameVerts; v++ )
				{
					// If necessary this process duplicates or throws out vertices...
					NewVerts(FullFrameStart + v) = Verts( RemapAnimVerts(v) + FrameStart); 					
				}
			}

			// Kludgy array exchange.
			Verts.Empty();
			Verts.Add(NewVerts.Num());
			ExchangeArray(Verts, NewVerts);
			NewVerts.Empty();			

			// Discard remapping array
			RemapAnimVerts.Empty();
		}
	}

	unguardobj;
}
IMPLEMENT_CLASS(ULodMesh);

void ULodMesh::SetScale( FVector NewScale )
{
	guard(ULodMesh::SetScale);

	Scale = NewScale;
	// Maximum mesh scaling dimension for LOD gauging (lodmesh.rs: mesh_scale_max).
	MeshScaleMax = hp2_rs_engine_lod_mesh_scale_max( BoundingSphere.W, &Scale );

	unguardobj;
}



/*-----------------------------------------------------------------------------
	ULodMesh constructor.
-----------------------------------------------------------------------------*/
//
// ULodMesh constructor.
//
ULodMesh::ULodMesh( INT NumPolys, INT NumVerts, INT NumFrames)
{
	guard(ULodMesh::ULodMesh);

	// Default LOD settings.
	LODMinVerts		= 10;		// Minimum number of vertices with which to draw a model. (Minimum for a cube = 8...)
	LODStrength		= 1.00f;	// Scales the (not necessarily linear) falloff of vertices with distance.
	LODMorph        = 0.30f;	// Morphing range. 0.0f = no morphing.
	LODZDisplace    = 0.00f;    // Z-displacement (in world units) for falloff function tweaking.
	LODHysteresis	= 0.00f;	// Controls LOD-level change delay/morphing. (unused)

	// Set counts.
	FrameVerts	= NumVerts;
	AnimFrames	= NumFrames;

	// Allocate all stuff.
	Tris			.Add(NumPolys);
	Verts			.Add(NumVerts * NumFrames);
	BoundingBoxes	.Add(NumFrames);
	BoundingSpheres .Add(NumFrames);

	// Init textures.
	for( INT i=0; i<Textures.Num(); i++ )
		Textures(i) = NULL;

	unguardobj;
}


/*-----------------------------------------------------------------------------
	ULodMesh animation interface.
-----------------------------------------------------------------------------*/

void ULodMesh::GetFrame
(
	FVector*	ResultVerts,
	INT			Size,
	FCoords		Coords,
	AActor*		Owner,
	INT&		LODRequest
)
{
	guard(UMesh::GetFrame);

	// Make sure any used lazy-loadable arrays are ready.
	Verts.Load();

	// The LOD tween cache and frame math live in Rust (crates/hp2-engine/src/lodmesh.rs).
	Hp2LodArg LodArg;
	Hp2FillMeshArg( LodArg.Mesh, *this );
	LodArg.SpecialVerts = SpecialVerts;
	LodArg.LODRequest   = LODRequest;
	Hp2OwnerArg OwnerArg;
	Hp2FillOwnerArg( OwnerArg, *Owner );
	OwnerArg.CacheID = MakeCacheID( CID_TweenAnim, Owner, NULL );
	LODRequest = hp2_rs_engine_lod_mesh_get_frame( &GHp2MeshServices, &LodArg, &OwnerArg, &Coords, ResultVerts, Size );

	unguardobj;
}


/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

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
	// Maximum mesh scaling dimension for LOD gauging.
	MeshScaleMax = (1.f/ 128.f) * BoundingSphere.W * Max(Abs(Scale.X), Max(Abs(Scale.Y), Abs(Scale.Z)));
	//debugf(TEXT("New MeshScaleMax %f "),MeshScaleMax); 

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

// Cached frame header struct for temp cached meshes.
struct CFLodHeader
{
	UMesh	*CachedMesh;
	FLOAT	CachedFrame;
	FName	CachedSeq;	
	INT     CachedLodVerts;
	FLOAT   TweenIndicator;
};	

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

	AActor*	AnimOwner = NULL;

	// Check to see if bAnimByOwner
	if ((Owner->bAnimByOwner) && (Owner->Owner != NULL))
		AnimOwner = Owner->Owner;
	else
		AnimOwner = Owner;

	// Determine how many vertices to lerp; in case of tweening, we're limited 
	// by the previous cache size also.
	INT VertsRequested = Min(LODRequest + SpecialVerts, FrameVerts);	
	INT VertexNum = VertsRequested;

	// Create or get cache memory.
	FMemCache::FCacheItem* Item = NULL;
	UBOOL WasCached  = 1;
	QWORD CacheID    = MakeCacheID( CID_TweenAnim, Owner, NULL );
	BYTE* Mem = GCache.Get( CacheID, Item );
	CFLodHeader* FrameHdr = (CFLodHeader*)Mem;

	SIZE_T AlignHint = Align(sizeof(CFLodHeader), 16) - sizeof(CFLodHeader); // GM: AlignHint
	if( Mem==NULL || FrameHdr->CachedMesh !=this )
	{
		if( Mem != NULL )
		{
			// Actor's mesh changed.
			Item->Unlock();
			GCache.Flush( CacheID );
		}
		// Full size cache (for now.) We don't want to have to realloc every time our LOD scales up a bit...
		Mem = GCache.Create( CacheID, Item, appCheckedIntSize(sizeof(CFLodHeader) + AlignHint + static_cast<SIZE_T>(FrameVerts) * sizeof(FVector)) );
		//check(Mem%16 == 0);
		FrameHdr = (CFLodHeader*)Mem;
		WasCached = 0;
		FrameHdr->TweenIndicator = 1.0f;
	}

	if( !WasCached )
	{
		FrameHdr->CachedMesh  = this;
		FrameHdr->CachedSeq   = NAME_None;
		FrameHdr->CachedFrame = 0.f;
		FrameHdr->CachedLodVerts = 0;
	}

	// Get stuff.
	FVector* CachedVerts    = (FVector*)((BYTE*)Mem + sizeof(CFLodHeader) + AlignHint);
	Coords                  = Coords * (Owner->Location + Owner->PrePivot) * Owner->Rotation * RotOrigin * FScale(Scale * Owner->DrawScale,0.f,SHEER_None);
	const FMeshAnimSeq* Seq = GetAnimSeq( AnimOwner->AnimSequence );

	// Integrate origin offset into screenspace trafo.
	FCoords LocalCoords = Coords;
	LocalCoords.Origin += Origin;

	if( AnimOwner->AnimFrame>=0.f || !WasCached )
	{		
		LODRequest = VertexNum - SpecialVerts; // How many regular vertices returned.
		FrameHdr->CachedLodVerts = VertexNum;  //

		// Compute interpolation numbers.
		FLOAT Alpha=0.f;
		INT iFrameOffset1=0, iFrameOffset2=0;
		if( Seq )
		{
			FLOAT Frame   = ::Max(AnimOwner->AnimFrame,0.f) * Seq->NumFrames;
#if !__PSX2_EE__
			INT iFrame    = appFloor(Frame);
#else
			INT iFrame    = appRound(Frame-0.5f); // Temporary PSX2 appFloor bug workaround
#endif
			Alpha         = Frame - iFrame;
			iFrameOffset1 = (Seq->StartFrame + ((iFrame + 0) % Seq->NumFrames)) * FrameVerts;
			iFrameOffset2 = (Seq->StartFrame + ((iFrame + 1) % Seq->NumFrames)) * FrameVerts;
		}

		// Special case Alpha 0. 
		if( Alpha <= 0.f )
		{
			guard(SpecialAlphaZero);
			// Initialize a single frame.
			FMeshVert* MeshVertex1 = &Verts( iFrameOffset1 );

			for( INT i=0; i<VertexNum; i++ )
			{
				// Expand new vector from stored compact integers.
				CachedVerts[i] = FVector( MeshVertex1[i].X, MeshVertex1[i].Y, MeshVertex1[i].Z );

				// Transform all points into screenspace.
				*ResultVerts = CachedVerts[i].TransformPointBy(LocalCoords);
				*(BYTE**)&ResultVerts += Size;
			}	
			unguard;
		}
		else
		{	
			guard(InterpolateTwoFrames);
			// Interpolate two frames.
			FMeshVert* MeshVertex1 = &Verts( iFrameOffset1 );
			FMeshVert* MeshVertex2 = &Verts( iFrameOffset2 );

			
			for( INT i=0; i<VertexNum; i++ )
			{
				// Expand new vector from stored compact integers.
				FVector V1( MeshVertex1[i].X, MeshVertex1[i].Y, MeshVertex1[i].Z );
				FVector V2( MeshVertex2[i].X, MeshVertex2[i].Y, MeshVertex2[i].Z );
				CachedVerts[i] = V1 + (V2-V1)*Alpha;

				// Transform all points into screenspace.
				*ResultVerts = CachedVerts[i].TransformPointBy(LocalCoords);
				*(BYTE**)&ResultVerts += Size;
			}
			unguard;
		}	
	}
	else // Tween: cache present, and starting from Animframe < 0.f
	{		
		// Any requested number within CACHE limit is ok, since 
		// we cannot tween more than we have in the cache.
		VertexNum  = Min(VertexNum,FrameHdr->CachedLodVerts);
		FrameHdr->CachedLodVerts = VertexNum;
		LODRequest = VertexNum - SpecialVerts; // how many regular vertices returned.

		// Compute tweening numbers.
		FLOAT StartFrame = Seq ? (-1.f / Seq->NumFrames) : 0.f;
		INT iFrameOffset = Seq ? Seq->StartFrame * FrameVerts : 0;
		FLOAT Alpha = 1.f - AnimOwner->AnimFrame / FrameHdr->CachedFrame;

		if( FrameHdr->CachedSeq!=AnimOwner->AnimSequence )
		{
			FrameHdr->TweenIndicator = 0.f;
		}
		
		// Original:
		if( FrameHdr->CachedSeq!=AnimOwner->AnimSequence || Alpha<0.f || Alpha>1.f)
		{
			FrameHdr->CachedFrame = StartFrame; 
			Alpha       = 0.f;
			FrameHdr->CachedSeq = AnimOwner->AnimSequence;
		}
				
		// Tween indicator says destination has been (practically) reached ?
		FrameHdr->TweenIndicator += (1.f - FrameHdr->TweenIndicator) * Alpha;
		if( FrameHdr->TweenIndicator > 0.97f ) 
		{
			// We can set Alpha=0 (faster).
			Alpha = 0.f;

			// LOD fix: if the cache has too little vertices, 
			// now is the time to fill it out to the requested number.
			if (VertexNum < VertsRequested )
			{
				FMeshVert* MeshVertex = &Verts( iFrameOffset );
				for( INT i=VertexNum; i<VertsRequested; i++ )
				{
					CachedVerts[i]= FVector( MeshVertex[i].X, MeshVertex[i].Y, MeshVertex[i].Z );
				}
				VertexNum = VertsRequested;
				LODRequest = VertexNum - SpecialVerts; 
				FrameHdr->CachedLodVerts = VertexNum;   
			}
		}
		
		// Special case Alpha 0.
		if (Alpha <= 0.f)
		{
			for( INT i=0; i<VertexNum; i++ )
			{
				*ResultVerts = CachedVerts[i].TransformPointBy(LocalCoords);
				*(BYTE**)&ResultVerts += Size;
			}
		}
		else
		{
			// Tween all points between cached value and new one.
			FMeshVert* MeshVertex = &Verts( iFrameOffset );
			for( INT i=0; i<VertexNum; i++ )
			{
				FVector V2( MeshVertex[i].X, MeshVertex[i].Y, MeshVertex[i].Z );
				CachedVerts[i] += (V2 - CachedVerts[i]) * Alpha;
				*ResultVerts = CachedVerts[i].TransformPointBy(LocalCoords);
				*(BYTE**)&ResultVerts += Size;
			}
		}
		// Update cached frame.
		FrameHdr->CachedFrame = AnimOwner->AnimFrame;
	}

	Item->Unlock();
	unguardobj;
}


/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

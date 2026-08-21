/*=============================================================================
	UnMesh.h: Unreal mesh objects.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#ifndef _ENGINE_MESH_H
#define _ENGINE_MESH_H

/*-----------------------------------------------------------------------------
	FMeshVert.
-----------------------------------------------------------------------------*/

// Packed mesh vertex point for skinned meshes.
struct FMeshVert
{
	// Variables.
#if __INTEL_BYTE_ORDER__
	INT X:11; INT Y:11; INT Z:10;
#else
	INT Z:10; INT Y:11; INT X:11;
#endif

	// Constructor.
	FMeshVert()
	{}
	FMeshVert( const FVector& In )
	: X(appRound(In.X)), Y(appRound(In.Y)), Z(appRound(In.Z))
	{}

	// Functions.
	FVector Vector() const
	{
		return FVector( X, Y, Z );
	}

	// Serializer.
	friend FArchive& operator<<( FArchive& Ar, FMeshVert& V )
	{
		DWORD Packed = 0;
		if( !Ar.IsLoading() )
		{
			Packed
				=  (static_cast<DWORD>(V.X) & 0x7ffu)
				| ((static_cast<DWORD>(V.Y) & 0x7ffu) << 11)
				| ((static_cast<DWORD>(V.Z) & 0x3ffu) << 22);
		}

		Ar << Packed;
		if( Ar.IsLoading() )
		{
			const DWORD PackedX = Packed & 0x7ffu;
			const DWORD PackedY = (Packed >> 11) & 0x7ffu;
			const DWORD PackedZ = (Packed >> 22) & 0x3ffu;
			V.X = (PackedX & 0x400u) ? static_cast<INT>(PackedX) - 0x800 : static_cast<INT>(PackedX);
			V.Y = (PackedY & 0x400u) ? static_cast<INT>(PackedY) - 0x800 : static_cast<INT>(PackedY);
			V.Z = (PackedZ & 0x200u) ? static_cast<INT>(PackedZ) - 0x400 : static_cast<INT>(PackedZ);
		}
		return Ar;
	}
};

/*-----------------------------------------------------------------------------
	FMeshTri.
-----------------------------------------------------------------------------*/

// Texture coordinates associated with a vertex and one or more mesh triangles.
// All triangles sharing a vertex do not necessarily have the same texture
// coordinates at the vertex.
struct FMeshUV
{
	BYTE U;
	BYTE V;
	friend FArchive &operator<<( FArchive& Ar, FMeshUV& M )
		{return Ar << M.U << M.V;}
};

struct FMeshFloatUV
{
	FLOAT U;
	FLOAT V;
	friend FArchive &operator<<( FArchive& Ar, FMeshFloatUV& M )
		{return Ar << M.U << M.V;}
};

// One triangular polygon in a mesh, which references three vertices,
// and various drawing/texturing information.
struct FMeshTri
{
	_WORD		iVertex[3];		// Vertex indices.
	FMeshUV		Tex[3];			// Texture UV coordinates.
	DWORD		PolyFlags;		// Surface flags.
	INT			TextureIndex;	// Source texture index.
	friend FArchive &operator<<( FArchive& Ar, FMeshTri& T )
	{
		Ar << T.iVertex[0] << T.iVertex[1] << T.iVertex[2];
		Ar << T.Tex[0] << T.Tex[1] << T.Tex[2];
		Ar << T.PolyFlags << T.TextureIndex;
		return Ar;
	}
};

/*-----------------------------------------------------------------------------
	LOD Mesh structs.
-----------------------------------------------------------------------------*/

// Compact structure to send processing info to mesh digestion at import time.
struct ULODProcessInfo
{
	UBOOL LevelOfDetail;
	UBOOL NoUVData;
	UBOOL OldAnimFormat;
	INT   SampleFrame;
	INT   MinVerts;
	INT   Style;
};

// LOD-style Textured vertex struct. references one vertex, and 
// contains texture U,V information. 4 bytes.
// One triangular polygon in a mesh, which references three vertices,
// and various drawing/texturing information. 
struct FMeshWedge
{
	_WORD		iVertex;		// Vertex index.
	FMeshUV		TexUV;			// Texture UV coordinates. ( 2 bytes)
	friend FArchive &operator<<( FArchive& Ar, FMeshWedge& T )
	{
		Ar << T.iVertex << T.TexUV;
		return Ar;
	}

};

// Extended wedge - floating-point UV coordinates.
struct FMeshExtWedge
{
	_WORD		    iVertex;		// Vertex index.
	_WORD           Flags;          // Reserved 16 bits of flags
	FMeshFloatUV	TexUV;			// Texture UV coordinates. ( 2 DWORDS)
	friend FArchive &operator<<( FArchive& Ar, FMeshExtWedge& T )
	{
		Ar << T.iVertex << T.Flags << T.TexUV;
		return Ar;
	}
};



// LOD-style triangular polygon in a mesh, which references three textured vertices.  8 bytes.
struct FMeshFace
{
	_WORD		iWedge[3];		// Textured Vertex indices.
	_WORD		MaterialIndex;	// Source Material (= texture plus unique flags) index.

	friend FArchive &operator<<( FArchive& Ar, FMeshFace& F )
	{
		Ar << F.iWedge[0] << F.iWedge[1] << F.iWedge[2];
		Ar << F.MaterialIndex;
		return Ar;
	}

	FMeshFace& operator=( const FMeshFace& Other )
	{
		guardSlow(FMeshFace::operator=);
		this->iWedge[0] = Other.iWedge[0];
		this->iWedge[1] = Other.iWedge[1];
		this->iWedge[2] = Other.iWedge[2];
		this->MaterialIndex = Other.MaterialIndex;
		return *this;
		unguardSlow;
	}	
};

// LOD-style mesh material.
struct FMeshMaterial
{
	DWORD		PolyFlags;		// Surface flags.
	INT			TextureIndex;	// Source texture index.
	friend FArchive &operator<<( FArchive& Ar, FMeshMaterial& M )
	{
		return Ar << M.PolyFlags << M.TextureIndex;
	}
};



/*-----------------------------------------------------------------------------
	FMeshVertConnect.
-----------------------------------------------------------------------------*/

// Says which triangles a particular mesh vertex is associated with.
// Precomputed so that mesh triangles can be shaded with Gouraud-style
// shared, interpolated normal shading.
struct FMeshVertConnect
{
	INT	NumVertTriangles;
	INT	TriangleListOffset;
	friend FArchive &operator<<( FArchive& Ar, FMeshVertConnect& C )
		{return Ar << C.NumVertTriangles << C.TriangleListOffset;}
};

/*-----------------------------------------------------------------------------
	UMesh.
-----------------------------------------------------------------------------*/

//
// A mesh, completely describing a 3D object (creature, weapon, etc) and
// its animation sequences.  Does not reference textures.
//
class ENGINE_API UMesh : public UBoxPrim
{
	DECLARE_CLASS(UMesh,UBoxPrim,0,Engine)

	// Objects.
	TLazyArray<FMeshVert>			Verts;
	TLazyArray<FMeshTri>			Tris;
	TArray<FMeshAnimSeq>			AnimSeqs;
	TLazyArray<FMeshVertConnect>	Connects;
	TArray<FBox>					BoundingBoxes;
	TArray<FSphere>					BoundingSpheres;//!!currently broken
	TLazyArray<INT>					VertLinks;
	TArray<UTexture*>				Textures;
	TArray<FLOAT>					TextureLOD;

	// Counts.
	INT						FrameVerts;
	INT						AnimFrames;

	// Render info.
	DWORD					AndFlags;
	DWORD					OrFlags;

	// Scaling.
	FVector					Scale;		// Mesh scaling.
	FVector 				Origin;		// Origin in original coordinate system.
	FRotator				RotOrigin;	// Amount to rotate when importing (mostly for yawing).

	// Editing info.
	INT						CurPoly;	// Index of selected polygon.
	INT						CurVertex;	// Index of selected vertex.

	// UObject interface.
	UMesh();
	void Serialize( FArchive& Ar );

	// UPrimitive interface.
	virtual FCoords GetRenderBoundingBox( const AActor* Owner, UBOOL Exact );
	FBox GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const;
	FVector GetCollisionExtent( const AActor* Owner ) const;

	// UMesh interface.
	UMesh( INT NumPolys, INT NumVerts, INT NumFrames );
	virtual const FMeshAnimSeq* GetAnimSeq( FName SeqName ) const
	{
		guardSlow(UMesh::GetAnimSeq);
		for( INT i=0; i<AnimSeqs.Num(); i++ )
			if( SeqName == AnimSeqs(i).Name )
				return &AnimSeqs(i);
		return NULL;
		unguardSlow;
	}
	virtual FMeshAnimSeq* GetAnimSeq( FName SeqName )
	{
		guardSlow(UMesh::GetAnimSeq);
		for( INT i=0; i<AnimSeqs.Num(); i++ )
			if( SeqName == AnimSeqs(i).Name )
				return &AnimSeqs(i);
		return NULL;
		unguardSlow;
	}
	virtual void GetFrame( FVector* Verts, INT Size, FCoords Coords, AActor* Owner );
	void AMD3DGetFrame( FVector* Verts, INT Size, FCoords Coords, AActor* Owner );
	virtual INT GetNumTris() const
	{
		return Tris.Num();
	}
	virtual void GetTriVerts( INT T, INT Verts[3] ) const
	{
		const FMeshTri& Tri = Tris(T);
		Verts[0] = Tri.iVertex[0];
		Verts[1] = Tri.iVertex[1];
		Verts[2] = Tri.iVertex[2];
	}
	virtual UTexture* GetTexture( INT Count, AActor* Owner )
	{
		guardSlow(UMesh::GetTexture);
		if( Owner && Owner->MultiSkins[Count] )
			return Owner->MultiSkins[Count];
//		if( Owner && Owner->GetSkin( Count ) )
//			return Owner->GetSkin( Count );
		else if( Count!=0 && Textures(Count) )
			return Textures(Count);
		else if( Owner && Owner->Skin )
			return Owner->Skin;
		else
			return Textures(Count);
		unguardSlow;
	}
	virtual void SetScale( FVector NewScale );
	virtual FVector GetRootMovement( AActor* Owner )
	{
		return FVector(0);
	}
	virtual void AdjustRootMovement( AActor* Owner, const FVector& Adjust )
	{
	}
	virtual FVector AnimCycleMovement( AActor* Owner )
	{
		return FVector(0);
	}
	virtual FCoords GetBoneCoords( AActor* Owner, int Bone ) const;
	virtual INT MemFootprint()
	{
		return appCheckedIntSize
		(
			sizeof(*this)
			+ Verts.Mem() + Tris.Mem() + AnimSeqs.Mem() + Connects.Mem()
			+ BoundingBoxes.Mem() + BoundingSpheres.Mem()
			+ VertLinks.Mem() + Textures.Mem() + TextureLOD.Mem()
		);
	}
};

/*-----------------------------------------------------------------------------
	ULodMesh.
-----------------------------------------------------------------------------*/

/*-----------------------------------------------------------------------------
	ULodMesh.
-----------------------------------------------------------------------------*/

//
// A LodMesh, completely describing a 3D object (creature, weapon, etc) and
// its animation sequences.  Does not reference textures.
//
class ENGINE_API ULodMesh : public UMesh
{
	DECLARE_CLASS(ULodMesh,UMesh,0,Engine)

	// LOD-specific objects.
	// Make lazy arrays where useful.
	TArray<_WORD>			CollapsePointThus;  // Lod-collapse single-linked list for points.
	TArray<_WORD>           FaceLevel;          // Minimum lod-level indicator for each face.
	TArray<FMeshFace>       Faces;              // Faces 
	TArray<_WORD>			CollapseWedgeThus;  // Lod-collapse single-linked list for the wedges.
	TArray<FMeshWedge>		Wedges;             // 'Hoppe-style' textured vertices.
	TArray<FMeshMaterial>   Materials;          // Materials
	TArray<FMeshFace>       SpecialFaces;       // Invisible special-coordinate faces.

	// Misc Internal.
	INT    ModelVerts;     // Number of 'visible' vertices.
	INT	   SpecialVerts;   // Number of 'invisible' (special attachment) vertices.

	// Max of x/y/z mesh scale for LOD gauging (works on top of drawscale).
	FLOAT  MeshScaleMax;

	// Script-settable LOD controlling parameters.
	FLOAT  LODStrength;    // Scales the (not necessarily linear) falloff of vertices with distance.
	INT    LODMinVerts;    // Minimum number of vertices with which to draw a model.
	FLOAT  LODMorph;       // >0.0 = allow morphing ; 0.0-1.0 = range of vertices to morph.
	FLOAT  LODZDisplace;   // Z displacement for LOD distance-dependency tweaking.
	FLOAT  LODHysteresis;  // Controls LOD-level change delay and morphing.

	// Remapping of animation vertices.
	TArray<_WORD> RemapAnimVerts;
	INT    OldFrameVerts;  // Possibly different old per-frame vertex count.
	
	//  UObject interface.
	ULodMesh(){};
	void Serialize( FArchive& Ar );

	//  UMesh interface.
	void SetScale( FVector NewScale );
	ULodMesh( INT NumPolys, INT NumVerts, INT NumFrames );
	
	// GetFrame for LOD.
	virtual void GetFrame( FVector* Verts, INT Size, FCoords Coords, AActor* Owner, INT& LODRequest );
	virtual void GetFrame( FVector* Verts, INT Size, FCoords Coords, AActor* Owner )
	{
		INT LODVerts = FrameVerts;
		GetFrame( Verts, Size, Coords, Owner, LODVerts );
	}
	virtual INT GetNumTris() const
	{
		return Faces.Num();
	}
	virtual void GetTriVerts( INT T, INT Verts[3] ) const
	{
		const FMeshFace& Face = Faces(T);
		Verts[0] = Wedges( Face.iWedge[0] ).iVertex;
		Verts[1] = Wedges( Face.iWedge[1] ).iVertex;
		Verts[2] = Wedges( Face.iWedge[2] ).iVertex;
	}
	virtual INT MemFootprint()
	{
		return appCheckedIntSize
		(
			Super::MemFootprint()
			+ sizeof(*this) - sizeof(Super)
			+ CollapsePointThus.Mem() + FaceLevel.Mem() + Faces.Mem()
			+ CollapseWedgeThus.Mem() + Wedges.Mem()
			+ Materials.Mem() + SpecialFaces.Mem() + RemapAnimVerts.Mem()
		);
	}
};

/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/

#endif
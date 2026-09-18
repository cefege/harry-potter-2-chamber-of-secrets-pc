/*=============================================================================
	UnMeshEd.cpp: Unreal editor mesh code
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EditorPrivate.h"
#include "UnRender.h"
#include "UnSkeletalMesh.h"
#include "UnSkeletalImport.h"

/*-----------------------------------------------------------------------------
	Data types for importing James' creature meshes.
-----------------------------------------------------------------------------*/

/* debug logging */
#undef  NLOG                
#define NLOG(func) {}
//#define NLOG(func) func


// James mesh info.
struct FJSDataHeader
{
	_WORD	NumPolys;
	_WORD	NumVertices;
	_WORD	BogusRot;
	_WORD	BogusFrame;
	DWORD	BogusNormX,BogusNormY,BogusNormZ;
	DWORD	FixScale;
	DWORD	Unused1,Unused2,Unused3;
};

// James animation info.
struct FJSAnivHeader
{
	_WORD	NumFrames;		// Number of animation frames.
	_WORD	FrameSize;		// Size of one frame of animation.
};

// Mesh triangle.
struct FJSMeshTri
{
	_WORD		iVertex[3];		// Vertex indices.
	BYTE		Type;			// James' mesh type.
	BYTE		Color;			// Color for flat and Gouraud shaded.
	FMeshUV		Tex[3];			// Texture UV coordinates.
	BYTE		TextureNum;		// Source texture offset.
	BYTE		Flags;			// Unreal mesh flags (currently unused).
};



/*-----------------------------------------------------------------------------
	Import functions.
-----------------------------------------------------------------------------*/

// Mesh sorting function.
static QSORT_RETURN CDECL CompareTris( const FMeshTri* A, const FMeshTri* B )
{
	if     ( (A->PolyFlags&PF_Translucent) > (B->PolyFlags&PF_Translucent) ) return  1;
	else if( (A->PolyFlags&PF_Translucent) < (B->PolyFlags&PF_Translucent) ) return -1;
	else if( A->TextureIndex               > B->TextureIndex               ) return  1;
	else if( A->TextureIndex               < B->TextureIndex               ) return -1;
	else if( A->PolyFlags                  > B->PolyFlags                  ) return  1;
	else if( A->PolyFlags                  < B->PolyFlags                  ) return -1;
	else                                                                     return  0;
}



//
// Import a mesh from James' editor.  Uses file commands instead of object
// manager.  Slow but works fine.
//
void UEditorEngine::meshImport
(
	const TCHAR*		MeshName,
	UObject*			InParent,
	const TCHAR*		AnivFname, 
	const TCHAR*		DataFname,
	UBOOL				Unmirror,
	UBOOL				ZeroTex,
	INT					UnMirrorTex,
	ULODProcessInfo*	LODInfo
)
{
	guard(UEditorEngine::meshImport);

	UMesh*			Mesh;
	FArchive*		AnivFile;
	FArchive*		DataFile;
	FJSDataHeader	JSDataHdr;
	FJSAnivHeader	JSAnivHdr;
	INT				i;
	INT				Ok = 0;
	INT				MaxTextureIndex = 0;

	debugf( NAME_Log, TEXT("Importing %s"), MeshName );
	GWarn->BeginSlowTask( TEXT("Importing mesh"), 1, 0 );
	GWarn->StatusUpdatef( 0, 0, TEXT("%s"), TEXT("Reading files") );

	// Open James' animation vertex file and read header.
	AnivFile = GFileManager->CreateFileReader( AnivFname, 0, GLog );
	if( !AnivFile )
	{
		debugf( NAME_Log, TEXT("Error opening file %s"), AnivFname );
		goto Out1;
	}
	AnivFile->Serialize( &JSAnivHdr, sizeof(FJSAnivHeader) );
	if( AnivFile->IsError() )
	{
		debugf( NAME_Log, TEXT("Error reading %s"), AnivFname );
		goto Out2;
	}

	// Open James' mesh data file and read header.
	DataFile = GFileManager->CreateFileReader( DataFname, 0, GLog );
	if( !DataFile )
	{
		debugf( NAME_Log, TEXT("Error opening file %s"), DataFname );
		goto Out2;
	}
	DataFile->Serialize( &JSDataHdr, sizeof(FJSDataHeader) );
	if( DataFile->IsError() )
	{
		debugf( NAME_Log, TEXT("Error reading %s"), DataFname );
		goto Out3;
	}

	// Allocate mesh or lodmesh object.
	if( !LODInfo->LevelOfDetail )
		Mesh = new( InParent, MeshName, RF_Public|RF_Standalone )UMesh( JSDataHdr.NumPolys, JSDataHdr.NumVertices, JSAnivHdr.NumFrames );
	else 
		Mesh = new( InParent, MeshName, RF_Public|RF_Standalone )ULodMesh( JSDataHdr.NumPolys, JSDataHdr.NumVertices, JSAnivHdr.NumFrames );

	// Display summary info.
	debugf(NAME_Log,TEXT(" * Triangles  %i"),Mesh->Tris.Num());
	debugf(NAME_Log,TEXT(" * Vertices   %i"),Mesh->FrameVerts);
	debugf(NAME_Log,TEXT(" * AnimFrames %i"),Mesh->AnimFrames);
	debugf(NAME_Log,TEXT(" * FrameSize  %i"),JSAnivHdr.FrameSize);
	debugf(NAME_Log,TEXT(" * AnimSeqs   %i"),Mesh->AnimSeqs.Num());

	// Import mesh triangles.
	debugf( NAME_Log, TEXT("Importing triangles") );
	GWarn->StatusUpdatef( 0, 0, TEXT("%s"), TEXT("Importing Triangles") );
	DataFile->Seek( DataFile->Tell() + 12 );
	for( i=0; i<Mesh->Tris.Num(); i++ )
	{
		guard(Importing triangles);

		// Load triangle.
		FJSMeshTri Tri;
		DataFile->Serialize( &Tri, sizeof(Tri) );
		if( DataFile->IsError() )
		{
			debugf( NAME_Log, TEXT("Error processing %s"), DataFname );
			goto Out4;
		}
		if( Unmirror )
		{
			Exchange( Tri.iVertex[1], Tri.iVertex[2] );
			Exchange( Tri.Tex    [1], Tri.Tex    [2] );
			if( Tri.TextureNum == UnMirrorTex )
			{
				Tri.Tex[0].U = 255 - Tri.Tex[0].U;
				Tri.Tex[1].U = 255 - Tri.Tex[1].U;
				Tri.Tex[2].U = 255 - Tri.Tex[2].U;
			}
		}
		if( ZeroTex )
		{
			Tri.TextureNum = 0;
		}

		// Copy to Unreal structures.
		Mesh->Tris(i).iVertex[0]	= Tri.iVertex[0];
		Mesh->Tris(i).iVertex[1]	= Tri.iVertex[1];
		Mesh->Tris(i).iVertex[2]	= Tri.iVertex[2];
		Mesh->Tris(i).Tex[0]		= Tri.Tex[0];
		Mesh->Tris(i).Tex[1]		= Tri.Tex[1];
		Mesh->Tris(i).Tex[2]		= Tri.Tex[2];
		Mesh->Tris(i).TextureIndex	= Tri.TextureNum;
		MaxTextureIndex = Max<INT>(MaxTextureIndex,Tri.TextureNum);		


		// Set style based on triangle type.
		DWORD PolyFlags=0;
		if     ( (Tri.Type&15)==MTT_Normal         ) PolyFlags |= 0;
		else if( (Tri.Type&15)==MTT_NormalTwoSided ) PolyFlags |= PF_TwoSided;
		else if( (Tri.Type&15)==MTT_Modulate       ) PolyFlags |= PF_TwoSided | PF_Modulated;
		else if( (Tri.Type&15)==MTT_Translucent    ) PolyFlags |= PF_TwoSided | PF_Translucent;
		else if( (Tri.Type&15)==MTT_Masked         ) PolyFlags |= PF_TwoSided | PF_Masked;
		else if( (Tri.Type&15)==MTT_Placeholder    ) PolyFlags |= PF_TwoSided | PF_Invisible;

		// Handle effects.
		if     ( Tri.Type&MTT_Unlit             ) PolyFlags |= PF_Unlit;
		if     ( Tri.Type&MTT_Flat              ) PolyFlags |= PF_Flat;
		if     ( Tri.Type&MTT_Environment       ) PolyFlags |= PF_Environment;
		if     ( Tri.Type&MTT_NoSmooth          ) PolyFlags |= PF_NoSmooth;

		// per-pixel Alpha flag ( Reuses Flatness triangle tag and Wavy engine tag...)
		if     ( Tri.Type&MTT_Flat				) PolyFlags |= PF_BigWavy; 

		// Set flags.
		Mesh->Tris(i).PolyFlags = PolyFlags;

		unguard;
	}

	// Sort triangles by texture and flags.
	appQsort( &Mesh->Tris(0), Mesh->Tris.Num(), sizeof(Mesh->Tris(0)), (QSORT_COMPARE)CompareTris );

	// Texture LOD.
	for( i=0; i<MaxTextureIndex+1; i++ )
	{
		Mesh->TextureLOD.AddItem( 1.0f );
	}
	while( MaxTextureIndex >= Mesh->Textures.Num() )
		Mesh->Textures.AddItem( NULL );

	debugf(TEXT(" Mesh Textures: %i LodItems: %i"),Mesh->Textures.Num(),Mesh->TextureLOD.Num());

	// Import mesh vertices.
	debugf( NAME_Log, TEXT("Importing vertices") );
	GWarn->StatusUpdatef( 0, 0, TEXT("%s"), TEXT("Importing Vertices") );
	for( i=0; i<Mesh->AnimFrames; i++ )
	{
		guard(Importing animation frames);
		AnivFile->Serialize( &Mesh->Verts(i * Mesh->FrameVerts), sizeof(FMeshVert) * Mesh->FrameVerts );
		if( AnivFile->IsError() )
		{
			debugf( NAME_Log, TEXT("Vertex error in mesh %s, frame %i: expecting %i verts"), AnivFname, i, Mesh->FrameVerts );
			break;
		}
		if( Unmirror )
			for( INT j=0; j<Mesh->FrameVerts; j++ )
				Mesh->Verts(i * Mesh->FrameVerts + j).X *= -1;
		AnivFile->Seek( AnivFile->Tell() + JSAnivHdr.FrameSize - Mesh->FrameVerts * sizeof(FMeshVert) );
		unguard;
	}

	// Build list of triangles per vertex.
	if( !LODInfo->LevelOfDetail )
	{
		GWarn->StatusUpdatef( i, Mesh->FrameVerts, TEXT("%s"), TEXT("Linking mesh") );
		for( i=0; i<Mesh->FrameVerts; i++ )
		{
			guard(ImportingVertices);
			Mesh->Connects(i).NumVertTriangles = 0;
			Mesh->Connects(i).TriangleListOffset = Mesh->VertLinks.Num();
			for( INT j=0; j<Mesh->Tris.Num(); j++ )
			{
				for( INT k=0; k<3; k++ )
				{
					if( Mesh->Tris(j).iVertex[k] == i )
					{
						Mesh->VertLinks.AddItem(j);
						Mesh->Connects(i).NumVertTriangles++;
					}
				}
			}
			unguard;
		}
		debugf( NAME_Log, TEXT("Made %i links"), Mesh->VertLinks.Num() );
	}

	// Compute per-frame bounding volumes plus overall bounding volume.
	meshBuildBounds(Mesh);

	// Process for LOD. Called last; needs the mesh bounds from above.
	if( LODInfo->LevelOfDetail )
		meshLODProcess( (ULodMesh*)Mesh, LODInfo );

	// Memory diag.
	debugf(TEXT("Total %s memory: %iK"), Mesh->GetName(), Mesh->MemFootprint()/1024);

	// Exit labels.
	Ok = 1;
	Out4: if (!Ok) {delete Mesh;}
	Out3: delete DataFile;
	Out2: delete AnivFile;
	Out1: GWarn->EndSlowTask();
	unguard;
}

void UEditorEngine::meshDropFrames
(
	UMesh*			Mesh,
	INT				StartFrame,
	INT				NumFrames
)
{
	guard(UEditorEngine::meshDropFrames);
	Mesh->Verts.Remove( StartFrame*Mesh->FrameVerts, NumFrames*Mesh->FrameVerts );
	Mesh->AnimFrames -= NumFrames;
	unguard;
}

/*-----------------------------------------------------------------------------
	Bounds.
-----------------------------------------------------------------------------*/

//
// Build bounding boxes for each animation frame of the mesh,
// and one bounding box enclosing all animation frames.
//
void UEditorEngine::meshBuildBounds( UMesh* Mesh )
{
	guard(UEditorEngine::meshBuildBounds);
	GWarn->StatusUpdatef( 0, 0, TEXT("%s"), TEXT("Bounding mesh") );

	// Bound all frames.
	TArray<FVector> AllFrames;
	for( INT i=0; i<Mesh->AnimFrames; i++ )
	{
		TArray<FVector> OneFrame;
		for( INT j=0; j<Mesh->FrameVerts; j++ )
		{
			FVector Vertex = Mesh->Verts( i * Mesh->FrameVerts + j ).Vector();
			OneFrame .AddItem( Vertex );
			AllFrames.AddItem( Vertex );
		}
		Mesh->BoundingBoxes  (i) = FBox   ( &OneFrame(0), OneFrame.Num() );
		Mesh->BoundingSpheres(i) = FSphere( &OneFrame(0), OneFrame.Num() );
	}
	Mesh->BoundingBox    = FBox   ( &AllFrames(0), AllFrames.Num() );
	Mesh->BoundingSphere = FSphere( &AllFrames(0), AllFrames.Num() );

	// Display bounds.
	debugf
	(
		NAME_Log,
		TEXT("BoundingBox (%f,%f,%f)-(%f,%f,%f) BoundingSphere (%f,%f,%f) %f"),
		Mesh->BoundingBox.Min.X,
		Mesh->BoundingBox.Min.Y,
		Mesh->BoundingBox.Min.Z,
		Mesh->BoundingBox.Max.X,
		Mesh->BoundingBox.Max.Y,
		Mesh->BoundingBox.Max.Z,
		Mesh->BoundingSphere.X,
		Mesh->BoundingSphere.Y,
		Mesh->BoundingSphere.Z,
		Mesh->BoundingSphere.W
	);
	unguard;
}


void UEditorEngine::modelBuildBounds( USkeletalMesh* Mesh )
{
	guard(UEditorEngine::modelBuildBounds);
	GWarn->StatusUpdatef( 0, 0, TEXT("%s"), TEXT("Bounding skeletal mesh") );

	// Bound using the reference skin stretched to the reference skeleton pose
	TArray <FCoords> SpaceBases;
	SpaceBases.Add( Mesh->RefSkeleton.Num() );

	// Weapon bone 
	Mesh->WeaponBoneIndex = -1;

	// Assume the default pose.
	TArray <FVector> OutVerts;
    OutVerts.AddZeroed(Mesh->FrameVerts);
	for( INT w=0; w<Mesh->BoneWeightIdx.Num(); w++)
	{
		SpaceBases(w) = FCoords( Mesh->RefSkeleton(w).BonePos.Place );
		INT Parent = Mesh->RefSkeleton(w).ParentIndex;
		if( Parent != w )
			// Build the hierarchy.
			SpaceBases(w) <<= SpaceBases(Parent);
		Mesh->RefSkeleton(w).BonePos.Length = 0.f;

		INT Index  = Mesh->BoneWeightIdx(w).WeightIndex;
		INT Number = Mesh->BoneWeightIdx(w).Number;
		if ( Number > 0)
		{
			for( INT b=Index; b<(Index+Number); b++ )
			{
				INT VertIndex = Mesh->BoneWeights(b).PointIndex;
				FLOAT Weight = (FLOAT)Mesh->BoneWeights(b).BoneWeight * ( 1.f/65535.f );					
				OutVerts(VertIndex) +=  Weight * ( Mesh->LocalPoints(b) << SpaceBases(w) );

				// Compute bounding radius as well.
				Mesh->RefSkeleton(w).BonePos.Length = Max( Mesh->RefSkeleton(w).BonePos.Length, Mesh->LocalPoints(b).SizeApprox()*Weight );
			}
		}
	}

	// Compute bounding box of static pose, relative to root joint.
	Mesh->BoundingBox = FBox( OutVerts.GetData(), OutVerts.Num() );

	// Compute max bounding sphere.
	TArray<FLOAT> Radii( Mesh->RefSkeleton.Num() );
	INT t;
	for( t=0; t<Mesh->RefSkeleton.Num(); t++ )
		Radii(t) = Mesh->RefSkeleton(t).BonePos.Length;
	for( t=Mesh->RefSkeleton.Num()-1; t>0; t-- )
	{
		INT p = Mesh->RefSkeleton(t).ParentIndex;
		if( p != t )
		{
			FLOAT PLength = Mesh->RefSkeleton(t).BonePos.Place.Pos.SizeApprox() + Radii(t);
			Radii(p) = Max( Radii(p), PLength );
		}
	}
	Mesh->BoundingSphere = FSphere( Mesh->RefSkeleton(0).BonePos.Place.Pos, Radii(0) );

	if( Mesh->DefaultAnimation && Mesh->DefaultAnimation->MovesInfo.Num() )
	{
		// Build a bounding box for each anim, relative to root bone.
		UAnimation* Anim = Mesh->DefaultAnimation;
		Mesh->BoundingBoxes.Set( Anim->MovesInfo.Num() );

		// Link anim bones to skeleton.
		TArray<INT> Links( Mesh->RefSkeleton.Num() );
		for_array( p, Mesh->RefSkeleton )
		{
			Links[p] = -1;
			for_array( b, Anim->RefBones )
			{				
				if( Mesh->RefSkeleton(p).Name == Anim->RefBones(b).Name )
				{
					Links[p] = b;
					break;
				}
			}
		}

		// Compute const bounds for root bone.
		FBox RootBox(0);
		INT Index = Mesh->BoneWeightIdx(0).WeightIndex;
		for_count( b, Mesh->BoneWeightIdx(0).Number )
			RootBox += Mesh->LocalPoints(Index+b);

		// Build box for each animation.
		TArray<FPlace> AbsPlaces( Mesh->RefSkeleton.Num() );
		for_array( a, Anim->MovesInfo )
		{
			FBox& Box = Mesh->BoundingBoxes(a);
			Box = RootBox;
			const MotionChunkDigestInfo& Info = Anim->MovesInfo(a);

			// Compute skeletal position for each frame.
			for_count( f, Info.NumRawFrames )
			{
				// Ignore root bone.
				for (INT b=1; b<Mesh->RefSkeleton.Num(); b++ )
				{
					if( Links[b] < 0 )
					{
						AbsPlaces(b) = Mesh->RefSkeleton(b).BonePos.Place;
					}
					else
					{
						INT KeyIdx = Min( (Info.FirstRawFrame + f) * Anim->RefBones.Num() + Links[b], Anim->RawAnimKeys.Num()-1 );
						AbsPlaces(b).Pos = Anim->RawAnimKeys(KeyIdx).Position;
						AbsPlaces(b).Quat = Anim->RawAnimKeys(KeyIdx).Orientation;
					}

					// Convert rel to abs.
					INT Parent = Mesh->RefSkeleton(b).ParentIndex;
					if( Parent != 0 )
						AbsPlaces(b) = AbsPlaces(b) << AbsPlaces(Parent);

					// Add bone's bounding sphere here.
					FVector Ext( Mesh->RefSkeleton(b).BonePos.Length );
					Box += AbsPlaces(b).Pos - Ext;
					Box += AbsPlaces(b).Pos + Ext;
				}
			}
		}
	}

	// Display bounds.
	debugf
	(
		NAME_Log,
		TEXT("BoundingBox (skeletal) (%f,%f,%f)-(%f,%f,%f) BoundingSphere (%f,%f,%f) %f"),
		Mesh->BoundingBox.Min.X,
		Mesh->BoundingBox.Min.Y,
		Mesh->BoundingBox.Min.Z,
		Mesh->BoundingBox.Max.X,
		Mesh->BoundingBox.Max.Y,
		Mesh->BoundingBox.Max.Z,
		Mesh->BoundingSphere.X,
		Mesh->BoundingSphere.Y,
		Mesh->BoundingSphere.Z,
		Mesh->BoundingSphere.W
	);
	unguard;
}



/*-----------------------------------------------------------------------------
	Special importers for skeletal data.
-----------------------------------------------------------------------------*/

//
// To do: Need much better error and version handling on all skeletal file reading - Erik
//

void UEditorEngine::modelImport//modelImport
(
	const TCHAR*		MeshName,
	UObject*			InParent,
	const TCHAR*		SkinFname, 
	UBOOL				Unmirror,
	UBOOL				ZeroTex,
	INT					UnMirrorTex,
	ULODProcessInfo*	LODInfo
)
{
	guard(UEditorEngine::modelImport);

	/*	 
	// File header structure. 
	struct VChunkHeader
	{
		ANSICHAR	ChunkID[20];  // string ID of up to 19 chars (usually zero-terminated..)
		INT			TypeFlag;     // Flags/reserved
		INT         DataSize;     // size per struct following;
		INT         DataCount;    // number of structs/
	};
	*/
	
	// Temp structs for importing
	USkelImport RawData; // local struct

	//
	// Alternate loading and construction of skeletal mesh object 
	//
	// 2nd draft: loads animation data with a SEPARATE script #exec, as a separate
	// object type, and all animations and bones link up by name.
	//

	USkeletalMesh*			Mesh;
	FArchive*		SkinFile;
	VChunkHeader	ChunkHeader;

	debugf( NAME_Log, TEXT("Importing skin %s"), MeshName );
	GWarn->BeginSlowTask( TEXT("Importing skeletal mesh"), 1, 0 );
	GWarn->StatusUpdatef( 0, 0, TEXT("%s"), TEXT("Reading files") );

	// Allocate skeletal mesh object.
	Mesh = new( InParent, MeshName, RF_Public|RF_Standalone )USkeletalMesh();

	// Open skeletal skin file and read header.
	SkinFile = GFileManager->CreateFileReader( SkinFname, 0, GLog );
	if( !SkinFile )
	{
		appErrorf( NAME_Log, TEXT("Error opening skin file %s"), SkinFname );
		//goto Out1; &&
	}

	SkinFile->Serialize( &ChunkHeader, sizeof(VChunkHeader) );
	if( SkinFile->IsError() )
	{
		appErrorf( NAME_Log, TEXT("Error reading skin file %s"), SkinFname );
		//goto Out2; &&
	}

	guard(ReadPointData);
	// Read the temp skin structures..
	// 3d points "vpoints" datasize*datacount....
	SkinFile->Serialize( &ChunkHeader, sizeof(VChunkHeader) );
	RawData.Points.Add(ChunkHeader.DataCount);
	SkinFile->Serialize( &RawData.Points(0), sizeof(VPoint) * ChunkHeader.DataCount);	
	unguard;

	guard(ReadWedgeData);
	//  Wedges (VVertex)
	SkinFile->Serialize(&ChunkHeader, sizeof(VChunkHeader) );
	RawData.Wedges.Add(ChunkHeader.DataCount);
	SkinFile->Serialize( &RawData.Wedges(0), sizeof(VVertex) * ChunkHeader.DataCount);
	unguard;

	guard(ReadFaceData);
	// Faces (VTriangle)
	SkinFile->Serialize(&ChunkHeader, sizeof(VChunkHeader) );
	RawData.Faces.Add(ChunkHeader.DataCount);
	SkinFile->Serialize( &RawData.Faces(0), sizeof(VTriangle) * ChunkHeader.DataCount);
	unguard

	guard(ReadMaterialData);
	// Materials (VMaterial)
	SkinFile->Serialize(&ChunkHeader, sizeof(VChunkHeader) );
	RawData.Materials.Add(ChunkHeader.DataCount);
	SkinFile->Serialize( &RawData.Materials(0), sizeof(VMaterial) * ChunkHeader.DataCount);
	unguard;

	guard(ReadRefSkeleton);
	// Reference skeleton (VBones)
	SkinFile->Serialize(&ChunkHeader, sizeof(VChunkHeader) );
	RawData.RefBonesBinary.Add(ChunkHeader.DataCount);
	SkinFile->Serialize( &RawData.RefBonesBinary(0), sizeof(VBone) * ChunkHeader.DataCount);
	unguard;
	
	guard(ReadBoneInfluences);
	// Raw bone influences (VRawBoneInfluence)
	SkinFile->Serialize(&ChunkHeader, sizeof(VChunkHeader) );
	RawData.Influences.Add(ChunkHeader.DataCount);
	SkinFile->Serialize( &RawData.Influences(0), sizeof(VRawBoneInfluence) * ChunkHeader.DataCount);
	unguard;

	delete SkinFile;

	// Allocate textures pointers  - Todo: Materials may share textures 
	// so this should probably really only add unique textures...
	while( RawData.Materials.Num() >= Mesh->Textures.Num() )
			Mesh->Textures.AddItem( NULL );

	// display summary info
	debugf(NAME_Log,TEXT(" * Skeletal skin VPoints: %i "),RawData.Points.Num());
	debugf(NAME_Log,TEXT(" * Skeletal skin VVertices: %i "),RawData.Wedges.Num());
	debugf(NAME_Log,TEXT(" * Skeletal skin VTriangles: %i "),RawData.Faces.Num());
	debugf(NAME_Log,TEXT(" * Skeletal skin VMaterials: %i "),RawData.Materials.Num());
	debugf(NAME_Log,TEXT(" * Skeletal skin VBones: %i "),RawData.RefBonesBinary.Num());
	debugf(NAME_Log,TEXT(" * Skeletal skin VRawBoneInfluences: %i "),RawData.Influences.Num());

	modelLODProcess( Mesh, LODInfo, &RawData );

	// Compute per-frame bounding volumes plus overall bounding volume.
	modelBuildBounds( Mesh ); 
		
	debugf(NAME_Log,TEXT(" * Total materials: %i "),((USkeletalMesh*)Mesh)->Materials.Num());

	// Set LOD defaults.
	Mesh->LODMinVerts	  = Min(10, Mesh->FrameVerts);		// Minimum number of vertices with which to draw a model. (Minimum for a cube = 8...)
	Mesh->LODStrength	  = 1.00f;	// Scales the (not necessarily linear) falloff of vertices with distance.
	Mesh->LODMorph        = 0.30f;	// Morphing range. 0.0 = no morphing.
	Mesh->LODZDisplace    = 0.00f;  // Z-displacement (in world units) for falloff function tweaking.
	Mesh->LODHysteresis	  = 0.00f;	// Controls LOD-level change delay/morphing. (unused)

	// display summary info
	debugf(TEXT("Total %s memory: %iK"), Mesh->GetName(), Mesh->MemFootprint()/1024);
	debugf(NAME_Log,TEXT(" * Skeletal skin Points: %i size %i "),Mesh->Points.Num(), sizeof(FVector) );
	debugf(NAME_Log,TEXT(" * Skeletal skin Wedges: %i size %i "),Mesh->Wedges.Num(), sizeof(FMeshWedge) );
	debugf(NAME_Log,TEXT(" * Skeletal skin Triangles: %i size %i "),Mesh->Faces.Num(), sizeof(FMeshFace) );
	debugf(NAME_Log,TEXT(" * Skeletal skin Skeleton: %i size %i "),Mesh->RefSkeleton.Num(), sizeof(FMeshBone) );
	debugf(NAME_Log,TEXT(" * Skeletal skin Materials: %i size %i "),Mesh->Materials.Num(), sizeof(FMeshMaterial));
	debugf(NAME_Log,TEXT(" * Skeletal skin BoneWeights: %i size %i "),Mesh->BoneWeights.Num(), sizeof(VBoneInfluence) );
	debugf(NAME_Log,TEXT(" * Skeletal skin BoneIndices: %i size %i "),Mesh->BoneWeightIdx.Num(), sizeof(VBoneInfIndex) );

	// Display bone hierarchy.
	for_array( b, Mesh->RefSkeleton )
		debugf( NAME_Log, TEXT(" %.*s %s (%d verts, size %.2f, dist %.2f)"), 
			Mesh->RefSkeleton(b).Depth, 
			TEXT("............................................."),
			*Mesh->RefSkeleton(b).Name,
			Mesh->BoneWeightIdx(b).Number,
			Mesh->RefSkeleton(b).BonePos.Length,
			Mesh->RefSkeleton(b).BonePos.Place.Pos.Size() );

	unguard;
}

void UEditorEngine::modelAssignWeaponBone
(
    USkeletalMesh* Mesh,
    FName TempFname 
)
{
	guard(UEditorEngine::modelAssignWeaponBone);
	for( INT b=0; b< Mesh->RefSkeleton.Num(); b++)
	{
		if ( Mesh->RefSkeleton(b).Name == TempFname )
		{
			Mesh->WeaponBoneIndex = b;
			debugf(TEXT("Classic weapon bone link assigned to bone: %s"),*Mesh->RefSkeleton(b).Name);
			break;
		}
	}
	unguard;
}



INT UEditorEngine::animGetBoneIndex
( 
	UAnimation* Anim,
	FName TempFname 
)
{
	guard(UEditorEngine::animGetBoneIndex);
	for( INT b=0; b< Anim->RefBones.Num(); b++)
	{
		if ( Anim->RefBones(b).Name == TempFname )
		{
			return b;
		}
	}
	return 0;
	unguard;
}


void UEditorEngine::modelSetWeaponPosition
( 
	USkeletalMesh* Mesh, 
	FCoords WeaponCoords 
)
{
	guard(UEditorEngine::modelSetWeaponPosition);
	// Set the private weapon coordinate system (constructed from a vector and a rotation in UnEdSrv.cpp, guaranteed not
	// to change scale.
	Mesh->WeaponAdjust = WeaponCoords;
	unguard;
}

void UEditorEngine::animationImport
(
	const TCHAR*		AnimName,
	UObject*			InParent,
	const TCHAR*		DataFname,
	UBOOL				Unmirror,
	UBOOL               ImportSeqs,
	FLOAT				CompDefault
)
{
	guard(UEditorEngine::animationImport);

	UAnimation*	    NewAnimation;
	FArchive*		AnimationFile;
	VChunkHeader	ChunkHeader;

	debugf( NAME_Log, TEXT("Importing animation %s"), AnimName );
	GWarn->BeginSlowTask( TEXT("Importing skeletal mesh"), 1, 0 );
	GWarn->StatusUpdatef( 0, 0, TEXT("%s"), TEXT("Reading files") );

	// Allocate skeletal mesh object.
	NewAnimation = new( InParent, AnimName, RF_Public|RF_Standalone )UAnimation();
	
	// Open skeletal animation key file and read header.
	AnimationFile = GFileManager->CreateFileReader( DataFname, 0, GLog );
	if( !AnimationFile )
	{
		appErrorf( NAME_Log, TEXT("Error opening animation file %s"), DataFname );
	}

	// Read main header
	AnimationFile->Serialize( &ChunkHeader, sizeof(VChunkHeader) );
	if( AnimationFile->IsError() )
	{
		appErrorf( NAME_Log, TEXT("Error reading animation file %s"), DataFname );
	}

	
	// Read the header and bone names.
	AnimationFile->Serialize( &ChunkHeader, sizeof(VChunkHeader) );

	
	TArray<FNamedBoneBinary> RawBoneNames;
	guard( BoneNames );
	RawBoneNames.Add( ChunkHeader.DataCount );	
	NewAnimation->RefBones.Add( ChunkHeader.DataCount );
	AnimationFile->Serialize( &RawBoneNames(0), sizeof( FNamedBoneBinary ) * ChunkHeader.DataCount );
	unguard;

	guard(RawBoneNames);
	// Translate the raw data from the bones to Animation->RefBones FNames
	for( INT n=0; n<RawBoneNames.Num(); n++ )
	{
		appTrimSpaces(&RawBoneNames(n).Name[0]);
		NewAnimation->RefBones(n).Name  = FName( appFromAnsi(&RawBoneNames(n).Name[0]) );
		NewAnimation->RefBones(n).Flags = RawBoneNames(n).Flags;
		NewAnimation->RefBones(n).ParentIndex = RawBoneNames(n).ParentIndex;
	}
	unguard;

	guard(SeqInfoRaw);
	// Read the header and the animation sequence info if present...
	AnimationFile->Serialize( &ChunkHeader, sizeof(VChunkHeader) );
	NewAnimation->RawAnimSeqInfo.Add(ChunkHeader.DataCount);
	AnimationFile->Serialize( &NewAnimation->RawAnimSeqInfo(0), sizeof(AnimInfoBinary) * ChunkHeader.DataCount);
	// Remember to change  the raw animation name and group to FNames... =  FName(appFromAnsi( &name ));
	unguard;
	
	guard(animheader);
	// Read the header and beta keys.
	AnimationFile->Serialize( &ChunkHeader, sizeof(VChunkHeader) );
	NewAnimation->RawAnimKeys.Add(ChunkHeader.DataCount);
	AnimationFile->Serialize( &NewAnimation->RawAnimKeys(0), sizeof(VQuatAnimKey) * ChunkHeader.DataCount);	
	NewAnimation->RawNumFrames = ChunkHeader.DataCount / NewAnimation->RefBones.Num();
	NewAnimation->CompFactor = CompDefault;
	delete AnimationFile;
	unguard;

	guard(ImportSeqs);
	// Add binary sequence info to queue.
	if( ImportSeqs )
	{
		debugf(TEXT("New animation %s has %i imported sequences."), NewAnimation->GetName(), NewAnimation->RawAnimSeqInfo.Num());
			
		// Convert any animation info as defined in the input file RawAnimSeqInfo into MovesInfo...
		for( INT i=0; i<NewAnimation->RawAnimSeqInfo.Num(); i++)
		{
			MotionChunkDigestInfo NewMoveInfo;
			// Explicit copying - we need to convert names to FNames...
			NewMoveInfo.Name = FName(appFromAnsi( NewAnimation->RawAnimSeqInfo(i).Name));
			NewMoveInfo.Group = FName(appFromAnsi( NewAnimation->RawAnimSeqInfo(i).Group));

			NewMoveInfo.FirstRawFrame		= NewAnimation->RawAnimSeqInfo(i).FirstRawFrame;
			NewMoveInfo.KeyCompressionStyle = NewAnimation->RawAnimSeqInfo(i).KeyCompressionStyle;
			NewMoveInfo.KeyQuotum			= NewAnimation->RawAnimSeqInfo(i).KeyQuotum;
			NewMoveInfo.KeyReduction		= NewAnimation->RawAnimSeqInfo(i).KeyReduction;
			NewMoveInfo.NumRawFrames		= NewAnimation->RawAnimSeqInfo(i).NumRawFrames;
			NewMoveInfo.RootInclude			= NewAnimation->RawAnimSeqInfo(i).RootInclude;
			NewMoveInfo.StartBone			= NewAnimation->RawAnimSeqInfo(i).StartBone;
			NewMoveInfo.TrackTime			= NewAnimation->RawAnimSeqInfo(i).TrackTime;
			NewMoveInfo.AnimRate			= NewAnimation->RawAnimSeqInfo(i).AnimRate;

			// Force Rate/Time.Framerate match
			if( NewMoveInfo.AnimRate > 0.0f )
				NewMoveInfo.TrackTime = NewMoveInfo.NumRawFrames / NewMoveInfo.AnimRate;

			// Addunique - based on name ? 
			// Check whether legal, then add...
			if( (NewMoveInfo.NumRawFrames != 0) && ( (NewMoveInfo.KeyQuotum != 0) || (NewMoveInfo.KeyReduction !=0.0f ) ) )
				NewAnimation->MovesInfo.AddItem(NewMoveInfo);
		}
	}
	NewAnimation->RawAnimSeqInfo.Empty();		
	unguard;

	unguard;
}

// Quick inter-key error evaluation. Assumes indices are valid.
template<class T>
FLOAT GetInterKeyError( T& Track, INT a, INT b, INT c, float BoneSize )
{
	FLOAT IntervalSize = Track.KeyTime(c) - Track.KeyTime(a);
	FLOAT Alpha = (Track.KeyTime(b)- Track.KeyTime(a))/ IntervalSize;  

	FLOAT RotError;
	if( (Track.KeyQuat(a) | Track.KeyQuat(c)) < -0.9f )
		// Don't interpolate if nearly 180 degrees apart.
		RotError = 1.f;
	else
	{
		FQuat LerpedQuat = SlerpQuat( Track.KeyQuat(a), Track.KeyQuat(c), Alpha );
		RotError = QuatError( LerpedQuat, Track.KeyQuat(b) );
	}

	FVector LerpedPos = ( Track.KeyPos(a)*Alpha + Track.KeyPos(c)*(1.f-Alpha) );
	FLOAT PosError = (LerpedPos - Track.KeyPos(b)).Size();
	
	return RotError*BoneSize + PosError;
}

//
// Compress single animation from the raw data ( bones * frames ) 
// as found in RawAnimKeys()
// Anim->Moves(MoveIndex)
// Anim->MovesInfo(MoveIndex)
// Anim->AnimSeqs(MoveIndex)
//
void UEditorEngine::movementDigest(	UAnimation* Anim, INT MoveIndex )
{
	static FLOAT POSERRCMP = 0.01f;		// Spatial difference below which positions are considered identical.
	
	guard(movementDigest);

	MotionChunk* ThisMove = &Anim->Moves(MoveIndex);
	MotionChunkDigestInfo* ThisMoveInfo = &Anim->MovesInfo(MoveIndex);

	// Builds a FMeshAnimSeq and associated animation data for each new Move.
	Anim->AnimSeqs.AddZeroed();
	//FMeshAnimSeq* Seq = &Anim->AnimSeqs(MoveIndex);
	FMeshAnimSeq* Seq = &Anim->AnimSeqs(Anim->AnimSeqs.Num()-1);
	
	debugf(TEXT("Digesting movement number %i  name %s "),MoveIndex,*ThisMoveInfo->Name); 

	//  ! Nonzero startbone not supported by GetFrame yet.

	// Refill the remap array according to Startbone
	
	// Fill array with reduced hierarchy bone indices.
	TArray <INT> Hierarchy;
	TArray <INT> MarkParent;

	MarkParent.AddZeroed(Anim->RefBones.Num());
	
	ThisMove->StartBone = ThisMoveInfo->StartBone;
	// Rip out the sub-hierarchy.
	if( ThisMove->StartBone )
		for(INT i=ThisMove->StartBone; i<Anim->RefBones.Num(); i++)
		{		
			if( i==ThisMove->StartBone || MarkParent( Anim->RefBones(i).ParentIndex ) ) MarkParent(i)=1;
			if (MarkParent(i)) Hierarchy.AddItem(i);
		}
	else
		for(INT i=0; i<Anim->RefBones.Num(); i++)
		{
			Hierarchy.AddItem(i);
		}

	debugf(TEXT(" movement Digestion: Sub-hierarchy startbone: %i Total nodes: %i "),ThisMove->StartBone, Anim->RefBones.Num());

	// AnimTracks workspace.
	ThisMove->AnimTracks.Empty();
	ThisMove->AnimTracks.AddZeroed(Hierarchy.Num()); // AddZeroed needed - these contain dynamic arrays.

	////////////////////////////////////////////////////////////////////////////////

	// To do: Actually the KeyReduction factor should apply AFTER we threw out all trivial keys.
	
	INT KeyMaximum = (INT)Abs( (ThisMove->AnimTracks.Num() * ThisMoveInfo->NumRawFrames)*ThisMoveInfo->KeyReduction); 
	
	if (ThisMoveInfo->KeyQuotum > 0)
		KeyMaximum = Min(ThisMoveInfo->KeyQuotum,KeyMaximum);

	ThisMove->AnimTracks.Empty();
	ThisMove->AnimTracks.AddZeroed(Hierarchy.Num()); 

	debugf(TEXT("Processing uAnimation: %s - number of Bones: %i  KeyReduction %f NumRawFrames %i"),Anim->GetName(),Anim->RefBones.Num(), ThisMoveInfo->KeyReduction, ThisMoveInfo->NumRawFrames );

	ThisMove->TrackTime = ThisMoveInfo->TrackTime;

	// Fill in the backward-compatible sequence data.
	// Adding notifys: only done AFTER the digestion.
	Seq->Group = ThisMoveInfo->Group;
	Seq->Name =  ThisMoveInfo->Name;
	Seq->NumFrames = ThisMoveInfo->NumRawFrames;
	Seq->Rate = ThisMoveInfo->AnimRate; // Should still be 'frames per second'.
	Seq->StartFrame = 0; // Always the start of a compressed skeletal move.

	// Does the range overrun the actual amount of keys ?
	if( (ThisMoveInfo->FirstRawFrame + ThisMoveInfo->NumRawFrames) * Anim->RefBones.Num() > Anim->RawAnimKeys.Num() )
		debugf(TEXT("Skeletal frame number overrun warning for sequence %s : Total %i Requested: %i to %i"),Anim->GetName(),Anim->RawAnimKeys.Num()/Anim->RefBones.Num(),  ThisMoveInfo->FirstRawFrame, ThisMoveInfo->FirstRawFrame+ThisMoveInfo->NumRawFrames ) ;

	TArray<FRawTrack> RawTracks;
	RawTracks.AddZeroed( ThisMove->AnimTracks.Num() );
	INT i;

	// Reorder raw data into the appropriate tracks - Full bones.
	for(i=0; i<ThisMove->AnimTracks.Num(); i++)
	{
		INT b = Hierarchy(i);
		NLOG( debugf(TEXT(" Bone B:%i for hierarchy I: %i"),b,i);)
		RawTracks(i).KeyPos.Empty(ThisMoveInfo->NumRawFrames);
		RawTracks(i).KeyQuat.Empty(ThisMoveInfo->NumRawFrames);
		RawTracks(i).KeyTime.Empty(ThisMoveInfo->NumRawFrames);

		for( INT f=0; f< ThisMoveInfo->NumRawFrames; f++ )
		{		
			// Min() makes sure no illegal key index is used.
			INT KeyIdx = Min(Anim->RawAnimKeys.Num()-1,( ThisMoveInfo->FirstRawFrame + f ) * Anim->RefBones.Num() + b);

			//debugf(TEXT("Raw frame %i KeyIndex %i rawanimkeystotal %i "), f, KeyIdx, Anim->RawAnimKeys.Num());
			RawTracks(i).KeyPos.AddItem( Anim->RawAnimKeys(KeyIdx).Position );
			if( b == 0 )
				// Due to historical idiosyncrasy, root orientation is negated. !
				RawTracks(i).KeyQuat.AddItem( -Anim->RawAnimKeys(KeyIdx).Orientation );
			else
				RawTracks(i).KeyQuat.AddItem( Anim->RawAnimKeys(KeyIdx).Orientation );
			RawTracks(i).KeyTime.AddItem( f );  
		}
	}

	// Nothing to eliminate if there are < 3 keys in all tracks. => To do:  except for static position tracks.
	if( ThisMoveInfo->NumRawFrames > 2 )
	{
		// Find largest bone size (seems to range from 10 to 40..) -> to factor into the error.
		TArray<FLOAT> BoneSizes( RawTracks.Num() );
		INT b;
		{for_array( b, BoneSizes )
			// Set to minimum value. It would be nice to have mesh info here.
			BoneSizes(b) = 1.f;}

		for( i=RawTracks.Num()-1; i>=0; i-- )
		{
			INT b=Hierarchy(i);
			if( b != 0 ) // Ignore root track offset which does not represent a bone.
			{
				FLOAT BoneDist = 0.f;
				for( INT f=0; f < RawTracks(i).KeyPos.Num(); f++ )
				{
					BoneDist = Max( BoneDist, RawTracks(i).KeyPos(f).Size() );
				}

				// Pretend size of this bone is same as its distance from parent.
				BoneSizes(b) = Max( BoneSizes(b), BoneDist );
				INT p = Anim->RefBones(b).ParentIndex;
				if( p != b )
				{
					// Contribute to parent's size.
					BoneSizes(p) = Max( BoneSizes(p), BoneDist + BoneSizes(b) );
				}
			}
		}

		// For leaf bones, add the average size.
		for( i=RawTracks.Num(); i>=0; i-- )
		NLOG( debugf(TEXT("Max bone size for this animation: %f"), BoneMax) );
		
		// Scale bone errors depending on max bone size.
		NLOG( debugf(TEXT("Keys before culling: %i  Target: %i"), RawTracks.Num() * ThisMoveInfo->NumRawFrames,KeyMaximum) );

		// Stats keeping
		INT TotalKeys = RawTracks.Num() * ThisMoveInfo->NumRawFrames;
		INT RemovedMatched = 0;
		INT RemovedLerped = 0;	

		// First culling step.
		for( b=0; b<RawTracks.Num(); b++)
		{
			FRawTrack& Track = RawTracks(b);
		
			// From tail on down, convert data into contiguously interpolatable segments.
			for( INT e=Track.KeyTime.Num()-1; e>=2; )
			{
				// Test every point below as the start of segment.
				// Due to compression format, can't eliminate more than 255 consecutive frames.
				INT s;
				for( s=e-2; s>=0 && s>=e-255; s-- )
				{
					// Test every point in between for error.
					INT m;
					for( m=s+1; m<e; m++ )
					{
						FLOAT PosErr = GetInterKeyError( Track, s, m, e, BoneSizes(b) );
						if( PosErr > POSERRCMP )
							break;
					}
					if( m<e )
						// Failure.
						break;
				}

				s += 2;
				if( s < e )
				{
					// We can eliminate all between s and e.
					INT Kill = e-s;
					Track.KeyQuat.Remove(s, Kill);
					Track.KeyPos.Remove(s, Kill);
					Track.KeyTime.Remove(s, Kill);
					RemovedMatched += Kill;
					TotalKeys -= Kill;
				}
				e = s-1;
			}
		} 
		
		
		/*
		  ### Note:
		  Hierarchy.Num() instead of Anim->RefBones.Num() usually;
		  Error arrays etc all size Hierarchy.Num(), or Thismove->animTracks.Num()....
		  Unless you need actual full skeleton bone info keep to this size/index otherwise b=Hierarchy(i)

		*/

#if 0
		// Allocate error arrays
		TArray <FLOAT> MinTrackError;
		TArray <FLOAT> MinTrackErrIdx;
		MinTrackError.AddZeroed( RawTracks.Num());
		MinTrackErrIdx.AddZeroed( RawTracks.Num());

		TArray < TArray<FLOAT> > DevTrack;
		DevTrack.AddZeroed(RawTracks.Num());

		// Main interpolation/compression loop

		// Precaculate per-track smallest lerp error.
		for( b=0; b<RawTracks.Num(); b++)
		{
			FRawTrack& Track = RawTracks(b);

			DevTrack(b).AddZeroed( Track.KeyQuat.Num() );

			if ( Track.KeyQuat.Num() > 2)
			{
				// Fill error arrays, and min-error index & error;
				MinTrackErrIdx(b) = -1;
				MinTrackError(b) = 1000000.0f;

				for( INT i=1; i<Track.KeyQuat.Num()-1; i++)
				{
					DevTrack(b)(i) = GetInterKeyError(Track, i, BoneSizes(b) );
					if (DevTrack(b)(i) < MinTrackError(b))
					{
						MinTrackError(b) = DevTrack(b)(i);
						MinTrackErrIdx(b) = i;
					}
				}			
			}
		}


		debugf(TEXT("Start: Keymax %i Totalkeys %i "),KeyMaximum, TotalKeys );


		guard(While);
		while ( 1/*KeyMaximum < TotalKeys*/ ) // TotalKeys must keep track of all keys
		{
			// Find the smallest overall error
			INT SmallestIdx = -1;
			FLOAT SmallestError = 100000000.f;

			//debugf(TEXT("Keymax %i Totalkeys %i "),KeyMaximum, TotalKeys );

			guard(Errortest);
			for( i=0; i<RawTracks.Num(); i++)
			{
				if ( (MinTrackError(i) < SmallestError) && RawTracks(i).KeyQuat.Num() > 2 )
				{
					SmallestIdx = i;
					SmallestError = MinTrackError(i);
				}
			}
			unguard;

			guard(Deletelerp);
			if( SmallestIdx == -1) break; // Less than 3 keys left in all tracks, so exit.
			// Check whether we need to exit because the smallest overall error is too big to lerp
			if( SmallestError > POSERRCMP) break;

			// Delete the most lerp-able key.
			INT i = MinTrackErrIdx(SmallestIdx);
			FRawTrack& Track = RawTracks(SmallestIdx);

			//debugf(TEXT("Removing %i from keys totals %i %i %i "),i,Track.KeyQuat.Num(),Track.KeyPos.Num(),Track.KeyTime.Num());
			//debugf(TEXT("Removing %i from  err totals %i %i smallestidx %i "), i,DevTrack(SmallestIdx).QuatErr.Num(),DevTrack(SmallestIdx).PosErr.Num(),SmallestIdx);
			Track.KeyQuat.Remove(i);
			Track.KeyPos.Remove(i);
			Track.KeyTime.Remove(i);
			DevTrack(SmallestIdx).Remove(i);

			RemovedLerped++;
			TotalKeys--;		

			// Update the error for all (bordering) keys in this track,
			// And the MinTrackError for this track.
			if( Track.KeyQuat.Num() > 2)
			{			
				if (i < Track.KeyQuat.Num() - 1)
				{
					GetInterKeyError( Track, i, BoneSizes(b) );
				}
				if (i > 1) 
				{
					GetInterKeyError( Track, i-1, BoneSizes(b) );
				}

				MinTrackError(SmallestIdx)  = 1000000.f;
				MinTrackErrIdx(SmallestIdx) = -1;

				// update MinTrackError and MinTrackErrIdx for this track.
				for( INT i=1; i<Track.KeyQuat.Num()-1; i++)
				{				
					FLOAT ThisError = DevTrack(SmallestIdx)(i);
					if (ThisError < MinTrackError(SmallestIdx))
					{
						MinTrackError(SmallestIdx) = ThisError;
						MinTrackErrIdx(SmallestIdx) = i;
					}
				}			
			}		
			unguard;
		}
		unguard;

		DevTrack.Empty();

		NLOG( debugf(TEXT(" QuatKeys after lossy culling 1: %i Duplicates: %i LerpMatches: %i "),TotalKeys,RemovedMatched,RemovedLerped) );
#endif

		// Turn any 2-key tracks into 1 key tracks if possible.
		for( b=0; b<RawTracks.Num(); b++)
		{
			FRawTrack& Track = RawTracks(b);
			
			if( Track.KeyQuat.Num() == 2 )
			{
				// Collapse 2nd key only if both Quat and Pos difference fall within the max delta limits.
				FLOAT QuatDiff = QuatError( Track.KeyQuat(0),Track.KeyQuat(1) );
				FLOAT PosDiff  = ( Track.KeyPos(0) - Track.KeyPos(1) ).Size();

				if ( (QuatDiff*BoneSizes(b) + PosDiff) < POSERRCMP )
				{
					Track.KeyQuat.Remove(1);
					Track.KeyPos.Remove(1);
					Track.KeyTime.Remove(1);
					RemovedMatched++;
					TotalKeys--;
				}		
			}
		
			if( RawTracks(b).KeyQuat.Num() > 1 )
				NLOG( debugf(TEXT("#-> [%5i] QuatKeys in track %20s [%5i]"),RawTracks(b).KeyQuat.Num(),*Anim->RefBones(b).Name, b ) );
		}
			
		// Compress positions: Now that tracks have been compressed, decide for each track
		// if it only needs to hold one static position or remain a full position track ( usually 
		// the root bone Pos track remains )

		INT RemovedPos = 0;
		
		for( b=0; b<RawTracks.Num(); b++ )
		{
			FLOAT MaxDelta = 0.0f;
			for(INT i=0; i<RawTracks(b).KeyPos.Num(); i++)
			{
				FVector DiffPos = RawTracks(b).KeyPos(i) - RawTracks(b).KeyPos(0);
				FLOAT LocalDiff;
				LocalDiff = DiffPos.Size();
				if( LocalDiff>MaxDelta ) MaxDelta = LocalDiff;
			}
			if( MaxDelta < POSERRCMP) // No significant deviation in all local positions => turn it into a static track.
			{
				FVector SinglePos = RawTracks(b).KeyPos(0);
				RawTracks(b).KeyPos.Empty();
				RawTracks(b).KeyPos.AddItem(SinglePos);
				RemovedPos++;
			}
			if( RawTracks(b).KeyPos.Num() > 1 )
				NLOG( debugf(TEXT("#-> [%5i] PosKeys in track %20s [%5i]"),RawTracks(b).KeyPos.Num(),*Anim->RefBones(b).Name, b ) );
		}
		
		debugf(TEXT("QuatKeys after lossy culling 2:%i  Duplicates: %i LerpMatches: %i Removed Pos tracks: %i"),TotalKeys,RemovedMatched,RemovedLerped,RemovedPos);
		
		// Align quats. The first and last quats in looping animations can only be aligned at runtime...
		for( b=0; b<RawTracks.Num(); b++)
		{
			for(INT i=1; i< RawTracks(b).KeyQuat.Num(); i++)
			{
				AlignQuatWith( RawTracks(b).KeyQuat(i), RawTracks(b).KeyQuat(i-1));		
			}
		}

		// When limited to a bone subset, copy the hierarchy bonemapping array into our ThisMove
		ThisMove->BoneIndices.Empty();
		for( INT t=0; t<Anim->RefBones.Num(); t++)
		{
			INT NodeIdx =  Hierarchy.FindItemIndex(t);
			if( NodeIdx!=INDEX_NONE )
				ThisMove->BoneIndices.AddItem(NodeIdx);
			else
				ThisMove->BoneIndices.AddItem(-1);
		}
	}

	// Merge RawTracks into compressed MasterTrack, and store counts.
	INT q, s, p, t;
	for( i=0; i<RawTracks.Num(); i++)
	{
		for_array( q, RawTracks(i).KeyQuat )
			//Anim->MasterTrack.KeyQuat.AddItem( FAnimVec(RawTracks(i).KeyQuat(q)) );
			Anim->MasterTrack.KeyQuat.AddItem( RawTracks(i).KeyQuat(q) );

		// Find max translation scale.
		FLOAT Scale = 0.f;
		for_array( s, RawTracks(i).KeyPos )
			Scale = Max( Scale, RawTracks(i).KeyPos(s).MaxVal() );
		for_array( p, RawTracks(i).KeyPos )
			Anim->MasterTrack.KeyPos.AddItem( FAnimVec( RawTracks(i).KeyPos(p), 1.f/Scale ) );
		for_array( t, RawTracks(i).KeyTime )
		{
			INT Delta = t>0 ? RawTracks(i).KeyTime(t) - RawTracks(i).KeyTime(t-1) : 0;
			check( Delta < 256 );
			Anim->MasterTrack.KeyDelta.AddItem(Delta);
		}

		ThisMove->AnimTracks(i).PosScale = Scale;
		ThisMove->AnimTracks(i).TimeScale = ThisMove->TrackTime / ThisMoveInfo->NumRawFrames;
		ThisMove->AnimTracks(i).KeyQuat.Set( NULL, RawTracks(i).KeyQuat.Num() );
		ThisMove->AnimTracks(i).KeyPos.Set( NULL, RawTracks(i).KeyPos.Num() );
		ThisMove->AnimTracks(i).KeyDelta.Set( NULL, RawTracks(i).KeyTime.Num() );
	}

	unguard;
}

//
// Digest the raw frame data into a movement repertoire.
//
void UEditorEngine::digestMovementRepertoire( UAnimation* Anim)
{	
	debugf(TEXT("## Digesting %i movements for animation %s "),Anim->MovesInfo.Num(),Anim->GetName());

	// Discard raw sequence info.
	Anim->RawAnimSeqInfo.Empty();

	// Allocate moves.
	Anim->Moves.Empty();
	Anim->Moves.AddZeroed(Anim->MovesInfo.Num());

	{for(INT i=0; i<Anim->MovesInfo.Num(); i++)
	{		
		debugf(TEXT("Digesting motion [%s] number %i  raw keys: %i reduction: %f "), (*Anim->MovesInfo(i).Name), i, Anim->MovesInfo(i).NumRawFrames * Anim->RefBones.Num(),Anim->MovesInfo(i).KeyReduction);

		if (1) //#debug
		{
			debugf(TEXT("Group: %s  Rate %f  Time %f  StartBone %i RootInclude %i NumRawFrames %i KeyReduction %f KeyQuotum %i KeyCompStyle %i FirstRawFrame %i"),
			*Anim->MovesInfo(i).Group,
			Anim->MovesInfo(i).AnimRate, 
			Anim->MovesInfo(i).TrackTime,
			Anim->MovesInfo(i).StartBone,
			Anim->MovesInfo(i).RootInclude,
			Anim->MovesInfo(i).NumRawFrames,
			Anim->MovesInfo(i).KeyReduction,
			Anim->MovesInfo(i).KeyQuotum,
			Anim->MovesInfo(i).KeyCompressionStyle,
			Anim->MovesInfo(i).FirstRawFrame );
		}
		
		movementDigest( Anim, i ); 
		// debugf(TEXT("  digested motion : %i keytracks:  %i rate: %f "),Anim->Moves.Num(), Anim->Moves(i).AnimTracks.Num(),Anim->MovesInfo(i).AnimRate );
	}}

	Anim->AnimSeqs.Shrink(); 
	Anim->Moves.Shrink();
	Anim->RefBones.Shrink();
}

void UEditorEngine::patternImport( const TCHAR* InName, UObject* InParent, const TCHAR* FileName )
{
	debugf( NAME_Log, TEXT("Importing pattern %s"), InName );

	// Open skeletal animation key file and read header.
	FArchive* Ar = GFileManager->CreateFileReader( FileName, 0, GLog );
	if( !Ar )
	{
		GWarn->Logf( NAME_ExecWarning, TEXT("Error opening gesture file %s"), FileName );
		return;
	}

	DWORD ID;
	*Ar << ID;
	if( ID == *(DWORD*)"HPGF" )
	{
		unsigned short Count;
		*Ar << Count;

		UGesture* Gesture = new( InParent, InName, RF_Public|RF_Standalone ) UGesture;

		// Read the data. 
		Gesture->Points.Add( Count );
//		Gesture->Points.Add( Count+1 );
		for_count( i, (int)Count )
		{
			*Ar << Gesture->Points(i).X << Gesture->Points(i).Y;
			Gesture->Points(i).Z = 0.0f;
		}

		// Assume it's a closed shape, and duplicate the first point.
//		Gesture->Points.Last() = Gesture->Points(0);

		// Read the segments.
		Gesture->Segments.Add(32);
		{ for_count( i, 32 )
		{
			unsigned short Seg;
			*Ar << Seg;
			Gesture->Segments(i) = Seg;
		} }

		if( !Ar->IsError() )
		{
			delete Ar;
			return;
		}
	}

	GWarn->Logf( NAME_ExecWarning, TEXT("Invalid gesture file %s"), FileName );
	delete Ar;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

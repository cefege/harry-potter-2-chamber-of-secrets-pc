 /*==================================================================================================
	UnMeshLP.cpp: Unreal Mesh LODProcess. Level-of-detail mesh preprocessing code.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Erik de Neve.
        * Textured vertex 'wedges' concept inspired by Hugues Hoppe's papers (Microsoft Research).
		* Edge collapse processing code based on article: 'A Simple, Fast, and Effective Polygon 
		  Reduction Algorithm' by Stan Melax, Game Developer Magazine Nov. 1998.
		* Jan 2000 - skeletal mesh (LOD) processing code. Duplicates part of the regular mesh processing but starts
		  from different binary/raw format data.

	NOTES:
	[Mesh lod processing] This code takes a regular Unreal1 mesh & animation data *internally* at content-compile
	time, after it has been imported as a regular UMesh, and re-digests it into new structures 
	fit for real-time LOD collapse and morphing. 

	TODO:
	Currently there's a number of collapsing and noncollapsing arrays, the latter are
	(but not all) inside the Mesh-> hierarchy, but some in TModel too - a bit messy.

======================================================================================================*/

#include "EditorPrivate.h"
#include "UnRender.h"
#include "UnSkeletalMesh.h"
#include "UnSkeletalImport.h"

#undef  NOTE
#define NOTE(func)  {}
//#define NOTE(func) func

#undef  NLOG
#define NLOG(func) {}
//#define NLOG(func) func

/*-----------------------------------------------------------------------------
	Level of Detail processing.
-----------------------------------------------------------------------------*/

// Temporary LOD processing classes ( never serialized - for intermediate use only )
// Analogous to the ULodMesh structures, this is a setup with vertices, textured
// vertices and Faces.

// Globals
INT LODStyle;

// Forward declarations.
struct TVertex;
struct TFace;		
struct TWedge;

// Textured vertex ('wedge').
struct TWedge
{
	INT             OriginalIndex; 
	INT             VertexIndex;    
	INT             DestinationIndex;

	union
	{
		struct{	BYTE U;	BYTE V;};
		struct{ _WORD W;};
	};

	TWedge& operator=( const TWedge& Other )
	{
		OriginalIndex = Other.OriginalIndex;
		VertexIndex = Other.VertexIndex;
		DestinationIndex = Other.DestinationIndex;
		W = Other.W;
		return *this;
	}

};

// Structure used for sorting wedges.
struct TWedgeSort
{
	INT		SortedWedgeIndex;
	DWORD	Key;
};

QSORT_RETURN CDECL CompareWedgeKey( const TWedgeSort* A, const TWedgeSort* B )
{
	return A->Key - B->Key;
}


// Vertex
struct TVertex
{
	FVector             Point3D;        //  Our 3d point.
	INT					OriginalIndex;  //  place of vertex in original list.	

	TArray<TVertex*>    Verts;		    //  all directly connected vertices  (TransientVtxes)
	TArray<TFace*>		Faces;		    //  all directly connected triangles (Mesh->Faces)
	TArray<TWedge*>     Wedges;         //  a wedge* for each Face: ie, these give the UV of a Face at _this_ vertex.

	INT                 Flag;           //  
	FLOAT				MergeCost;	    //  Cost of collapsing to ColTarget
	TVertex*			ColTarget;      //  candidate vertex for collapse

	                 TVertex(FVector v, INT OrigIndex, INT FlagInit);  
	                 ~TVertex();        
	INT              RemoveIfNonNeighbor(TVertex *n);
	INT              IsBorder();
};


// Face
struct TFace
{
	INT					OriginalIndex;    // 
	TVertex*	        Verts[3];         // Nontextured vertices 
	TWedge*             Wedges[3];        // The same, textured vertices. 

	INT                 MaterialID;       // Material Index.
	FVector             Normal;           // Normalized normal vector.

	UBOOL	HasVertex( TVertex *U );
	int     GetVertexIndex(TVertex *U);
			TFace( INT MatID, TWedge *v0, TWedge *v1,TWedge *v2, INT StartIndex);
			~TFace();
	void    ComputeNormal();
	void    ReplaceVertex( TVertex *VOld, TVertex *VNew );
};



// Global mesh for LOD processing.
class TMesh
{
public:

	TArray<TFace*>      GFaces;      // Faces (allocated individually)
	TArray<TVertex*>    GVerts;      // Vertices (allocated individually)
	TArray<TWedge>      GWedges;     // Wedges 
	TArray<TWedge>      GAuxWedges;  // Auxiliary wedge copies.
	TArray<INT>         GOriginalVertIdx; // Original vertex indices mirroring GVerts(x).OriginalIndex;

	TArray<_WORD>         FaceLevel;    // mirrors Mesh's facelevel
	TArray<FMeshMaterial> Materials;    // mirrors Mesh's materials

	// Vertex based evaluation and collapse methods
	FLOAT	GetCollapseCost( TVertex *U, TVertex *V );
	INT     IsBorderVertex ( TVertex *U );
	void    UpdateEdgeCost ( TVertex *U );
	void	Collapse( TVertex *U );		

	void Reset()
	{
		GFaces.Empty();
		GVerts.Empty();
		GWedges.Empty();
		GAuxWedges.Empty();
		GOriginalVertIdx.Empty();
		FaceLevel.Empty();
		Materials.Empty();
	}

}TModel;


// Methods.

// By making the TMesh TModel a global, TFace and TVertex methods can update 
// the arrays that they're part of, themselves.

TFace::TFace(INT MatID, TWedge *v0, TWedge *v1,TWedge *v2, INT StartIndex)
{
	guard(TFace::TFace);

	MaterialID = MatID;
	OriginalIndex = StartIndex;

	Wedges[0] = v0;
	Wedges[1] = v1;
	Wedges[2] = v2;
	//! kludgy ! GVerts must have been set up completely.
	Verts[0] = TModel.GVerts(v0->VertexIndex);
	Verts[1] = TModel.GVerts(v1->VertexIndex);
	Verts[2] = TModel.GVerts(v2->VertexIndex);

	// Discard triangles with non-unique vertices.
	if ( Verts[0]==Verts[1] || Verts[1]==Verts[2] || Verts[2]==Verts[0] )
	{
		debugf(TEXT("Mesh LOD processing warning - non-unique vertices for face %5i are: %5i %5i %5i"),
			   this->OriginalIndex,
			   Verts[0]->OriginalIndex,
			   Verts[1]->OriginalIndex,
			   Verts[2]->OriginalIndex );
		return; // Don't add this face to the big array then !!!
	}

	ComputeNormal();
	TModel.GFaces.AddItem( this );

	for( INT i=0; i<3; i++) 
	{
		// Update bordering vertices with Face.
		Verts[i]->Faces.AddItem( this );
		// Back-link Wedge to its vert.
		Verts[i]->Wedges.AddItem( Wedges[i] ); 

		// Update neighboring vertices with these vertices.
		for( INT j=0; j<3; j++ ) 
		{
			if ( i!=j ) Verts[i]->Verts.AddUniqueItem( Verts[j] );
		}
	}
	unguard;
}

TFace::~TFace()
{
	guard(TFace::~TFace);
	TModel.GFaces.RemoveItem( this );

	// Record the vertex count at which this face became obsolete.
	TModel.FaceLevel(this->OriginalIndex) = TModel.GVerts.Num();
	NOTE(debugf(TEXT("%% Original Face %i  Collapsed when there were %i vertices left."),this->OriginalIndex,TModel.GVerts.Num());)
	NOTE(debugf(TEXT("%% And its original wedges were %i %i %i "),Wedges[0]->OriginalIndex,Wedges[1]->OriginalIndex,Wedges[2]->OriginalIndex );)

	for( INT i=0; i<3; i++ ) 
	{
		if( Verts[i] ) 
		{
			//debugf(TEXT("Removing face %i from vertex %i"),this->OriginalIndex,Verts[i]->OriginalIndex);
		    INT FaceIndex = Verts[i]->Faces.FindItemIndex(this);
			// remove the face and the wedge with the same index as the face just removed.
		    Verts[i]-> Faces.Remove(FaceIndex);
			Verts[i]->Wedges.Remove(FaceIndex);
		}
	}

	for( i=0; i<3; i++) 
	{
		INT i2 = (i+1)%3;
		if( ! Verts[i] || ! Verts[i2] ) continue;

		Verts[i ]->RemoveIfNonNeighbor( Verts[i2] );
		//debugf(TEXT("Removing vertex %i if nonneighbor vertex %i "),Verts[i]->OriginalIndex,Verts[i2]->OriginalIndex);

		Verts[i2]->RemoveIfNonNeighbor( Verts[i ] );
		//debugf(TEXT("Removing vertex %i if nonneighbor vertex %i "),Verts[i2]->OriginalIndex,Verts[i]->OriginalIndex);
	}
	unguard;
}

int TFace::HasVertex(TVertex *U) 
{
	return ( U==Verts[0] || U==Verts[1] || U==Verts[2] );
}

int TFace::GetVertexIndex(TVertex *U)
{
	if (U==Verts[0]) return 0;
	if (U==Verts[1]) return 1;
	if (U==Verts[2]) return 2;

	debugf(TEXT("LOD preprocessing error: Vertex index not found in face."));
	return 0;
}

void TFace::ComputeNormal()
{
	guard(TFace::ComputeNormal);

	FVector v0 = Verts[0]->Point3D;
	FVector v1 = Verts[1]->Point3D;
	FVector v2 = Verts[2]->Point3D;

	Normal = (v1-v0) ^ (v2-v1);
	Normal /= (FLOAT)appSqrt(Normal.SizeSquared());

	unguard;
}


//  Replace a collapsing vertex in a triangle.
//  Might create a new wedge if UV texture coordinate morphing 
//  is desirable for this collapse.

void TFace::ReplaceVertex(TVertex *VOld, TVertex *VNew) 
{
	guard(TFace::ReplaceVertex);
	check(VOld && VNew);
	check(VOld==Verts[0] || VOld==Verts[1] || VOld==Verts[2]);
	check(VNew!=Verts[0] && VNew!=Verts[1] && VNew!=Verts[2]);

	NOTE(debugf(TEXT("Replacevertex  %i  by %i"),VOld->OriginalIndex,VNew->OriginalIndex));

	INT NewVertexIndex;

	if( VOld==Verts[0] )
	{
		NewVertexIndex = 0;
	}
	else if( VOld==Verts[1] )
	{
		NewVertexIndex = 1;
	}
	else 
	{
		check(VOld==Verts[2]);
		NewVertexIndex = 2;
	}
	Verts[NewVertexIndex] = VNew;

	// Kludgy wedge save/restore to make them match the faces array at each vertex.

	// Remove corresponding face and wedge connections from collapsing vertex.
	INT RemoveIndex = VOld->Faces.FindItemIndex(this);
	VOld->Faces.Remove(RemoveIndex);

	TWedge *OldWedge = VOld->Wedges(RemoveIndex); //Save...
	VOld->Wedges.Remove(RemoveIndex);

	check( VNew->Faces.FindItemIndex(this) == INDEX_NONE );
	// Add new face and wedge connectivity info to this new vertex.
	VNew->Faces.AddItem(this);
	VNew->Wedges.AddItem(OldWedge); // insert old one again.

	for( INT i=0; i<3; i++ ) 
	{
		VOld->RemoveIfNonNeighbor(Verts[i]);
		Verts[i]->RemoveIfNonNeighbor(VOld);
	}

	for( i=0; i<3; i++ ) 
	{
		check( Verts[i]->Faces.FindItemIndex(this) != INDEX_NONE ); 
		for( INT j=0; j<3; j++ ) if( i!=j ) 
		{ 
			Verts[i]->Verts.AddUniqueItem(Verts[j]);
		}
	}

	ComputeNormal();

	unguard;
}


TVertex::TVertex(FVector V,INT OrigIdx, INT FlagInit) 
{
	guard(TVertex::TVertex);
	Point3D = V;
	OriginalIndex = OrigIdx;
	Flag = FlagInit;
	TModel.GVerts.AddItem( this );
	unguard;
}

TVertex::~TVertex()
{
	guard(TVertex::~TVertex);

	// The vertex should have been detached from any face before deletion.
	check( Faces.Num()==0 );

	// Remove all mutual references between neighbors and us.
	while( Verts.Num() ) 
	{
		Verts(0)->Verts.RemoveItem(this);
		Verts.RemoveItem(Verts(0));
	}

	// Remove all Wedges references.
	Wedges.Empty();
	
	TModel.GVerts.RemoveItem(this);
	unguard;
}


int TVertex::RemoveIfNonNeighbor(TVertex *n) 
{
	guard(TVertex::RemoveIfNonNeighbor);
	// removes n from neighbor list if n isn't a neighbor.
	if( Verts.FindItemIndex(n) == INDEX_NONE ) return 0;

	for( INT i=0;i<Faces.Num();i++ ) 
	{
		if( Faces(i)->HasVertex(n) ) return 0;
	}
	Verts.RemoveItem(n);
	return 1;

	unguard;
}

//
// Edge detection.
//

int TVertex::IsBorder() 
{
	guard(TVertex::IsBorder);
    INT i,j;
    for( i=0; i < Verts.Num(); i++ ) 
	{
        INT Count=0;
        for( j=0; j < Faces.Num(); j++ ) 
		{
			if( Faces(j)->HasVertex( Verts(i) ) ) 
			{
				Count++;
            }
        }
        check( Count>0 );
        if( Count==1 ) return 1;
    }
    return 0;
	unguard;
}


//
//  Collapsing vertex A to B : Any triangle that has a wedge in point A, when that wedge is also present
//  as part of one of 1-2 triangles along the edge to be collapsed, then it should lerp it's UV's as it
//  collapses.  
//
//  Future refinements?
//  1: only if those common wedges are adjacent, or indirectly adjacent. 
//  2: lerping shouldn't make previously unused parts of textures visible if at all avoidable
//  3: there should be a lot more merging if at all possible, to get greater visual consistency
//     especially for body parts that are 'sort of' projection-mapped.
//  4: Texture fitness? Any two vertices that merge, the more wedges they DON't have in common _WITH THE wedges
//	   around the two bordering tris_ , the more expensive the collapse.
//

//
// Cost of collapsing u -> v
//
FLOAT ComputeEdgeCollapseCost( TVertex *u, TVertex *v ) 
{
	guard(ComputeEdgeCollapseCost);
	INT i;
	FLOAT EdgeLength; 

	guard(FDist);
	EdgeLength = FDist(v->Point3D, u->Point3D); 
	unguard;

	FLOAT Curvature=0.0f;
	FLOAT TexRespect = 1.0f;
	FLOAT TransBias = 1.0f;
	INT   Translucent = 0;
	INT   Planar = 1;

	// Find the "sides" triangles that are on the edge uv.
	TArray<TFace*>    SideFaces;
	TArray<TWedge*> SideWedges;
	TArray<TWedge*> NonSideWedges;
	TArray<TWedge*> UniqueWedges;

	// Detect side wedges, materials.
	for( i=0; i<u->Faces.Num(); i++ ) 
	{
		if(u->Faces(i)->HasVertex(v))
		{
			SideFaces.AddItem(u->Faces(i));		
			// check if not a double-sided poly
			if( ! (TModel.Materials( u->Faces(i)->MaterialID).PolyFlags & PF_TwoSided) )
			{
				Planar = 0;
			}

			if( (TModel.Materials( u->Faces(i)->MaterialID).PolyFlags & PF_Translucent) )
			{
				Translucent = 1;
			}

			SideWedges.AddItem(u->Wedges(i));
		}
		else
		{
			NonSideWedges.AddItem(u->Wedges(i));
		}
	}

	//    Gauge texture 'collapsability' ease: perfect if there's only one wedge (one texture
	//    UV pair shared by all bordering triangles.)
	for ( i=0; i<u->Wedges.Num(); i++)
	{
		UniqueWedges.AddUniqueItem(u->Wedges(i));
	}

	// This vertex is on a material SEAM: increase the 'texture respect' factor.
	if( UniqueWedges.Num() > 1 )
	{
		// Extra texture seam protection
		if( LODStyle & 4 ) 
		{
			TexRespect = 2.1f * UniqueWedges.Num();
		}
		else
		{
			// Default seam protection
			TexRespect = 0.7f * UniqueWedges.Num(); 
		}
	}

	// Extra translucent-material protection bit.
	if( LODStyle & 16 ) 
	{
		if( Translucent )
		{
			TransBias = 1.7f;
		}
	}

	// Planar (edge only borders doublesided polys): 'fixes' minimum curvature.
	if( ( LODStyle & 2 ) && ( Planar > 0 ) )
	{
		Curvature = 0.35f;  // Was 0.7f in 224v.
	}
	

	// But fix minimum Curvature if u is on a border but v isn't.
	if( u->IsBorder() && (SideFaces.Num() > 1) ) 
	{
		Curvature = 1.f; 
	}
	else
	{
		// Use the triangle facing most away from the sides 
		// to determine our Curvature term.
		// The more coplanar the lower the Curvature term 
		for( i=0; i<u->Faces.Num(); i++ ) 
		{
			FLOAT Mincurv=1.f; // Curve for face i and closer side to it.
			for( INT j=0; j<SideFaces.Num(); j++ ) 
			{
				// Use dot product of face normals.
				FLOAT Dotprod = u->Faces(i)->Normal | SideFaces(j)->Normal;
				Mincurv = Min(Mincurv,(1-Dotprod)/2.0f);
			}
			Curvature = Max(Curvature,Mincurv);
		}
	}

	// Emphasize lenght by disregarding curvature. Cool for blocky low-poly picups, boxes.
	if( LODStyle & 8 ) 
	{
		Curvature = 1.0f;
	}

	// Emphasize curvature by disregarding lengths.
	if( LODStyle & 1 )
	{
		EdgeLength = 1.0f;
	}

	// Make edge length twice as important.
	if( LODStyle & 32 )
	{
		EdgeLength = EdgeLength*EdgeLength;
	}

	return EdgeLength * Curvature * TexRespect * TransBias;

	unguard;
}

void ComputeEdgeCostAtVertex(TVertex *v) 
{
	guard(ComputeEdgeCostAtVertex);

	if( v->Verts.Num()==0 ) 
	{
		// v Doesn't have neighbors so it costs nothing to collapse.
		v->ColTarget = NULL;
		v->MergeCost= -0.01f;
		return;
	}

	v->MergeCost = 1000000.f;
	v->ColTarget= NULL;

	// Search all neighboring edges for "least cost" edge.
	for( INT i=0; i<v->Verts.Num(); i++ ) 
	{
		FLOAT dist;
		dist = ComputeEdgeCollapseCost( v, v->Verts(i) );
		if( dist<v->MergeCost ) 
		{
			v->ColTarget= v->Verts(i);  // Candidate for edge collapse.
			v->MergeCost = dist;        // Cost of the collapse.
		}
	}
	unguard;
}



void Collapse(TVertex *u, TVertex *v)
{
	guard(Collapse);

	// Collapse the edge uv by moving vertex u onto v
	// Actually remove tris on uv, then update tris that
	// have u to have v, and then remove u.
	INT i;

	if(!v) 
	{
		// u is a vertex all by itself so just delete it.
		if( u )
		{
			// Delete lone triangles.
			for( i=u->Faces.Num()-1; i>=0; i--) 
			{				
				delete u->Faces(i);
			}
			delete u;
		}
		return;
	}

	// Before deleting ANY faces, decide what to do with
	// each wedge involved: it can either morph to a wedge at the destination,
	// or we need to make a new wedge with the adapted vertex index.

	// Check to see if we kept wedges and faces matched.
	check( u->Faces.Num() == u->Wedges.Num() );
	check( v->Faces.Num() == v->Wedges.Num() );

	TArray<TFace*>  SideFaces;
	TArray<TWedge>  SideWedges; // No pointers - because content changes during loop.

	TArray<TWedge*> NonSideWedges;
	TArray<TWedge*> UniqueNonSideWedges;

	SideFaces.Empty();
	SideWedges.Empty();
	NonSideWedges.Empty();
	UniqueNonSideWedges.Empty();

	for( i=0; i<u->Faces.Num(); i++ ) 
	{
		if( u->Faces(i)->HasVertex(v) )
		{
			SideFaces.AddItem(u->Faces(i));
			SideWedges.AddItem(*u->Wedges(i));
		}
		else
		{
			NonSideWedges.AddItem(u->Wedges(i));
		}
	}

	for( i=0; i<NonSideWedges.Num(); i++ )
	{
		UniqueNonSideWedges.AddUniqueItem(NonSideWedges(i));		
	}


	// Update wedges per unique non-side wedge.
	for( i=0; i<UniqueNonSideWedges.Num(); i++ )
	{
		// Does it match any of the side wedges - if so,
		// prepare a new wedge for UV-morphing.

		INT MatchedWedgeIdx = INDEX_NONE;
		// MatchedWedgeIdx = SideWedges.FindItemIndex( UniqueNonSideWedges(i)); //NO! we want to compare UV only.	
		// Vertex match is implied.
		for( INT w=0; w< SideWedges.Num(); w++)
		{
			NOTE(debugf(TEXT("matchtry UVunsw %i %i with UVSide %i %i"),UniqueNonSideWedges(i)->U,UniqueNonSideWedges(i)->V,SideWedges(w).U,SideWedges(w).V);)

			if( UniqueNonSideWedges(i)->W == SideWedges(w).W )// == compares only U V
				MatchedWedgeIdx = w;
		}	

		if ( MatchedWedgeIdx == INDEX_NONE ) // We can't morph, therefore make a new wedge in this place.
		{
			// With each wedge's 'originalindex' intact, and the (to be sorted in collapse order)
			// Vertex it points to, the whole collapse history is retrievable.
			// Add copy of current wedge to auuxiliary wedge array.
			TModel.GAuxWedges.AddItem( *UniqueNonSideWedges(i) ); 

			/*
			if ( UniqueNonSideWedges(i)->VertexIndex == v->OriginalIndex ) debugf(TEXT(" DOUBLE COLLAPSE DETECTED:UNSW %i  U %i  V  %i"), UniqueNonSideWedges(i)->VertexIndex,u->OriginalIndex,v->OriginalIndex);
			if ( UniqueNonSideWedges(i)->VertexIndex != u->OriginalIndex ) debugf(TEXT(" VERTEX IDENTITY CRISIS:UNSW %i  U %i  V  %i"), UniqueNonSideWedges(i)->VertexIndex,u->OriginalIndex,v->OriginalIndex);
			*/

			NOTE(debugf(TEXT("Update wedge oix %i from vertex %i to vertex %i"),UniqueNonSideWedges(i)->OriginalIndex,UniqueNonSideWedges(i)->VertexIndex,v->OriginalIndex ));
			NOTE(debugf(TEXT(" Keep UV %i %i  "), UniqueNonSideWedges(i)->U,UniqueNonSideWedges(i)->V );)

			// Stretch wedge to the destination vertex:
			// Construct the new wedge.
			UniqueNonSideWedges(i)->VertexIndex = v->OriginalIndex; 
			
		}
		else // We can morph, therefore copy the destination wedge content to this wedge.
		{
			// Add copy of current wedge to auxiliary wedge array.
			TModel.GAuxWedges.AddItem( *UniqueNonSideWedges(i) );  

			// Find the exact wedge we're collapsing to.
			// It's he wedge of the SideFaces face(MatchedWedgeIdx) that has the same index as vertex V.
			TWedge *DestinationWedge;
			DestinationWedge = SideFaces(MatchedWedgeIdx)->Wedges[ SideFaces(MatchedWedgeIdx)->GetVertexIndex(v) ];

			NOTE(debugf(TEXT("Morphed wedgeoix %i to oix %i , from vertex %i to vertex %i"),UniqueNonSideWedges(i)->OriginalIndex,DestinationWedge->OriginalIndex,UniqueNonSideWedges(i)->VertexIndex,v->OriginalIndex ));
			NOTE(debugf(TEXT("Morph from UV %i %i to %i %i"), UniqueNonSideWedges(i)->U,UniqueNonSideWedges(i)->V,DestinationWedge->U,DestinationWedge->V );)

			// We morphed to another wedge:
			// Construct that wedge (but keep our originalindex.)
			// UniqueNonSideWedges(i)->OriginalIndex = DestinationWedge->OriginalIndex; 
			UniqueNonSideWedges(i)->W			= DestinationWedge->W; 
			UniqueNonSideWedges(i)->VertexIndex = DestinationWedge->VertexIndex; 
		}
	}


	// Make tmp a list of all the neighbors of u.
	TArray <TVertex*> Tmp;

	for( i=0; i<u->Verts.Num(); i++ ) 
	{
		Tmp.AddItem(u->Verts(i));
	}

	// Delete triangles on edge uv:
	for( i=u->Faces.Num()-1; i>=0; i--) 
	{
		if( u->Faces(i)->HasVertex(v) ) 
		{
			delete(u->Faces(i));
		}
	}

	// Update remaining triangles to have v instead of u.
	for( i=u->Faces.Num()-1; i>=0; i-- ) 
	{
		u->Faces(i)->ReplaceVertex(u,v);
	}
	delete u; 


	// Recompute the edge collapse costs for neighboring vertices.
	for( i=0; i<Tmp.Num(); i++ ) 
	{
		ComputeEdgeCostAtVertex( Tmp(i) );
	}
	
	unguard;
}



// Find the edge that if collapsed will affect model the least.
// Returns the second vertex of the edge (collapse candidate) 
// as stored in the first vertex' data.	

TVertex *MinimumCostEdge()
{
	guard(MinimumCostEdge);
	
	TVertex *MinCostVtx = TModel.GVerts(0);

	for( INT i=0; i<TModel.GVerts.Num(); i++) 
	{
		if( TModel.GVerts(i)->MergeCost < MinCostVtx->MergeCost) 
		{
			MinCostVtx = TModel.GVerts(i);
		}
	}
	return MinCostVtx;
	unguard;
}


/////////////////////////////////////////////////////////////////////
//
// Build the Level-Of-Detail collapse sequences.
//
/////////////////////////////////////////////////////////////////////

void UEditorEngine::meshLODProcess( ULodMesh* Mesh,	ULODProcessInfo* LODInfo)
{
	guard(UEditorEngine::meshLODProcess);

	FMemMark Mark(GMem);

	GWarn->Logf( NAME_Log, TEXT("Mesh LOD processing: %s"), Mesh->GetName() );

	// The global temporary collapsible helper mesh is your friend.
	TModel.Reset();

	LODStyle = LODInfo->Style;

	// Build the materials, and vertices-with-UV list.
	// Each unique Wedge has unique UV, unique 3d vertex index, and
	// a unique material index, thought that last one is iplicitly
	// inherited from the Face that points to it.

	TArray<INT> TempMatArray;
	TArray<INT> SpecialCoordVerts;

	Mesh->SpecialFaces.Empty();
	Mesh->Materials.Empty();

	// if No UV data required, erase it here..
	if( LODInfo->NoUVData )
	{
		for ( INT t=0; t< Mesh->Tris.Num(); t++)
		{
			Mesh->Tris(t).Tex[0].U = 0;
			Mesh->Tris(t).Tex[0].V = 0;
			Mesh->Tris(t).Tex[1].U = 0;
			Mesh->Tris(t).Tex[1].V = 0;
			Mesh->Tris(t).Tex[2].U = 0;
			Mesh->Tris(t).Tex[2].V = 0;
		}
	}

	// Duplicate TModel vertices for sampling. Use frame # SampleFrame.
	INT FrameOffset = Mesh->FrameVerts * LODInfo->SampleFrame;
	for( INT t=0; t< Mesh->FrameVerts; t++ )
	{
		new TVertex( Mesh->Verts(t + FrameOffset).Vector(), t, 0 );
	}   


	// Go backwards so we can immediately delete any special-coordinate faces.
	for(INT p=Mesh->Tris.Num()-1; p>=0; p-- )
	{
		// Always exclude 'special coordinates': meaning N triangles with the invisible
		// flag set, and record their vertices.
		// Count number of 'special coordinates' tri's, skip these faces for LOD.

		if( ( Mesh->Tris(p).PolyFlags & PF_Invisible ) ) 
		{
			// Accumulate unique vertices.
			// Store the invisible faces directly with local vertex indices (no wedges.)
			FMeshFace SPCFace;
			SPCFace.iWedge[0] = SpecialCoordVerts.AddUniqueItem(Mesh->Tris(p).iVertex[0]);
			SPCFace.iWedge[1] = SpecialCoordVerts.AddUniqueItem(Mesh->Tris(p).iVertex[1]);
			SPCFace.iWedge[2] = SpecialCoordVerts.AddUniqueItem(Mesh->Tris(p).iVertex[2]);
			Mesh->SpecialFaces.AddItem(SPCFace);

			NOTE( debugf(TEXT(" Weapon orig verts are: %i %i %i tri-index: %i"), Mesh->Tris(p).iVertex[0], Mesh->Tris(p).iVertex[1], Mesh->Tris(p).iVertex[2],p);)

			Mesh->Tris.Remove(p);
			continue;
		}
		// Mark these vertices as being used by a visible triangle.
		TModel.GVerts(Mesh->Tris(p).iVertex[0])->Flag = 0xFFFF;
		TModel.GVerts(Mesh->Tris(p).iVertex[1])->Flag = 0xFFFF;
		TModel.GVerts(Mesh->Tris(p).iVertex[2])->Flag = 0xFFFF;	
	}

	// Number of weapon/special coordinate vertices.
	Mesh->SpecialVerts = SpecialCoordVerts.Num(); 


	// Materials building.
	// A material is any unique combination of a texture and the flags.
	for( p=0; p<Mesh->Tris.Num(); p++ )
	{
		UBOOL Unique = true;
		INT ThisMatIndex = 0;
		FMeshFace NewFace;

		

		// Test for unique materials.
		for( INT m=0; m<Mesh->Materials.Num(); m++ )
		{
			if(  ( Mesh->Materials(m).PolyFlags == Mesh->Tris(p).PolyFlags )
			   &&( Mesh->Materials(m).TextureIndex == Mesh->Tris(p).TextureIndex) )
			{
				ThisMatIndex = m;
				Unique = false;
				break;
			}
		}

		//NOTE( debugf(TEXT("MATERIAL for triangle %i is %i, textureindex %i"),p,m,Mesh->Tris(p).TextureIndex);)

		// Add new material.
		if( Unique )
		{
			ThisMatIndex = Mesh->Materials.Num();
			FMeshMaterial NewStuff;
			NewStuff.PolyFlags    = Mesh->Tris(p).PolyFlags;
			NewStuff.TextureIndex = Mesh->Tris(p).TextureIndex;
			Mesh->Materials.AddItem(NewStuff);
		}

		NOTE( debugf(TEXT("NEW face added, material index: %i, Materialtotal: %i, Polyflags: %i, TextureIndex: %i"),
			    ThisMatIndex,Mesh->Materials.Num(),Mesh->Materials(Mesh->Materials.Num()-1).PolyFlags, Mesh->Materials(Mesh->Materials.Num()-1).TextureIndex );)

		// Add new Face, no vertex indices yet.
		NewFace.MaterialIndex = ThisMatIndex; 

		TModel.FaceLevel.AddItem(0xFFFF);    // The LOD bound.
		Mesh->Faces.AddItem(NewFace);		 // Note the Faces array now precisely mirrors the Tri's.
	}

	// Record Wedges.
	Mesh->Wedges.Empty();
	for( p=0; p<Mesh->Tris.Num(); p++ )
	{
		for( INT t=0; t<3; t++)
		{
			// Accumulate unique textured vertices, and 
			// create a vertex->Wedge remapping array?
			FMeshWedge NewWedge;

			UBOOL Unique = true;
			// Test for unique Wedges.
			for( INT v=0; v<Mesh->Wedges.Num(); v++ )
			{
				if( ( Mesh->Wedges(v).iVertex == Mesh->Tris(p).iVertex[t] )
					&&
					( Mesh->Wedges(v).TexUV.U == Mesh->Tris(p).Tex[t].U ) 
					&&
					( Mesh->Wedges(v).TexUV.V == Mesh->Tris(p).Tex[t].V ) 
					&&
					( TempMatArray(v) == Mesh->Faces(p).MaterialIndex )
					)
				{
					// Now point a Face's iVertex INT o the already existing Wedge.
					Mesh->Faces(p).iWedge[t] = v;
					Unique = false;
					break;					
				}
			}

			if( Unique ) // Add new Wedge: unique UV, Material, and vertex index.
			// Still, should be significantly lower than the number of Faces.
			{
				NewWedge.iVertex   = Mesh->Tris(p).iVertex[t];
				NewWedge.TexUV.U   = Mesh->Tris(p).Tex[t].U;
				NewWedge.TexUV.V   = Mesh->Tris(p).Tex[t].V;

				// New unique textured vertex.
				Mesh->Wedges.AddItem( NewWedge );
				// Temporary associated material for that Wedge
				TempMatArray.AddItem( Mesh->Faces(p).MaterialIndex );

				// now point a Face's iVertex INT o the new Wedge.
				Mesh->Faces(p).iWedge[t] = Mesh->Wedges.Num() - 1;
			}			
		}
	}

	TempMatArray.Empty();


	// If there are special coordinate vertices, delete them but *only*
	// if they're not used by any other triangles.
	if( SpecialCoordVerts.Num() )
	{
		for( INT p=TModel.GVerts.Num()-1; p>=0; p-- )
		{
			if( TModel.GVerts(p)->Flag == 0)  
				TModel.GVerts.Remove(p);
		}

		// Now we'll have to remap our vertex indices in ALL our wedges:
		// build a simple remapper array.

		TArray<INT> VertRemap;
		VertRemap.Add(Mesh->FrameVerts);
		for( INT t=0; t<TModel.GVerts.Num(); t++)
		{
			VertRemap(TModel.GVerts(t)->OriginalIndex) = t;
		}
		for( INT v=0; v<Mesh->Wedges.Num(); v++)
		{
			Mesh->Wedges(v).iVertex = VertRemap(Mesh->Wedges(v).iVertex);	
		}
		VertRemap.Empty();		
	}

	// Full vertex set for LOD now known.
	INT TrueFrameVertNum = TModel.GVerts.Num();
	// Backup original indices.
	for( t=0; t<TModel.GVerts.Num(); t++)
	{
		TModel.GOriginalVertIdx.AddItem(TModel.GVerts(t)->OriginalIndex);
		TModel.GVerts(t)->OriginalIndex = t; // Create permutation using CURRENT index set.
	}

	// From now on we'll use only our TModel.GVerts array, and only at the
	// end we'll remap the vertices in our actual animation frames.

	NOTE(debugf(TEXT("## Tris Verts Faces Wedges Materials Textures FaceLevels:  %i %i %i  %i %i %i  %i"),Mesh->Tris.Num(),Mesh->FrameVerts,Mesh->Faces.Num(),Mesh->Wedges.Num(),Mesh->Materials.Num(),Mesh->Textures.Num(),TModel.FaceLevel.Num());)

	TModel.GWedges.Empty();
	TModel.GFaces.Empty();

	// Fill more of the TModel.
	
	// Wedges
	for( t=0; t<Mesh->Wedges.Num(); t++ )
	{
		TWedge NewWedge;
		// NewWedge.MaterialID
		NewWedge.U = Mesh->Wedges(t).TexUV.U;
		NewWedge.V = Mesh->Wedges(t).TexUV.V;
		NewWedge.VertexIndex = Mesh->Wedges(t).iVertex; 
		NewWedge.DestinationIndex = -1;
		NewWedge.OriginalIndex = t;

		NOTE(debugf(TEXT("# Wedges setup:  Wedge %i  Vertex  %i  Via TModel: %i"),t, NewWedge.VertexIndex, TModel.GVerts(Mesh->Wedges(t).iVertex)->OriginalIndex  ));

		TModel.GWedges.AddItem(NewWedge); 
	}
	

	// Faces, Wedge materials.
	for( t=0; t<Mesh->Faces.Num(); t++)
	{
		TWedge* TV[3];
		for( int j=0; j<3; j++ )
		{
			TV[j] = &TModel.GWedges( Mesh->Faces(t).iWedge[j] );
		}
		new TFace( Mesh->Faces(t).MaterialIndex, TV[0], TV[1], TV[2], t );
	}

	NOTE(debugf(TEXT("# TModel vertices: %i  Triangles: %i Wedges+AuxVerts: %i "),TModel.GVerts.Num(),TModel.GFaces.Num(),TModel.GWedges.Num()+TModel.GAuxWedges.Num()));

	// Copy materials to TModel array.
	for( INT m=0; m<Mesh->Materials.Num(); m++ )
	{
		TModel.Materials.AddItem(Mesh->Materials(m));
	}

	// Precompute all the collapse costs.
	for( INT i=0; i<TModel.GVerts.Num(); i++ ) 
	{
		ComputeEdgeCostAtVertex( TModel.GVerts(i) );
	}

	// The permutation array.
	TArray<INT> Permutation;
	Permutation.AddZeroed(TrueFrameVertNum);
	// The collapse list.
	Mesh->CollapsePointThus.Empty();
	Mesh->CollapsePointThus.AddZeroed(TrueFrameVertNum);

	// Reduce the object down to no triangles.
	while( TModel.GVerts.Num() > 0) // Mesh->SpecialVerts )
	{
		NOTE(debugf(TEXT(" #reducing mesh - vertices: %i  Triangles: %i Wedges: %i "),TModel.GVerts.Num(),TModel.GFaces.Num(),TModel.GWedges.Num()));
		// Get the next vertex to collapse.
		TVertex *mn = MinimumCostEdge();
		// Keep track of this vertex, i.e. the collapse ordering.
		Permutation(mn->OriginalIndex)= TModel.GVerts.Num()-1;
		// Keep track of vertex to which we collapse to.
		Mesh->CollapsePointThus(TModel.GVerts.Num()-1) = (mn->ColTarget)? mn->ColTarget->OriginalIndex:0xFFFF;
		// Collapse this edge.
		Collapse(mn,mn->ColTarget);
	}

	NOTE(debugf(TEXT("##reduced mesh - vertices: %i  Triangles: %i Wedges: %i "),TModel.GVerts.Num(),TModel.GFaces.Num(),TModel.GWedges.Num()+TModel.GAuxWedges.Num()));

	// Debugging
	for ( t=0; t<Mesh->CollapsePointThus.Num(); t++ )
	{
		NOTE( debugf(TEXT(" CollapsePointThus # %5i    %5i "), t, Mesh->CollapsePointThus(t) );)
	}
	for ( t=0; t<Permutation.Num(); t++ )
	{
		NOTE( debugf(TEXT(" Permutation # %5i    %5i "), t, Permutation(t) );)
	}


	//  Vertex collapse list preparation .
	NOTE( debugf(TEXT(" Vertex collapse list preparation - collapsingarray size: %i"),Mesh->CollapsePointThus.Num());)
	NOTE( debugf(TEXT(" Permutation array size: %i"),Permutation.Num());)
	for( i=0; i<Mesh->CollapsePointThus.Num(); i++ ) 
	{
		if( Mesh->CollapsePointThus(i) != 0xFFFF )
		{
			NOTE( debugf(TEXT("CollapseThus  %4i: %4i permuted: %4i"), i, Mesh->CollapsePointThus(i),Permutation(Mesh->CollapsePointThus(i))); )
		}

		Mesh->CollapsePointThus(i) = (Mesh->CollapsePointThus(i)==0xFFFF)? 0 : Permutation(Mesh->CollapsePointThus(i));	
	}


	for (INT v=0; v<Mesh->FrameVerts; v++)
	{
		NOTE(debugf(TEXT(" Vertex: %4i  XYZ: %5i %5i %5i"), v, Mesh->Verts(v).X, Mesh->Verts(v).Y, Mesh->Verts(v).Z );)
	}

	// Move all our extra wedge copies over to the full GWedges list.
	for( INT w=0; w<TModel.GAuxWedges.Num(); w++ )
	{
		TModel.GWedges.AddItem(TModel.GAuxWedges(w));
	}
	TModel.GAuxWedges.Empty();

	// Reorder all vertex indices in the Wedges accordingly, and prepare its internal resort array.
	TWedgeSort* WedgePermutation = New<TWedgeSort>(GMem,TModel.GWedges.Num());

	for( w=0; w<TModel.GWedges.Num(); w++ )
	{
		// Permutate the vertices inside all our wedges.
		TModel.GWedges(w).VertexIndex = Permutation(TModel.GWedges(w).VertexIndex);

		WedgePermutation[w].SortedWedgeIndex = w;
		WedgePermutation[w].Key = TModel.GWedges(w).W + ( TModel.GWedges(w).VertexIndex << 16 );
	}
	// Sort entire Wedges array: by vertex (collapse) index, secondary by original index.
	appQsort( WedgePermutation, TModel.GWedges.Num(), sizeof(WedgePermutation[0]),(QSORT_COMPARE)CompareWedgeKey);


	// Wedge array: how to trace the collapse sequence *trees* and build:
	// - The collapse links list.
	// - A sorted wedge list & update the faces accordingly.
	// Set up a special permutation map needed to remap all Wedge indices inside the faces
	// to only their highest LOD-level wedge. WedgePermutation needed (has OriginalIndices)

	// Build our final Wedge collapse list - !!!!!!!!! use .OriginalIndex....
	// Mesh->CollapseWedgeThus.Add(TModel.GWedges.Num());  // The collapse list
	NOTE( debugf(TEXT("Applying the permutation..") ));

	
	// Apply the sorted WedgePermutation.
	TArray<TWedge>  WorkWedges;
	for( w=0; w<TModel.GWedges.Num(); w++ )
	{
		INT PermIdx = WedgePermutation[w].SortedWedgeIndex;
		WorkWedges.AddItem( TModel.GWedges(PermIdx) );

		NOTE(debugf(TEXT(" WorkWedge: # %4i Wedge UV %3i %3i   Vertex: %4i Origindex: %4i "),w,WorkWedges(w).U,WorkWedges(w).V,WorkWedges(w).VertexIndex,WorkWedges(w).OriginalIndex ));
	}
	TModel.GWedges.Empty();
	
	// Digest duplicate wedges, create the wedge remap list, and the wedge LOD collapse list.
	TArray<INT> WedgeRemapList;
	TArray<INT> WedgeRemapToUnique;
	TArray<TWedge> UniqueWedges;

	WedgeRemapList.Add(WorkWedges.Num()); 

	for ( w=0; w<WorkWedges.Num(); w++)
	{
		// Build the condensed version.
		if ( w==0 )
			UniqueWedges.AddItem(WorkWedges(w));

		if ( w>0 )
		{
			// Only store if UV or 3d Vertex differ.
			if(   ( WorkWedges(w).W != WorkWedges(w-1).W )
				||( WorkWedges(w).VertexIndex != WorkWedges(w-1).VertexIndex ) )
			UniqueWedges.AddItem(WorkWedges(w));
		}

		WedgeRemapToUnique.AddItem( UniqueWedges.Num()-1 ); 

		NOTE(debugf(TEXT(" WorkWedge %4i remaps to unique wedge %4i"), w, UniqueWedges.Num()-1 );)

		// Make sure the unique wedge with highest lod gets used for the WedgeRemapList ???
		WedgeRemapList( WorkWedges(w).OriginalIndex ) = UniqueWedges.Num()-1;

		NOTE(debugf(TEXT(" OriginalIndex of WorkWedge %4i remaps to unique wedge %4i"), WorkWedges(w).OriginalIndex, UniqueWedges.Num()-1 );)
	}

	NOTE(debugf(TEXT("## Wedges after condensation: %4i "),UniqueWedges.Num()));

	TArray<INT> WedgeFlagger;
	WedgeFlagger.Add(WorkWedges.Num());
	for ( w=0; w<WorkWedges.Num(); w++) 
	{
		WedgeFlagger(w)=-1;
	}
	
	//
	// Create the wedge LOD collapse list. Mesh->CollapseWedgeThus...
	//
	Mesh->CollapseWedgeThus.Add(UniqueWedges.Num());
	for ( w=0; w<UniqueWedges.Num(); w++) 
	{
		Mesh->CollapseWedgeThus(w)= w;
	}
	for( w=0; w<WorkWedges.Num(); w++)
	{
		INT WOrig = WorkWedges(w).OriginalIndex;
		INT UIndex = WedgeRemapToUnique(w);
		INT WFOld = WedgeFlagger(WOrig);
		WedgeFlagger(WOrig) = UIndex; // Get current position into Unique.
		// Store Unique (but not highest-LOD) vertex destination for any Wedge.
		if (WFOld != -1) // An original index already encountered: try and make a backlink.
		{
			 NOTE(debugf(TEXT("CCWorkwedge %4i  WRTU(W) %4i  WFOldbacklink:  %4i , OriginalIndex %4i, Old CWT[worigdest] %4i "),w,WedgeRemapToUnique(w), WFOld,WorkWedges(w).OriginalIndex, Mesh->CollapseWedgeThus(UIndex) );)
				 Mesh->CollapseWedgeThus(UIndex) = WFOld;  // put into collapse at current unique pos.		
		}		
	}

	//
	// Move final wedges to the Mesh->Wedges wedge array.
	//
	Mesh->Wedges.Empty();
	Mesh->Wedges.Add(UniqueWedges.Num());
	for( w=0; w<UniqueWedges.Num(); w++ )
	{
		Mesh->Wedges(w).TexUV.U = UniqueWedges(w).U;
		Mesh->Wedges(w).TexUV.V = UniqueWedges(w).V;
		Mesh->Wedges(w).iVertex = UniqueWedges(w).VertexIndex;

		NOTE(debugf(TEXT(" Finalwedge: # %4i Wedge UV %3i %3i   Vertex: %4i Origindex: %4i "),w,UniqueWedges(w).U,UniqueWedges(w).V,UniqueWedges(w).VertexIndex,UniqueWedges(w).OriginalIndex ));  
	}

	for ( w=0; w<UniqueWedges.Num(); w++) 
	{
		 NOTE(debugf(TEXT(" CollapseWedgeThus ( %4i ) = %4i , vertex: %i"), w, Mesh->CollapseWedgeThus(w),UniqueWedges(w).VertexIndex ););
	}

	//
	// Remap wedges inside the Mesh's Tri's.
	//
	for( INT f=(Mesh->Faces.Num()-1); f>=0; f--)
	{
		// Indicates a triangle that didn't get processed because of 2-3 identical verts.
		if( TModel.FaceLevel(f) == 0xFFFF )
		{
			NOTE(debugf(TEXT("%% RRemoving face number %i which has wedges %i %i %i"),f,Mesh->Faces(f).iWedge[0],Mesh->Faces(f).iWedge[1],Mesh->Faces(f).iWedge[2]);)
			Mesh->Faces.Remove(f); 
			TModel.FaceLevel.Remove(f);
			
		}
		else
		{
			Mesh->Faces(f).iWedge[0] = WedgeRemapList( Mesh->Faces(f).iWedge[0]);
			Mesh->Faces(f).iWedge[1] = WedgeRemapList( Mesh->Faces(f).iWedge[1]);
			Mesh->Faces(f).iWedge[2] = WedgeRemapList( Mesh->Faces(f).iWedge[2]);
		}				
	}

	// Copy TModel's Facelevel array over to Mesh
	for( f=0; f<TModel.FaceLevel.Num(); f++)
	{
		Mesh->FaceLevel.AddItem(TModel.FaceLevel(f));
	}

	// test: are FACE collapse FaceLevels set correctly ?
	for( w=0; w< Mesh->Faces.Num() ; w++ )
		NOTE(debugf(TEXT(" Face: %4i  Matx: %i  Wedges: %4i %4i %4i  FaceLevel %4i  Verts: %4i %4i %4i"),w,Mesh->Faces(w).MaterialIndex, Mesh->Faces(w).iWedge[0],Mesh->Faces(w).iWedge[1],Mesh->Faces(w).iWedge[2],Mesh->FaceLevel(w),
		              Mesh->Wedges(Mesh->Faces(w).iWedge[0]).iVertex,Mesh->Wedges(Mesh->Faces(w).iWedge[1]).iVertex,Mesh->Wedges(Mesh->Faces(w).iWedge[2]).iVertex); )

	NOTE(debugf(TEXT("## Tris Verts Faces Wedges Materials Textures FaceLevels:  %i %i %i  %i %i %i  %i"),Mesh->Tris.Num(),Mesh->FrameVerts,Mesh->Faces.Num(),Mesh->Wedges.Num(),Mesh->Materials.Num(),Mesh->Textures.Num(),Mesh->FaceLevel.Num()));

	//
	// Finally : sort vertices throughout all the animation frames,
	// also important for moving the Mesh->SpecialVerts special-coordinate vertices to the start of each frame.
	//
	// ALL our wedge-indices refer to the vertices in the animation that start BEYOND the
	// Mesh->SpecialVerts vertices.
	//
	// Needs to be done both with Permutation and also with the OriginalIndex parts
	// of TModel.GVerts.
	//

	NOTE(debugf(TEXT("## FrameVertNum %i   TrueFrameVertNum %i "),Mesh->FrameVerts, TrueFrameVertNum);)
	
	INT FullFrameVertNum = TrueFrameVertNum + SpecialCoordVerts.Num();

	TArray<INT> FullRemap; 
	FullRemap.Add(FullFrameVertNum);

	guard(Bigremap);

	/*
	for( t=0; t<FullFrameVertNum; t++)
	{
		FullRemap(t) = -t;
	}
	*/

	for( t=0; t<SpecialCoordVerts.Num(); t++)
	{
		FullRemap(t) = SpecialCoordVerts(t); // map special coords to the beginning.
	}

	for( t=0; t<TrueFrameVertNum; t++)
	{
		// Map all regular used verts to their sorted position after the special ones.
		NOTE( debugf(TEXT(" # %5i FR DestIndex %5i  NewIn %5i "),t, SpecialCoordVerts.Num()+Permutation(t),TModel.GOriginalVertIdx(t));)

		FullRemap(SpecialCoordVerts.Num() + Permutation(t)) =  TModel.GOriginalVertIdx(t); 
	}

	
	for( t=0; t<FullFrameVertNum; t++)
	{
		NOTE(debugf(TEXT(" Fullremap index %5i  content %5i "),t, FullRemap(t));)
	}
	
	// Bigass new array.
	TArray<FMeshVert> NewVerts;
	NewVerts.Add( FullFrameVertNum * Mesh->AnimFrames);

	for( f=0; f<Mesh->AnimFrames; f++ )
	{
		NOTE(debugf(TEXT(" fullremap AnimFrames %i "),f);)
		INT FrameStart = f * Mesh->FrameVerts;
		INT FullFrameStart = f * FullFrameVertNum;
		// Copy permutated.
		for( INT v=0; v<FullFrameVertNum; v++ )
		{
			NewVerts(FullFrameStart + v) = Mesh->Verts( FullRemap(v) + FrameStart); // If necessary this duplicates verts also.
		}
	}
	

	NOTE(debugf(TEXT("## FrameVertNum %i   TrueFrameVertNum %i "),Mesh->FrameVerts, TrueFrameVertNum);)

	// Kludgy array exchange - maybe cool to have the =operator: Mesh->Verts = NewVerts ?
	Mesh->Verts.Empty();
	Mesh->Verts.Add(NewVerts.Num());
	ExchangeArray(Mesh->Verts, NewVerts);
	NewVerts.Empty();


	// BUT we now also have to update the indices inside the Tri's.....
	// even though they're not used for the LODMesh rendering any more, they're used
	// for the texture LOD gauging !
	TArray<INT> UnFullRemap; 
	UnFullRemap.Add(Mesh->FrameVerts);
	for( t=0; t<FullFrameVertNum; t++)
	{
		UnFullRemap(FullRemap(t))= t;
	}
	for( t=0; t<Mesh->Tris.Num(); t++)
	{
		Mesh->Tris(t).iVertex[0] = UnFullRemap( Mesh->Tris(t).iVertex[0] );
		Mesh->Tris(t).iVertex[1] = UnFullRemap( Mesh->Tris(t).iVertex[1] );
		Mesh->Tris(t).iVertex[2] = UnFullRemap( Mesh->Tris(t).iVertex[2] );
	}
	// OldFrameVerts is needed in the texture LOD gauging.
	// Mesh->OldFrameVerts = FullFrameVertNum;
	
	Mesh->FrameVerts = FullFrameVertNum;
	Mesh->ModelVerts = TrueFrameVertNum;

	TModel.Reset();
	
	Mark.Pop();
	unguard;


	unguard;
}

/*-----------------------------------------------------------------------------
	Special skeletal skin&bones LOD processing.
	Tasks: 
	Standard mesh LOD sequence determination for the mesh, 
	except specific bone links can now play a part in the level-of-detail bias.
-----------------------------------------------------------------------------*/

// Structure used for sorting wedges.
struct TBoneSort
{
	INT		SortedInfluenceIndex;
	DWORD	Key;
};
QSORT_RETURN CDECL CompareRawBoneKey( const TBoneSort* A, const TBoneSort* B )
{
	return A->Key - B->Key;
}



// #TODO: Translucency order !
QSORT_RETURN CDECL CompareFaces( const VTriangle* A, const VTriangle* B )
{
	if     ( A->MatIndex    > B->MatIndex    ) return 1;
	else if( A->MatIndex    < B->MatIndex    ) return -1;
	else if( A->AuxMatIndex > B->AuxMatIndex ) return 1;
	else if( A->AuxMatIndex < B->AuxMatIndex ) return -1;
	else if( A->WedgeIndex  > A->WedgeIndex  ) return 1;
	else if( A->WedgeIndex  < A->WedgeIndex  ) return -1;
	else if( A->SmoothingGroups > B->SmoothingGroups ) return 1;		
	else if( A->SmoothingGroups < B->SmoothingGroups ) return -1;		
	else                                               return 0;
}

/*

// Skeletal mesh variables: 

  	// Make lazy arrays where useful.
	TArray<FVector>          Points;            // Floating point vectors form our skin vertex 3d points.
	TArray<_WORD>			CollapsePointThus;  // Lod-collapse single-linked list for points.
	TArray<_WORD>           FaceLevel;          // Minimum lod-level indicator for each face.
	TArray<FMeshFace>       Faces;              // Faces 
	TArray<_WORD>			CollapseWedgeThus;  // Lod-collapse single-linked list for the wedges.
	TArray<FMeshWedge>		Wedges;             // 'Hoppe-style' textured vertices.
	TArray<FMeshMaterial>   Materials;          // Materials

	TArray<FMeshBone>       RefSkeleton;        // Reference skeleton

	// Bone weights
	TArray <VBoneInfIndex>  BoneWeightIdx; // ,, ,, contains Index, number of influences per bone (+ N detail level sizers! ..)
	TArray <VBoneInfluence> BoneWeights;   // Each element contians weight and vertex number.

*/


void UEditorEngine::modelLODProcess( USkeletalMesh* Mesh, ULODProcessInfo* LODInfo, USkelImport* RawData)
{
	guard(UEditorEngine::modelLODProcess);

	FMemMark Mark(GMem);

	// Todo: * do we need the memmark/pop stuff ?
	//       * check whether exporter gives us proper wedges and vertices/indices faces etc.
	//       * do mesh->textures need to be filled/ready at this point?
	//
	// * Mesh and bones delivered in raw form in USkelImport.
	//
	// * Bone influence on LOD will be through a few conditional tests in the collapse cost
	//   calculation routines, to keep the whole LOD collapse process
	//   compatible with non-skeletal meshes.
	//
	// * Bone influences need LOD sorting for best results also.
	//
	// We can copy most of the above data but need to
	// change the input from old-style vertices/faces to the new style;
	// From then on it's almost identical - except we ignore the animation frame vertex reordering.	
	//
	// Input Old style: 
	// Tris ( Triangles with UV on each corner )
	// Vertices: sample mesh from certain frame's packed 3d points.
	//
	// Output used for rendering: 
	// (partly depends on what GETFRAME uses:)
	// - all the lod-specific stuff above, minus any (packed) vertices.
	//

	/*
	struct USkelImport 
	{
		TArray <VMaterial>   Materials; // Materials
		TArray <FVector>		 Points;    // 3D Points
		TArray <VVertex>     Wedges;    // Wedges
		TArray <VTriangle>   Faces;     // Faces
		TArray <VBone>       RefBones;      // reference skeleton
		TArray <VRawBoneInfluence> Influences; //
		//TArray <> AnimationKeys           //
	};
	*/

	// The global temporary collapsible helper mesh is your friend.
	TModel.Reset();

	LODStyle = LODInfo->Style;
	debugf(TEXT("Skeletal mesh processing. LODStyle: %i"),LODStyle); 

	// Build the materials, and vertices-with-UV list.
	// Each unique Wedge has unique UV, unique 3d vertex index, and
	// a unique material index, thought that last one is iplicitly
	// inherited from the Face that points to it.

	TArray<INT> TempMatArray;

	Mesh->Materials.Empty();

	// Erase UV data if not required (entirely envmapped things)
	if( LODInfo->NoUVData )
	{
		for ( INT t=0; t< RawData->Wedges.Num(); t++)
		{
			RawData->Wedges(t).U = 0;
			RawData->Wedges(t).V = 0;
		}
	}


	/*
	{for(INT t=0; t< RawData->Faces.Num(); t++ )
	{
			NLOG( debugf(TEXT("Matindex triangle %i is %i "),t, RawData->Faces(t).MatIndex);)
	}}
	{for(INT t=0; t< RawData->Wedges.Num(); t++ )
	{
			NLOG( debugf(TEXT("Wedge %i Matindex %i "),t, RawData->Wedges(t).MatIndex);)
	}}
	*/


	//
	// Initialize TModel vertices for sampling. Uses reference skin pose only.
	// Fills the TModel.GVerts array automatically. Full FVectors.
	//
	for( INT t=0; t< RawData->Points.Num(); t++ )
	{
		new TVertex( RawData->Points(t), t, 0 );
	}

	NLOG( debugf(TEXT("Raw points %i "), RawData->Points.Num() ));

	// Sort raw data mesh faces by material index
	appQsort( &RawData->Faces(0),RawData->Faces.Num(), sizeof(RawData->Faces(0)), (QSORT_COMPARE)CompareFaces );
	
	//
	// Mesh faces from RawDataFaces ->   Mesh->Faces; also size TModel.FaceLevel.AddItem(0xFFFF) !!
	//
	for( t=0; t< RawData->Faces.Num(); t++ )
	{
		FMeshFace NewFace;

		NewFace.iWedge[0] = RawData->Faces(t).WedgeIndex[0];
		NewFace.iWedge[1] = RawData->Faces(t).WedgeIndex[1];
		NewFace.iWedge[2] = RawData->Faces(t).WedgeIndex[2];
		NewFace.MaterialIndex = RawData->Wedges(NewFace.iWedge[0]).MatIndex;

		NLOG(debugf(TEXT("# Making new face nr %i Wedges123 %i %i %i  Matidx %i"),t,NewFace.iWedge[0],NewFace.iWedge[1],NewFace.iWedge[2], NewFace.MaterialIndex ) );

		NOTE( debugf(TEXT("MATERIAL for triangle %i is %i, textureindex %i"),t,NewFace.MaterialIndex);)

		Mesh->Faces.AddItem(NewFace);
		TModel.FaceLevel.AddItem(0xFFFF);
	}   

	//
	//  Mesh Materials
	//
	for( t=0; t< RawData->Materials.Num(); t++)
	{
		FMeshMaterial NewStuff;
	
		DWORD RawFlags = RawData->Materials(t).PolyFlags;

		// Set style based on triangle type. Classic flag conversion as in UnMeshEd.cpp
		DWORD PolyFlags=0;
		if     ( (RawFlags&15)==MTT_Normal         ) PolyFlags |= 0;
		else if( (RawFlags&15)==MTT_NormalTwoSided ) PolyFlags |= PF_TwoSided;
		else if( (RawFlags&15)==MTT_Modulate       ) PolyFlags |= PF_TwoSided | PF_Modulated;
		else if( (RawFlags&15)==MTT_Translucent    ) PolyFlags |= PF_Translucent;
		else if( (RawFlags&15)==MTT_Masked         ) PolyFlags |= PF_Masked;
		else if( (RawFlags&15)==MTT_Placeholder    ) PolyFlags |= PF_TwoSided | PF_Invisible;
		// Handle effects.
		if     ( RawFlags&MTT_Unlit             ) PolyFlags |= PF_Unlit;
		if     ( RawFlags&MTT_Flat              ) PolyFlags |= PF_Flat;
		if     ( RawFlags&MTT_Environment       ) PolyFlags |= PF_Environment;
		if     ( RawFlags&MTT_NoSmooth          ) PolyFlags |= PF_NoSmooth;

		NewStuff.PolyFlags    = PolyFlags; 
		NewStuff.TextureIndex = RawData->Materials(t).TextureIndex;
		NLOG( debugf(TEXT("Raw Material %i texture index %i PolyFlags 0x%15x"),t,RawData->Materials(t).TextureIndex,NewStuff.PolyFlags));
		Mesh->Materials.AddItem(NewStuff);
	}

	NLOG( debugf(TEXT("Raw materials %i "), RawData->Materials.Num() ));

	//
	// Mesh Wedges 
	//
	Mesh->Wedges.Empty();
	for( t=0; t< RawData->Wedges.Num(); t++)
	{
		FMeshWedge NewWedge;
		NewWedge.iVertex = RawData->Wedges(t).PointIndex;
		// Rawdata Wedges also have a material index, but it's redundant.
		NewWedge.TexUV.U = Min( (INT)(256.f * RawData->Wedges(t).U),255);
		NewWedge.TexUV.V = Min( (INT)(256.f * RawData->Wedges(t).V),255);
		Mesh->Wedges.AddItem( NewWedge );
	}

	//
	// Full vertex set for LOD now known.
	//
	INT TrueFrameVertNum = TModel.GVerts.Num();

	// Backup original Point indices = needed to get wedges pointing properly after points
	// are sorted in collapse order.
	for( t=0; t<TModel.GVerts.Num(); t++)
	{
		TModel.GOriginalVertIdx.AddItem(TModel.GVerts(t)->OriginalIndex);
		TModel.GVerts(t)->OriginalIndex = t; // Create permutation using CURRENT index set.
	}

	NLOG(debugf(TEXT("# Tmodel.GVerts number: %i \n"),TModel.GVerts.Num() ));

	//
	// From now on we'll use only our TModel.GVerts array. Copied back at the very end.
	//
	NLOG(debugf(TEXT("## Faces Wedges Materials Textures FaceLevels:  %i %i %i  %i %i %i  %i"),Mesh->Faces.Num(),Mesh->Wedges.Num(),Mesh->Materials.Num(),Mesh->Textures.Num(),TModel.FaceLevel.Num());)

	TModel.GWedges.Empty();
	TModel.GFaces.Empty(); 

	// Fill more of the TModel.
	
	// Wedges
	for( t=0; t<Mesh->Wedges.Num(); t++ )
	{

		TWedge NewWedge;
		// NewWedge.MaterialID
		NewWedge.U = Mesh->Wedges(t).TexUV.U;
		NewWedge.V = Mesh->Wedges(t).TexUV.V;
		NewWedge.VertexIndex = Mesh->Wedges(t).iVertex; 
		NewWedge.DestinationIndex = -1;
		NewWedge.OriginalIndex = t;

		//NLOG(debugf(TEXT("# Wedges setup:  Wedge %i  Vertex  %i  Via TModel: %i"),t, NewWedge.VertexIndex, TModel.GVerts(Mesh->Wedges(t).iVertex)->OriginalIndex  ));
		TModel.GWedges.AddItem(NewWedge); 
	}
	
	// Faces, Wedge materials.
	for( t=0; t<Mesh->Faces.Num(); t++)
	{
		TWedge* TV[3];
		for( int j=0; j<3; j++ )
		{
			TV[j] = &TModel.GWedges( Mesh->Faces(t).iWedge[j] );
  			NLOG(debugf(TEXT("# Making new face nr %i GWedgeidx %i Matidx %i"),t,Mesh->Faces(t).iWedge[j],Mesh->Faces(t).MaterialIndex ) );
		}

		NLOG(debugf(TEXT("# new faces wedge vertidxs: %i %i %i Max vertices: %i"),
			Mesh->Wedges(Mesh->Faces(t).iWedge[0]).iVertex,
			Mesh->Wedges(Mesh->Faces(t).iWedge[1]).iVertex,
			Mesh->Wedges(Mesh->Faces(t).iWedge[2]).iVertex, TModel.GVerts.Num());)

		new TFace( Mesh->Faces(t).MaterialIndex, TV[0], TV[1], TV[2], t );
	}

	NLOG(debugf(TEXT("# TModel vertices: %i  Triangles: %i Wedges+AuxVerts: %i "),TModel.GVerts.Num(),TModel.GFaces.Num(),TModel.GWedges.Num()+TModel.GAuxWedges.Num()));

	// Copy materials to TModel array.
	for( INT m=0; m<Mesh->Materials.Num(); m++ )
	{
		TModel.Materials.AddItem(Mesh->Materials(m));
	}

	// Precompute all the collapse costs.
	for( INT i=0; i<TModel.GVerts.Num(); i++ ) 
	{
		ComputeEdgeCostAtVertex( TModel.GVerts(i) );
	}


	
	// The permutation array.
	TArray<INT> Permutation;
	Permutation.AddZeroed(TrueFrameVertNum);
	// The collapse list.
	Mesh->CollapsePointThus.Empty();
	Mesh->CollapsePointThus.AddZeroed(TrueFrameVertNum);

	// Reduce the object down to no triangles.
	while( TModel.GVerts.Num() > 0) // Mesh->SpecialVerts )
	{
		NLOG(debugf(TEXT(" #reducing mesh - vertices: %i  Triangles: %i Wedges: %i "),TModel.GVerts.Num(),TModel.GFaces.Num(),TModel.GWedges.Num()));
		// Get the next vertex to collapse.
		TVertex *mn = MinimumCostEdge();
		// Keep track of this vertex, i.e. the collapse ordering.
		Permutation(mn->OriginalIndex)= TModel.GVerts.Num()-1;
		// Keep track of vertex to which we collapse to.
		Mesh->CollapsePointThus(TModel.GVerts.Num()-1) = (mn->ColTarget)? mn->ColTarget->OriginalIndex:0xFFFF;
		// Collapse this edge.
		Collapse(mn,mn->ColTarget);
	}

	NLOG(debugf(TEXT("##reduced mesh - vertices: %i  Triangles: %i Wedges: %i "),TModel.GVerts.Num(),TModel.GFaces.Num(),TModel.GWedges.Num()+TModel.GAuxWedges.Num()));

	// Debugging
	for ( t=0; t<Mesh->CollapsePointThus.Num(); t++ )
	{
		NLOG(debugf(TEXT(" CollapsePointThus # %5i    %5i "), t, Mesh->CollapsePointThus(t) );)
	}
	for ( t=0; t<Permutation.Num(); t++ )
	{
		NLOG(debugf(TEXT(" Permutation # %5i    %5i "), t, Permutation(t) );)
	}

	//  Vertex collapse list preparation .
	NLOG(debugf(TEXT(" Vertex collapse list preparation - collapsingarray size: %i"),Mesh->CollapsePointThus.Num());)
	NLOG(debugf(TEXT(" Permutation array size: %i"),Permutation.Num());)
	for( i=0; i<Mesh->CollapsePointThus.Num(); i++ ) 
	{
		if( Mesh->CollapsePointThus(i) != 0xFFFF )
		{
			NLOG(debugf(TEXT("CollapseThus  %4i: %4i permuted: %4i"), i, Mesh->CollapsePointThus(i),Permutation(Mesh->CollapsePointThus(i))); )
		}

		Mesh->CollapsePointThus(i) = (Mesh->CollapsePointThus(i)==0xFFFF)? 0 : Permutation(Mesh->CollapsePointThus(i));	
	}


	// Move all our extra wedge copies over to the full GWedges list.
	for( INT w=0; w<TModel.GAuxWedges.Num(); w++ )
	{
		TModel.GWedges.AddItem(TModel.GAuxWedges(w));
	}
	TModel.GAuxWedges.Empty();

	// Reorder all vertex indices in the Wedges accordingly, and prepare its internal resort array.
	TWedgeSort* WedgePermutation = New<TWedgeSort>(GMem,TModel.GWedges.Num());

	for( w=0; w<TModel.GWedges.Num(); w++ )
	{
		// Permutate the vertices inside all our wedges.
		TModel.GWedges(w).VertexIndex = Permutation(TModel.GWedges(w).VertexIndex);

		WedgePermutation[w].SortedWedgeIndex = w;
		WedgePermutation[w].Key = TModel.GWedges(w).W + ( TModel.GWedges(w).VertexIndex << 16 );
	}
	// Sort entire Wedges array: by vertex (collapse) index, secondary by original index.
	appQsort( WedgePermutation, TModel.GWedges.Num(), sizeof(WedgePermutation[0]),(QSORT_COMPARE)CompareWedgeKey);


	// Wedge array: how to trace the collapse sequence *trees* and build:
	// - The collapse links list.
	// - A sorted wedge list & update the faces accordingly.
	// Set up a special permutation map needed to remap all Wedge indices inside the faces
	// to only their highest LOD-level wedge. WedgePermutation needed (has OriginalIndices)

	// Build our final Wedge collapse list - !!!!!!!!! use .OriginalIndex....
	// Mesh->CollapseWedgeThus.Add(TModel.GWedges.Num());  // The collapse list
	NLOG(debugf(TEXT("Applying the permutation..")) );

	
	// Apply the sorted WedgePermutation.
	TArray<TWedge>  WorkWedges;
	for( w=0; w<TModel.GWedges.Num(); w++ )
	{
		INT PermIdx = WedgePermutation[w].SortedWedgeIndex;
		WorkWedges.AddItem( TModel.GWedges(PermIdx) );

		NLOG(debugf(TEXT(" WorkWedge: # %4i Wedge UV %3i %3i   Vertex: %4i Origindex: %4i "),w,WorkWedges(w).U,WorkWedges(w).V,WorkWedges(w).VertexIndex,WorkWedges(w).OriginalIndex ));
	}
	TModel.GWedges.Empty();
	
	// Digest duplicate wedges, create the wedge remap list, and the wedge LOD collapse list.
	TArray<INT> WedgeRemapList;
	TArray<INT> WedgeRemapToUnique;
	TArray<TWedge> UniqueWedges;

	WedgeRemapList.Add(WorkWedges.Num()); 

	for ( w=0; w<WorkWedges.Num(); w++)
	{
		// Build the condensed version.
		if ( w==0 )
			UniqueWedges.AddItem(WorkWedges(w));

		if ( w>0 )
		{
			// Only store if UV or 3d Vertex differ.
			if(   ( WorkWedges(w).W != WorkWedges(w-1).W )
				||( WorkWedges(w).VertexIndex != WorkWedges(w-1).VertexIndex ) )
			UniqueWedges.AddItem(WorkWedges(w));
		}

		WedgeRemapToUnique.AddItem( UniqueWedges.Num()-1 ); 

		NLOG(debugf(TEXT(" WorkWedge %4i remaps to unique wedge %4i"), w, UniqueWedges.Num()-1 );)

		// Make sure the unique wedge with highest lod gets used for the WedgeRemapList ???
		WedgeRemapList( WorkWedges(w).OriginalIndex ) = UniqueWedges.Num()-1;

		NLOG(debugf(TEXT(" OriginalIndex of WorkWedge %4i remaps to unique wedge %4i"), WorkWedges(w).OriginalIndex, UniqueWedges.Num()-1 );)
	}

	NLOG(debugf(TEXT("## Wedges after condensation: %4i "),UniqueWedges.Num()));

	TArray<INT> WedgeFlagger;
	WedgeFlagger.Add(WorkWedges.Num());
	for ( w=0; w<WorkWedges.Num(); w++) 
	{
		WedgeFlagger(w)=-1;
	}
	
	// Create the wedge LOD collapse list. Mesh->CollapseWedgeThus...
	Mesh->CollapseWedgeThus.Add(UniqueWedges.Num());
	for ( w=0; w<UniqueWedges.Num(); w++) 
	{
		Mesh->CollapseWedgeThus(w)= w;
	}
	for( w=0; w<WorkWedges.Num(); w++)
	{
		INT WOrig = WorkWedges(w).OriginalIndex;
		INT UIndex = WedgeRemapToUnique(w);
		INT WFOld = WedgeFlagger(WOrig);
		WedgeFlagger(WOrig) = UIndex; // Get current position into Unique.
		// Store Unique (but not highest-LOD) vertex destination for any Wedge.
		if (WFOld != -1) // An original index already encountered: try and make a backlink.
		{
			 NLOG(debugf(TEXT("CCWorkwedge %4i  WRTU(W) %4i  WFOldbacklink:  %4i , OriginalIndex %4i, Old CWT[worigdest] %4i "),w,WedgeRemapToUnique(w), WFOld,WorkWedges(w).OriginalIndex, Mesh->CollapseWedgeThus(UIndex) );)
				 Mesh->CollapseWedgeThus(UIndex) = WFOld;  // put into collapse at current unique pos.		
		}		
	}


	
	// Move final wedges to the Mesh->Wedges wedge array.
	Mesh->Wedges.Empty();
	Mesh->Wedges.Add(UniqueWedges.Num());
	for( w=0; w<UniqueWedges.Num(); w++ )
	{
		Mesh->Wedges(w).TexUV.U = UniqueWedges(w).U;
		Mesh->Wedges(w).TexUV.V = UniqueWedges(w).V;
		Mesh->Wedges(w).iVertex = UniqueWedges(w).VertexIndex;

		NLOG(debugf(TEXT(" Finalwedge: # %4i Wedge UV %3i %3i   Vertex: %4i Origindex: %4i "),w,UniqueWedges(w).U,UniqueWedges(w).V,UniqueWedges(w).VertexIndex,UniqueWedges(w).OriginalIndex ));  
	}

	for ( w=0; w<UniqueWedges.Num(); w++) 
	{
		 NLOG(debugf(TEXT(" CollapseWedgeThus ( %4i ) = %4i , vertex: %i"), w, Mesh->CollapseWedgeThus(w),UniqueWedges(w).VertexIndex ););
	}

		
	// Remap wedges inside the Mesh's Tri's.
	for( INT f=(Mesh->Faces.Num()-1); f>=0; f--)
	{
		// Indicates a triangle that didn't get processed because of 2-3 identical verts.
		if( TModel.FaceLevel(f) == 0xFFFF )
		{
			NLOG(debugf(TEXT("%% RRemoving face number %i which has wedges %i %i %i"),f,Mesh->Faces(f).iWedge[0],Mesh->Faces(f).iWedge[1],Mesh->Faces(f).iWedge[2]);)
			Mesh->Faces.Remove(f); 
			TModel.FaceLevel.Remove(f);	
		}
		else
		{
			Mesh->Faces(f).iWedge[0] = WedgeRemapList( Mesh->Faces(f).iWedge[0]);
			Mesh->Faces(f).iWedge[1] = WedgeRemapList( Mesh->Faces(f).iWedge[1]);
			Mesh->Faces(f).iWedge[2] = WedgeRemapList( Mesh->Faces(f).iWedge[2]);
		}				
	}

	// Copy TModel's Facelevel array over to Mesh
	for( f=0; f<TModel.FaceLevel.Num(); f++)
	{
		Mesh->FaceLevel.AddItem(TModel.FaceLevel(f));
	}


	

	// test: are FACE collapse FaceLevels set correctly ?
	for( w=0; w< Mesh->Faces.Num() ; w++ )
		NLOG(debugf(TEXT(" Face: %4i  Matx: %i  Wedges: %4i %4i %4i  FaceLevel %4i  Verts: %4i %4i %4i"),w,Mesh->Faces(w).MaterialIndex, Mesh->Faces(w).iWedge[0],Mesh->Faces(w).iWedge[1],Mesh->Faces(w).iWedge[2],Mesh->FaceLevel(w),
		              Mesh->Wedges(Mesh->Faces(w).iWedge[0]).iVertex,Mesh->Wedges(Mesh->Faces(w).iWedge[1]).iVertex,Mesh->Wedges(Mesh->Faces(w).iWedge[2]).iVertex); )

	NLOG(debugf(TEXT("## Tris Verts Faces Wedges Materials Textures FaceLevels:  %i %i %i  %i %i %i  %i"),Mesh->Tris.Num(),Mesh->FrameVerts,Mesh->Faces.Num(),Mesh->Wedges.Num(),Mesh->Materials.Num(),Mesh->Textures.Num(),Mesh->FaceLevel.Num()));

	// 
	// Finally : reorder vertices ; FullRemap is the inverted Permutation...
	// 

	TArray<INT> FullRemap; 
	FullRemap.Add(TrueFrameVertNum);

	/*
	for( t=0; t<TrueFrameVertNum; t++)
	{
		FullRemap(t) = -t;
	}
	*/



	for( t=0; t<TrueFrameVertNum; t++)
	{
		// Map all regular used verts to their sorted position 
		NLOG(debugf(TEXT(" # %5i FR DestIndex %5i  NewIn %5i "),t, Permutation(t),TModel.GOriginalVertIdx(t));)
		FullRemap(Permutation(t)) =  TModel.GOriginalVertIdx(t); 
	}

	for( t=0; t<TrueFrameVertNum; t++)
	{
		NLOG(debugf(TEXT(" Fullremap index %5i  content %5i "),t, FullRemap(t));)
	}


	NLOG(debugf(TEXT("Start rawdata remap - Number: %i"),RawData->Influences.Num()));

	// Remap  point indices inside raw bone influences also...
	for( t=0; t<RawData->Influences.Num(); t++)
	{
		RawData->Influences(t).PointIndex = Permutation( RawData->Influences(t).PointIndex );
		
	}

	NLOG(debugf(TEXT("End rawdata remap")));

	// Temp new vertex array
	TArray<FVector> NewPoints;

	Mesh->Points.Empty(); 
	Mesh->Points.Add( TrueFrameVertNum );

	guard(1stp);
	// Copy permutated.
	for( INT v=0; v<TrueFrameVertNum; v++ )
	{
		Mesh->Points(v) = RawData->Points( FullRemap(v) ); 
	}
	unguard;

	NLOG(debugf(TEXT("## MeshPoints %i   TrueFrameVertNum %i Rawdatapoints %i "), Mesh->Points.Num(), TrueFrameVertNum, RawData->Points.Num());)

	Mesh->ModelVerts = TrueFrameVertNum;
	Mesh->FrameVerts = TrueFrameVertNum; 

	TModel.Reset();

	//
	// Process/reorder reference bones, bone weights & indices
	//


	// Reorder all vertex indices in the Wedges accordingly, and prepare its internal resort array.
	TBoneSort* BonePermutation = New<TBoneSort>(GMem, RawData->Influences.Num());

	for( w=0; w<RawData->Influences.Num(); w++ )
	{
		//NLOG(
		//debugf(TEXT("## [ %4i ] RawBoneInf BoneIDX %8i PointIDX %8i Weight %9f "),w,RawData->Influences(w).BoneIndex, RawData->Influences(w).PointIndex, RawData->Influences(w).Weight); 
		//)

		BonePermutation[w].SortedInfluenceIndex = w;
		BonePermutation[w].Key = //(DWORD)  RawData->Influences(w).BoneIndex;
			(DWORD)RawData->Influences(w).PointIndex + ((DWORD)RawData->Influences(w).BoneIndex << 16);
	}


	debugf(TEXT("Sorting raw influences by bone index"));
	//
	// Sort the raw influences by bone index.
	//
	appQsort( BonePermutation, RawData->Influences.Num(), sizeof(BonePermutation[0]),(QSORT_COMPARE)CompareRawBoneKey);

	//
	// Apply permutation as we fill Mesh->BoneWeights /~idx
	//
	Mesh->BoneWeightIdx.Empty();
	Mesh->BoneWeightIdx.AddZeroed( RawData->RefBonesBinary.Num() );

	//debugf(TEXT("Apply rawdata influences permutation "));
	//debugf(TEXT("## Sorted BoneWeightIdx-es %i BoneWeights %i "), Mesh->BoneWeightIdx.Num(), Mesh->BoneWeights.Num());

	for( w=0; w<RawData->Influences.Num(); w++ )
	{	
		INT Idx = BonePermutation[w].SortedInfluenceIndex;		
		_WORD ThisBone = _WORD(RawData->Influences( Idx ).BoneIndex ) ; 
	
		// legal index?
		if (ThisBone < Mesh->BoneWeightIdx.Num() )
		{
			// First index for a particular bone. 
			if ( Mesh->BoneWeightIdx(ThisBone).Number == 0 )
			{
				Mesh->BoneWeightIdx(ThisBone).WeightIndex = Mesh->BoneWeights.Num();
			}
			Mesh->BoneWeightIdx(ThisBone).Number++;

			VBoneInfluence NewBW;
			NewBW.PointIndex = RawData->Influences(Idx).PointIndex;
			NewBW.BoneWeight = Min( 65535, (INT) (65535.0f * RawData->Influences(Idx).Weight ) );
			Mesh->BoneWeights.AddItem( NewBW );
			NLOG(debugf(TEXT("## [ %4i ] Sorted Idx %8i BoneIDX %8i PointIDX %8i Weight %9f "),w, Idx, RawData->Influences( Idx ).BoneIndex, RawData->Influences( Idx ).PointIndex, RawData->Influences( Idx ).Weight);)		
		}		
	}

	// Setup skeletal hierarchy + names structure.
	Mesh->RefSkeleton.Add( RawData->RefBonesBinary.Num() );

	// Digest bones to the serializable format.
    for( INT b=0; b<RawData->RefBonesBinary.Num(); b++ )
	{
		FMeshBone& Bone = Mesh->RefSkeleton(b);
		// Bone = Mesh->RefSkeleton(b);
		// GWarn->Logf( NAME_Log, TEXT("1") );
		
		Bone.Flags = 0;
		// FPlace - orientation and position of bone in parent's Trans, or root.
		Bone.BonePos.Place = RawData->RefBonesBinary(b).BonePos.Place;
		if( b == 0 )
			// Correct historical idiosyncrasy.
			Bone.BonePos.Place.Quat = -Bone.BonePos.Place.Quat;
		Bone.BonePos.Length=0;
		Bone.BonePos.XSize=0;
		Bone.BonePos.YSize=0;
		Bone.BonePos.ZSize=0;

		Bone.NumChildren = 0;
		Bone.ParentIndex = RawData->RefBonesBinary(b).ParentIndex;
		if( Bone.ParentIndex != b )
			Mesh->RefSkeleton(Bone.ParentIndex).NumChildren++;

		appTrimSpaces( &RawData->RefBonesBinary(b).Name[0] );
		Bone.Name = FName( appFromAnsi(&RawData->RefBonesBinary(b).Name[0]) );
	}

#if 0
	// Remove dummy roots.
	while( Mesh->RefSkeleton.Num() > 1 
		&& Mesh->BoneWeightIdx(0).Number == 0
		&& Mesh->RefSkeleton(0).NumChildren == 1 
		&& Mesh->RefSkeleton(1).ParentIndex == 0 )
	{
		Mesh->RefSkeleton(1).BonePos.Place = Mesh->RefSkeleton(1).BonePos.Place << Mesh->RefSkeleton(0).BonePos.Place;
		Mesh->RefSkeleton.Remove(0);
		Mesh->BoneWeightIdx.Remove(0);

		for( INT b=1; b<Mesh->RefSkeleton.Num(); b++ )
			Mesh->RefSkeleton(b).ParentIndex--;
	}
#endif

	// Make NumChildren include all descendents.
    for( b=Mesh->RefSkeleton.Num()-1; b>= 0; b-- )
	{
		FMeshBone& Bone = Mesh->RefSkeleton(b);
		if( Bone.ParentIndex != b )
			Mesh->RefSkeleton(Bone.ParentIndex).NumChildren += Bone.NumChildren;
	}

	// Add hierarchy index to each bone and detect max depth.
	Mesh->SkeletalDepth = 0;
	for( b=0; b<Mesh->RefSkeleton.Num(); b++)
	{
		INT Parent = Mesh->RefSkeleton(b).ParentIndex;
		INT Depth = 1;
		Mesh->RefSkeleton(b).Depth = 1;
		if( Parent != b )
		{
			Depth += Mesh->RefSkeleton(Parent).Depth;
		}
		if( Mesh->SkeletalDepth < Depth )
		{
			Mesh->SkeletalDepth = Depth;
		}
		Mesh->RefSkeleton(b).Depth = Depth;
	}
	debugf( TEXT("Bones digested - %i "), Mesh->RefSkeleton.Num() );

	debugf( TEXT("Converting quaternions") );

	// Tree-transform the hierarchy...
	
	TArray <FCoords> SpaceBases( Mesh->RefSkeleton.Num() );
	
	// Trickle down all transformations.
	for( INT s=0; s<SpaceBases.Num(); s++ )
	{
		SpaceBases(s) = FCoords( Mesh->RefSkeleton(s).BonePos.Place );
		INT Parent = Mesh->RefSkeleton(s).ParentIndex;
		if ( Parent != s ) 
			SpaceBases(s) <<= SpaceBases(Parent);
	}

	// Invert the spacebases
	for( s=0; s < SpaceBases.Num(); s++ )
	{
		SpaceBases(s) = SpaceBases(s).Inverse();
	}
	
	// Allocate 'local' FVector points.
	// Initialize the bone-localized 3d points
	Mesh->LocalPoints.Add( Mesh->BoneWeights.Num() );

	// debugf( TEXT("Points into bone spaces. MeshPoints %i  BoneWeights %i SpaceBases %i"), Mesh->Points.Num(), Mesh->BoneWeights.Num(), SpaceBases.Num() );

	// Transform points into their bone space.
	for( w=0; w< Mesh->BoneWeightIdx.Num(); w++)
	{
		INT Index = Mesh->BoneWeightIdx(w).WeightIndex;
		INT Number = Mesh->BoneWeightIdx(w).Number;

		// debugf(TEXT("Index %i Num %i BoneIdx %i"),Index,Number,w);
	
		if ( Number > 0)
		{
			for( b=Index; b<(Index+Number); b++)
			{
				FVector Point3D = Mesh->Points( Mesh->BoneWeights(b).PointIndex );
				Mesh->LocalPoints(b) = Point3D << SpaceBases(w);
			}
		}
	}

	Mark.Pop();
	unguard; //modelLODProcess
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

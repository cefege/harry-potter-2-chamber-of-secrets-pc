/*=============================================================================
	UnMeshRn.cpp: Unreal mesh rendering.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "Render.h"
#include "UnMesh.h"

/*------------------------------------------------------------------------------
	Globals.
------------------------------------------------------------------------------*/

UBOOL               HasSpecialCoords;
FCoords             SpecialCoords;
static FLOAT        UScale, VScale;
static UTexture*    Textures[16];
static FTextureInfo TextureInfo[16];
static FTextureInfo EnvironmentInfo;
static FVector      GUnlitColor;

/*------------------------------------------------------------------------------
	Environment mapping.
------------------------------------------------------------------------------*/

/*static void EnviroMap( FSceneNode* Frame, FTransTexture& P )
{
	FVector T = P.Point.UnsafeNormal().MirrorByVector( P.Normal ).TransformVectorBy( Frame->Uncoords );
	if( T.Z < 0.f )
		T /= T.Size2D();
	else
		T /= appSqrt( T.Size2D() );
	P.U = (T.X+1.0f) * 0.5f * 256.0f * UScale;
	P.V = (T.Y+1.0f) * 0.5f * 256.0f * VScale;
}*/

/*--------------------------------------------------------------------------
	Clippers.
--------------------------------------------------------------------------*/

static FLOAT Dot[32];
static inline INT Clip( FSceneNode* Frame, FTransTexture** Dest, FTransTexture** Src, INT SrcNum )
{
	INT DestNum=0;
	for( INT i=0,j=SrcNum-1; i<SrcNum; j=i++ )
	{
		if( Dot[j]>=0.0 )
		{
			Dest[DestNum++] = Src[j];
		}
		if( Dot[j]*Dot[i]<0.0 )
		{
			FTransTexture* T = Dest[DestNum] = New<FTransTexture>(GMem);
			*T = FTransTexture( *Src[j] + (*Src[i]-*Src[j]) * (Dot[j]/(Dot[j]-Dot[i])) );
			T->Project( Frame );
			DestNum++;
		}
	}
	return DestNum;
}

/*------------------------------------------------------------------------------
	Subsurface rendering.
------------------------------------------------------------------------------*/

// Triangle subdivision table.
static const int CutTable[8][4][3] =
{
	{{0,1,2},{9,9,9},{9,9,9},{9,9,9}},
	{{0,3,2},{2,3,1},{9,9,9},{9,9,9}},
	{{0,1,4},{4,2,0},{9,9,9},{9,9,9}},
	{{0,3,2},{2,3,4},{4,3,1},{9,9,9}},
	{{0,1,5},{5,1,2},{9,9,9},{9,9,9}},
	{{0,3,5},{5,3,1},{1,2,5},{9,9,9}},
	{{0,1,4},{4,2,5},{5,0,4},{9,9,9}},
	{{0,3,5},{3,1,4},{5,4,2},{3,4,5}}
};

#define INDEX	1

struct MeshRn
{
	TArray<FTransTexture*> 
					VertBuffer;
	INT				VertCount;
#if INDEX
	TArray<_WORD>	VertBufferInds;			// Map from mesh-to-HW indices.
	FTransTexture*	BasePt;
	TArray<_WORD>	IndexBuffer;
	INT				IndexCount;
#endif
	FSceneNode*		SaveFrame;
	FSpanBuffer*	SaveSpan;
	FTextureInfo*	SaveTexture;
	DWORD			SavePolyFlags;

	MeshRn()
	{
		appMemzero(this, sizeof(*this));
	}

	void Init( FTransTexture* Base, INT NumPts )
	{
#if INDEX
		BasePt = Base;
		VertBufferInds.Set( NumPts );
		appMemset( VertBufferInds.GetData(), -1, VertBufferInds.Num()*sizeof(_WORD) );
#endif
	}

	void Alloc(FSceneNode* Frame)
	{
		if( !VertBuffer.GetData() )
		{
			INT VBSize = Frame->Viewport->RenDev->MaxVertices();
			VertBuffer.Set( VBSize );
#if INDEX
			IndexBuffer.Set( VBSize*6 );
#endif
		}
	}
	void Flush()
	{
		if( VertCount )
		{
#if INDEX
			SaveFrame->Viewport->RenDev->DrawTriangles
			( 
				SaveFrame, *SaveTexture, 
				VertBuffer.GetData(), VertCount, IndexBuffer.GetData(), IndexCount,
				SavePolyFlags, SaveSpan 
			);
			VertCount = IndexCount = 0;
			appMemset( VertBufferInds.GetData(), -1, VertBufferInds.Num()*sizeof(_WORD) );
#else
			SaveFrame->Viewport->RenDev->DrawTriangles
			( 
				SaveFrame, *SaveTexture, 
				VertBuffer.GetData(), VertCount, NULL, 0,
				SavePolyFlags, SaveSpan 
			);
			VertCount = 0;
#endif
		}
	}
#if INDEX
	inline INT ProcessVert( FTransTexture* P )
	{
		const UPTRINT PointAddress = reinterpret_cast<UPTRINT>(P);
		const UPTRINT BaseAddress = reinterpret_cast<UPTRINT>(BasePt);
		if( PointAddress >= BaseAddress )
		{
			const UPTRINT ByteDistance = PointAddress - BaseAddress;
			if( ByteDistance % sizeof(FTransTexture) == 0 )
			{
				const UPTRINT Distance = ByteDistance / sizeof(FTransTexture);
				if( Distance < static_cast<UPTRINT>(VertBufferInds.Num()) )
				{
					const INT ID = static_cast<INT>(Distance);
					if( VertBufferInds(ID) == _WORD(-1) )
					{
						// Add to VB, and remember index.
						VertBufferInds(ID) = VertCount;
						VertBuffer(VertCount++) = P;
					}
					return VertBufferInds(ID);
				}
			}
		}

		// Add points outside the indexed source array as isolated vertices.
		VertBuffer(VertCount) = P;
		return VertCount++;
	}
#endif
	void DrawPolygon
	(
		FSceneNode*		Frame,
		FSpanBuffer*	Span,
		FTextureInfo*	Texture,
		DWORD			PolyFlags,
		FTransTexture**	Pts,
		INT				NumPts
	)
	{
		if( Frame!=SaveFrame || Span!=SaveSpan || Texture!=SaveTexture || PolyFlags!=SavePolyFlags )
		{
			Flush();
			Alloc(Frame);
			SaveFrame = Frame;
			SaveSpan = Span;
			SaveTexture = Texture;
			SavePolyFlags = PolyFlags;
		}

#if INDEX
		if( VertCount+NumPts > VertBuffer.Num() )
			Flush();

		// Convert to tris, converting vert pointers to indices.
		IndexBuffer(IndexCount++) = ProcessVert( Pts[0] );
		IndexBuffer(IndexCount++) = ProcessVert( Pts[1] );
		IndexBuffer(IndexCount++) = ProcessVert( Pts[2] );

		for_count( t, NumPts-3 )
		{
			IndexBuffer(IndexCount)   = IndexBuffer(IndexCount-3);
			IndexBuffer(IndexCount+1) = IndexBuffer(IndexCount-1);
			IndexBuffer(IndexCount+2) = ProcessVert( Pts[t+3] );
			IndexCount += 3;
		}
#else
		INT NumVerts = (NumPts-2)*3;
		if( VertCount+NumVerts > VertBuffer.Num() )
			Flush();

		// Convert to tris.
		for_count( t, NumPts-2 )
		{
			VertBuffer(VertCount++) = Pts[0];
			VertBuffer(VertCount++) = Pts[t+1];
			VertBuffer(VertCount++) = Pts[t+2];
		}
#endif
	}
};
static MeshRn GMeshRn;

void RenderInit( FTransTexture* Base, INT NumPts )
{
	Clock(GStat.MeshTmapTime);
	GMeshRn.Init( Base, NumPts );
}

void RenderSubsurface
(
	FSceneNode*		Frame,
	FTextureInfo&	Texture,
	FSpanBuffer*	Span,
	FTransTexture**	Pts,
	DWORD			PolyFlags,
	INT				SubCount
)
{
	guard(RenderSubsurface);

	// Handle effects.
	if( PolyFlags & (PF_Environment | PF_Unlit) )
	{
		// Environment mapping.
		if( PolyFlags & PF_Environment )
		{
			for_count( i, 3 )
			{
				FVector T = Pts[i]->Point.UnsafeNormal().MirrorByVector( Pts[i]->Normal ).TransformVectorBy( Frame->Uncoords );
				Pts[i]->Light = FVector( appPow( Max(T.Z, 0.f), 0.25f ) );
				Pts[i]->U = (T.X+1.0f) * 0.5f * 256.0f * UScale;
				Pts[i]->V = (T.Y+1.0f) * 0.5f * 256.0f * VScale;
			}
		}

		// Handle unlit.
		if( PolyFlags & PF_Unlit )
			for( int j=0; j<3; j++ )
				Pts[j]->Light = GUnlitColor;
	}

#ifdef MESH_SUBDIVIDE
	// Handle subdivision.
	if( SubCount<3 && !(PolyFlags & PF_Flat) )
	{
		// Compute side distances.
		INT CutSide[3], Cuts=0;
		FLOAT Alpha[3];
		STAT(clock(GStat.MeshSubTime));
		for( INT i=0,j=2; i<3; j=i++ )
		{
			FLOAT Dist   = FDistSquared(Pts[j]->Point,Pts[i]->Point);
			FLOAT Curvy  = (Pts[j]->Normal ^ Pts[i]->Normal).SizeSquared();
			FLOAT Thresh = 50.0f * Frame->FX * SqrtApprox(Dist * Curvy) / Max(1.f, Pts[j]->Point.Z + Pts[i]->Point.Z);
			Alpha[j]     = Min( Thresh / Square(32.f) - 1.f, 1.f );
			CutSide[j]   = Alpha[j]>0.0;
			Cuts        += (CutSide[j]<<j);
		}
		STAT(unclock(GStat.MeshSubTime));

		// See if it should be subdivided.
		if( Cuts )
		{
			STAT(clock(GStat.MeshSubTime));
			FTransTexture Tmp[3];
			Pts[3]=Tmp+0;
			Pts[4]=Tmp+1;
			Pts[5]=Tmp+2;
			INT i, j;
			for( i=0,j=2; i<3; j=i++ ) if( CutSide[j] )
			{
				// Compute midpoint.
				FTransTexture& MidPt = *Pts[j+3];
				MidPt = (*Pts[j]+*Pts[i])*0.5;

				// Compute midpoint normal.
				MidPt.Normal = Pts[j]->Normal + Pts[i]->Normal;
				MidPt.Normal *= DivSqrtApprox( MidPt.Normal.SizeSquared() );

				// Enviro map it.
				if( PolyFlags & PF_Environment )
				{
					FLOAT U=MidPt.U, V=MidPt.V;
					EnviroMap( Frame, MidPt );
					MidPt.U = U + (MidPt.U - U)*Alpha[j];
					MidPt.V = V + (MidPt.V - V)*Alpha[j];
				}

				// Shade the midpoint.
				MidPt.Light += (GLightManager->Light( MidPt, PolyFlags ) - MidPt.Light) * Alpha[j];

				// Curve the midpoint.
				(FVector&)MidPt
				+=	0.15f
				*	Alpha[j]
				*	(FVector&)MidPt.Normal
				*	SqrtApprox
					(
						(Pts[j]->Point  - Pts[i]->Point ).SizeSquared()
					*	(Pts[i]->Normal ^ Pts[j]->Normal).SizeSquared()
					);

				// Outcode and optionally transform midpoint.
				MidPt.ComputeOutcode( Frame );
				MidPt.Project( Frame );
			}
			FTransTexture* NewPts[6];
			for( i=0; i<4 && CutTable[Cuts][i][0]!=9; i++ )
			{
				for( INT j=0; j<3; j++ )
					NewPts[j] = Pts[CutTable[Cuts][i][j]];
				RenderSubsurface( Frame, Texture, Span, NewPts, PolyFlags, SubCount+1 );
			}
			STAT(unclock(GStat.MeshSubTime));
			return;
		}
	}
#endif

	// If outcoded, skip it.
	INT NumPts=3;
	{
		Clock(GStat.MeshClipTime);
		if( Pts[0]->Flags & Pts[1]->Flags & Pts[2]->Flags )
			return;

		// Backface reject it.
		if( (PolyFlags & PF_TwoSided) && FTriple(Pts[0]->Point,Pts[1]->Point,Pts[2]->Point) <= 0.0 )
		{
			if( !(PolyFlags & PF_TwoSided) )
				return;
			Exchange( Pts[2], Pts[0] );
		}

		// Clip it.
		BYTE AllCodes = Pts[0]->Flags | Pts[1]->Flags | Pts[2]->Flags;
		if( AllCodes )
		{
			if( AllCodes & FVF_OutXMin )
			{
				static FTransTexture* LocalPts[8];
				for( INT i=0; i<NumPts; i++ )
					Dot[i] = Frame->PrjXM * Pts[i]->Point.Z + Pts[i]->Point.X;
				NumPts = Clip( Frame, LocalPts, Pts, NumPts );
				if( NumPts==0 ) return;
				Pts = LocalPts;
			}
			if( AllCodes & FVF_OutXMax )
			{
				static FTransTexture* LocalPts[8];
				for( INT i=0; i<NumPts; i++ )
					Dot[i] = Frame->PrjXP * Pts[i]->Point.Z - Pts[i]->Point.X;
				NumPts = Clip( Frame, LocalPts, Pts, NumPts );
				if( NumPts==0 ) return;
				Pts = LocalPts;
			}
			if( AllCodes & FVF_OutYMin )
			{
				static FTransTexture* LocalPts[8];
				for( INT i=0; i<NumPts; i++ )
					Dot[i] = Frame->PrjYM * Pts[i]->Point.Z + Pts[i]->Point.Y;
				NumPts = Clip( Frame, LocalPts, Pts, NumPts );
				if( NumPts==0 ) return;
				Pts = LocalPts;
			}
			if( AllCodes & FVF_OutYMax )
			{
				static FTransTexture* LocalPts[8];
				for( INT i=0; i<NumPts; i++ )
					Dot[i] = Frame->PrjYP * Pts[i]->Point.Z - Pts[i]->Point.Y;
				NumPts = Clip( Frame, LocalPts, Pts, NumPts );
				if( NumPts==0 ) return;
				Pts = LocalPts;
			}
		}
		if( Frame->NearClip.W != 0.0 )
		{
			UBOOL Clipped=0;
			for( INT i=0; i<NumPts; i++ )
			{
				Dot[i] = Frame->NearClip.PlaneDot(Pts[i]->Point);
				Clipped |= (Dot[i]<0.0);
			}
			if( Clipped )
			{
				static FTransTexture* LocalPts[8];
				NumPts = Clip( Frame, LocalPts, Pts, NumPts );
				if( NumPts==0 ) return;
				Pts = LocalPts;
			}
		}
		for( INT i=0; i<NumPts; i++ )
		{
			ClipFloatFromZero(Pts[i]->ScreenX, Frame->FX);
			ClipFloatFromZero(Pts[i]->ScreenY, Frame->FY);
		}
	}

	// Render it.
	Clock(GStat.MeshTmapTime);
	GMeshRn.DrawPolygon( Frame, Span, &Texture, PolyFlags, Pts, NumPts );

	unguard;
}

void RenderFlush()
{
	Clock(GStat.MeshTmapTime);
	GMeshRn.Flush();
}

/*------------------------------------------------------------------------------
	High level mesh rendering.
------------------------------------------------------------------------------*/

//
// Structure used by DrawMesh for sorting triangles.
//
struct FMeshTriSort
{
	FMeshTri* Tri;
	INT Key;
};
INT Compare( const FMeshTriSort& A, const FMeshTriSort& B )
{
	return B.Key - A.Key;
}
INT Compare( const FTransform* A, const FTransform* B )
{
	return appRound(B->Point.Z - A->Point.Z);
}

/*------------------------------------------------------------------------------
	Normal mesh drawing.
------------------------------------------------------------------------------*/

//
// Draw a mesh map.
//
void URender::DrawMesh
(
	FSceneNode*		Frame,
	FDynamicSprite*	Sprite,
	AActor*			Owner,
	const FCoords&	Coords, 
	DWORD			ExtraFlags
)
{
#if 0
	if( !Engine->Client->CurvedSurfaces )
		ExtraFlags |= PF_Flat;
#else
	ExtraFlags |= PF_Flat; /* Currently disabled curved surfaces, too much performance drain */
#endif

	// Draw a mesh with level-of-detail triangle morphing. No 3DNow support yet.
	if( Owner->Mesh->IsA(ULodMesh::StaticClass()))
	{
		DrawLodMesh
		(
			Frame,
			Sprite,
			Owner,
			Coords,
			ExtraFlags
		);
		return;
	}

	guard(URender::DrawMesh);
	STAT(clock(GStat.MeshTime));
	FMemMark Mark(GMem);
	UMesh*  Mesh = Owner->Mesh;
	FVector Hack = FVector(0,-8,0);
	UBOOL NotWeaponHeuristic=(Owner->Owner!=Frame->Viewport->Actor);
	if( !Engine->Client->CurvedSurfaces )
		ExtraFlags |= PF_Flat;

	// Get transformed verts.
	FTransTexture* Samples=NULL;
	UBOOL bWire=0;
	guardSlow(Transform);
	STAT(clock(GStat.MeshGetFrameTime));
	Samples = New<FTransTexture>(GMem,Mesh->FrameVerts);
	bWire = Frame->Viewport->IsOrtho() || Frame->Viewport->Actor->RendMap==REN_Wire;
	Mesh->GetFrame( &Samples->Point, sizeof(Samples[0]), bWire ? GMath.UnitCoords : Coords, Owner );
	STAT(unclock(GStat.MeshGetFrameTime));
	unguardSlow;

	// Compute outcodes.
	BYTE Outcode = FVF_OutReject;
	guardSlow(Outcode);
	for( INT i=0; i<Mesh->FrameVerts; i++ )
	{
		Samples[i].Light.X = -1;
		Samples[i].ComputeOutcode( Frame );
		Outcode &= Samples[i].Flags;
	}
	STAT(GStat.MeshVertCount += Mesh->FrameVerts);
	STAT(GStat.MeshSubCount += Mesh->FrameVerts);
	unguardSlow;

	// Render a wireframe view or textured view.
	if( bWire )
	{
		// Render each wireframe triangle.
		guardSlow(RenderWire);
		FPlane Color = Owner->bSelected ? FPlane(.2f,.8f,.1f,0) : FPlane(.6f,.4f,.1f,0);
		for( INT i=0; i<Mesh->Tris.Num(); i++ )
		{
			FMeshTri& Tri    = Mesh->Tris(i);
			FVector*  P1     = &Samples[Tri.iVertex[2]].Point;
			for( int j=0; j<3; j++ )
			{
				FVector* P2 = &Samples[Tri.iVertex[j]].Point;
				if( (Tri.PolyFlags & PF_TwoSided) || P1->X>=P2->X  )
					Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LINE_DepthCued, *P1, *P2 );
				P1 = P2;
			}
		}
		STAT(unclock(GStat.MeshTime));
		Mark.Pop();
		unguardSlow;
		return;
	}

	// Coloring.
	FLOAT Unlit  = Clamp( Owner->ScaleGlow*0.5f + Owner->AmbientGlow/256.f, 0.f, 1.f );
	GUnlitColor  = FVector( Unlit, Unlit, Unlit );
	if( GIsEditor && (ExtraFlags & PF_Selected) )
		GUnlitColor = GUnlitColor*0.4f + FVector(0.0f,0.6f,0.0f);

	// Set up triangles.
	INT VisibleTriangles = 0;
	HasSpecialCoords = 0;
	FMeshTriSort* TriPool=NULL;
	FVector* TriNormals=NULL;
	if( Outcode == 0 )
	{
		// Process triangles.
		guardSlow(Process);
		TriPool    = New<FMeshTriSort>(GMem,Mesh->Tris.Num());
		TriNormals = New<FVector>(GMem,Mesh->Tris.Num());

		// Set up list for triangle sorting, adding all possibly visible triangles.
		STAT(clock(GStat.MeshProcessTime));
		FMeshTriSort* TriTop = &TriPool[0];
		for( INT i=0; i<Mesh->Tris.Num(); i++ )
		{
			FMeshTri*   Tri = &Mesh->Tris(i);
			FTransform& V1  = Samples[Tri->iVertex[0]];
			FTransform& V2  = Samples[Tri->iVertex[1]];
			FTransform& V3  = Samples[Tri->iVertex[2]];
			DWORD PolyFlags = ExtraFlags | Tri->PolyFlags;

			// Compute triangle normal.
			TriNormals[i] = (V1.Point-V2.Point) ^ (V3.Point-V1.Point);
			TriNormals[i] *= DivSqrtApprox(TriNormals[i].SizeSquared()+0.001f);

			// See if potentially visible.
			if( !(V1.Flags & V2.Flags & V3.Flags) )
			{
				if
				(	(PolyFlags & (PF_TwoSided|PF_Flat|PF_Invisible))!=(PF_Flat)
				||	Frame->Mirror*(V1.Point|TriNormals[i])<0.0 )
				{
					// This is visible.
					TriTop->Tri = Tri;

					// Set the sort key.
					TriTop->Key
					=	NotWeaponHeuristic
					?	appRound( V1.Point.Z + V2.Point.Z + V3.Point.Z )
					:	appRound( FDistSquared(V1.Point,Hack)*FDistSquared(V2.Point,Hack)*FDistSquared(V3.Point,Hack) );

					// Add to list.
					VisibleTriangles++;
					TriTop++;
				}
			}
		}
		STAT(unclock(GStat.MeshProcessTime));
		unguardSlow;
	}

	// Render triangles.
	if( VisibleTriangles>0 )
	{
		guardSlow(Render);

		// Fatness.
		UBOOL Fatten   = Owner->Fatness!=128;
		FLOAT Fatness  = (Owner->Fatness/16.0f)-8.0f;
		//FLOAT Detail   = Owner->DrawScale * GlobalMeshLOD / (0.5f * Frame->RProj.Z * Max(1.0f,Owner->Location.TransformPointBy(Coords).Z));

		// Sort by depth.
		if( Frame->Viewport->RenDev->SpanBased )
			Sort( TriPool, VisibleTriangles );

		// Lock the textures.
		UTexture* EnvironmentMap = NULL;
		guardSlow(Lock);
		check(Mesh->Textures.Num()<=ARRAY_COUNT(TextureInfo));
		// INT TexLOD = Max( Engine->Client->TextureLODSet[LODSET_Skin],
		INT TexLOD = Max( Engine->Client->TextureLODSet[LODSET_World]-1,
						  Frame->Viewport->RenDev->RecommendedLOD );
		for( INT i=0; i<Mesh->Textures.Num(); i++ )
		{
			Textures[i] = Mesh->GetTexture( i, Owner );
			if( Textures[i] )
			{
				Clock(GStat.MeshTmapTime);
				Textures[i] = Textures[i]->Get( Frame->Viewport->CurrentTime );
				INT ThisLOD = Min( TexLOD, Engine->Client->TextureLODSet[Textures[i]->LODSet] );
				Textures[i]->Lock( TextureInfo[i], Frame->Viewport->CurrentTime, ThisLOD, Frame->Viewport->RenDev );
				EnvironmentMap = Textures[i];
			}
		}
		if( Owner->Texture )
			EnvironmentMap = Owner->Texture;
		else if( Owner->Region.Zone && Owner->Region.Zone->EnvironmentMap )
			EnvironmentMap = Owner->Region.Zone->EnvironmentMap;
		else if( Owner->Level->EnvironmentMap )
			EnvironmentMap = Owner->Level->EnvironmentMap;
		if( EnvironmentMap==NULL )
			return; //!!temporary work around for screwup
		check(EnvironmentMap);
		Clock(GStat.MeshTmapTime);
		EnvironmentMap->Lock( EnvironmentInfo, Frame->Viewport->CurrentTime, -1, Frame->Viewport->RenDev );
		unguardSlow;

		// Build list of all incident lights on the mesh.
		STAT(clock(GStat.MeshLightSetupTime));
		ExtraFlags |= GLightManager->SetupForActor( Frame, Owner, Sprite->LeafLights, Sprite->Volumetrics );
		STAT(unclock(GStat.MeshLightSetupTime));

		// Perform all vertex lighting.
		guardSlow(Light);
		for( INT i=0; i<VisibleTriangles; i++ )
		{
			FMeshTri& Tri = *TriPool[i].Tri;
			for( INT j=0; j<3; j++ )
			{
				INT iVert = Tri.iVertex[j];
				FTransSample& Vert = Samples[iVert];
				if( Vert.Light.X == -1 )
				{
					// Compute vertex normal.
					FVector Norm(0,0,0);
					FMeshVertConnect& Connect = Mesh->Connects(iVert);
					for( INT k=0; k<Connect.NumVertTriangles; k++ )
						Norm += TriNormals[Mesh->VertLinks(Connect.TriangleListOffset + k)];
					Vert.Normal = FPlane( Vert.Point, Norm * DivSqrtApprox(Norm.SizeSquared()) );

					// Fatten it if desired.
					if( Fatten )
					{
						Vert.Point += Vert.Normal * Fatness;
						Vert.ComputeOutcode( Frame );
					}

					// Compute effect of each lightsource on this vertex.
					GLightManager->LightAndFog( Vert, ExtraFlags );

					// Project it.
					if( !Vert.Flags )
						Vert.Project( Frame );
				}
			}
		}
		unguardSlow;

		// Draw the triangles.
		guardSlow(DrawVisible);
		STAT(GStat.MeshPolyCount+=VisibleTriangles);
		RenderInit( Samples, Mesh->FrameVerts );
		for( INT i=0; i<VisibleTriangles; i++ )
		{
			// Set up the triangle.
			FMeshTri& Tri = *TriPool[i].Tri;
			if( !(Tri.PolyFlags & PF_Invisible) )
			{
				// Get texture.
				DWORD PolyFlags = Tri.PolyFlags | ExtraFlags;
				INT Index = TriPool[i].Tri->TextureIndex;
				FTextureInfo& Info = (Textures[Index] && !(PolyFlags & PF_Environment)) ? TextureInfo[Index] : EnvironmentInfo;
				UScale = Info.UScale * Info.USize / 256.0f;
				VScale = Info.VScale * Info.VSize / 256.0f;

				// Set up texture coords.
				FTransTexture* Pts[6];
				for( INT j=0; j<3; j++ )
				{
					Pts[j]    = &Samples[Tri.iVertex[j]];
					Pts[j]->U = Tri.Tex[j].U * UScale;
					Pts[j]->V = Tri.Tex[j].V * VScale;
				}
				if( Frame->Mirror == -1 )
					Exchange( Pts[2], Pts[0] );
				RenderSubsurface( Frame, Info, Sprite->SpanBuffer, Pts, PolyFlags, 0 );
			}
			else
			{
				// Remember coordinate system.
				FCoords C;
				C.Origin      = FVector(0,0,0);
				C.XAxis	      = (Samples[Tri.iVertex[1]].Point - Samples[Tri.iVertex[0]].Point).SafeNormal();
				C.YAxis	      = (C.XAxis ^ (Samples[Tri.iVertex[0]].Point - Samples[Tri.iVertex[2]].Point)).SafeNormal();
				C.ZAxis	      = C.YAxis ^ C.XAxis;
				FVector Mid   = 0.5*(Samples[Tri.iVertex[0]].Point + Samples[Tri.iVertex[2]].Point);
				SpecialCoords = GMath.UnitCoords * Mid * C;
				HasSpecialCoords = 1;
			}
		}
		RenderFlush();
		GLightManager->FinishActor();
		unguardSlow;

		Clock(GStat.MeshTmapTime);
		for( INT i=0; i<Mesh->Textures.Num(); i++ )
			if( Textures[i] )
				Textures[i]->Unlock( TextureInfo[i] );
		EnvironmentMap->Unlock( EnvironmentInfo );		
		unguardSlow;
	}

	STAT(GStat.MeshCount++);
	STAT(unclock(GStat.MeshTime));
	Mark.Pop();
	unguardf(( TEXT("(%s)"), Owner->Mesh->GetFullName() ));
}

/*------------------------------------------------------------------------------
	LOD mesh code.
------------------------------------------------------------------------------*/

#include "UnMeshRnLOD.h"

/*------------------------------------------------------------------------------
	The End.
------------------------------------------------------------------------------*/

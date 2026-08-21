/*=============================================================================
	UnMeshRnLOD.cpp: Unreal mesh LOD rendering.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Erik de Neve

	Note:
		* Included by UnMeshRn.cpp.
=============================================================================*/

#include "UnSkeletalMesh.h"

//
// Structure used by DrawLodMesh for sorting triangles.
//
struct FMeshFaceSort
{
	FMeshFace* Face;
	INT Key;
};
inline INT Compare( const FMeshFaceSort& A, const FMeshFaceSort& B )
{
	return B.Key - A.Key;
}

//
// Draw a mesh map with level-of-detail support.
//
void URender::DrawLodMesh
(
	FSceneNode*		Frame,
	FDynamicSprite*	Sprite,
	AActor*			Owner,
	const FCoords&	Coords, 
	DWORD			ExtraFlags
)
{
	guard(URender::DrawLodMesh);

	INT DrawTime; 
	STAT(DrawTime = GStat.MeshTime;)
	STAT(clock(GStat.MeshTime));
	FMemMark Mark(GMem);
	ExtraFlags |= PF_Flat; /* LOD doesn't support curved surfaces (yet) */
	ULodMesh*  Mesh = (ULodMesh*)Owner->Mesh;
	FVector Hack = FVector(0.f,-8.f,0.f);
	UBOOL SoftwareRendering =  Frame->Viewport->RenDev->SpanBased;
	UBOOL NotWeaponHeuristic= (Owner->Owner!=Frame->Viewport->Actor);


	//
	// ShapeLODMode: LOD drawing mode.
	//
	// 0    Draw everything at full (or at ShapeLODFix) detail.
	// 1    Normal level-of-detail mode
	// 2    Disable smooth morphing.
	// 3    Ignore the field-of-view (great for debugging)
	// 4    Disable smooth morphing, ignore field-of-view.
	// 

	// Skip LOD strength calculation if at all possible.
	FLOAT TargetSubset = Mesh->ModelVerts;
	INT   VertexSubset = Mesh->ModelVerts;

	UBOOL DoLOD =( ( *(DWORD*)&Mesh->LODStrength != 0 ) && (Mesh->CollapsePointThus.Num() != 0) );

	if( DoLOD )
	{
		FLOAT Bias = Owner->LODBias  / (Mesh->LODStrength * GlobalShapeLOD * GlobalShapeLODAdjust);
		FLOAT Pix = (Sprite->Y2+1 - Sprite->Y1) * (Sprite->X2+1 - Sprite->X1);

		// Geometric mean of vertex-based and fraction-based LOD.
		const FLOAT VERTS_PER_PIX2 = 0.04f;		// 1 vert/25 pixels.
		const FLOAT LOD_PER_FRAC = 4.f;			// Full LOD at 1/4 screen width.
		FLOAT LODPix = Pix*VERTS_PER_PIX2/Mesh->ModelVerts,
			  LODFrac = appSqrt(Pix)/Frame->FX*LOD_PER_FRAC;
		FLOAT LOD = Bias * Max( LODPix, LODFrac );
		TargetSubset = Mesh->LODMinVerts + (Mesh->ModelVerts - Mesh->LODMinVerts) * LOD;

		// Command line debugging variables.
		if ( ShapeLODMode == 0 )
		{
			if( ShapeLODFix != 0.f)
				TargetSubset = ShapeLODFix * Mesh->ModelVerts;
			else
				TargetSubset = Mesh->ModelVerts;
		}
		VertexSubset    = Min( appRound(TargetSubset), Mesh->ModelVerts );
		if( VertexSubset >= Mesh->ModelVerts ) 
			DoLOD = false; 
	}

	// Get transformed verts.
	STAT(GStat.MeshVertCount += Mesh->ModelVerts);
	STAT(GStat.MeshSubCount += VertexSubset);
	UBOOL bWire = WireShow || Frame->Viewport->IsOrtho() || Frame->Viewport->Actor->RendMap==REN_Wire;
	UBOOL bSkeletal = Mesh->IsA(USkeletalMesh::StaticClass());

	// Special features when drawing wireframe skeletal meshes.
	if( bSkeletal && bWire)
	{
		((USkeletalMesh*)Mesh)->DisplayBones = BoneShow;
		((USkeletalMesh*)Mesh)->DisplayInfluences = BlendShow; 
	}

	// Allocate the necessary amount of our vertex lighting/transforming/texturing structures.
	FTransTexture* AllSamples;	
	AllSamples = New<FTransTexture>(GMem, VertexSubset + Mesh->SpecialVerts); 
	// The real samples start after the special-coordinate ones.
	FTransTexture* Samples = &AllSamples[Mesh->SpecialVerts]; 

	STAT(clock(GStat.MeshGetFrameTime));
	// On top of a possibly changed VertexSubSet, we always get Mesh->SpecialVerts extra vertices returned.
	Mesh->GetFrame( &AllSamples->Point, sizeof(AllSamples[0]), bWire ? GMath.UnitCoords : Coords, Owner, VertexSubset );
	STAT(unclock(GStat.MeshGetFrameTime));

#ifdef LOD_MORPH
	// Smooth morphing.
	bool DoMorph = ( (ShapeLODMode & 1) && (Mesh->LODMorph > 0.f) );		
	INT NonMorphSubset = 0;
	if ( DoMorph ) 
	{
		NonMorphSubset = appRound( TargetSubset * (1.f - Mesh->LODMorph) ); 

		if( NonMorphSubset >= VertexSubset )
		{
			DoMorph = false;			
		}
		else
		{
			FLOAT InvLODMorph = 1.f/( Mesh->LODMorph * TargetSubset );
			// prepare an upper subset for morphing.
			for ( INT i=VertexSubset-1; i>NonMorphSubset; i-- ) 
			{
				// Vertices morph according to their closeness to collapse.
				FLOAT Alpha = Min( 1.f, (FLOAT)(i-NonMorphSubset) * InvLODMorph );
				// Only go down once. 
				INT Morph2 = Mesh->CollapsePointThus(i);
				if ( Morph2 > 0 )
				{
					// because we count DOWN, morphed samples don't hinder each other.
					Samples[i].Point += ( Samples[Morph2].Point - Samples[i].Point ) * Alpha;			
				}
				else
				{
					Alpha = 0.f;
				}			
				// Stored in U before we need it later as texture coordinate.
				Samples[i].U = Alpha;
			}
		}
	}
#endif

	// Compute outcodes.
	// If resulting Outcode & FVF_OutReject == 0  then entire mesh is out of view.
	DWORD MeshOutcode = FVF_OutReject;
	DWORD WeaponOutcode = FVF_OutReject;
	INT i;
	for( i=0; i<Mesh->SpecialVerts; i++ )
	{
		AllSamples[i].Normal = FPlane(0.f,0.f,0.f,0.f);
		AllSamples[i].ComputeOutcode( Frame );
		WeaponOutcode &= AllSamples[i].Flags;
	}
	for( i=0; i<VertexSubset; i++ )
	{
		Samples[i].Light.X = 0;  // Indicate unprocessed vertex.
		Samples[i].Normal = FPlane(0.f,0.f,0.f,0.f);
		Samples[i].ComputeOutcode( Frame );
		MeshOutcode &= Samples[i].Flags;
	}

	// Special coordinates setup.
	HasSpecialCoords = 0;

	
	if ( WeaponOutcode == 0 ) // ( Mesh->SpecialFaces.Num() )
	{
		// Only the first SpecialFace is used - for now.
		FMeshFace& Face = Mesh->SpecialFaces(0);  
		FTransform& V0  = AllSamples[Face.iWedge[0]];
		FTransform& V1  = AllSamples[Face.iWedge[1]];
		FTransform& V2  = AllSamples[Face.iWedge[2]];

		// See if potentially visible. Warning: potential flickering - the weapon itself 
		// might be visible depending on its size, even while these 3 vertices aren't ?
		if ( !(V0.Flags & V1.Flags & V2.Flags) ) 
		{
			FCoords C;
			C.Origin      = FVector(0.f,0.f,0.f);
			C.XAxis	      = (V1.Point - V0.Point).SafeNormal();
			C.YAxis	      = (C.XAxis ^ (V0.Point - V2.Point)).SafeNormal();
			C.ZAxis	      = C.YAxis ^ C.XAxis;
			FVector Mid   = 0.5f*(V0.Point + V2.Point);
			SpecialCoords = GMath.UnitCoords * Mid * C;
			HasSpecialCoords = 1;
		}
	}

	guardSlow(RenderBones);
	// Render bones for debugging
	if( bWire &&  bSkeletal && BoneShow ) 
	{
		guardSlow(DrawBones);
		INT BoneNum=((USkeletalMesh*)Mesh)->DebugPivots.Num(); 
		// debugf(TEXT("Number of bones in wireframe drawing: %i "),BoneNum);

		if( BoneNum > 1 )
		{
			FVector* Pivots  = &((USkeletalMesh*)Mesh)->DebugPivots(0);
			INT* ParentIndex = &((USkeletalMesh*)Mesh)->DebugParents(0);

			// Draw bones
			for( INT b=0; b<BoneNum; b++)
			{
				FPlane Color = Owner->bSelected ? FPlane( .3f,1.2f,.3f,0.f ) : FPlane( 2.f,2.f,2.f, 0.f );
				// Color bones randomly 
				if( 1 )
				{
					FLOAT RColor1 = 2.f* GRandoms->Random(b*3)+0.2f;
					FLOAT RColor2 = 2.f* GRandoms->Random(b*3+1)+0.2f;
					FLOAT RColor3 = 2.f* GRandoms->Random(b*3+2)+0.2f;
					Color = FPlane(RColor1,RColor2,RColor3,0.f);
				}

				// from each child to its parent.
				if( ParentIndex[b] != b )
				{
					FVector B1 = Pivots[b];
					FVector B2 = Pivots[ParentIndex[b]];
					Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LINE_None, B1, B2 );	
				}			
			}
		}
		
		unguardSlow;	
	}
	unguardSlow;


	guardSlow(Wire);
	// Render a wireframe view, and exit.
	if ( bWire )
	{
		guardSlow(Wireframe);
		// Render each wireframe triangle.
		FPlane Color = Owner->bSelected ? FPlane(0.2f,0.8f,0.1f,0.f) : FPlane(0.6f,0.4f,0.1f,0.f);


		INT i;
		for( i=0; i<Mesh->Faces.Num(); i++ )
		{			
			// Draw only if face's FaceLevel indicates it falls within our vertex budget.
			if( Mesh->FaceLevel(i) <= VertexSubset ) 
			{			
				FMeshFace& Face = Mesh->Faces(i);  
				INT LVert[3]; 
				for( INT v=0; v<3; v++ )
				{
					INT WedgeIndex = Face.iWedge[v];
					INT LODVertIndex =  Mesh->Wedges(WedgeIndex).iVertex;

					// Go down LOD wedge collapse list until below the current LOD vertex count.				
					while( LODVertIndex >= VertexSubset ) // + Mesh->SpecialVerts ??
					{
						WedgeIndex = Mesh->CollapseWedgeThus(WedgeIndex);
						LODVertIndex = Mesh->Wedges(WedgeIndex).iVertex;
					}					
					LVert[v] = LODVertIndex;
				}

				//Render.
				FVector*  P1 = &Samples[ LVert[2] ].Point;
				for( int j=0; j<3; j++ )
				{
					FVector* P2 = &Samples[ LVert[j] ].Point;					
					Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LINE_DepthCued, *P1, *P2 );					
					P1 = P2;
				}
			}
		}
		// Render any special/weapon triangles: for debugging purposes only.
		for( i=0; i<Mesh->SpecialFaces.Num(); i++ )
		{			
			// Draw only if FaceLevel indicates this face falls within our vertex budget.
			FMeshFace& Face = Mesh->SpecialFaces(i);  
			INT LVert[3]; 
			LVert[0] = Face.iWedge[0];
			LVert[1] = Face.iWedge[1];
			LVert[2] = Face.iWedge[2];
			// Render - weapon triangle for debugging..
			FVector*  P1 = &AllSamples[ LVert[2] ].Point;
			for( int j=0; j<3; j++ )
			{
				FVector* P2 = &AllSamples[ LVert[j] ].Point;
				Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LINE_DepthCued, *P1, *P2 );
				P1 = P2;
			}
		}

		// Vertex influences		
		if( bSkeletal && BlendShow) 
		{
			INT PointNum = ((USkeletalMesh*)Mesh)->DebugVerts.Num(); //FPlane colors signifying the influences
			if( PointNum)
			{
				BYTE* SkelPoint  = &((USkeletalMesh*)Mesh)->DebugVerts(0);
				// Draw vertices, color follows the points index.
				for(INT i=0; i<VertexSubset; i++ )
				{
					FVector P = Samples[i].Point; 

					//if( i==0 )debugf(TEXT("$$$ Vertex 0 origin: %f %f %f "),P.X,P.Y,P.Z);

					INT VertInfluences = SkelPoint[i];
					FPlane Color;
					if( VertInfluences==0 )
						Color = FPlane( 0.f,0.f,0.f,1.f ); // Black - no links - should never occur.
					else if( VertInfluences==1 )
						Color = FPlane( 0.f,0.8f,0.f,1.f ); // Green for single influence
					else if( VertInfluences==2 )
						Color = FPlane( 0.8f,0.2f,0.f,1.f ); // red for two
					else if( VertInfluences==3 )
						Color = FPlane( 0.8f,0.f,1.f,1.f ); // pink for three
					else if( VertInfluences==4 )				
						Color = FPlane(0.15f,0.75f,1.f,1.f); // light blue 
					else if( VertInfluences>=5 )
						Color = FPlane(0.9f,0.9f,0.9f,1.f); // white for 5 +...  using this many links is often unnecessary.
					
					if( !Frame->Viewport->IsOrtho() )
					{
						FLOAT SX2 = Frame->FX2;
						FLOAT SY2 = Frame->FY2;
						// Transform
						P = P.TransformPointBy( Frame->Coords );
						// Calculate perspective.
						P.Z = 1.f/P.Z; 
						P.X = appRound( P.X * Frame->Proj.Z * P.Z + SX2 );
						P.Y = appRound( P.Y * Frame->Proj.Z * P.Z + SY2 );
						// Draw a little cross/rectangle
						Frame->Viewport->RenDev->Draw2DClippedLine( Frame, Color, LINE_None, P+FVector( 1.f, 1.f,0.f),P+FVector(-1.f,-1.f,0.f ) );
						Frame->Viewport->RenDev->Draw2DClippedLine( Frame, Color, LINE_None, P+FVector( 1.f,-1.f,0.f),P+FVector(-1.f, 1.f,0.f ) );
					}
				}
			}
		}
		STAT(unclock(GStat.MeshTime));
		Mark.Pop();
		unguardSlow;
		return;
	}
	unguardSlow;

	// Skeletal weapon coordinates
	if( bSkeletal && ( ((USkeletalMesh*)Mesh)->WeaponBoneIndex > -1 ))
	{
		// Grab the pre-made weapon coordinate system by GetFrame.
		// Weapon bone was chosen at script compile time.
		SpecialCoords = ((USkeletalMesh*)Mesh)->ClassicWeaponCoords;
		HasSpecialCoords = 1;
	}

	// Coloring.
	FLOAT Unlit = Clamp( Owner->ScaleGlow*0.5f + Owner->AmbientGlow/256.f, 0.f, 1.f );
	GUnlitColor = FVector( Unlit, Unlit, Unlit );
	if( GIsEditor && (ExtraFlags & PF_Selected) )
		GUnlitColor = GUnlitColor*0.4f + FVector(0.0f,0.6f,0.0f);

	if( Owner->Opacity < 1.f )
		// Special mode for alpha blending.
		ExtraFlags |= PF_Highlighted | PF_Translucent;

	// In transparent (not modulated) modes, sort back-to-front, for more consistent blending.
	bool DepthSorting = SoftwareRendering != 0 || (ExtraFlags & (PF_Highlighted | PF_Translucent));

	// Dynamic Face array setup. All faces with valid LOD level get their 3 wedges LOD-processed/morphed,
	// and these get flagged as processed using the (full sized) WedgePool table.	
	TArray<FMeshFaceSort> FacePool;
	TArray<FMeshWedge>    WedgePool;
	WedgePool.AddZeroed( Mesh->Wedges.Num() );
	INT NumWedges = Mesh->Wedges.Num();
	// Minor kludge: *if* UV==0 and ivertex==0 it will assume an uninitialized one.

	INT MatIndex = -1;
	DWORD MatFlags = 0;
	FLOAT SortBias = 0.f;

	guardSlow(Process);
	if (MeshOutcode == 0)
	{
		guardSlow(LODFacesProcessing);

		if( DoLOD )
			NumWedges = 0;
		for( INT i=0; i<Mesh->Faces.Num(); i++) 
		{
			// This face does not even exist if its FaceLevel 
			// indicates it does not fall within our vertex budget.
			if( Mesh->FaceLevel(i) <= VertexSubset ) 
			{
				FMeshFace& Face = Mesh->Faces(i);
				// Faces sorted by materials so don't often change.
				if( MatIndex != Face.MaterialIndex )
				{
					MatIndex = Face.MaterialIndex;
					MatFlags = ExtraFlags | Mesh->Materials(Face.MaterialIndex).PolyFlags;
					SortBias = Owner->bTwoSidedBias && (MatFlags & PF_TwoSided) ? Sprite->Actor->CollisionHeight * -0.3f : 0.f;
				}

				FTransTexture* V[3];

				if( DoLOD )
				{
					// When morphing: Samples[i].U = alpha.
					for( INT w=0; w<3; w++)
					{
						INT iStartWedge = Face.iWedge[w];
						// Only one DWORD.
						FMeshWedge Wedge = Mesh->Wedges(iStartWedge); 

						// Uninitialized wedge ?
						if( *(DWORD*)&WedgePool(iStartWedge) == 0) 
						{
							INT iWedge = iStartWedge;

							while( Wedge.iVertex >= VertexSubset )
							{							
								iWedge = Mesh->CollapseWedgeThus( iWedge );
								Wedge.iVertex = Mesh->Wedges(iWedge).iVertex;								
							};
							Wedge.TexUV = Mesh->Wedges(iWedge).TexUV;
							
#ifdef LOD_MORPH
							// Morphing: a fractional collapse.
							if ( DoMorph  && ( Wedge.iVertex > NonMorphSubset ))
							{
								FLOAT Alpha = Samples[Wedge.iVertex].U;
								if( *(DWORD*)&Alpha != 0 ) 
								{
									INT iNext = Mesh->CollapseWedgeThus( iWedge );
									// Actually a different wedge ?
									if (iWedge != iNext)
									{
										FMeshWedge Wedge2 = Mesh->Wedges(iNext);
										// Actually a different UV ? -> occurs about 10x more than same UV...
										if( Wedge.TexUV.U!=Wedge2.TexUV.U
										||	Wedge.TexUV.V!=Wedge2.TexUV.V )
										{
											Wedge.TexUV.U = appRound( (FLOAT)Wedge.TexUV.U + (FLOAT)((FLOAT)Wedge2.TexUV.U - (FLOAT)Wedge.TexUV.U) *  Alpha );
											Wedge.TexUV.V = appRound( (FLOAT)Wedge.TexUV.V + (FLOAT)((FLOAT)Wedge2.TexUV.V - (FLOAT)Wedge.TexUV.V) *  Alpha );
										}
									}
								}
							}
#endif
							WedgePool(iStartWedge) = Wedge; // Cache it, including the possibly morphed UV.
							NumWedges = Max(NumWedges, iStartWedge+1);
						}
						else
						{
							Wedge = WedgePool(iStartWedge);
						}
						V[w] = &Samples[Wedge.iVertex];
					}
				}
				else
				{
					INT iStartWedge0 = Face.iWedge[0];
					INT iStartWedge1 = Face.iWedge[1];
					INT iStartWedge2 = Face.iWedge[2];
					FMeshWedge Wedge0 = Mesh->Wedges(iStartWedge0); 
					FMeshWedge Wedge1 = Mesh->Wedges(iStartWedge1); 
					FMeshWedge Wedge2 = Mesh->Wedges(iStartWedge2); 
					WedgePool(iStartWedge0) = Wedge0;
					WedgePool(iStartWedge1) = Wedge1; 
					WedgePool(iStartWedge2) = Wedge2;
					V[0] = &Samples[Wedge0.iVertex]; 
					V[1] = &Samples[Wedge1.iVertex]; 
					V[2] = &Samples[Wedge2.iVertex];  
				}
				
				// Compute triangle normal whether visible or not.
				FVector FaceNormal = (V[0]->Point-V[1]->Point) ^ (V[2]->Point-V[0]->Point);
				//FaceNormal *= DivSqrtApprox(FaceNormal.SizeSquared()+0.001f);

				if( !Owner->bMeshCurvy )
				{
					// Accumulate into normals of all vertices that make up this face.
					V[0]->Normal += FaceNormal;
					V[1]->Normal += FaceNormal;
					V[2]->Normal += FaceNormal;
				}

				// See if potentially visible.
				if( !(V[0]->Flags & V[1]->Flags & V[2]->Flags) )
				{
					if(	(MatFlags & PF_TwoSided) || Frame->Mirror * (V[0]->Point| FaceNormal ) < 0.f )
					{					
						// Indicate these vertices need to be lit later.
						V[0]->Light.X = -1;
						V[1]->Light.X = -1;
						V[2]->Light.X = -1;

						// This face is visible. Add to the list.
						INT FaceTop = FacePool.Num();
						FacePool.Add();
						FacePool(FaceTop).Face = &Face;

						//Set the sort key ONLY if we're in software.
						if( DepthSorting )
						{
							FacePool(FaceTop).Key
							=	NotWeaponHeuristic
							?	appRound( 1000*( V[0]->Point.Z + V[1]->Point.Z + V[2]->Point.Z + SortBias ) )
							:	appRound( FDistSquared(V[0]->Point,Hack)*FDistSquared(V[1]->Point,Hack)*FDistSquared(V[2]->Point,Hack) );
						}
					}
				}
			}
		}
		unguardSlow;
	}

	unguardSlow;


	//
	// Render triangles.
	//

	guardSlow(TriangleRender);
	if( FacePool.Num() )
	{
		guardSlow(RenderTexx);
		// Fatness.
		UBOOL Fatten   = Owner->Fatness!=128;
		FLOAT Fatness  = (Owner->Fatness/16.0f)-8.0f;
		//FLOAT Detail   = GlobalMeshLOD * Owner->DrawScale / (0.5f * Frame->RProj.Z * Max(1.f,Owner->Location.TransformPointBy(Coords).Z));

		// Sort by depth.
		if( DepthSorting ) 
		{
			Sort( FacePool );
		}

		// Lock the textures.
		UTexture* EnvironmentMap = NULL;
		check(Mesh->Textures.Num()<=ARRAY_COUNT(TextureInfo));

		guardSlow(Locktextures);
		Clock(GStat.MeshTmapTime);
		// INT TexLOD = Max( Engine->Client->TextureLODSet[LODSET_Skin],
		INT TexLOD = Max( Engine->Client->TextureLODSet[LODSET_World]-1,
						  Frame->Viewport->RenDev->RecommendedLOD );
		for( INT i=0; i<Mesh->Textures.Num(); i++ )
		{
			Textures[i] = Mesh->GetTexture( i, Owner );
			if( Textures[i] )
			{
				Textures[i] = Textures[i]->Get( Frame->Viewport->CurrentTime );
				INT ThisLOD = Min( TexLOD, Engine->Client->TextureLODSet[Textures[i]->LODSet] );
				Textures[i]->Lock( TextureInfo[i], Frame->Viewport->CurrentTime, ThisLOD, Frame->Viewport->RenDev );
				EnvironmentMap = Textures[i];								
			}
		}
		unguardSlow;

		guardSlow(Texxx);
		if( Owner->Texture )
			EnvironmentMap = Owner->Texture;
		else if( Owner->Region.Zone && Owner->Region.Zone->EnvironmentMap )
			EnvironmentMap = Owner->Region.Zone->EnvironmentMap;
		else if( Owner->Level->EnvironmentMap )
			EnvironmentMap = Owner->Level->EnvironmentMap;
		if( EnvironmentMap==NULL )
			return;
		unguardSlow;
		check(EnvironmentMap);
		guardSlow(EnvLock);
		Clock(GStat.MeshTmapTime);
		EnvironmentMap->Lock( EnvironmentInfo, Frame->Viewport->CurrentTime, -1, Frame->Viewport->RenDev );
		unguardSlow;

		// Build list of all incident lights on the mesh.
		STAT(clock(GStat.MeshLightSetupTime));
		ExtraFlags |= GLightManager->SetupForActor( Frame, Sprite->Actor, Sprite->LeafLights, Sprite->Volumetrics );
		STAT(unclock(GStat.MeshLightSetupTime));

		STAT(clock(GStat.MeshLightTime)); 
		// Perform all vertex lighting.

		guardSlow(Subset);
		FVector Loc = Owner->Location.TransformPointBy(Coords);
		for(INT i=0; i<VertexSubset; i++ )
		{
			guardSlow(TransSet);
			FTransSample& Vert = Samples[i];
			if( Vert.Light.X == -1 ) // Only light/project if part of a visible triangle.
			{
				guardSlow(Normal);
				if( Owner->bMeshCurvy )
					// Set normal from position.
					Vert.Normal = Vert.Point - Loc;
				Vert.Normal *= DivSqrtApprox(Vert.Normal.SizeSquared()+0.001f);					
				unguardSlow;
					
				// Fatten it if desired.
				guardSlow(Fatten);				
				if( Fatten )
				{
					Vert.Point += Vert.Normal * Fatness;
					Vert.ComputeOutcode( Frame );
				}
				unguardSlow;
				

				guardSlow(Light);

				// Compute effect of each lightsource on this vertex.
				GLightManager->LightAndFog( Vert, ExtraFlags );
				if( SoftwareRendering )
					Vert.Light *= Owner->Opacity;
				Vert.Light.W = Owner->Opacity;
				unguardSlow;

				guardSlow(Project);
				// Project it: 
				if( !Vert.Flags ) // Project if visible only.
				{
					Vert.Project( Frame );
				}
				unguardSlow;
			}
			unguardSlow;
		}
		unguardSlow;
		STAT(unclock(GStat.MeshLightTime));

		// Draw the triangles.
		STAT(GStat.MeshPolyCount+=FacePool.Num());

		// Reset cached material indicator.
		MatIndex = -1;
		FTextureInfo* Info = NULL; 

		guardSlow(Pool);

		// Allocate and fill in final samples from wedge info.
		FTransTexture* Verts = New<FTransTexture>(GMem, NumWedges); 
		for_count( w, NumWedges )
		{
			Verts[w] = Samples[WedgePool(w).iVertex];
		}

		RenderInit( Verts, NumWedges );
		for(INT i=0; i<FacePool.Num(); i++ )
		{
			// Set up the triangle.			
			guardSlow(facearr);
			FMeshFace &Face = *FacePool(i).Face;			

			guardSlow(Materialz);
			// Update material if changed since last face.
			if ( MatIndex != Face.MaterialIndex )
			{
				MatIndex = Face.MaterialIndex;
				MatFlags = ExtraFlags | Mesh->Materials( MatIndex ).PolyFlags;
				INT TexIndex =          Mesh->Materials( MatIndex ).TextureIndex;
				Info = ( Textures[TexIndex] && !(MatFlags & PF_Environment)) ? &TextureInfo[TexIndex] : &EnvironmentInfo;
				//if( !(Info->Texture->PolyFlags & PF_Masked) )
				//	MatFlags &= ~PF_Masked;
				UScale = Info->UScale * Info->USize/256.f;
				VScale = Info->VScale * Info->VSize/256.f;
				SortBias = Owner->bTwoSidedBias && (MatFlags & PF_TwoSided) ? -0.001f : 0.f;
			}
			unguardSlow;
			
			// Set up texture coords.
			FTransTexture* Pts[6];
			guardSlow(InPool);

			for_count( v, 3 )
			{
				FMeshWedge Wedge = WedgePool( Face.iWedge[v] );
				Pts[v]    = &Verts[ Face.iWedge[v] ];
				Pts[v]->U = Wedge.TexUV.U * UScale;
				Pts[v]->V = Wedge.TexUV.V * VScale;
				Pts[v]->RZ /= (1.f + Pts[v]->RZ*SortBias);
			}
			unguardSlow;

			guardSlow(Subsurface);
			if( Frame->Mirror == -1 ) 
				Exchange( Pts[2], Pts[0] );
			RenderSubsurface( Frame, *Info, Sprite->SpanBuffer, Pts, MatFlags, 0 );
			unguardSlow;

			unguardSlow;
		}
		RenderFlush();
		unguardSlow;

		GLightManager->FinishActor();

		guardSlow(TUnlock);
		Clock(GStat.MeshTmapTime);
		for(INT i=0; i<Mesh->Textures.Num(); i++ )
		{
			if( Textures[i] ) Textures[i]->Unlock( TextureInfo[i] );
		}
		EnvironmentMap->Unlock( EnvironmentInfo );
		unguardSlow;

		unguardSlow;
	}
	unguardSlow;

	STAT(GStat.MeshCount++);
	STAT(unclock(GStat.MeshTime));
	STAT(DrawTime = (GStat.MeshTime - DrawTime));
	Mark.Pop();	

	// Dump one line of animation and mesh stats if in STAT ANIM mode and enough screen space left.
	if( AnimStats )
	{	
		AActor* AnimOwner = NULL;
		// Check to see if bAnimByOwner
		if ((Owner->bAnimByOwner) && (Owner->Owner != NULL))
			AnimOwner = Owner->Owner;
		else
			AnimOwner = Owner;

		if ( AnimOwner->AnimSequence != NAME_None )
		{
			const FMeshAnimSeq* Seq = NULL; // = Mesh->GetAnimSeq( AnimOwner->AnimSequence );
			if( bSkeletal )
			{
				//Seq = Owner->SkelAnim[0]->GetAnimSeq(Owner->AnimSequence);
				guard(Skelseqget);
				if (Owner->SkelAnim) 
					Seq = Owner->SkelAnim->GetAnimSeq(AnimOwner->AnimSequence);
				unguard;
			}
			else
			{
				Seq = Mesh->GetAnimSeq( AnimOwner->AnimSequence );
			}

			if( Seq && ((StatLine + 8) < Frame->Viewport->Canvas->Frame->Y) )
			{
				// Compute (mesh animation) interpolation numbers.
				INT iFrameOffset1=0, iFrameOffset2=0;
				FLOAT Alpha = 0.f;

				FLOAT MeshTime = DrawTime * GSecondsPerCycle*1000.f;
				Frame->Viewport->Canvas->CurY = StatLine+=8;
				TCHAR TempStr[512];

				if (AnimOwner->AnimFrame >= 0.f)
				{
					FLOAT InFrame = ::Max(AnimOwner->AnimFrame,0.f)* Seq->NumFrames;
					INT iFrame    = appFloor(InFrame);
					Alpha         = InFrame - iFrame;
					iFrameOffset1 = ((iFrame + 0) % Seq->NumFrames);
					iFrameOffset2 = ((iFrame + 1) % Seq->NumFrames);
					// Animations
					Frame->Viewport->Canvas->Color = FColor(230,230,230);
					if( (iFrameOffset1<=iFrameOffset2) || (Alpha == 0.f) )
						appSprintf(TempStr,TEXT("%21s  anim:%10s [%5.2f/%2i] rate:%4.2f verts:%4i msec:%4.2f"),Owner->GetName(),*(AnimOwner->AnimSequence),(FLOAT)iFrameOffset1+Alpha, Seq->NumFrames, AnimOwner->AnimRate,VertexSubset, MeshTime);
					//#debug: the scaling
					else
						appSprintf(TempStr,TEXT("%21s  anim:%10s [%5.2f/%2i] rate:%4.2f verts:%4i msec:%4.2f"),Owner->GetName(),*(AnimOwner->AnimSequence),Alpha-1.f, Seq->NumFrames, AnimOwner->AnimRate,VertexSubset, MeshTime );
				}
				else
				{
					Alpha = AnimOwner->AnimFrame;// 1.f + AnimOwner->AnimFrame; //??
					// Tweens.
					Frame->Viewport->Canvas->Color = FColor(215,215,255);
					appSprintf(TempStr,TEXT("%21s tween:%10s [  -> /%2i] rate:%4.2f verts:%4i msec:%4.2f alpha:%4.2f"),Owner->GetName(),*(AnimOwner->AnimSequence), Seq->NumFrames, AnimOwner->TweenRate, VertexSubset, MeshTime, Alpha);
				}		
				Frame->Viewport->Canvas->WrappedPrintf( Frame->Viewport->Canvas->SmallFont, 0, TEXT(" %s\n"), TempStr );
			}
		}		
	}
	unguardf(( TEXT("(%s)"), Owner->Mesh->GetFullName() ));
}

/*------------------------------------------------------------------------------
	The End.
------------------------------------------------------------------------------*/
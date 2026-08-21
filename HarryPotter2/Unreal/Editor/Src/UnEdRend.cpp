/*=============================================================================
	UnEdRend.cpp: Unreal editor rendering functions.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EditorPrivate.h"
#include "UnRender.h"
#pragma DISABLE_OPTIMIZATION

#if 1 //U2Ed (Vertex Editing)
extern TArray<FVertexHit> VertexHitList;
#endif

// For Interpolation Point drawing
extern UBOOL matShowPathOrientation;
extern TArray<HBezierControlPoint> BezierControlPointList;
extern TArray<NAME_INDEX> InterpolationPathCheckList;		// List of tag name indexes already processed

/*-----------------------------------------------------------------------------
	FPoly drawing.
-----------------------------------------------------------------------------*/

//
// Draw an editor polygon
//
void UEditorEngine::DrawFPoly( FSceneNode* Frame, FPoly* EdPoly, FPlane WireColor, DWORD LineFlags )
{
	guard(UEditorEngine::DrawFPoly);

	FVector* V1 = &EdPoly->Vertex[0];
	FVector* V2 = &EdPoly->Vertex[EdPoly->NumVertices-1];
	for( int i=0; i<EdPoly->NumVertices; i++ )
	{
		if( (EdPoly->PolyFlags & PF_NotSolid) || (V1->X >= V2->X) )
			Frame->Viewport->RenDev->Draw3DLine( Frame, WireColor, LineFlags, *V1, *V2 );
		V2 = V1++;
	}

	unguard;
}

/*-----------------------------------------------------------------------------
	Bounding box drawing.
-----------------------------------------------------------------------------*/

//
// Draw the brush's bounding box.
//
void UEditorEngine::DrawBoundingBox( FSceneNode* Frame, FBox* Bound, AActor* Actor )
{
	guard(UEditorEngine::DrawBoundingBox);

	FVector B[2],P,Q;
	FLOAT SX,SY;
	int i,j,k;

	B[0]=Bound->Min;
	B[1]=Bound->Max;

	for( i=0; i<2; i++ ) for( j=0; j<2; j++ )
	{
		P.X=B[i].X; Q.X=B[i].X;
		P.Y=B[j].Y; Q.Y=B[j].Y;
		P.Z=B[0].Z; Q.Z=B[1].Z;
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_ScaleBox.Plane(), LINE_Transparent, P, Q );

		P.Y=B[i].Y; Q.Y=B[i].Y;
		P.Z=B[j].Z; Q.Z=B[j].Z;
		P.X=B[0].X; Q.X=B[1].X;
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_ScaleBox.Plane(), LINE_Transparent, P, Q );

		P.Z=B[i].Z; Q.Z=B[i].Z;
		P.X=B[j].X; Q.X=B[j].X;
		P.Y=B[0].Y; Q.Y=B[1].Y;
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_ScaleBox.Plane(), LINE_Transparent, P, Q );
	}
	for( i=0; i<2; i++ ) for( j=0; j<2; j++ ) for( k=0; k<2; k++ )
	{
		P.X=B[i].X; P.Y=B[j].Y; P.Z=B[k].Z;
		if( Render->Project( Frame, P, SX, SY, NULL ) )
		{
			if( Actor ) PUSH_HIT(Frame,HBrushVertex(CastChecked<ABrush>(Actor),P));
			Frame->Viewport->RenDev->Draw2DPoint( Frame, C_ScaleBoxHi.Plane(), LINE_None, SX-1, SY-1, SX+1, SY+1, P.Z );
			if( Actor ) POP_HIT(Frame);
		}
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Level brush drawing.
-----------------------------------------------------------------------------*/
#if 1
// far-plane Z clipping distance.
//!! adding EdClipZ to UEditorEngine causes crash due to data structure size delta
// For now, I'm going to hack it to avoid changes to EditorEngine.uc and Editor.u
FLOAT EdClipZ = 0.0;
#endif

#if 1
//!! Z clipping hack to avoid mucking with Render.dll (see below)
void UEditorEngine::SetZClipping()
{
	guard(UEditorEngine::SetZClipping);

	// toggle Z clipping
	if( EdClipZ > 0.0 )
	{
		ResetZClipping();
		return;
	}

	// use the default brush max distance as the basis for setting the Z clipping distance
	ABrush* Actor = Level->Brush();
	UModel* Brush = Actor->Brush;

	// get the Frame for the first wireframe/perspective Viewport available
	FSceneNode* Frame = 0;
	for( int i=0; i<Client->Viewports.Num(); i++ )
	{
		UViewport* Viewport = Client->Viewports(i);
		if( Viewport && !Viewport->IsOrtho() && Viewport->IsWire() )
		{
			Frame = Render->CreateMasterFrame( Viewport, Viewport->Actor->Location, Viewport->Actor->ViewRotation, NULL );
			break;
		}
	}

	// abort if no suitable viewport was found
	if( Frame!=NULL )
	{
		// find the distance to the default brush -- if it's behind the viewport, abort (don't set EdClipZ)
		FBox Box = Brush->GetRenderBoundingBox( Actor, 0 );
		FVector	Temp = Box.Max - Frame->Coords.Origin;
		Temp     = Temp.TransformVectorBy( Frame->Coords );
		if( Temp.Z > 1.0 )
			EdClipZ = Temp.Z;
		Render->FinishMasterFrame();
	}

	unguard;
}

void UEditorEngine::ResetZClipping()
{
	EdClipZ = 0.0;
}
#endif

void UEditorEngine::DrawLevelBrush( FSceneNode* Frame, ABrush* Actor, UBOOL bStatic, UBOOL bDynamic, UBOOL bActive )
{
	guard(UEditorEngine::DrawLevelBrush);
	UModel* Brush = Actor->Brush;
	check(Brush);

	// Quick reject
	if( Actor==Level->Brush() )
	{
		if( !bActive )
			return;
	}
	else if( Actor->IsMovingBrush() )
	{
		if( !bDynamic )
			return;
	}
	else if( Actor->IsStaticBrush() )
	{
		if( !bStatic && !Actor->bSelected )
			return;
	}

#if 1
	// reject hidden brushes
	if( Actor->bHiddenEd )
	{
		return;
	}
#endif
	// See if we can reject the brush.
	FScreenBounds Bounds;
	FBox Box = Brush->GetRenderBoundingBox( Actor, 0 );
#if 1
	// For Z, XY, and XYZ view frustum clipping, the best design solution seems to be to modify 
	// URender::BoundVisible to add offsets in the comparisons with >0.0, >0.0, <Frame->FX, <Frame->FY, 
	// and add far-plane (Z) clipping in URender::Project()...?
	//
	//!! since Render.dll is off-limits, for now, hack Z clipping with an editor global set in UnEdSrv.cpp
	if( EdClipZ > 0.0 && !Frame->Viewport->IsOrtho() )
	{
		FVector	Temp = Box.Max - Frame->Coords.Origin;
		Temp     = Temp.TransformVectorBy( Frame->Coords );
		FLOAT Z  = Temp.Z; if (Abs (Z)<0.01) Z+=0.02f;

		if( Z < 1.0 || Z > EdClipZ )
			return;
	}
#endif
	if( !Render->BoundVisible( Frame, Box.GetCoords(), NULL, Bounds ) )
		return;

	// Figure out color and effects to draw.
	FPlane WireColor(0,0,0,0);
	DWORD	LineFlags     = LINE_None;
	UBOOL   bDrawPivot    = 0;
	UBOOL   bDrawVertices = 0;
	UBOOL   bDrawSelected = 0;
	if( Actor==Level->Brush() )
	{
		WireColor     = C_BrushWire.Plane();
		LineFlags    |= LINE_Transparent;
		bDrawPivot    = 1;
		bDrawVertices = 1;
		bDrawSelected = Actor->bSelected;
		if( GEditor->Mode==EM_BrushSnap )
		{
			FBox Box = Frame->Viewport->Actor->GetLevel()->Brush()->Brush->GetRenderBoundingBox( Frame->Viewport->Actor->GetLevel()->Brush(), 1 );
			DrawBoundingBox( Frame, &Box, Frame->Viewport->Actor->GetLevel()->Brush() );
		}
		PUSH_HIT(Frame,HActor(Actor));
	}
	else if( Actor->IsMovingBrush() )
	{
		WireColor     = C_Mover.Plane();
		bDrawPivot    = Actor->bSelected;
		bDrawVertices = Actor->bSelected;
		bDrawSelected = Actor->bSelected;	
		PUSH_HIT(Frame,HActor(Actor));
	}
	else if( Actor->IsStaticBrush() )
	{
		WireColor
		=	(Actor->bColored                  ) ? Actor->BrushColor.Plane()
		:	(Actor->CsgOper==CSG_Subtract     ) ? C_SubtractWire.Plane()
		:	(Actor->CsgOper!=CSG_Add          )	? C_GreyWire.Plane()
		:	(Actor->PolyFlags & PF_Portal     )	? C_SemiSolidWire.Plane()
		:	(Actor->PolyFlags & PF_NotSolid   ) ? C_NonSolidWire.Plane()
		:	(Actor->PolyFlags & PF_Semisolid  )	? C_ScaleBoxHi.Plane()
		:										  C_AddWire.Plane();
		bDrawPivot    = Actor->bSelected;
		bDrawVertices = Actor->bSelected;
		bDrawSelected = Actor->bSelected;
		PUSH_HIT(Frame,HActor(Actor));
	}

	// Get the polys.
	FMemMark Mark(GMem);
	FPoly* TransformedEdPolys = new(GMem,Brush->Polys->Element.Num())FPoly;
	FVector		Vertex, *VertPtr, *V1;
	FLOAT       X,Y;
	FPlane      DrawColor,VertexColor,PivColor;

	// Figure out brush movement constraints.
	FVector   Location = Actor->Location;
	FRotator Rotation = Actor->Rotation;

	// Make coordinate system from camera.
	FCoords Coords = GMath.UnitCoords * Actor->PostScale * Rotation * Actor->MainScale;
	Coords.Origin  = Frame->Coords.Origin;

	// Setup colors.
	DrawColor   = WireColor * (bDrawSelected ? 1.0f : 0.5f);
	VertexColor = WireColor * 1.2f;
	PivColor    = WireColor;

	// Transform and draw all FPolys.
	INT NumTransformed = 0;
	FPoly* EdPoly = &TransformedEdPolys[0];
	for( int i=0; i<Brush->Polys->Element.Num(); i++ )
	{
		*EdPoly = Brush->Polys->Element(i);
		EdPoly->Normal = EdPoly->Normal.TransformVectorBy( Coords );
#if 1 //U2Ed
		EdPoly->SavePolyIndex = i;
#endif
		if
		(	(!Frame->Viewport->IsOrtho())
		||	(Frame->Viewport->Actor->OrthoZoom<ORTHO_LOW_DETAIL)
		||	(EdPoly->PolyFlags & PF_NotSolid)
		||	(Frame->Coords.ZAxis | EdPoly->Normal) != 0.0 )
		{
			// Transform it.
			VertPtr = &EdPoly->Vertex[0];
			for( int j=0; j<EdPoly->NumVertices; j++ )
				*VertPtr++ = (*VertPtr - Actor->PrePivot).TransformVectorBy(Coords) + Location;

			// Draw this brush's EdPoly's.
			DrawFPoly( Frame, EdPoly, DrawColor, LineFlags );

			NumTransformed++;
			EdPoly++;
		}
	}
	POP_HIT(Frame);

	// Draw all vertices.
	if( bDrawVertices && Brush->Polys->Element.Num()>0 )
	{
		for( INT i=0; i<NumTransformed; i++ )
		{
			EdPoly = &TransformedEdPolys[i];
			V1 = &EdPoly->Vertex[0];
			for( INT j=0; j<EdPoly->NumVertices; j++,V1++ )
			{
      			if( Render->Project( Frame, *V1, X, Y, NULL ) )
				{
					PUSH_HIT(Frame,HBrushVertex(Actor,EdPoly->Vertex[j]));

					// In vertex edit mode, draw extra large vertices.
					if( Mode == EM_VertexEdit && Actor->bSelected )
					{
						VertexColor = FColor( 255, 255, 255 ).Plane() * 0.5;

						// If this vertex is part of the selected vertex list, draw it as highlighted.
						for( int vertex = 0 ; vertex < VertexHitList.Num() ; vertex++ )
							if( VertexHitList(vertex) == FVertexHit( Actor, EdPoly->SavePolyIndex, j ) )
							{
								VertexColor = FColor( 255, 255, 255 ).Plane();
								break;
							}

         				Frame->Viewport->RenDev->Draw2DPoint( Frame, VertexColor, LINE_None, X-2, Y-2, X+2, Y+2, V1->Z );
					}
					else
         				Frame->Viewport->RenDev->Draw2DPoint( Frame, VertexColor, LINE_None, X-1, Y-1, X+1, Y+1, V1->Z );

					POP_HIT(Frame);
         		}
			}
		}

		// Draw the origin.
		Vertex = -Actor->PrePivot.TransformVectorBy(Coords) + Location;
		if( Render->Project( Frame, Vertex, X, Y, NULL ) )
		{
			PUSH_HIT(Frame,HBrushVertex(Actor,Vertex));
			Frame->Viewport->RenDev->Draw2DPoint( Frame, VertexColor, LINE_None, X-1, Y-1, X+1, Y+1, Vertex.Z );
			POP_HIT(Frame);
		}
	}

	// Finished.
	Mark.Pop();
	unguard;
}

//
// Draw all moving brushes in the level as wireframes.
//
void UEditorEngine::DrawLevelBrushes( FSceneNode* Frame, UBOOL bStatic, UBOOL bDynamic, UBOOL bActive )
{
	guard(UEditorEngine::DrawLevelBrushes);
	ULevel* Level=Frame->Viewport->Actor->GetLevel();
	for( DWORD bStaticPass=0; bStaticPass<2; bStaticPass++ )
	{
		for( INT iActor=1; iActor<Level->Actors.Num(); iActor++ )
		{
			AActor* Actor=Level->Actors(iActor);
			if( Actor && Actor->IsBrush() && Actor->bStatic!=bStaticPass )
				DrawLevelBrush( Frame, (ABrush*)Actor, bStatic, bDynamic, bActive );
		}
		DrawLevelBrush( Frame, Level->Brush(), bStatic, bDynamic, bActive );
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Grid drawing.
-----------------------------------------------------------------------------*/

//
// Draw a piece of an orthogonal grid (arbitrary axes):
//
void UEditorEngine::DrawGridSection
(
	FSceneNode*		Frame,
	INT				ViewportLocX,
	INT				ViewportSXR,
	INT				ViewportGridY,
	FVector*		A,
	FVector*		B,
	FLOAT*			AX,
	FLOAT*			BX,
	INT				AlphaCase
)
{
	guard(UEditorEngine::DrawGridSection);

	if( !ViewportGridY ) return;
	check(Frame->Viewport->IsOrtho());

	FLOAT	Start = (INT)((ViewportLocX - (ViewportSXR>>1) * Frame->Zoom)/ViewportGridY) - 1.0f;
	FLOAT	End   = (INT)((ViewportLocX + (ViewportSXR>>1) * Frame->Zoom)/ViewportGridY) + 1.0f;
	INT     Dist  = (INT)(Frame->X * Frame->Zoom / ViewportGridY);

	// Figure out alpha interpolator for fading in the grid lines.
	FLOAT Alpha;
	INT IncBits=0;
	if( Dist+Dist >= Frame->X/4 )
	{
		while( (Dist>>IncBits) >= Frame->X/4 )
			IncBits++;
		Alpha = 2 - 2*(FLOAT)Dist / (FLOAT)((1<<IncBits) * Frame->X/4);
	}
	else Alpha = 1.0;

	INT iStart  = ::Max((INT)Start,-32768/ViewportGridY) >> IncBits;
	INT iEnd    = ::Min((INT)End,  +32768/ViewportGridY) >> IncBits;

	for( INT i=iStart; i<iEnd; i++ )
	{
		*AX = (i * ViewportGridY) << IncBits;
		*BX = (i * ViewportGridY) << IncBits;
		if( (i&1) != AlphaCase )
		{
			FPlane Background = C_OrthoBackground.Plane();
			FPlane Grid       = FPlane(.5f,.5f,.5f,0.0f);
			FPlane Color      = Background + (Grid-Background) * (((i<<IncBits)&7) ? 0.5f : 1.0f);
			if( i&1 ) Color = Background + (Color-Background) * Alpha;
			Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LINE_None, *A, *B );
		}
	}
	unguard;
}

//
// Draw worldbox and groundplane lines, if desired.
//
void UEditorEngine::DrawWireBackground( FSceneNode* Frame )
{
	guard(UEditorEngine::DrawWireBackground);

	// If clicked on nothing else, clicked on backdrop.
	FVector V;
	Render->Deproject( Frame, Frame->Viewport->HitX+Frame->Viewport->HitXL/2, Frame->Viewport->HitY+Frame->Viewport->HitYL/2, V );
	PUSH_HIT(Frame,HBackdrop(V));
	POP_HIT_FORCE(Frame);

	// Vector defining worldbox lines.
	FVector	Origin = Frame->Coords.Origin;
	FVector B1( 32768.0, 32767.0, 32767.0);
	FVector B2(-32768.0, 32767.0, 32767.0);
	FVector B3( 32768.0,-32767.0, 32767.0);
	FVector B4(-32768.0,-32767.0, 32767.0);
	FVector B5( 32768.0, 32767.0,-32767.0);
	FVector B6(-32768.0, 32767.0,-32767.0);
	FVector B7( 32768.0,-32767.0,-32767.0);
	FVector B8(-32768.0,-32767.0,-32767.0);
	FVector A,B;
	INT i,j;

	// Draw it.
	if( Frame->Viewport->IsOrtho() )
	{
		if( Frame->Viewport->Actor->ShowFlags & SHOW_Frame )
		{
			// Draw grid.
			for( int AlphaCase=0; AlphaCase<=1; AlphaCase++ )
			{
				if( Frame->Viewport->Actor->RendMap==REN_OrthXY )
				{
					// Do Y-Axis lines.
					A.Y=+32767.0; A.Z=0.0;
					B.Y=-32767.0; B.Z=0.0;
					DrawGridSection( Frame, Origin.X, Frame->X, GEditor->Constraints.GridSize.X, &A, &B, &A.X, &B.X, AlphaCase );

					// Do X-Axis lines.
					A.X=+32767.0; A.Z=0.0;
					B.X=-32767.0; B.Z=0.0;
					DrawGridSection( Frame, Origin.Y, Frame->Y, GEditor->Constraints.GridSize.Y, &A, &B, &A.Y, &B.Y, AlphaCase );
				}
				else if( Frame->Viewport->Actor->RendMap==REN_OrthXZ )
				{
					// Do Z-Axis lines.
					A.Z=+32767.0; A.Y=0.0;
					B.Z=-32767.0; B.Y=0.0;
					DrawGridSection( Frame, Origin.X, Frame->X, GEditor->Constraints.GridSize.X, &A, &B, &A.X, &B.X, AlphaCase );

					// Do X-Axis lines.
					A.X=+32767.0; A.Y=0.0;
					B.X=-32767.0; B.Y=0.0;
					DrawGridSection( Frame, Origin.Z, Frame->Y, GEditor->Constraints.GridSize.Z, &A, &B, &A.Z, &B.Z, AlphaCase );
				}
				else if( Frame->Viewport->Actor->RendMap==REN_OrthYZ )
				{
					// Do Z-Axis lines.
					A.Z=+32767.0; A.X=0.0;
					B.Z=-32767.0; B.X=0.0;
					DrawGridSection( Frame, Origin.Y, Frame->X, GEditor->Constraints.GridSize.Y, &A, &B, &A.Y, &B.Y, AlphaCase );

					// Do Y-Axis lines.
					A.Y=+32767.0; A.X=0.0;
					B.Y=-32767.0; B.X=0.0;
					DrawGridSection( Frame, Origin.Z, Frame->Y, GEditor->Constraints.GridSize.Z, &A, &B, &A.Z, &B.Z, AlphaCase );
				}
			}

			// Draw axis lines.
			FPlane Color = Frame->Viewport->IsOrtho() ? C_WireGridAxis.Plane() : C_GroundHighlight.Plane();

			A.X=+32767.0;  A.Y=0; A.Z=0;
			B.X=-32767.0;  B.Y=0; B.Z=0;
        	Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LINE_None, A, B );

			A.X=0; A.Y=+32767.0; A.Z=0;
			B.X=0; B.Y=-32767.0; B.Z=0;
        	Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LINE_None, A, B );

			A.X=0; A.Y=0; A.Z=+32767.0;
			B.X=0; B.Y=0; B.Z=-32767.0;
	       	Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LINE_None, A, B );
		}

		// Draw orthogonal worldframe.
    	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B1, B2 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B3, B4 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B5, B6 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B7, B8 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B1, B3 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B5, B7 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B2, B4 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B6, B8 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B1, B5 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B2, B6 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B3, B7 );
     	Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_Transparent, B4, B8 );
	}
	else if
	(	(Frame->Viewport->Actor->ShowFlags & SHOW_Frame)
	&& !(Frame->Viewport->Actor->ShowFlags & SHOW_Backdrop) )
	{
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B1, B2 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B1, B2 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B3, B4 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B5, B6 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B7, B8 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B1, B3 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B5, B7 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B2, B4 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B6, B8 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B1, B5 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B2, B6 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B3, B7 );
		Frame->Viewport->RenDev->Draw3DLine( Frame, C_WorldBox.Plane(), LINE_DepthCued, B4, B8 );

		// Index of middle line (axis).
		j=(63-1)/2;
		for( i=0; i<63; i++ )
		{
			A.X=32767.0*(-1.0+2.0*i/(63-1));	B.X=A.X;

			A.Y=32767;                          B.Y=-32767.0;
			A.Z=0.0;							B.Z=0.0;
			Frame->Viewport->RenDev->Draw3DLine( Frame, (i==j)?C_GroundHighlight.Plane():C_GroundPlane.Plane(), LINE_DepthCued, A, B );

			A.Y=A.X;							B.Y=B.X;
			A.X=32767.0;						B.X=-32767.0;
			Frame->Viewport->RenDev->Draw3DLine( Frame, (i==j)?C_GroundHighlight.Plane():C_GroundPlane.Plane(), LINE_DepthCued, A, B );
		}
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Interpolation Path drawing.
-----------------------------------------------------------------------------*/

int CDECL IntPointCompare(const void *A, const void *B)
{
	return (*(AInterpolationPoint**)A)->Position - (*(AInterpolationPoint**)B)->Position;
}

//Interpolation Paths (from Ion Storm, Austin) added by Legend on 4/12/2000
//
// DEUS_EX CNN - for drawing interpolation paths
// copied from Engine.UnScript.cpp
//
// pk -- modified with UW 3/28/2001
// dpl -- moved to UnEdRend.cpp 9/2/2002

FLOAT Splerp( FLOAT F )
{
	FLOAT S = Square(F);
	return (1.0f/16.0f)*S*S - (1.0f/2.0f)*S + 1;
}

//
// Draw a part of a spline path defined by a pair of Bezier control points
//
void UEditorEngine::DrawInterpolationPathSegment
(
	FSceneNode*				Frame,
	AInterpolationPoint*	From,
	AInterpolationPoint*	To,
	const FPlane&			SegmentColor
)
{
	guard(UEditorEngine::DrawInterpolationPath);

	if( !From || !To )
		return;

	UViewport*		Viewport = Frame->Viewport;

	FLOAT alpha = 0.f;
	FLOAT alphastep = 0.02f;
	FLOAT arrow = 0.0f;
	FLOAT arrowstep = 0.02f;
	To->PathDist = 0.f;

	if ( To->bInstantNextPath )
	{
		// draw straight purple line
		Viewport->RenDev->Draw3DLine(Frame, SegmentColor, Viewport->IsOrtho()?LINE_None:LINE_DepthCued, From->Location, To->Location);
	}
	else 
	{	
		FCoords OldCoords = GMath.UnitCoords / From->Rotation;
		OldCoords.Origin = From->Location;
		while (alpha < 1.0-alphastep)
		{
			// interpolation.
			FCoords FirstCoords = To->GetInterpolatedPosition(OldCoords,  From->StartControlPoint, alpha, 0);	
			FCoords SecondCoords = To->GetInterpolatedPosition(OldCoords,  From->StartControlPoint, alpha+alphastep, 0);	
			
			// draw the path in white
			Viewport->RenDev->Draw3DLine(Frame, SegmentColor, Viewport->IsOrtho()?LINE_None:LINE_DepthCued, FirstCoords.Origin, SecondCoords.Origin);
			To->PathDist += (SecondCoords.Origin - FirstCoords.Origin).Size();
			if( matShowPathOrientation )
			{
				// draw orientation axes on the path (Xfront = red, Yright = green, Zup = blue)
				if (arrow <= alpha)
				{
					Viewport->RenDev->Draw3DLine(Frame, FPlane(1,0,0,0), LINE_DepthCued, FirstCoords.Origin, FirstCoords.Origin + FirstCoords.XAxis * 32);
					Viewport->RenDev->Draw3DLine(Frame, FPlane(0,1,0,0), LINE_DepthCued, FirstCoords.Origin, FirstCoords.Origin + FirstCoords.YAxis * 32);
					Viewport->RenDev->Draw3DLine(Frame, FPlane(0,0,1,0), LINE_DepthCued, FirstCoords.Origin, FirstCoords.Origin + FirstCoords.ZAxis * 32);
					arrow += arrowstep;
				}
			}
			
			// inc the alpha
			alpha += alphastep;
		}
	}

	unguard;
}

//
// Draw a spline path defined by a set Bezier control points from a named path
//
void UEditorEngine::DrawInterpolationPath
(
	FSceneNode*		Frame,
	FName			PathTag
)
{
	guard(UEditorEngine::DrawInterpolationPath);

	UViewport*		Viewport = Frame->Viewport;
	AActor*			Actor;
	INT				iActor;
	INT				numPoints;
	INT				cur;
	INT				i;

	// If path already drawn, skip it; otherwise note that drawing is being now being handled
	if ( InterpolationPathCheckList.FindItemIndex( PathTag.GetIndex() ) != INDEX_NONE )
		return;
	else
		InterpolationPathCheckList.AddItem( PathTag.GetIndex() );

	// generate a list of all matching points
	#define MAX_INTERP_POINTS 128
	AInterpolationPoint* PointList[MAX_INTERP_POINTS];
	for (i=0; i<MAX_INTERP_POINTS; i++)
		PointList[i] = NULL;

	numPoints = 0;
	for (iActor=0; iActor<Viewport->Actor->GetLevel()->Actors.Num(); iActor++)
	{
		Actor = Viewport->Actor->GetLevel()->Actors(iActor);
		if(Actor && Actor->IsA(AInterpolationPoint::StaticClass()) && (Actor->Tag == PathTag))
		{
			AInterpolationPoint* Dest = Cast<AInterpolationPoint>(Actor);

			PointList[numPoints] = Dest;
			numPoints++;
			if (numPoints >= MAX_INTERP_POINTS)
				break;
		}
	}

	if( numPoints > 0 )
	{
		appQsort( &PointList[0], numPoints, sizeof(AInterpolationPoint*), IntPointCompare );

		// link up interpolation points forward
		for ( cur = 0; cur < numPoints-1; ++cur )
			PointList[cur]->Next = PointList[cur+1];

		// link up interpolation points backward
		for ( cur = 1; cur < numPoints; ++cur )
			PointList[cur]->Prev = PointList[cur-1];

		// Splice ends together
		PointList[0]->Prev = PointList[numPoints-1];
		PointList[numPoints-1]->Next = PointList[0];

		// Draw the entire chain
		for ( cur = 0; cur < numPoints; ++cur )
		{
			FVector v1, v2;
			FRotator r1, r2;
			FCoords C;

			// Draw Control Points
			FPlane ControlPointColor = FPlane(0,.5,0,0);
			UBOOL bSelected;

			bSelected = ( BezierControlPointList.FindItemIndex( HBezierControlPoint(PointList[cur],PointList[cur]->Location,1) ) != INDEX_NONE );
			PUSH_HIT(Frame,HBezierControlPoint(PointList[cur],PointList[cur]->Location,1));
				Viewport->RenDev->Draw3DLine(Frame, ControlPointColor*(bSelected?2:1), LINE_DepthCued, PointList[cur]->Location,  PointList[cur]->Location + PointList[cur]->StartControlPoint);
				v1 = PointList[cur]->Location + PointList[cur]->StartControlPoint;
				Render->DrawCircle( Frame, ControlPointColor*(bSelected?2:1), LINE_DepthCued, v1, 4 );
			POP_HIT(Frame);

			bSelected = ( BezierControlPointList.FindItemIndex( HBezierControlPoint(PointList[cur],PointList[cur]->Location,0) ) != INDEX_NONE );
			PUSH_HIT(Frame,HBezierControlPoint(PointList[cur],PointList[cur]->Location,0));
				Viewport->RenDev->Draw3DLine(Frame, ControlPointColor*(bSelected?2:1), LINE_DepthCued, PointList[cur]->Location,  PointList[cur]->Location + PointList[cur]->EndControlPoint);
				v2 = PointList[cur]->Location + PointList[cur]->EndControlPoint;
				Render->DrawCircle( Frame, ControlPointColor*(bSelected?2:1), LINE_DepthCued, v2, 4 );
			POP_HIT(Frame);

			// Find all switch destinations
			for ( i=0; i<8; ++i )
			{
				PointList[cur]->Switches_Forward_Priv[i].Next = NULL;
				PointList[cur]->Switches_Reverse_Priv[i].Next = NULL;
			}

			for (INT iActor=0; iActor<Viewport->Actor->GetLevel()->Actors.Num(); iActor++)
			{
				Actor = Viewport->Actor->GetLevel()->Actors(iActor);
				if ( Actor && Actor->IsA(AInterpolationPoint::StaticClass()) )
				{
					AInterpolationPoint *	IP = Cast<AInterpolationPoint>(Actor);
					FName	SwitchPathName;

					for ( i=0; i<8; ++i )
					{
						SwitchPathName = PointList[cur]->Switches_Forward[i].PathName;
						if ( SwitchPathName == NAME_None && PointList[cur]->Switches_Forward[i].PathPosition > 0 )
							SwitchPathName = PathTag;
						if (    IP->Tag == SwitchPathName
							 && IP->Position >= PointList[cur]->Switches_Forward[i].PathPosition )
						{
							if (    PointList[cur]->Switches_Forward_Priv[i].Next == NULL
								 || PointList[cur]->Switches_Forward_Priv[i].Next->Position > IP->Position )
							PointList[cur]->Switches_Forward_Priv[i].Next = IP;
						}

						SwitchPathName = PointList[cur]->Switches_Reverse[i].PathName;
						if ( SwitchPathName == NAME_None && PointList[cur]->Switches_Reverse[i].PathPosition > 0 )
							SwitchPathName = PathTag;
						if (    IP->Tag == SwitchPathName
							 && IP->Position <= PointList[cur]->Switches_Reverse[i].PathPosition )
						{
							if (    PointList[cur]->Switches_Reverse_Priv[i].Next == NULL
								 || PointList[cur]->Switches_Reverse_Priv[i].Next->Position < IP->Position )
							PointList[cur]->Switches_Reverse_Priv[i].Next = IP;
						}
					}
				}
			}

			// Compute total chance used in this point
			float	fForwardChance = 0.0;
			float	fReverseChance = 0.0;

			if ( PointList[cur]->bEndOfPath )
			{
				for ( i=0; i<8; ++i )	// If main line is shut off, spread chance over all alternatives
				{
					fForwardChance += PointList[cur]->Switches_Forward[i].Chance;
					fReverseChance += PointList[cur]->Switches_Reverse[i].Chance;
				}
			}
			else
			{
				fForwardChance = 100.0;
				fReverseChance = 100.0;
			}

			// Draw path segments leading to alternative paths and draw the paths too
			FPlane	SegmentColor;
			for ( i=0; i<8; ++i )
			{
				if ( PointList[cur]->Switches_Forward_Priv[i].Next != NULL )
				{
					if (    PointList[cur]->Switches_Forward[i].Chance <= 0
						 || PointList[cur]->Switches_Forward[i].Chance > fForwardChance + 0.05 )
						SegmentColor = FPlane(.8,.7,.7,0);
					else if ( PointList[cur]->Switches_Forward_Priv[i].Next->bInstantNextPath )
						SegmentColor = FPlane(.7,0,.5,0);
					else
						SegmentColor = FPlane(1,.5,.5,0);
					fForwardChance -= PointList[cur]->Switches_Forward[i].Chance;

					DrawInterpolationPathSegment(
						Frame,
						PointList[cur],
						PointList[cur]->Switches_Forward_Priv[i].Next,
						SegmentColor
					);

					DrawInterpolationPath( Frame, PointList[cur]->Switches_Forward[i].PathName );
				}

				if ( PointList[cur]->Switches_Reverse_Priv[i].Next != NULL )
				{
					if (    PointList[cur]->Switches_Reverse[i].Chance <= 0
						 || PointList[cur]->Switches_Reverse[i].Chance > fReverseChance + 0.05 )
						SegmentColor = FPlane(.8,.8,.7,0);
					else if ( PointList[cur]->Switches_Reverse_Priv[i].Next->bInstantNextPath )
						SegmentColor = FPlane(.8,0,.6,0);
					else
						SegmentColor = FPlane(.9,.7,.5,0);
					fReverseChance -= PointList[cur]->Switches_Reverse[i].Chance;

					DrawInterpolationPathSegment(
						Frame,
						PointList[cur]->Switches_Reverse_Priv[i].Next,
						PointList[cur],
						SegmentColor
					);

					DrawInterpolationPath( Frame, PointList[cur]->Switches_Reverse[i].PathName );
				}
			}

			// Draw next path segment (unless end of a non-looping path)
			if ( !PointList[cur]->bEndOfPath )
			{
				if ( fForwardChance < 0.05 )
					SegmentColor = FPlane(.8,.8,.8,0);
				else if ( PointList[cur]->Next->bInstantNextPath )
					SegmentColor = FPlane(1,0,1,0);
				else
					SegmentColor = FPlane(1,1,1,0);

				DrawInterpolationPathSegment(
					Frame,
					PointList[cur],
					PointList[cur]->Next,
					SegmentColor
				);
			}
		}
	}

	unguard;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

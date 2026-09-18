/*=============================================================================
	UnMatinee.cpp: Unreal matinee system functions
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall
=============================================================================*/

#include "EditorPrivate.h"
#include "UnRender.h"
#include "UnMatinee.h"

// Gets the total length of the renderable list of interpolation points in the level
FLOAT EDITOR_API matGetLength()
{
	guard(matGetLength);
	FLOAT Length = 0;
	for( INT x = 0 ; x < matIPCount ; x++ )
		Length += DEF_SECTION_LENGTH;
	return Length;
	unguard;
}

// Counts up the number of interpolation points in the level.
inline INT EDITOR_API matGetCount()
{
	guard(matGetCount);
	return matIPCount;
	unguard;
}

// Syncs the viewport to a specific position.
void EDITOR_API matSyncToPos( AInterpolationPoint* InIP )
{
	guard(matGetCount);

	check(InIP);

	FLOAT Pos = 0;
	for( INT x = 0 ; x < matGetCount() && matIPList[x] != InIP ; x++ )
		Pos += DEF_SECTION_LENGTH + MAT_ICON_HANDLE_WIDTH;

	GMatineeIPCamLocation.X = Pos;

	unguard;
}

void EDITOR_API matDrawBox( FSceneNode* InFrame, FPlane InColor, FVector InMin, FVector InMax )
{
	guard(matDrawBox);

	FVector FrameVerts[8] =
	{
		FVector(InMin.X+1, InMin.Y, 0),
		FVector(InMax.X, InMin.Y, 0),

		FVector(InMax.X, InMin.Y, 0),
		FVector(InMax.X, InMax.Y+1, 0),
		
		FVector(InMax.X, InMax.Y, 0),
		FVector(InMin.X, InMax.Y, 0),
		
		FVector(InMin.X, InMax.Y, 0),
		FVector(InMin.X, InMin.Y, 0),
	};

	for( INT x = 0 ; x < 4 ; x++ )
		InFrame->Viewport->RenDev->Draw2DClippedLine( InFrame, InColor, LINE_None, FrameVerts[x*2], FrameVerts[(x*2)+1] );

	unguard;
}

FVector EDITOR_API matDrawIcon( FSceneNode* InFrame, AActor* InActor, INT X, INT Y, UBOOL InLeftBox, UBOOL InRightBox )
{
	guard(matDrawIcon);

	check(InActor->Texture);

	X -= GMatineeIPCamLocation.X;

	INT UClamp = InActor->Texture->UClamp;
	INT VClamp = InActor->Texture->VClamp;
	FVector RightHandleAttach = FVector(X, Y, 0);

	// ICON
	FPlane Color = InActor->bSelected ? FPlane(.5f,.9f,.5f,0) : FPlane(1,1,1,0);
	InFrame->Viewport->Canvas->DrawIcon( InActor->Texture, X, Y, UClamp, VClamp, NULL, 1.0, Color, FPlane(0,0,0,0), 0 );

	// FRAME OUTLINE
	matDrawBox( InFrame, Color, FVector(X-1, Y-1, 0), FVector(X+UClamp+2, Y+VClamp+2, 0) );

	// HANDLES
	if( InRightBox )
	{
		InFrame->Viewport->Canvas->DrawIcon( GEditor->HandleRight, X+UClamp+3, Y+(VClamp/2), GEditor->HandleRight->UClamp, GEditor->HandleRight->VClamp, NULL, 1.0, Color, FPlane(0,0,0,0), 0 );
		RightHandleAttach = FVector(X+UClamp+MAT_ICON_HANDLE_WIDTH, Y+(VClamp/2)+2, 0);
	}
	if( InLeftBox )
		InFrame->Viewport->Canvas->DrawIcon( GEditor->HandleLeft, X-MAT_ICON_HANDLE_WIDTH-1, Y+(VClamp/2), GEditor->HandleLeft->UClamp, GEditor->HandleLeft->VClamp, NULL, 1.0, Color, FPlane(0,0,0,0), 0 );

	// LABEL
	if( InActor->IsA(AInterpolationPoint::StaticClass() ) )
	{
//		INT XL, YL;
//		InFrame->Viewport->Canvas->WrappedStrLenf( InFrame->Viewport->Canvas->SmallFont, XL, YL, TEXT("M") );
//		InFrame->Viewport->Canvas->SetClip( X, Y+VClamp+2+2, UClamp+2, YL );

		InFrame->Viewport->Canvas->CurX = X+UClamp/2+1;
		InFrame->Viewport->Canvas->CurY = Y+VClamp+2+2;
		InFrame->Viewport->Canvas->Color = FColor(192,192,192);
		InFrame->Viewport->Canvas->WrappedPrintf
		(
			InFrame->Viewport->Canvas->SmallFont,
			1,
 			TEXT("%d"),
			Cast<AInterpolationPoint>(InActor)->Position
		);
	}

	return RightHandleAttach;

	unguard;
}

int CDECL IPPosCompare(const void *A, const void *B)
{
	return (*(AInterpolationPoint**)A)->Position - (*(AInterpolationPoint**)B)->Position;
}

void EDITOR_API matRefreshIPList( ULevel* InLevel )
{
	guard(matRefreshIPList);

	matIPCount = 0;
	for( INT x = 0 ; x < InLevel->Actors.Num() ; x++ )
	{
		AActor* Actor = InLevel->Actors(x);
		if( Actor && Actor->IsA(AInterpolationPoint::StaticClass()) )
			matIPList[ matIPCount++ ] = (AInterpolationPoint*)Actor;
	}

	appQsort( &matIPList[0], matIPCount, sizeof(AInterpolationPoint*), IPPosCompare );

	unguard;
}

// Renders the matinee viewport showing the interpolation points.
void EDITOR_API matDrawMatineeIP( FSceneNode* InFrame )
{
	guard(matDrawMatineeIP);

	//
	// Make sure the camera is within limits.
	//

	if( GMatineeIPCamLocation.X < 0 ) GMatineeIPCamLocation.X = 0;
	if( GMatineeIPCamLocation.X > matGetLength() ) GMatineeIPCamLocation.X = matGetLength();

	//
	// Gather up a list of interpolation points in the level and sort them by position.
	//

	matRefreshIPList( InFrame->Viewport->Actor->GetLevel() );
	if( !matIPCount ) return;

	//
	// Render them as a string of icons/boxes, connected by variable length lines.
	//

	int XPos = 4, YPos = 4;
	FVector LineStart, LineEnd;
	for( INT x = 0 ; x < matGetCount() ; x++ )
	{
		PUSH_HIT(InFrame,HActor(matIPList[x]));
		LineStart = matDrawIcon( InFrame, matIPList[x], XPos, YPos, x>0, 1 );
		POP_HIT(InFrame);

		LineEnd = LineStart + FVector(DEF_SECTION_LENGTH,0,0);
		InFrame->Viewport->RenDev->Draw2DClippedLine( InFrame, FPlane(.5,.5,.5,0), LINE_None, LineStart, LineEnd );

		XPos += DEF_SECTION_LENGTH + MAT_ICON_HANDLE_WIDTH;
	}

	unguard;
}

void EDITOR_API matGetPlaybackPositions( UViewport* InViewport, TArray<FPosition>* InPositions )
{
	InPositions->Empty();

	// get the tag of the selected point
	FName matchTag = NAME_None;
	AActor* Actor;
	INT iActor;
	for( iActor=0; iActor<InViewport->Actor->GetLevel()->Actors.Num() ; iActor++ )
	{
		Actor = InViewport->Actor->GetLevel()->Actors(iActor);
		if( Actor && Actor->IsA(AInterpolationPoint::StaticClass()) )
		{
			matchTag = Actor->Tag;
			break;
		}
	}
	
	// generate a list of all matching points
	#define MAX_INTERP_POINTS 128
	AInterpolationPoint* PointList[MAX_INTERP_POINTS];
	INT numPoints = -1;
	for (INT i=0; i<MAX_INTERP_POINTS; i++)
		PointList[i] = NULL;
	for (iActor=0; iActor<InViewport->Actor->GetLevel()->Actors.Num(); iActor++)
	{
		Actor = InViewport->Actor->GetLevel()->Actors(iActor);
		if (Actor && Actor->IsA(AInterpolationPoint::StaticClass()) && (Actor->Tag == matchTag))
		{
			AInterpolationPoint* Dest = Cast<AInterpolationPoint>(Actor);
			if (Dest->Position < MAX_INTERP_POINTS)
			{
				PointList[Dest->Position] = Dest;
				if (Dest->Position > numPoints)
					numPoints = Dest->Position;
			}
		}
	}
	
	// link up interpolation points
	INT cur = 1;
	while( PointList[cur] )
	{
		PointList[cur]->Prev = PointList[cur-1];
		PointList[cur]->Next = PointList[cur+1];
		if ( !PointList[cur]->Next && !PointList[cur]->bEndOfPath )
			PointList[cur]->Next = PointList[0];
		cur++;
	}
	if ( PointList[0] )
	{
		PointList[0]->Next = PointList[1];
		PointList[0]->Prev = PointList[cur-1];
	}
	numPoints++;
	if (numPoints > 2)
	{
		if (numPoints < MAX_INTERP_POINTS-2)
		{
			// if this path should loop, link the points back to the beginning
			if (!PointList[numPoints-1]->bEndOfPath)
			{
				PointList[numPoints] = PointList[0];
				PointList[numPoints+1] = PointList[1];
				PointList[numPoints+2] = PointList[2];
			}
		}
		
		// draw the entire chain if something is selected
		INT cur = 1;
		
		while (PointList[cur])
		{
			FVector v1, v2;
			FRotator r1, r2;
			FCoords C;

			if ( PointList[cur-1] )
			{
				FLOAT alpha = 0.0f;
				FLOAT alphastep = 0.02f;
				FLOAT arrow = 0.0f;
				FLOAT arrowstep = 0.05f;
				while (alpha < 1.0-alphastep)
				{
					// interpolation.
					FCoords OldCoords = GMath.UnitCoords / PointList[cur]->Prev->Rotation;
					OldCoords.Origin = PointList[cur]->Prev->Location;
					FCoords FirstCoords = PointList[cur]->GetInterpolatedPosition(OldCoords, PointList[cur]->Prev->StartControlPoint, alpha, 0);	
					if (arrow <= alpha)
					{
						new((*InPositions))FPosition( FirstCoords.Origin, FirstCoords.OrthoRotation() );
						arrow += arrowstep;
					}
					
					// inc the alpha
					alpha += alphastep;
				}
			}
			
			// get the next point, but don't pass the end of the list
			cur++;
			if (cur >= MAX_INTERP_POINTS-2)
				break;
		}
	}
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

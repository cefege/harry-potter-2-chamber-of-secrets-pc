/*=============================================================================
	UnMatinee.h : Unreal matinee system
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall

	NOTES:
		This is just a convenient file for other projects to include if they
		want access to the matinee functions (i.e. UnrealEd)
=============================================================================*/

#pragma once

//
// DEFINES
//

#define DEF_SECTION_LENGTH		128
#define MAT_ICON_HANDLE_WIDTH	8

//
// VARIABLES
//

#ifdef _EDITOR_
	FVector EDITOR_API GMatineeIPCamLocation = FVector(0,0,0);
	INT EDITOR_API matIPCount = 0;
	EDITOR_API UBOOL matAlwaysShowPath = 1;
	EDITOR_API UBOOL matShowPathOrientation = 0;
#else
	FVector EDITOR_API GMatineeIPCamLocation;
	INT EDITOR_API matIPCount;
	EDITOR_API UBOOL matAlwaysShowPath;
	EDITOR_API UBOOL matShowPathOrientation;
#endif
EDITOR_API AInterpolationPoint* matIPList[1024];

//
// FUNCTIONS
//

inline INT EDITOR_API matGetCount();
FLOAT EDITOR_API matGetLength();
void EDITOR_API matSyncToPos( AInterpolationPoint* InIP );
void EDITOR_API matDrawBox( FSceneNode* InFrame, FPlane InColor, FVector InMin, FVector InMax );
FVector EDITOR_API matDrawIcon( FSceneNode* InFrame, AActor* InActor, INT X, INT Y, UBOOL InLeftBox, UBOOL InRightBox );
void EDITOR_API matDrawMatineeIP( FSceneNode* InFrame );
void EDITOR_API matGetPlaybackPositions( UViewport* InViewport, TArray<FPosition>* InPositions );

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

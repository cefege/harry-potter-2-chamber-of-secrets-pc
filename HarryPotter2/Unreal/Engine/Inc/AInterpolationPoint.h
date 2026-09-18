/*=============================================================================
	AInterpolationPoint.h.
	Copyright 2001 Epic Games, Inc. All Rights Reserved.
=============================================================================*/

FCoords GetInterpolatedPosition(FCoords OldCoords, FVector StartControlPoint, FLOAT PhysAlpha, INT PauseNum);
FRotator GetDesiredRotationAtPosition(INT PosOffset, INT PauseNum, FVector NewLocation);
FRotator GetDesiredRotationAtPause(INT PauseNum, FVector NewLocation);

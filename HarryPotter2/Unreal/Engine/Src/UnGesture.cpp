/*=============================================================================
	UnGesture.cpp

	Revision history:
		* Created Harry Potter's madd wizarding skillz
=============================================================================*/

#include "EnginePrivate.h"
#include "hp2_rs.h"

/*-----------------------------------------------------------------------------
	UGesture
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UGesture)

void UGesture::execCompareGesturePoint( FFrame& Stack, RESULT_DECL )
{
	P_GET_STRUCT(FVector, InMousePoint);
	P_GET_FLOAT(fAccuracy);
	P_FINISH;

	// fAccuracy is consumed only to advance the script stack: the original
	// InternalPointCompare never read it.
	(void)fAccuracy;

	*(float*)Result = hp2_rs_engine_gesture_compare_gesture_point( this, &InMousePoint );
}

// The actual exported funtion that does the comparison.  Takes in an array of mouse points
//	and a array of the segments expected for this Gesture, and outputs a floating point
//	value that describes the accuracy of what was drawn.

void UGesture::execCompareGesture( FFrame& Stack, RESULT_DECL )
{
	P_GET_STRUCT(TArray<FVector>, InMousePoints);
	P_GET_FLOAT(fAccuracy);
	P_FINISH;

	*(float*)Result = hp2_rs_engine_gesture_compare_gesture( this, &InMousePoints, fAccuracy );
}





/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/


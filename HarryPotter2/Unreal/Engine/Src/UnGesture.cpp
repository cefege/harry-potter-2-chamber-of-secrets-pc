/*=============================================================================
	UnGesture.cpp

	Revision history:
		* Created Harry Potter's madd wizarding skillz
=============================================================================*/

#include "EnginePrivate.h"

/*-----------------------------------------------------------------------------
	UGesture
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UGesture)

const int MAX_MOUSE_POINTS = 1024;
const int SEG_SECTIONS = 32;

class CMousePoint
{
public:
	float	xPos;
	float	yPos;
};

// Figure out what segment a pair of floats is in...
short PlaceSegment(float fXVal, float fYVal)
{
	if(fXVal <= 0.33)
	{
		if(fYVal <= 0.33)
			return 1;
		else if(fYVal <= 0.66)
			return 4;
		else
			return 7;
	}
	else if(fXVal <= 0.66)
	{
		if(fYVal <= 0.33)
			return 2;
		else if(fYVal <= 0.66)
			return 5;
		else
			return 8;
	}
	else
	{
		if(fYVal <= 0.33)
			return 3;
		else if(fYVal <= 0.66)
			return 6;
		else
			return 9;
	}
	return 0;
}

float InternalGestureCompare(CMousePoint MouseArray[], int nCount, int nGestureSegments[])
{ 
	short nDrawnSectors[SEG_SECTIONS * 3];
	int i, j;
	short g_nLevel = 1;				// This has to be some global or somesuch about what Level of Spell learning
									//		we are currently testung for...
									// **** SCOTT PETER **** Fix This

	// Brilliant comparison algorithm here...

	// Figure out what segments the mouse points are in,
	//	calculating the order on the fly

	for(j=0;j < (SEG_SECTIONS * 3);j++)
		nDrawnSectors[j] = 0;

	j=0;
	int nCurrentSegment = 0, nLastSegment = 0;

	for(i=0; i<nCount; i++)
	{
		nCurrentSegment = PlaceSegment(MouseArray[i].xPos,MouseArray[i].yPos);

		if(nCurrentSegment != nLastSegment)
		{
			nLastSegment = nCurrentSegment;
			nDrawnSectors[j] = nCurrentSegment;
			j++;
			if(j >= (SEG_SECTIONS * 3))
			{
				return 0.0;
			}
		}		
	}


	// Adjust for Level here by copying into a temp buffer...

	unsigned short nAdjustedGestureArray[SEG_SECTIONS * 3];
	i=0;
	j=0;

	while(nGestureSegments[i] != 0)
	{
		nAdjustedGestureArray[j] = nGestureSegments[i];
		i++;
		j++;
	}
	if(g_nLevel >= 2)
	{
		i = i -2;
		
		while(i >= 0)
		{
			nAdjustedGestureArray[j] = nGestureSegments[i];
			i--;
			j++;
		}

		if(g_nLevel >= 3)
		{
			i=1;
			while(nGestureSegments[i] != 0)
			{
				nAdjustedGestureArray[j] = nGestureSegments[i];
				i++;
				j++;
			}
		}
	}
	nAdjustedGestureArray[j] = 0; // "Terminate" the array with a '0'



	// Match Up Points with Gesture Order - Only works on Level 1 so far, because we're not checking m_nLevel
	i=0;
	j=0;
	int nNumCorrectSpots = 0;
	int nNumErrors = 0;
	float fCorrect = 0.0;
	while(nAdjustedGestureArray[i]!= 0)
	{
		if(nDrawnSectors[j] == 0)
			break;
		
		if(nDrawnSectors[j] == nAdjustedGestureArray[i])
		{
			j++;
			i++;
		}
		else
		{
			// check to see if one out either way
			if(nDrawnSectors[j+1] == nAdjustedGestureArray[i])
			{
				j += 2;
				i ++;
				nNumErrors++;
			}
			else if(nDrawnSectors[j] == nAdjustedGestureArray[i + 1])
			{
				i += 2;
				j ++;
				nNumErrors++;
			}
			else if(nDrawnSectors[j + 1] == nAdjustedGestureArray[i + 1])
			{
				i += 2;
				j += 2;
				nNumErrors++;
			}
			else
			{
				while(nDrawnSectors[j] != 0)		// Slide!
				{
					j++;
					nNumErrors++;
					if(nDrawnSectors[j] == nAdjustedGestureArray[i])
						break;
				}
			}
		}
	}

	// Account for unmatched overflow on the drawing
	while(nDrawnSectors[j] != 0)
	{
		j++;
		nNumErrors++;
	}

	while(nAdjustedGestureArray[i] != 0)
	{
		i++;
		nNumErrors++;
	}

	// Get Commonality Statistics
	nNumCorrectSpots = j - nNumErrors;
	if(nNumCorrectSpots < 0)
		nNumCorrectSpots = 0;
	fCorrect = (float) nNumCorrectSpots / (float) j;

	if(fCorrect < 0.0)
		fCorrect = 0.0;

	// Results
	return fCorrect;
}


float InternalGestureCompare2(CMousePoint MouseArray[], int iNumMousePoints, CMousePoint nGesturePoints[], int iNumGesturePoints, float fAccuracy)
{ 
	int	i, j;
	float	dMinVal, dVal = 0;
	float	dx, dy;
	float	fCorrect;
	int		iNumErrors;
	int		iError;
	int		iTotal;

	iNumErrors = 0;
	iTotal = 0;
	for (i = 0; i < iNumGesturePoints; i ++)
	{
		dMinVal = 1.0;
		for (j = 0; j < iNumMousePoints; j ++)
		{
			dx = MouseArray[j].xPos - nGesturePoints[i].xPos;
			dy = MouseArray[j].yPos - nGesturePoints[i].yPos;

			dVal = appSqrt(dx * dx + dy * dy);

			if (dVal < dMinVal)
			{
				dMinVal = dVal;
			}
		}

		if (dMinVal > fAccuracy)
		{
			iError = 1 + (dMinVal / (fAccuracy * 5));
			if (iError > 2)
			{
				iError = 2;
			}
			iNumErrors += iError;
			iTotal += iError;
		}
		else
		{
			iTotal += 2;
		}
	}

	for (i = 0; i < iNumMousePoints; i ++)
	{
		dMinVal = 1.0;
		for (j = 0; j < iNumGesturePoints; j ++)
		{
			dx = MouseArray[i].xPos - nGesturePoints[j].xPos;
			dy = MouseArray[i].yPos - nGesturePoints[j].yPos;

			dVal = appSqrt(dx * dx + dy * dy);

			if (dVal < dMinVal)
			{
				dMinVal = dVal;
			}
		}
		if (dMinVal > fAccuracy)
		{
			iError = dMinVal / fAccuracy;

			if (iError > 3)
			{
				iError = 3;
			}
			iNumErrors += iError;
			iTotal += iError;
		}
		else
		{
//			iTotal ++;
		}
	}

	// Make sure that there is a minimum number of points
	if (iNumMousePoints < 10)
	{
		iTotal += 500;
		iNumErrors += 500;
	}

	fCorrect = (1 - (double)iNumErrors / (double)(iTotal));

	if(fCorrect < 0.0)
	{
		fCorrect = 0.0;
	}
	else if (fCorrect > 1.0)
	{
		fCorrect = 1.0;
	}

	// Results
	return fCorrect;
}

// Checks to see if a particular point is within range

float InternalPointCompare(CMousePoint MousePoint, CMousePoint nGesturePoints[], int iNumGesturePoints, float fAccuracy)
{ 
	int	i;
	float	dMinVal, dVal = 0;
	float	dx, dy;

	dMinVal = 1.0;

	for (i = 0; i < iNumGesturePoints; i ++)
	{

		dx = MousePoint.xPos - nGesturePoints[i].xPos;
		dy = MousePoint.yPos - nGesturePoints[i].yPos;

		dVal = appSqrt(dx * dx + dy * dy);

		if (dVal < dMinVal)
		{
			dMinVal = dVal;
		}
	}

	// Results
	return dMinVal;
}

void UGesture::execCompareGesturePoint( FFrame& Stack, RESULT_DECL )
{
	P_GET_STRUCT(FVector, InMousePoint);
	P_GET_FLOAT(fAccuracy);
	P_FINISH;

	int			nNumTargetMousePoints = Points.Num();
	CMousePoint MousePoint;
	CMousePoint	TargetMousePointArray[MAX_MOUSE_POINTS];
	int			k;

	MousePoint.xPos = InMousePoint.X;
	MousePoint.yPos = InMousePoint.Y;

	if(nNumTargetMousePoints > MAX_MOUSE_POINTS)
		nNumTargetMousePoints = MAX_MOUSE_POINTS;

	// We need to do this before the call is made

	// Store the target points, add resolution to the points themselves
	// for the comparison/recognition code

	for(k = 0; k < nNumTargetMousePoints;k ++)
	{
		TargetMousePointArray[8 * k].xPos = Points(k).X;
		TargetMousePointArray[8 * k].yPos = Points(k).Y;

		if (k + 1 < nNumTargetMousePoints)
		{
			// add some extra points to increase resolution
			TargetMousePointArray[8 * k + 1].xPos = ((Points(k + 1).X - Points(k).X) / 8) + Points(k).X;
			TargetMousePointArray[8 * k + 1].yPos = ((Points(k + 1).Y - Points(k).Y) / 8) + Points(k).Y;
			TargetMousePointArray[8 * k + 2].xPos = ((Points(k + 1).X - Points(k).X) * 2.0 / 8) + Points(k).X;
			TargetMousePointArray[8 * k + 2].yPos = ((Points(k + 1).Y - Points(k).Y) * 2.0 / 8) + Points(k).Y;
			TargetMousePointArray[8 * k + 3].xPos = ((Points(k + 1).X - Points(k).X) * 3.0 / 8.0) + Points(k).X;
			TargetMousePointArray[8 * k + 3].yPos = ((Points(k + 1).Y - Points(k).Y) * 3.0 / 8.0) + Points(k).Y;
			TargetMousePointArray[8 * k + 4].xPos = ((Points(k + 1).X - Points(k).X) * 4.0 / 8) + Points(k).X;
			TargetMousePointArray[8 * k + 4].yPos = ((Points(k + 1).Y - Points(k).Y) * 4.0 / 8) + Points(k).Y;
			TargetMousePointArray[8 * k + 5].xPos = ((Points(k + 1).X - Points(k).X) * 5.0 / 8) + Points(k).X;
			TargetMousePointArray[8 * k + 5].yPos = ((Points(k + 1).Y - Points(k).Y) * 5.0 / 8) + Points(k).Y;
			TargetMousePointArray[8 * k + 6].xPos = ((Points(k + 1).X - Points(k).X) * 6.0 / 8.0) + Points(k).X;
			TargetMousePointArray[8 * k + 6].yPos = ((Points(k + 1).Y - Points(k).Y) * 6.0 / 8.0) + Points(k).Y;
			TargetMousePointArray[8 * k + 7].xPos = ((Points(k + 1).X - Points(k).X) * 7.0 / 8.0) + Points(k).X;
			TargetMousePointArray[8 * k + 7].yPos = ((Points(k + 1).Y - Points(k).Y) * 7.0 / 8.0) + Points(k).Y;
		}
	}

	nNumTargetMousePoints = (nNumTargetMousePoints * 8) - 7;

	*(float*)Result = InternalPointCompare(MousePoint, TargetMousePointArray, nNumTargetMousePoints, fAccuracy);
}

// The actual exported funtion that does the comparison.  Takes in an array of mouse points
//	and a array of the segments expected for this Gesture, and outputs a floating point
//	value that describes the accuracy of what was drawn.

void UGesture::execCompareGesture( FFrame& Stack, RESULT_DECL )
{
	P_GET_STRUCT(TArray<FVector>, InMousePoints);
	P_GET_FLOAT(fAccuracy);
	P_FINISH;

	int			nNumMousePoints = InMousePoints.Num();
	int			nNumTargetMousePoints = Points.Num();
	CMousePoint MousePointArray[MAX_MOUSE_POINTS];
	CMousePoint	TargetMousePointArray[MAX_MOUSE_POINTS];
	int			i, k;
	int			Number;
	bool		bUsedBefore;

//	float		StartY;

	if(nNumMousePoints > MAX_MOUSE_POINTS)
		nNumMousePoints = MAX_MOUSE_POINTS;

	Number = 0;

	for(k = 0; k < nNumMousePoints; k ++)
	{
		if (InMousePoints(k).Z != -1)	// Is the point used?
		{
			// make sure this point hasn't been used before

			bUsedBefore = false;
			for(i = 0; i < Number && !bUsedBefore; i ++)
			{
				if (Abs(MousePointArray[i].xPos - InMousePoints(k).X) < 0.001
						&& Abs(MousePointArray[i].yPos - InMousePoints(k).Y) < 0.001)
				{
					bUsedBefore = true;
				}
			}

			if (!bUsedBefore)
			{
				MousePointArray[Number].xPos = InMousePoints(k).X;
				MousePointArray[Number].yPos = InMousePoints(k).Y;
				Number ++;
			}
		}
	}

	nNumMousePoints = Number;

	if(nNumTargetMousePoints > MAX_MOUSE_POINTS)
		nNumTargetMousePoints = MAX_MOUSE_POINTS;

	// Store the target points, add resolution to the points themselves
	// for the comparison/recognition code

	for(k = 0; k < nNumTargetMousePoints;k ++)
	{
		TargetMousePointArray[8 * k].xPos = Points(k).X;
		TargetMousePointArray[8 * k].yPos = Points(k).Y;

		if (k + 1 < nNumTargetMousePoints)
		{
			// add some extra points to increase resolution
			TargetMousePointArray[8 * k + 1].xPos = ((Points(k + 1).X - Points(k).X) * 1.0 / 8) + Points(k).X;
			TargetMousePointArray[8 * k + 1].yPos = ((Points(k + 1).Y - Points(k).Y) * 1.0 / 8) + Points(k).Y;
			TargetMousePointArray[8 * k + 2].xPos = ((Points(k + 1).X - Points(k).X) * 2.0 / 8) + Points(k).X;
			TargetMousePointArray[8 * k + 2].yPos = ((Points(k + 1).Y - Points(k).Y) * 2.0 / 8) + Points(k).Y;
			TargetMousePointArray[8 * k + 3].xPos = ((Points(k + 1).X - Points(k).X) * 3.0 / 8.0) + Points(k).X;
			TargetMousePointArray[8 * k + 3].yPos = ((Points(k + 1).Y - Points(k).Y) * 3.0 / 8.0) + Points(k).Y;
			TargetMousePointArray[8 * k + 4].xPos = ((Points(k + 1).X - Points(k).X) * 4.0 / 8.0) + Points(k).X;
			TargetMousePointArray[8 * k + 4].yPos = ((Points(k + 1).Y - Points(k).Y) * 4.0 / 8.0) + Points(k).Y;
			TargetMousePointArray[8 * k + 5].xPos = ((Points(k + 1).X - Points(k).X) * 5.0 / 8) + Points(k).X;
			TargetMousePointArray[8 * k + 5].yPos = ((Points(k + 1).Y - Points(k).Y) * 5.0 / 8) + Points(k).Y;
			TargetMousePointArray[8 * k + 6].xPos = ((Points(k + 1).X - Points(k).X) * 3.0 / 4.0) + Points(k).X;
			TargetMousePointArray[8 * k + 6].yPos = ((Points(k + 1).Y - Points(k).Y) * 3.0 / 4.0) + Points(k).Y;
			TargetMousePointArray[8 * k + 7].xPos = ((Points(k + 1).X - Points(k).X) * 7.0 / 8.0) + Points(k).X;
			TargetMousePointArray[8 * k + 7].yPos = ((Points(k + 1).Y - Points(k).Y) * 7.0 / 8.0) + Points(k).Y;
		}
	}

	nNumTargetMousePoints = (nNumTargetMousePoints * 8) - 7;

	// This is how we return values in this crappy language.
//	*(float*)Result = InternalGestureCompare(MousePointArray, nNumMousePoints, Segments.GetData());
	*(float*)Result = InternalGestureCompare2(MousePointArray, nNumMousePoints, TargetMousePointArray, nNumTargetMousePoints, fAccuracy);
}





/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/


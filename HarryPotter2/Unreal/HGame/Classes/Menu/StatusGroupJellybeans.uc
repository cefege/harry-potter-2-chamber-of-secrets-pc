//===============================================================================
//  [StatusGroupJellybeans] 
//===============================================================================

class StatusGroupJellybeans extends StatusGroup;

//-----------------------------------------------------------------------------------
//  StatusGroup override functions
//-----------------------------------------------------------------------------------

// Normal display position.
function GetGroupFinalXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, 
						 out int nOutX, out int nOutY)
{
	GetGroupFinalXY_2(bMenuMode, Canvas.SizeX, Canvas.SizeY, nIconWidth, nIconHeight, 
		              nOutX, nOutY);
}

// Normal display position (but have canvas size instead of actual canvas as params).
function GetGroupFinalXY_2(bool bMenuMode, int nCanvasSizeX, int nCanvasSizeY, 
						   int nIconWidth, int nIconHeight, 
		                   out int nOutX, out int nOutY)
{
	local float fScaleFactor;

	fScaleFactor = GetScaleFactor(nCanvasSizeX);

	nOutX = fScaleFactor * 72;
	nOutY = fScaleFactor * 5;
}

// Fly in from this location.  (May not end up using.  If GameEffectType == ET_Fly,
// this function will never get called.)
function GetGroupFlyOriginXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, 
							 out int nX, out int nY)
{
	local int nFinalX, nFinalY;

	local float fScaleFactor;

	fScaleFactor = GetScaleFactor(Canvas.SizeX);

	// fly in will be from top
	GetGroupFinalXY(bMenuMode, Canvas, nIconWidth, nIconHeight, nFinalX, nFinalY);
	nX = nFinalX;
	nY = -(fScaleFactor * nIconHeight);

}

defaultproperties
{
	// Override group properties
	fTotalEffectInTime=0.3
	fTotalHoldTime=3.0
	fTotalEffectOutTime=0.3
	fCurrEffectInTime=0.0
	GameEffectType=ET_Fade
}


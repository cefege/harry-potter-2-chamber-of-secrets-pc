//===============================================================================
//  [StatusGroupHealth] 
//===============================================================================

class StatusGroupHealth extends StatusGroup;

//-----------------------------------------------------------------------------------
//  StatusGroup override functions
//-----------------------------------------------------------------------------------

// Normal display position
function GetGroupFinalXY_2(bool bMenuMode, int nCanvasSizeX, int nCanvasSizeY, 
						   int nIconWidth, int nIconHeight, 
		                   out int nOutX, out int nOutY)
{
	nOutX = 0;
	nOutY = 0;
}

// Fly in from this location.  (May not end up using.  If GameeEffectType == ET_Fly, 
// this function will never get called.)
function GetGroupFlyOriginXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, 
							 out int nOutX, out int nOutY)
{
	local int nFinalX, nFinalY;

	// Fly in will be from top
	GetGroupFinalXY(bMenuMode, Canvas, nIconWidth, nIconHeight, nFinalX, nFinalY);
	nOutX = nFinalX;
	nOutY = -nIconHeight;
}

defaultproperties
{
	// Override group properties
	fTotalEffectInTime=0.5
	fTotalHoldTime=3.0
	fTotalEffectOutTime=0.2
	fCurrEffectInTime=0.0
	GameEffectType=ET_Permanent
}


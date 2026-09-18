//===============================================================================
//  [StatusGroupPolyIngr 
//===============================================================================

class StatusGroupPolyIngr extends StatusGroup;

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

	nOutX = fScaleFactor * 500;
	nOutY = fScaleFactor * 5;
}


// Fly in from this location.  (May not end up using.  If GameEffectType == ET_Fly, 
// this function will never get called.)
function GetGroupFlyOriginXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, 
							 out int nOutX, out int nOutY)
{
	local int nFinalX, nFinalY;
	local float fScaleFactor;

	fScaleFactor = GetScaleFactor(Canvas.SizeX);

	// Fly in will be from top
	GetGroupFinalXY(bMenuMode, Canvas, nIconWidth, nIconHeight, nFinalX, nFinalY);
	nOutX = nFinalX;
	nOutY = -(nIconHeight * fScaleFactor);
}

defaultproperties
{
	// Override group properties
	bDisplayHorizontally=true
	GameEffectType=ET_PERMANENT
	MenuProps=Menu_IfCurrentlyHaveAny
}


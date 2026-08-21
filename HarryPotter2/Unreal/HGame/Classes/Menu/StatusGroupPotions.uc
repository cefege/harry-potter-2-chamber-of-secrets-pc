//===============================================================================
//  [StatusGroupPotions]
//===============================================================================

class StatusGroupPotions extends StatusGroup;

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
	local int   nObjectiveTop;
    local int   nCutBorderHeight;

	fScaleFactor = GetScaleFactor(nCanvasSizeX);

	nOutX = nCanvasSizeX - nIconWidth - (fScaleFactor * 5);

	// Potion ingredients are normally at bottom of the screen, but if we're at the
	// In Game menu and there is objective text, we want the icons to be displayed
	// above the objective text.
	if (bMenuMode && smParent.playerHarry.HaveObjectiveText())
	{
		// Wow, this is a long line, but it has the info we need.
		nObjectiveTop = hpconsole(smParent.playerHarry.player.console).MenuBook.InGamePage.GetObjectiveAreaTop(nCanvasSizeX, nCanvasSizeY);
		nOutY = nObjectiveTop - (nIconHeight * fScaleFactor) - (5 * fScaleFactor);
	}
	else
    {
        nCutBorderHeight = HPHud(smParent.playerHarry.myHud).managerCutScene.GetMaxBorderHeightFromCanvasHeight(nCanvasSizeY);
        nOutY = nCanvasSizeY - (nIconHeight * fScaleFactor) - (nCutBorderHeight);
    }
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
	nOutY = Canvas.SizeY + (nIconHeight * fScaleFactor);
}


defaultproperties
{
	// Override group properties
	bDisplayHorizontally=true
	fTotalEffectInTime=0.5
	fTotalHoldTime=3.0
	fTotalEffectOutTime=0.2
	fCurrEffectInTime=0.0
	GameEffectType=ET_FADE
	MenuProps=Menu_IfEverHadAny
}


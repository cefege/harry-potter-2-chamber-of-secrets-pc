//===============================================================================
//  [HudItemManager] 
//===============================================================================

class HudItemManager extends Actor;

const BASE_RESOLUTION_X     = 800.0;   // Icons are made to look good in
                                       // 800x600.  We'll scale when drawing icons
                                       // in other resolutions.

//-----------------------------------------------------------------------------------
//  Hud interface
//-----------------------------------------------------------------------------------

// RenderHudItemManager.  Called by Hud every render cycle.  
function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
{
}

// Get draw scale factor for current screen resolution.
function float GetScaleFactor(Canvas canvas)
{
	return (canvas.SizeX / BASE_RESOLUTION_X);
}

defaultproperties
{
	DrawType=DT_None
}


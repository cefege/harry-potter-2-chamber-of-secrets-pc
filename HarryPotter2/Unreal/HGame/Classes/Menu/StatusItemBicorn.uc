//===============================================================================
//  [StatusItemBicorn] 
//===============================================================================

class StatusItemBicorn extends StatusItem;

defaultproperties
{
	strHudIcon="HP_Menu.Hud.Bicorn"
	bDisplayCount=false
	bDisplayMaxCount=false
	nActualIconW=52			// Icon is 128x128, but image is only 40x62
	nActualIconH=58         
	strToolTipId="InGameMenu_0005"
    nMaxCount=1
    bDisplayWhenCountZero=false
}


//===============================================================================
//  [StatusItemStars] 
//===============================================================================

class StatusItemStars extends StatusItem;

defaultproperties
{
	strHudIcon="HP_Menu.Hud.StarCounter"
	bDisplayCount=true
	bDisplayMaxCount=true
	nCountMiddleX=26
	nCountMiddleY=52
	nActualIconW=60		
	nActualIconH=64         
	strToolTipId="InGameMenu_0018"
    bTravelStatus=false     // star counts don't travel between levels
    CountColor=CountColor_NearWhite
}


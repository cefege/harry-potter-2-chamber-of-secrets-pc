//===============================================================================
//  [StatusItemSilverCards] 
//===============================================================================

class StatusItemSilverCards extends StatusItemWizardCards;

// Let parent update count and then add a lock if a set just completed.
function UpdateCount()
{
    local int nOldCount;

    nOldCount = nCount;

    Super.UpdateCount();

    if (nOldCount != nCount)
    {
    	// If completed a set of 10 silver cards, a new lock status item will appear.
	    switch (nCount)
	    {
	    case (10) :
		    sgParent.smParent.GetStatusGroup(class'StatusGroupLocks').IncrementCount(class'StatusItemLock1', 1);
		    break;
	    case (20) :
		    sgParent.smParent.GetStatusGroup(class'StatusGroupLocks').IncrementCount(class'StatusItemLock2', 1);
		    break;
	    case (30) :
		    sgParent.smParent.GetStatusGroup(class'StatusGroupLocks').IncrementCount(class'StatusItemLock3', 1);
		    break;
	    case (40) :
		    sgParent.smParent.GetStatusGroup(class'StatusGroupLocks').IncrementCount(class'StatusItemLock4', 1);
		    break;
	    default:
		    break;
        }
	}
}

defaultproperties
{
	strHudIcon="HP_Menu.Hud.CardFolio"
	bDisplayCount=false
	bDisplayMaxCount=false
	bMenuModeOnly=false
	nMaxCount=40
}


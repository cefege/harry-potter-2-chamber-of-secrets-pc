//===============================================================================
//  BeanRoomTimerManager
//  
//  The BeanRoomTimerManger is intended to be used in the bean reward room
//  following the Housepoints Ceremony.  If Harry wins the Housepoint Ceremony, 
//  he will be given access to a level with a bunch of jellybeans.  The time 
//  Harry spends in this level is related to how many points Gryffindor is ahead
//  of the other houses.
//
//  The beanroom timer can be started in the same methods that its parent
//  CountdownTimerManager, but the CountdownTimerManager duration will 
//  be ignored and the duration will be automatically set based on Gryffindor 
//  housepoints.
//
//===============================================================================

class BeanRoomTimerManager extends CountdownTimerManager;

//-----------------------------------------------------------------------------------
//  Functions
//-----------------------------------------------------------------------------------

// Get timer duration.  Called by CountdownTimerManager when the timer is
// started.
function float GetTimerDuration()
{	
	local Harry       playerHarry;		// Good 'ol Harry
	local StatusGroup sgHousePts;       // Housepoints status group
	local int         nGryffPts;        // Curent Gryffindor housepoints
	local int         nRavenPts;		// Curent Ravenclaw housepoints
	local int         nHuffPts;         // Curent Hufflepuff housepoints
	local int         nSlythPts;        // Curent Slytherin housepoints
	local int         nBestNonGryffPts;	// Highest # of points by non Gryff house
	local int         nGryffAheadBy;    // Gryff ahead of other houses by this much

    // Normally bean room timer sets the time based on the number of housepoints
    // that Gryffindor is ahead.  For tuning purposes, the timer duration can
    // be set on the timer object that gets placed in the world and we'll use
    // that instead.
	if (fDuration != 0)
		return (fDuration);

	// Regular mode. Duration based on housepoints.

	// Get the Housepoints status group
	playerHarry = Harry(Level.PlayerHarryActor);
	sgHousePts  = playerHarry.managerStatus.GetStatusGroup
		                                    (class'StatusGroupHousepoints');

	// Log an error if couldn't get the housepoints object
	if (sgHousePts == None)
		log("ERROR: Couldn't get StatusGroupHousepoints object");

	// Get housepoint counts for all houses.
	nGryffPts = sgHousePts.GetStatusItem(class'StatusItemGryffindorPts').nCount;
	nRavenPts = sgHousePts.GetStatusItem(class'StatusItemRavenclawPts').nCount;
	nHuffPts  = sgHousePts.GetStatusItem(class'StatusItemHufflepuffPts').nCount;
	nSlythPts = sgHousePts.GetStatusItem(class'StatusItemSlytherinPts').nCount;

	// Get highest number of housepoints between Ravenclaw, Hufflepuff, and Slytherin
	nBestNonGryffPts = Max(nRavenPts, nHuffPts);
	nBestNonGryffPts = Max(nBestNonGryffPts, nSlythPts);

	// If this timer is being used, Gryffindor should be tied or ahead in points.
	// If this is not the case, log an error and make the points equal so we
	// can go on.
	if (nGryffPts < nBestNonGryffPts)
	{
		log("ERROR: Gryffindor not ahead-- should not be in bonus level");
		nGryffPts = nBestNonGryffPts;
	}

	// Calculate how many points Gryffindor is ahead by.
	nGryffAheadBy = nGryffPts - nBestNonGryffPts;

	// Based on the number of points Gryffindor is ahead, figure out how long
	// the player gets to be in the bonus room.
	//
	// @@@ These numbers are just placeholders.  Not sure what the real numbers 
	// will be yet or if we'll have a formula or hardcoded values.
	if (nGryffAheadBy > 1500)
		fDuration = 400;
	else if (nGryffAheadBy > 1200)
		fDuration = 370;
	else if (nGryffAheadBy > 1000)
		fDuration = 350;
	else if (nGryffAheadBy > 800)
		fDuration = 340;
	else if (nGryffAheadBy > 600)
		fDuration = 330;
	else if (nGryffAheadBy > 500)
		fDuration = 320;
	else if (nGryffAheadBy > 450)
		fDuration = 310;
	else if (nGryffAheadBy > 380)
		fDuration = 300;
	else if (nGryffAheadBy > 330)
		fDuration = 280;		
	else if (nGryffAheadBy > 300)
		fDuration = 250;
	else if (nGryffAheadBy > 280)
		fDuration = 220;
	else if (nGryffAheadBy > 250)
		fDuration = 190;
	else if (nGryffAheadBy > 220)
		fDuration = 170;
	else if (nGryffAheadBy > 190)
		fDuration = 150;
	else if (nGryffAheadBy > 175)
		fDuration = 140;
	else if (nGryffAheadBy > 185)
		fDuration = 130;
	else if (nGryffAheadBy > 165)
		fDuration = 120;
	else if (nGryffAheadBy > 150)
		fDuration = 110;
	else if (nGryffAheadBy > 120)
		fDuration = 100;
	else if (nGryffAheadBy > 100)
		fDuration = 90;
	else if (nGryffAheadBy > 85)
		fDuration = 80;
	else if (nGryffAheadBy > 70)
		fDuration = 70;
	else if (nGryffAheadBy > 55)
		fDuration = 60;
	else if (nGryffAheadBy > 40)
		fDuration = 50;
	else if (nGryffAheadBy > 30)
		fDuration = 40;
	else if (nGryffAheadBy > 20)
		fDuration = 35;
	else if (nGryffAheadBy > 15)
		fDuration = 30;
	else if (nGryffAheadBy > 10)
		fDuration = 25;
	else if (nGryffAheadBy > 5)
		fDuration = 20;
	else
		fDuration = 10;
	
	return (fDuration);
}

defaultproperties
{
	DrawType=DT_Sprite							// For editor drawing
	bHidden=true                                // Displays in editor, but not game
	CutName="BeanRoomTimerManager"
}


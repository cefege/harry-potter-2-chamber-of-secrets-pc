//===============================================================================
//  [StatusGroupHousePoints] 
//===============================================================================

class StatusGroupHousePoints extends StatusGroup;

const strCUT_NAME = "Housepoints";

event PostBeginPlay()
{
    CutName = strCUT_NAME;
}

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

	nOutX = nCanvasSizeX - (fScaleFactor * nIconWidth) - (fScaleFactor * 5);
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

// Award housespoints based on the game transition passed in.
function TransitionUpdateHousepoints(string strTransitionLetter)
{
	local int        nAddRavenclaw;
	local int        nAddHufflepuff;
	local int        nAddSlytherin;
    local int        nAddGryffindor;

    if (strTransitionLetter ~= "A")
    {
        nAddRavenclaw     = RandRange( 0, 20);
        nAddHufflepuff    = RandRange( 0, 20);
        nAddSlytherin     = RandRange(10, 20);
        nAddGryffindor    = RandRange(21, 30);  // Transition A is only transition where Gryff will
    }                                           // get points, too.  They should always win 1st ceremony.
    else if (strTransitionLetter ~= "B")
    {
        nAddRavenclaw     = RandRange(27, 40);
        nAddHufflepuff    = RandRange(27, 40);
        nAddSlytherin     = RandRange(35, 40);
        nAddGryffindor    = 0;
    }
    else if (strTransitionLetter ~= "C")
    {
        nAddRavenclaw     = RandRange(27, 40);
        nAddHufflepuff    = RandRange(27, 40);
        nAddSlytherin     = RandRange(35, 40);
        nAddGryffindor    = 0;
    }
    else if (strTransitionLetter ~= "D")
    {
        nAddRavenclaw     = RandRange(110, 160);
        nAddHufflepuff    = RandRange(110, 160);
        nAddSlytherin     = RandRange(140, 160);
        nAddGryffindor    = 0;
    }
    else if (strTransitionLetter ~= "E")
    {
        nAddRavenclaw     = RandRange(30, 60);
        nAddHufflepuff    = RandRange(30, 60);
        nAddSlytherin     = RandRange(45, 60);
        nAddGryffindor    = 0;
    }
    else if (strTransitionLetter ~= "F")
    {
        nAddRavenclaw     = RandRange(260, 340);
        nAddHufflepuff    = RandRange(260, 340);
        nAddSlytherin     = RandRange(310, 340);
        nAddGryffindor    = 0;
    }
    else
        smParent.playerHarry.ClientMessage("ERROR: Invalid transition letter " $strTransitionLetter);
    
	// Get housepoint status items
	GetStatusItem(class'StatusItemRavenclawPts').IncrementCount(nAddRavenclaw);
	GetStatusItem(class'StatusItemHufflepuffPts').IncrementCount(nAddHufflepuff);
	GetStatusItem(class'StatusItemSlytherinPts').IncrementCount(nAddSlytherin);
    GetStatusItem(class'StatusItemGryffindorPts').IncrementCount(nAddGryffindor);

    // Adjust some points if there is a tie.
    ResolveTies();
}

// This function should be called the after a Quidditch match finishes
// for the first time.
function QuidditchUpdateHousepoints(int nMatch)
{
	local int        nAddRavenclaw;
	local int        nAddHufflepuff;
	local int        nAddSlytherin;

    switch (nMatch)
    {
    case (0):
        nAddRavenclaw     = RandRange( 0, 10);
        nAddHufflepuff    = RandRange( 0, 10);
        nAddSlytherin     = RandRange( 5, 10);
        break;
    case (1):
        nAddRavenclaw     = RandRange(10, 50);
        nAddHufflepuff    = RandRange(10, 50);
        nAddSlytherin     = RandRange(25, 50);
        break;
    case (2):
        nAddRavenclaw     = RandRange(50, 100);
        nAddHufflepuff    = RandRange(50, 100);
        nAddSlytherin     = RandRange(75, 100);
        break;
    case (3):
        nAddRavenclaw     = RandRange(100, 150);
        nAddHufflepuff    = RandRange(100, 150);
        nAddSlytherin     = RandRange(125, 150);
        break;
    case (4):
        nAddRavenclaw     = RandRange(150, 200);
        nAddHufflepuff    = RandRange(150, 200);
        nAddSlytherin     = RandRange(175, 200);
        break;
    case (5):
        nAddRavenclaw     = RandRange(200, 250);
        nAddHufflepuff    = RandRange(200, 250);
        nAddSlytherin     = RandRange(225, 250);
        break;
    default:
        smParent.playerHarry.ClientMessage("ERROR: Invalid match number " $nMatch);
        break;
    }
    
	// Update housepoints
	GetStatusItem(class'StatusItemRavenclawPts').IncrementCount(nAddRavenclaw);
	GetStatusItem(class'StatusItemHufflepuffPts').IncrementCount(nAddHufflepuff);
	GetStatusItem(class'StatusItemSlytherinPts').IncrementCount(nAddSlytherin);

    // Adjust some points if there is a tie.
    ResolveTies();
}

function bool CutCommand(string command, optional string cue, optional bool bFastFlag)
{
	local string  sActualCommand;
	local string  sCutName;
	local actor   a;
    local string  strTransitionLetter;
	
	sActualCommand = ParseDelimitedString( command, " ", 1, false );


	if( sActualCommand ~= "Capture" )
	{
		return (true);
	}
	else 
	if( sActualCommand ~= "Release" )
	{
		return (true);
	}
    else if (sActualCommand ~= "UpdateHousepoints")
    {
        strTransitionLetter = ParseDelimitedString( command, " ", 2, false);
        TransitionUpdateHousepoints(strTransitionLetter);
        CutCue(cue);
        return (true);
    }
	else
		return (false);
}

// "if"  called from Cutscene script.
function bool CutQuestion(string question)
{
	CutErrorString="";	//clear error string.

	if (question ~= "IsGryffindorAhead")
		return (IsHouseAhead(class'StatusItemGryffindorPts'));
	else if (question ~= "IsSlytherinAhead")
		return (IsHouseAhead(class'StatusItemSlytherinPts'));
	else if (question ~= "IsHufflepuffAhead")
		return (IsHouseAhead(class'StatusItemHufflepuffPts'));
	else if (question ~= "IsRavenclawAhead")
		return (IsHouseAhead(class'StatusItemRavenclawPts'));
	else
		return Super.CutQuestion(question);
}


// We don't want there to be any ties at the time of a Housepoint 
// ceremony or we won't have a cutscene to play.  There are cutscenes
// distinct for each house winning, but no specific cutscenes for
// a tie.  So, before a Housepoint ceremony, call ResolveTies to 
// artificially change the points around so there are no ties.  Since
// the housepoints display in the ceremony room before the ceremony
// happens, this adjustment should be made whenever the player
// goes into the ceremony room.  A good place to call this from
// would be the load of the CeremonySTextures object.
function ResolveTies()
{
	local StatusItem siGryffindorPts;
	local StatusItem siSlytherinPts;
	local StatusItem siRavenclawPts;
	local StatusItem siHufflepuffPts;

	siGryffindorPts = GetStatusItem(class'StatusItemGryffindorPts');
	siSlytherinPts  = GetStatusItem(class'StatusItemSlytherinPts');	
	siHufflepuffPts = GetStatusItem(class'StatusItemHufflepuffPts');
	siRavenclawPts  = GetStatusItem(class'StatusItemRavenclawPts');

	// After this call Ravenclaw is not tied with anyone.
	AdjustIfTie(siRavenclawPts,siGryffindorPts,siHufflepuffPts,siSlytherinPts);

	// After this call Hufflepuff is not tied with anyone.
	AdjustIfTie(siHufflepuffPts,siRavenclawPts,siGryffindorPts,siSlytherinPts);

	// After this call Slytherin is not tied with anyone.
	AdjustIfTie(siSlytherinPts,siHufflepuffPts,siRavenclawPts,siGryffindorPts);

	// Since none of the other houses are tied with anyone, then neither is
	// Gryffindor.  And, we have not touched Gryffindor's points-- the player
	// earns those and we don't want to mess with them.
}

// The first status item passed in will have it's count adjusted down until it 
// is no longer tied with any of the other houses.  If adjusting the points down
// will make them fall below zero, they will be adjusted up instead.
function AdjustIfTie(out StatusItem siAdjust, StatusItem siCompare1,
			  	     StatusItem siCompare2, StatusItem siCompare3)
{
	// Adjust the adjustable item down until it is not tied with anyone.
	while (siAdjust.nCount > 0)
	{
		if (siAdjust.nCount == siCompare1.nCount ||
			siAdjust.nCount == siCompare2.nCount ||
			siAdjust.nCount == siCompare3.nCount)
		{
			--siAdjust.nCount;
		}
		else 
			return;
	}

	// Could not adjust the points down, so adjust them up.
	while (true)
	{
		if (siAdjust.nCount == siCompare1.nCount ||
			siAdjust.nCount == siCompare2.nCount ||
			siAdjust.nCount == siCompare3.nCount)
		{
			++siAdjust.nCount;
		}
		else 
			return;
	}
}
// Return true if the status item class passed in corresponds to the
// house that is ahead.  This function assumes that any ties
// have been already resolved.  If they haven't and all 4 schools
// are tied, the placement that this function comes up with is
// 1) Gryffindor 2) Slytherin 3) Hufflepuff 4) Ravenclaw.
function bool IsHouseAhead(class<StatusItem> classItem)
{
	local StatusItem siGryffindorPts;
	local StatusItem siSlytherinPts;
	local StatusItem siRavenclawPts;
	local StatusItem siHufflepuffPts;
	local StatusItem siWinning;

	// Get status item objects for each house
	siGryffindorPts = GetStatusItem(class'StatusItemGryffindorPts');
	siRavenclawPts  = GetStatusItem(class'StatusItemRavenclawPts');
	siHufflepuffPts = GetStatusItem(class'StatusItemHufflepuffPts');
	siSlytherinPts  = GetStatusItem(class'StatusItemSlytherinPts');

	// Determine which status item has the most points
	siWinning = siGryffindorPts;
	if (siSlytherinPts.nCount > siWinning.nCount)
		siWinning = siSlytherinPts;
	if (siHufflepuffPts.nCount > siWinning.nCount)
		siWinning = siHufflepuffPts;
	if (siRavenclawPts.nCount > siWinning.nCount)
		siWinning = siRavenclawPts;

	// Return true if class passed in corresponds to the status item class
	// with the most points.
	return (classItem == siWinning.class);
}


defaultproperties
{
	// Override group properties
	bDisplayHorizontally=false
	fTotalEffectInTime=0.5
	fTotalHoldTime=3.0
	fTotalEffectOutTime=0.2
	fCurrEffectInTime=0.0
	GameEffectType=ET_FADE
}


//===============================================================================
//  StatusManager
//
//  StatusManager maintains a list of Status Groups.  A Status Group maintains
//  a list of Status Items that need to be displayed as a unit.  A Status Group
//  can have just one Status Item if that Status Item's display does not need
//  to be coordinated with other status items.
//
//  For each object type that you want the Status Manager to keep track of,
//  you need to create a class derived from StatusGroup and a class
//  derived from StatusItem.  For example, if you want to keep track of how
//  many jellybeans have been picked up, you need to create a
//  StatusGroupJellybeans class and a StatusItemJellybeans class.  The derived
//  classes will be pretty empty with just data and data access functions.
//
//  The StatusManager (and StatusGroup) will handle dynamically creating
//  StatusGroups and StatusItems as they are referenced.  For example,
//  if you were to call 
//  
//      IncrementCount(class'StatusGroupJellybeans',class'StatusItemJellybeans');
//
//  The StatusManager would automatically create a StatusGroupJellybeans object
//  (as long as there exists implementation for a StatusGroupJellybeans class)
//  and add it to its list of Status Groups.  Likewise, the new 
//  StatusGroupJellybeans class would automatically create a 
//  StatusItemJellybeans object (as long as there exists a a class implementation
//  for it).
//
//  StatusManager functions can be called by anyone.  Below is a description
//  of some objects that work with the StatusManager.
//  
//       HProp - HProp derived objects have 2 default properties under the
//       StatusManager category.  If you want to associate an HProp with
//       a StatusGroup and StatusItem, simply specify the StatusGroupClass
//       and StatusItemClass properties.  Your HPrp[ can then do things like
//       call StatusManager.PickupItem(self) and the StatusManager will update
//       the status of the StatusGroupClass and StatusItemClass specified in
//       the pawn.
// 
//       HPHud - HPHud can call the Status Manager's RenderHudItemManager(Canvas) 
//       to perform any drawing needed for the status objects currently under
//       the Status Manager's control.
//
//===============================================================================

class StatusManager extends HudItemManager;

const nTOTAL_WIZARD_CARDS = 101;

var StatusGroup sgList;
var int         nCanvasSizeX, nCanvasSizeY;
var Harry		playerHarry;

event PreBeginPlay()
{
	Super.PreBeginPlay();
	sgList = None;
	nCanvasSizeX = 0;
	nCanvasSizeY = 0;
}

// Loop through all the StatusGroups and have them render themselves.
function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
{
	local StatusGroup sgLoop;

	// Save off current canvas size
	nCanvasSizeX = canvas.SizeX;
	nCanvasSizeY = canvas.SizeY;

	for (sgLoop=sgList; sgLoop!=None; sgLoop=sgLoop.sgNext)
		sgLoop.RenderHudItemManager(canvas, bMenuMode, bFullCutMode, bHalfCutMode);
}

// Call when an HProp object (that has StatusGroupClass and StatusItemClass 
// properties define) is picked up.  Corresponding StatusGroup and StatusItem
// objects will automatically get created and/or updated with an 
// incremented count.
function PickupItem(HProp prop)
{
	local StatusGroup sgUpdate;
	local StatusItem  siUpdate;
	local Actor       actorTemp;
	local int         nOthersInLevel;

	// We only carry about objects that have a StatusGroup and StatusItem
	// class specified.
	if (prop.classStatusGroup == None ||
		prop.classStatusItem  == None)
		return;

	// Get the StatusGroup.  Note:  If the group does not currently exist, 
	// it will be created.
	sgUpdate = GetStatusGroup(prop.classStatusGroup);

	// If found (or created)
	if (sgUpdate != None)
	{
		// Get the StatusItem
		siUpdate = sgUpdate.GetStatusItem(prop.classStatusItem);

		// If max count should be displayed with status item and this is the
		// first time we've picked up an object belonging to the same class
		// as prop, then set the status item max count.
		if (siUpdate != None &&
			siUpdate.bDisplayMaxCount == true && 
			siUpdate.nCount == 0)
		{
			nOthersInLevel = 0;
			foreach AllActors(prop.class, actorTemp)
				nOthersInLevel++;

			siUpdate.nMaxCount = nOthersInLevel;			
		}

		// Increment the count (increment through the group on purpose so that
		// it can handle hud display).
		sgUpdate.IncrementCount(prop.classStatusItem, prop.nPickupIncrement);
	}
}

function DropOffItem(HProp prop)
{
	local StatusGroup sgUpdate;

	// We only carry about objects that have a StatusGroup and StatusItem
	// class specified.
	if (prop.classStatusGroup == None ||
		prop.classStatusItem  == None)
		return;

	// Get the StatusGroup.  Note:  If the group does not currently exist, 
	// it will be created.
	sgUpdate = GetStatusGroup(prop.classStatusGroup);

	// Update count
	if (sgUpdate != None)
		sgUpdate.IncrementCount(prop.classStatusItem, -prop.nPickupIncrement);
}

// Increment count for specified StatusGroup/Item classes.
function IncrementCount(class<StatusGroup> classGroup, 
						class<StatusItem> classItem, int nNum)
{
	local StatusGroup sgUpdate;
	local StatusItem  siUpdate;
	
	// Get the Group.  Note:  If the group does not currently exist, it
	// will be created.
	sgUpdate = GetStatusGroup(classGroup);

	// If found (or created) the group.
	if (sgUpdate != None)
		sgUpdate.IncrementCount(classItem, nNum);
}

function SetCount(class<StatusGroup> classGroup, 
				  class<StatusItem> classItem, int nNum)
{
	local StatusItem  siUpdate;
	
	siUpdate = GetStatusItem(classGroup, classItem);

	// If found (or created) the group.
	if (siUpdate != None)
		siUpdate.SetCount(nNum);
}

// Increment count potential StatusGroup/Item classes.
function IncrementCountPotential(class<StatusGroup> classGroup, 
						class<StatusItem> classItem, int nNum)
{
	local StatusGroup sgUpdate;
	local StatusItem  siUpdate;
	
	// Get the Group.  Note:  If the group does not currently exist, it
	// will be created.
	sgUpdate = GetStatusGroup(classGroup);

	// If found (or created) the group.
	if (sgUpdate != None)
		sgUpdate.IncrementCountPotential(classItem, nNum);
}

// Get 3D location of a prop's corresponding hud item.
function vector GetHudLocation(HProp Prop)
{
	local StatusGroup sg;

	// We only care about objects that have a StatusGroup and StatusItem
	// class specified.
	if (Prop.classStatusGroup == None ||
		Prop.classStatusItem  == None)
	{
		log("Error: StatusManager data not setup correctly for " $prop.class);
		return (vect(0,0,0));
	}

	// Get the StatusGroup.  Note:  If the group does not currently exist, 
	// it will be created.
	sg = GetStatusGroup(Prop.classStatusGroup);
	return (sg.GetItemLocation(Prop.classStatusItem,false));
}

// Get the StatusItem specified by classGroup and classItem.  classItem and/or
// classGroup objects will get created if corresponding objects do not 
// already exist.
function StatusItem GetStatusItem(class<StatusGroup> classGroup, 
								  class<StatusItem> classItem)
{
	local StatusGroup sgFound;

	// Get the StatusGroup object
	sgFound = GetStatusGroup(classGroup);

	if (sgFound != None)
	{
		// Return the StatusGroup's StatusItem that matches classItem.  Note:
		// if classItem is not currently in the StatusGroup, it will get created
		// (unless classItem is an invalid class).
		return (sgFound.GetStatusItem(classItem));
	}
	else
	{
		log("Error: could not create or find StatusGroup " $classGroup);
	}
}

// Get the StatusGroup specified by classGroup.  A new classGroup object will
// be created if one does not already exist.
function StatusGroup GetStatusGroup(class<StatusGroup> classGroup)
{
	local StatusGroup sgLoop;

	// If no groups in the list yet, create a classGroup StatusGroup, add it
	// to the list and return it.
	if (sgList == None)
	{		
		sgList          = spawn(classGroup);
		sgList.sgNext   = None;
		sgList.smParent = self;
		return (sgList);
	}

	// Look through status group list and return StatusGroup of classGroup if
	// it exists.  If there is no classGroup in the list, create one, add
	// it to the list and return.
	for (sgLoop=sgList; sgLoop!=None; sgLoop=sgLoop.sgNext)
	{
		// If found the existing StatusGroup, return it.
		if (sgLoop.class == classGroup)
			return (sgLoop);

		// If we've reached the end of the StatusGroup list and no classGroup
		// object has been found, create one, add it to the list and return
		// it.  Note:  If classGroup is not a valid class, no StatusGroup
		// will be added to the list and the return will be None.
		if (sgLoop.sgNext == None)
		{
			// Spawn new classGroup object
			sgLoop.sgNext          = spawn(classGroup);
			sgLoop.sgNext.smParent = self;
	
			// Return the newly spawned object (or None if the spawn failed).
			return (sgLoop.sgNext);
		}
	}

	log("Error: StatusManager::GetStatusGroup- should not get to here");
	return (None);
}

// Create status items that need to exist at startup. Other status items
// will get created as an attempt to access them is made.  
// (GetStatusItem creates the status item if it does not already exist.)
function CreateStartupItems()
{
	GetStatusItem(class'StatusGroupHealth'     , class'StatusItemHealth');
	GetStatusItem(class'StatusGroupHousePoints', class'StatusItemGryffindorPts');
	GetStatusItem(class'StatusGroupHousePoints', class'StatusItemRavenclawPts');
	GetStatusItem(class'StatusGroupHousePoints', class'StatusItemHufflepuffPts');
	GetStatusItem(class'StatusGroupHousePoints', class'StatusItemSlytherinPts');
	GetStatusItem(class'StatusGroupJellybeans' , class'StatusItemJellybeans');
	GetStatusItem(class'StatusGroupWizardCards', class'StatusItemBronzeCards');
	GetStatusItem(class'StatusGroupWizardCards', class'StatusItemSilverCards');
	GetStatusItem(class'StatusGroupWizardCards', class'StatusItemGoldCards');
}

//-----------------------------------------------------------------------------------
// Helper functions
//-----------------------------------------------------------------------------------

function AddHPointsG(int nPoints)
{
	AddHousepoints(class'StatusItemGryffindorPts', nPoints);
}

function int GetHPointsG()
{
	return (GetStatusItem(class'StatusGroupHousePoints', class'StatusItemGryffindorPts').nCount);
}

function AddHPointsH(int nPoints)
{
	AddHousepoints(class'StatusItemHufflepuffPts', nPoints);
}

function int GetHPointsH()
{
	return (GetStatusItem(class'StatusGroupHousePoints', class'StatusItemHufflepuffPts').nCount);
}

function AddHPointsS(int nPoints)
{
	AddHousepoints(class'StatusItemSlytherinPts', nPoints);
}

function int GetHPointsS()
{
	return (GetStatusItem(class'StatusGroupHousePoints', class'StatusItemSlytherinPts').nCount);
}

function AddHPointsR(int nPoints)
{
	AddHousepoints(class'StatusItemRavenclawPts', nPoints);
}

function int GetHPointsR()
{
	return (GetStatusItem(class'StatusGroupHousePoints', class'StatusItemRavenclawPts').nCount);
}

function AddHousepoints(class<StatusItem> classItem, int nPoints)
{
	local StatusGroup sgHousePts;
	local StatusItem  siHousePts;

	sgHousePts = GetStatusGroup(class'StatusGroupHousePoints');
	sgHousePts.IncrementCount(classItem, nPoints);
	siHousePts = sgHousePts.GetStatusItem(classItem);

	// Display Gryff pts.
	siHousePts = sgHousePts.GetStatusItem(class'StatusItemGryffindorPts');
	playerHarry.ClientMessage("Gryffindor : " $siHousePts.nCount);

	// Display Huff pts
	siHousePts = sgHousePts.GetStatusItem(class'StatusItemHufflepuffPts');
	playerHarry.ClientMessage("Hufflepuff : " $siHousePts.nCount);

	// Display Ravenclaw pts
	siHousePts = sgHousePts.GetStatusItem(class'StatusItemRavenclawPts');
	playerHarry.ClientMessage("Ravenclaw : " $siHousePts.nCount);

	// Display Slytherin pts
	siHousePts = sgHousePts.GetStatusItem(class'StatusItemSlytherinPts');
	playerHarry.ClientMessage("Slytherin : " $siHousePts.nCount);
}

function AddFMucus(int nCount)
{
	IncrementCount(class'StatusGroupPotionIngr', class'StatusItemFlobberMucus', nCount);
}

function int GetFMucusCount()
{
	return (GetStatusItem(class'StatusGroupPotionIngr', class'StatusItemFlobberMucus').nCount);
}

function AddWBark(int nCount)
{
	IncrementCount(class'StatusGroupPotionIngr', class'StatusItemWiggenBark', nCount);
}

function int GetWBarkCount()
{
	return (GetStatusItem(class'StatusGroupPotionIngr', class'StatusItemWiggenBark').nCount);
}

function AddBicorn(int nCount)
{
    IncrementCount(class'StatusGroupPolyIngr', class'StatusItemBicorn', nCount);
}

function int GetBicornCount()
{
    return (GetStatusItem(class'StatusGroupPolyIngr', class'StatusItemBicorn').nCount);
}

function AddBoomslang(int nCount)
{
    IncrementCount(class'StatusGroupPolyIngr', class'StatusItemBoomslang', nCount);
}

function int GetBoomslangCount()
{
    return (GetStatusItem(class'StatusGroupPolyIngr', class'StatusItemBoomslang').nCount);
}

function AddBeans(int nCount)
{
	IncrementCount(class'StatusGroupJellybeans', class'StatusItemJellybeans', nCount);
}

function int GetBeanCount()
{
	return (GetStatusItem(class'StatusGroupJellybeans', class'StatusItemJellybeans').nCount);
}

function AddPotions(int nCount)
{
	IncrementCount(class'StatusGroupPotions', class'StatusItemWiggenwell', nCount);
}

function int GetPotionCount()
{
	return (GetStatusItem(class'StatusGroupPotions', class'StatusItemWiggenwell').nCount);
}

function AddHealth(int nCount)
{
	IncrementCount(class'StatusGroupHealth', class'StatusItemHealth', nCount);
}

function int GetHealthCount()
{
	return (GetStatusItem(class'StatusGroupHealth', class'StatusItemHealth').nCount);
}

function SetHealthCount(int nCount)
{
	SetCount(class'StatusGroupHealth', class'StatusItemHealth', nCount);
}

function AddHealthPotential(int nCount)
{
	IncrementCountPotential(class'StatusGroupHealth', class'StatusItemHealth', nCount);
}

function int GetHealthPotentialCount()
{
	return (GetStatusItem(class'StatusGroupHealth', class'StatusItemHealth').nCurrCountPotential);
}

function GiveCardToHarry(int nCardId)
{
	GiveCard(nCardId, true);
}

function GiveCardToVendors(int nCardId)
{
	GiveCard(nCardId, false);
}

function GiveCard(int nCardId, bool bHarry)
{
	local StatusItemWizardCards siWC;
	local class<actor>          classWC;
	local string                strDebug;

	if (bHarry)
		strDebug = "Harry has ";
	else
		strDebug = "Vendors have ";

	// Get class that corresponds to id passed in
	siWC = StatusItemWizardCards(GetStatusItem(class'StatusGroupWizardCards', class'StatusItemBronzeCards'));
	classWC = siWC.GetCardClassFromId(nCardId);

	// If got a class
	if (classWC != None)
	{
		// Add the card to the correct status item class
		if (ClassIsChildOf(classWC, class'BronzeCards'))
		{
			strDebug = strDebug $"bronze card ";
			siWC = StatusItemWizardCards(GetStatusItem(class'StatusGroupWizardCards', class'StatusItemBronzeCards'));
		}
		else if (ClassIsChildOf(classWC, class'SilverCards'))
		{
			strDebug = strDebug $"silver card ";
			siWC = StatusItemWizardCards(GetStatusItem(class'StatusGroupWizardCards', class'StatusItemSilverCards'));
		}
		else if (ClassIsChildOf(classWC, class'GoldCards'))
		{
			strDebug = strDebug $"gold card ";
			siWC = StatusItemWizardCards(GetStatusItem(class'StatusGroupWizardCards', class'StatusItemGoldCards'));
		}

		strDebug = strDebug $nCardId;
		playerHarry.ClientMessage(strDebug);
		if (bHarry)
			siWC.SetCardOwner(nCardId, siWC.ECardOwner.CardOwner_Harry);
		else
			siWC.SetCardOwner(nCardId, siWC.ECardOwner.CardOwner_Vendor);
	}
}

// Loop through all wizard cards and give them to Harry.
function GiveAllCardsToHarry()
{
	local StatusItemWizardCards siBronzeCards;
	local StatusItemWizardCards siSilverCards;
	local StatusItemWizardCards siGoldCards;
	local class<actor>          classWC;
	local int                   i;

	// Get status item corresponding to each card type
	siBronzeCards = StatusItemWizardCards(GetStatusItem(class'StatusGroupWizardCards', class'StatusItemBronzeCards'));
	siSilverCards = StatusItemWizardCards(GetStatusItem(class'StatusGroupWizardCards', class'StatusItemSilverCards'));
	siGoldCards   = StatusItemWizardCards(GetStatusItem(class'StatusGroupWizardCards', class'StatusItemGoldCards'));

	for (i=1; i<=nTOTAL_WIZARD_CARDS; i++)
	{
		// Get WizardCard class that corresponds to the current it.
		classWC = siBronzeCards.GetCardClassFromId(i);

		// If it's the Harry card, don't add him right now- we want to make sure he's added last
		if (classWC != class'WCPotter')
		{
			if (ClassIsChildOf(classWC, class'BronzeCards'))
				siBronzeCards.SetCardOwner(i, siBronzeCards.ECardOwner.CardOwner_Harry);
			else if (ClassIsChildOf(classWC, class'SilverCards'))
				siSilverCards.SetCardOwner(i, siSilverCards.ECardOwner.CardOwner_Harry);
			else if (ClassIsChildOf(classWC, class'GoldCards'))
				siGoldCards.SetCardOwner(i, siGoldCards.ECardOwner.CardOwner_Harry);
		}
	}

	// Add Harry card last
	siGoldCards.SetCardOwner(class'WCPotter'.default.ID, siGoldCards.ECardOwner.CardOwner_Harry);

    // Add all silver locks
    GetStatusItem(class'StatusGroupLocks',class'StatusItemLock1').SetCount(1);
	GetStatusItem(class'StatusGroupLocks',class'StatusItemLock2').SetCount(1);
	GetStatusItem(class'StatusGroupLocks',class'StatusItemLock3').SetCount(1);
	GetStatusItem(class'StatusGroupLocks',class'StatusItemLock4').SetCount(1);
}

function ShowCardData()
{
	local StatusGroupWizardCards sgWC;

	sgWC = StatusGroupWizardCards(GetStatusGroup(class'StatusGroupWizardCards'));
	sgWC.ShowCardData();
}

defaultproperties
{
	DrawType=DT_None
	bHidden=true
}


//===============================================================================
//  [StatusItemWizardCards] 
//===============================================================================

class StatusItemWizardCards extends StatusItem abstract;

enum ECardOwner
{
	CardOwner_None,   // Card not assigned to Harry of vendor yet
	CardOwner_Harry,  // Harry has picked card up
	CardOwner_Vendor  // Player missed card, vendor can sell, so vendor is owner
};

struct TCardData
{	
	var int        nId;          // Id of wizard card
	var ECardOwner Owner;        // Owner of wizard card
};

// Wizard cards encountered so far
var TCardData WizardCards[50];   // Use array of 50 each for bronze, silver and gold.
                                 // Only bronze cards will fill this array up--
                                 // there are 40 silver and 11 gold cards total

// Based on the card id, return who owns the card.  If a card has not been 
// encountered yet, it will not be in our array of WizardCards, and CardOwner_None
// will be returned (which is what we want).
function ECardOwner GetCardOwner(int nId)
{
	local int i;

	// If requested owner of id 0, show error- id's start at 1.
	if (nId == 0)
	{
		sgParent.smParent.playerHarry.ClientMessage("ERROR in GetCardOwner- 0 is an invalid card id");
		return (CardOwner_None);
	}

	// Loop thru card until card with specified it is found.
	for (i=0; i<ArrayCount(WizardCards); i++)
	{
		if (WizardCards[i].nId == nId)
			return (WizardCards[i].Owner);
	}

	// Card was not found in the WizardCards array- so it hasn't been encountered yet and
	// there is no owner.
	return (CardOwner_None);
}

function bool IsOwnedByHarry(int nId)
{
	return (GetCardOwner(nId) == CardOwner_Harry);
}

function bool IsOwnedByVendor(int nId)
{
	return (GetCardOwner(nId) == CardOwner_Vendor);
}

function bool IsOwnedByNone(int nId)
{
	return (GetCardOwner(nId) == CardOwner_None);
}

// Set the owner for the card id passed in.
function SetCardOwner(int nId, ECardOwner Owner)
{
	local int i;

	// Shouldn't be passing in id 0, id's start at 1.
	if (nId == 0)
	{
		sgParent.smParent.playerHarry.ClientMessage("ERROR in GetCardOwner- 0 is an invalid card id");
		return;
	}

	// Loop through WizardCards until find the matching id and set owner.  If no
	// matching id is found, the id passed in will be added to the end of the array
	// and the owner will be set as requested.
	for (i=0; i<ArrayCount(WizardCards); i++)
	{
		// If id already exists in array, set the owner.
		if (WizardCards[i].nId == nId)
		{
			WizardCards[i].Owner = Owner;
			break;
		}

		// Id wasn't found in the array, add it to the end and set owner.
		else if (WizardCards[i].nId == 0)
		{
			WizardCards[i].nId   = nId;
			WizardCards[i].Owner = Owner;
			break;
		}
	}

	// Keep track of whether last card picked up was bronze, silver or gold.
	if (Owner == CardOwner_Harry)
		StatusGroupWizardCards(sgParent).SetLastObtainedCardItem(self);

	// Update our nCount variable	
	UpdateCount();
}

function UpdateCount()
{
	local int i;

	// Reset count
	nCount = 0;

	// Loop thru cards and add 1 to nCount each time we find a card with an
	// id and it's assigned to Harry.
	for (i=0; i<ArrayCount(WizardCards); i++)
	{
		if (WizardCards[i].nId == 0)
			return;
		else if (WizardCards[i].Owner == CardOwner_Harry)
			++nCount;
	}
}

// Get the id of the first card to have owner set to vendor.
function bool GetFirstVendorCardId(out int nCardId)
{
	local int i;

	// Loop thru card array
	for (i=0; i<ArrayCount(WizardCards); i++)
	{
		// If get to end of array and haven't found a card owned by 
		// vendor, return false.
		if (WizardCards[i].nId == 0)
		{
			nCardId = 0;
			return (false);
		}

		// Found a vendor owned card.  Return the id.
		if (WizardCards[i].Owner == CardOwner_Vendor)
		{
			nCardId = WizardCards[i].nId;
			return (true);
		}
	}

	// No vendor cards currently available
	nCardId = 0;
	return (false);
}

function bool GetFirstVendorCardIdAndClass(out int nCardId, out class<actor> classWC)
{
	if (!GetFirstVendorCardId(nCardId))
		return (false);

	classWC = GetCardClassFromId(nCardId);

	return (classWC != None);
}

function class<actor> GetCardClassFromId(int nCardId)
{
	local class<actor> classWC;

	switch (nCardId)
	{
	case (1)   :	classWC = class'WCMerlin';		break;
	case (2)   :	classWC = class'WCAgrippa';		break;
	case (3)   :	classWC = class'WCClagg';		break;
	case (4)   :	classWC = class'WCStump';		break;
	case (5)   :	classWC = class'WCPokeby';		break;
	case (6)   :	classWC = class'WCPeakes';		break;
	case (7)   :	classWC = class'WCStarkey';		break;
	case (8)   :	classWC = class'WCShimpling';	break;
	case (9)   :	classWC = class'WCGunhilda';	break;
	case (10)  :	classWC = class'WCMuldoon';		break;
	case (11)  :	classWC = class'WCHerpo';		break;
	case (12)  :	classWC = class'WCMerwyn';		break;
	case (13)  :	classWC = class'WCAndros';		break;
	case (14)  :	classWC = class'WCFulbert';		break;
	case (15)  :	classWC = class'WCParacelsus';	break;
	case (16)  :	classWC = class'WCCliodne';		break;
	case (17)  :	classWC = class'WCFay';			break;
	case (18)  :	classWC = class'WCUlric';		break;
	case (19)  :	classWC = class'WCScamander';	break;
	case (20)  :	classWC = class'WCWendelin';	break;
	case (21)  :	classWC = class'WCWithers';		break;
	case (22)  :	classWC = class'WCCirce';		break;
	case (23)  :	classWC = class'WCChittock';	break;
	case (24)  :	classWC = class'WCWaffling';	break;
	case (25)  :	classWC = class'WCFancourt';	break;
	case (26)  :	classWC = class'WCSawbridge';	break;
	case (27)  :	classWC = class'WCPlunkett';	break;
	case (28)  :	classWC = class'WCToke';		break;
	case (29)  :	classWC = class'WCAlderton';	break;
	case (30)  :	classWC = class'WCLufkin';		break;
	case (31)  :	classWC = class'WCBlane';		break;
	case (32)  :	classWC = class'WCWenlock';		break;
	case (33)  :	classWC = class'WCMarjoribanks';break;
	case (34)  :	classWC = class'WCTremlett';	break;
	case (35)  :	classWC = class'WCWright';		break;
	case (36)  :	classWC = class'WCWadcock';		break;
	case (37)  :	classWC = class'WCVablatsky';	break;
	case (38)  :	classWC = class'WCOldridge';	break;
	case (39)  :	classWC = class'WCJones';		break;
	case (40)  :	classWC = class'WCPinkstone';	break;
	case (41)  :	classWC = class'WCGriffindor';	break;
	case (42)  :	classWC = class'WCCronk';		break;
	case (43)  :	classWC = class'WCYoudle';		break;
	case (44)  :	classWC = class'WCWhitehorn';	break;
	case (45)  :	classWC = class'WCOglethorpe';	break;
	case (46)  :	classWC = class'WCGoshawk';		break;
	case (47)  :	classWC = class'WCStroulger';	break;
	case (48)  :	classWC = class'WCSlytherin';	break;
	case (49)  :	classWC = class'WCKetteridge';	break;
	case (50)  :	classWC = class'WCBarkwith';	break;
	case (51)  :	classWC = class'WCEthelred';	break;
	case (52)  :	classWC = class'WCSummerbee';	break;
	case (53)  :	classWC = class'WCCatchlove';	break;
	case (54)  :	classWC = class'WCShingleton';	break;
	case (55)  :	classWC = class'WCNutcombe';	break;
	case (56)  :	classWC = class'WCCrumb';		break;
	case (57)  :	classWC = class'WCOllerton';	break;
	case (58)  :	classWC = class'WCHipworth';	break;
	case (59)  :	classWC = class'WCGregory';		break;
	case (60)  :	classWC = class'WCMontmorency';	break;
	case (61)  :	classWC = class'WCSweeting';	break;
	case (62)  :	classWC = class'WCWildsmith';	break;
	case (63)  :	classWC = class'WCWintringham';	break;
	case (64)  :	classWC = class'WCSykes';		break;
	case (65)  :	classWC = class'WCOliphant';	break;
	case (66)  :	classWC = class'WCBelby';		break;
	case (67)  :	classWC = class'WCPilliwickle';	break;
	case (68)  :	classWC = class'WCDuke';		break;
	case (69)  :	classWC = class'WCBott';		break;
	case (70)  :	classWC = class'WCSmethwyck';	break;
	case (71)  :	classWC = class'WCMaeve';		break;
	case (72)  :	classWC = class'WCHufflepuff';	break;
	case (73)  :	classWC = class'WCMopsus';		break;
	case (74)  :	classWC = class'WCKnightley';	break;
	case (75)  :	classWC = class'WCBonham';		break;
	case (76)  :	classWC = class'WCWagtail';		break;
	case (77)  :	classWC = class'WCTwonk';		break;
	case (78)  :	classWC = class'WCThruston';	break;
	case (79)  :	classWC = class'WCBeamish';		break;
	case (80)  :	classWC = class'WCBloxam';		break;
	case (81)  :	classWC = class'WCPo';			break;
	case (82)  :	classWC = class'WCRavenclaw';	break;
	case (83)  :	classWC = class'WCPlumpton';	break;
	case (84)  :	classWC = class'WCKegg';		break;
	case (85)  :	classWC = class'WCStalk';		break;
	case (86)  :	classWC = class'WCWellbeloved';	break;
	case (87)  :	classWC = class'WCThurkell';	break;
	case (88)  :	classWC = class'WCWarbeck';		break;
	case (89)  :	classWC = class'WCToothill';	break;
	case (90)  :	classWC = class'WCTugwood';		break;
	case (91)  :	classWC = class'WCElphick';		break;
	case (92)  :	classWC = class'WCRastrick';	break;
	case (93)  :	classWC = class'WCBarbary';		break;
	case (94)  :	classWC = class'WCGraves';		break;
	case (95)  :	classWC = class'WCPlatt';		break;
	case (96)  :	classWC = class'WCWoodcroft';	break;
	case (97)  :	classWC = class'WCGrunnion';	break;
	case (98)  :	classWC = class'WCFurmage';		break;
	case (99)  :	classWC = class'WCDodderidge';	break;
	case (100) :	classWC = class'WCPotter';		break;
	case (101) :	classWC = class'WCDumbledore';	break;
	default    :	break;
	}

	if (VerifyCardClass(nCardId, classWC))
		return (classWC);
	else
		return (None);
}

function bool VerifyCardClass(int nCardId, class<actor> classWC)
{
	if (ClassIsChildOf(classWC, class'WizardCardIcon'))
	{
		if (class<WizardCardIcon>(classWC).default.ID == nCardId)
			return (true);
		else
		{
			sgParent.smParent.playerHarry.ClientMessage("ERROR in wizard card id" $nCardId $" <> " $(class<WizardCardIcon>(classWC).default.ID));
			return (false);
		}
	}
	else
	{
		sgParent.smParent.playerHarry.ClientMessage("ERROR in VerifyCardClass- class is not WizardCardIcon " $nCardId $" " $classWC);		
		return (false);
	}
}

// Get id and owner of card at index passed in.
function GetCardData(int nIdx, out int nId, out int nOwner)
{
	// If valid index, return data at that index.
	if ((nIdx >=0) && (nIdx < ArrayCount(WizardCards)))
	{
		nId    = WizardCards[nIdx].nId;
		nOwner = WizardCards[nIdx].Owner;
	}

	// Passed in invalid index.
	else
	{
		sgParent.smParent.playerHarry.ClientMessage("ERROR: Invalid WizardCards index in GetCardData");
		nId = 0;
		nOwner = 0;
	}
}

// Get just id of card at index passed in
function int GetCardId(int nIdx)
{
	local int nId;
	local int nOwner;

	GetCardData(nIdx, nId, nOwner);
	return (nId);
}

// Set id and owner of card at index passed in.
function SetCardData(int nIdx, int nId, int nOwner)
{
	// If valid index, set id and owner
	if ((nIdx >=0) && (nIdx < ArrayCount(WizardCards)))
	{
		WizardCards[nIdx].nId = nId;
		if (nOwner == 0)
			WizardCards[nIdx].Owner = CardOwner_None;
		else if (nOwner == 1)
			WizardCards[nIdx].Owner = CardOwner_Harry;
		else if (nOwner == 2)
			WizardCards[nIdx].Owner = CardOwner_Vendor;
		else
			sgParent.smParent.playerHarry.ClientMessage("ERROR: Invalid owner in SetCardData");
	}

	// Passed in invalid index.
	else
		sgParent.smParent.playerHarry.ClientMessage("ERROR: Invalid WizardCards index in SetCardData");
}

// Show card data for all cards currently in the WizardCard array.
function ShowCardData()
{
	local int i;

	sgParent.smParent.playerHarry.ClientMessage("WizardCards: " $class);
	for (i=0; i<ArrayCount(WizardCards); i++)
	{
		if (WizardCards[i].nId == 0)
			break;
		sgParent.smParent.playerHarry.ClientMessage("ID: " $WizardCards[i].nId $" Owner: " $WizardCards[i].Owner);
	}
}

defaultproperties
{
	strHudIcon="HP_Menu.Hud.CardFolio"
	bDisplayCount=false
	bDisplayMaxCount=false
	bMenuModeOnly=false
	nActualIconW=72			// Icon is 128x128, but image is only 72x52
	nActualIconH=52   
	strToolTipId="InGameMenu_0021"
}


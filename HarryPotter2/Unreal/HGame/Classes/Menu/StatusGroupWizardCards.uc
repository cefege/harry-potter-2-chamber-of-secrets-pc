//===============================================================================
//  [StatusGroupWizardCardss] 
//===============================================================================

class StatusGroupWizardCards extends StatusGroup;

const nCARDTYPE_BRONZE = 0;
const nCARDTYPE_SILVER = 1;
const nCARDTYPE_GOLD   = 2;
const nCARDTYPE_NONE   = 3;

const nSTART_X = 132;
const nSTART_Y = 5;

enum ECardType					// NOTE: This enum should match the constants
{                               //       above.  We'd like to use enums all the
	CardType_Bronze,            //       time, but Harry has to travel with
	CardType_Silver,            //       the last obtained card type and he can't
	CardType_Gold,              //       travel an enum declared in another class.
	CardType_None
};

var ECardType LastObtainedCardType;

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

	nOutX = fScaleFactor * nSTART_X;
	nOutY = fScaleFactor * nSTART_Y;
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

// Before Harry travels, we look through the level and see if there are any
// wizard cards that single item vendors can sell if Harry fails
// to pick them up.  These cards include cards that are in levels that
// the player can only go through once- Adventure levels.  
function AssignVendorCards()
{
	local ChestBronze			Chest;            
	local BronzeCauldron		Cauldron;
	local WizardCardIcon        WCard;
	local int					i;
	local StatusItemWizardCards siCards;


	// Bronze chest objects can be setup to spawn wizard cards.  Go through each
	// bronze chest's "EjectedObjects" and if the are ejecting a wizard card,
	// see if that wizard card can be sold by a vendor.
	foreach AllActors(class'ChestBronze', Chest)
	{
		for (i=0; i<ArrayCount(Chest.EjectedObjects); i++)
		{
			if (ClassIsChildOf(Chest.EjectedObjects[i], class'WizardCardIcon'))
				AssignVendorCardFromClass(Chest.EjectedObjects[i]);
		}		
	}

	// Wizard cards can also spawn out of bronze chests.
	foreach AllActors(class'BronzeCauldron', Cauldron)
	{
		for (i=0; i<ArrayCount(Cauldron.EjectedObjects); i++)
		{
			if (ClassIsChildOf(Cauldron.EjectedObjects[i], class'WizardCardIcon'))
				AssignVendorCardFromClass(Cauldron.EjectedObjects[i]);
		}		
	}

	// Wizard cards that are placed directly in the world.
	foreach AllActors(class'WizardCardIcon', WCard)
	{
		// If vendor can sell the card and Harry does not aready own it,
		// set card's owner to vendor.
		if (WCard.bVendorsCanSell && HasCardGameStatePassed(WCard.strVendorOwnedAfterGState))
		{
			if (ClassIsChildOf(WCard.class, class'BronzeCards'))
				siCards = StatusItemWizardCards(GetStatusItem(class'StatusItemBronzeCards'));
			if (ClassIsChildOf(WCard.class, class'SilverCards'))
				siCards = StatusItemWizardCards(GetStatusItem(class'StatusItemSilverCards'));
			if (ClassIsChildOf(WCard.class, class'GoldCards'))
				siCards = StatusItemWizardCards(GetStatusItem(class'StatusItemGoldCards'));

			if (siCards.GetCardOwner(WCard.ID) != siCards.ECardOwner.CardOwner_Harry)
				siCards.SetCardOwner(WCard.ID, siCards.ECardOwner.CardOwner_Vendor);
		}
	}
}

// Assign the card of class classObject to vendor.  If classObject is not derived
// from WizardCardIcon, this class will just return.
function AssignVendorCardFromClass(class classObject)
{
	local class<WizardCardIcon> classWC;          // Will cast classObject to class<WizardCardIcon>
	local int                   nId;              // Card id corresponding to classObject
	local bool                  bVendorsCanSell;  // If card corresponding to classObject can be sold
	local string                strVendorOwnedAfterGState;
	local StatusItemWizardCards siCards;          // Bronze, Silver or Gold card status item

	// If the class is derived from WizardCardIcon
	if (ClassIsChildOf(classObject, class'WizardCardIcon'))
	{
		// Get data about the card of classObject
		classWC = class<WizardCardIcon>(classObject);
		nId             = classWC.default.id;
		bVendorsCanSell = classWC.default.bVendorsCanSell;
		strVendorOwnedAfterGState = classWC.default.strVendorOwnedAfterGState;

		// If the card can be sold by a vendor
		if (bVendorsCanSell && HasCardGameStatePassed(strVendorOwnedAfterGState))
		{
			// Determine what kind of StatusItem object holds the card.
			if (ClassIsChildOf(classObject, class'BronzeCards'))
				siCards = StatusItemWizardCards(GetStatusItem(class'StatusItemBronzeCards'));
			else if (ClassIsChildOf(classObject, class'SilverCards'))
				siCards = StatusItemWizardCards(GetStatusItem(class'StatusItemSilverCards'));
			else if (ClassIsChildOf(classObject, class'GoldCards'))
				siCards = StatusItemWizardCards(GetStatusItem(class'StatusItemGoldCards'));

			// Using the appropriate StatusItem object, set the owner.  If the
			// card does not already exist in the StatusItem object, it will be
			// added and then the owner set.
			if (siCards.GetCardOwner(nId) != siCards.ECardOwner.CardOwner_Harry)
				siCards.SetCardOwner(nId, siCards.ECardOwner.CardOwner_Vendor);
		}
	}
}

function RemoveHarryOwnedCardsFromLevel()
{
	local ChestBronze			Chest;            
	local BronzeCauldron		Cauldron;
	local WizardCardIcon        WCard;
	local int					i;
	local StatusItemWizardCards siCards;
    local class<WizardCardIcon> classWC;

	// Bronze chest objects can be setup to spawn wizard cards.  Go through each
	// bronze chest's "EjectedObjects" and if they are ejecting a wizard card that
    // Harry already owns, replace the card with a jellybean.
	foreach AllActors(class'ChestBronze', Chest)
	{
		for (i=0; i<ArrayCount(Chest.EjectedObjects); i++)
		{
			if (ClassIsChildOf(Chest.EjectedObjects[i], class'WizardCardIcon'))
            {
                // Get data about the card of classObject
        		classWC = class<WizardCardIcon>(Chest.EjectedObjects[i]);

                smParent.playerHarry.ClientMessage("Object in chest " $classWC $classWC.default.id);

                // If Harry owns the card in the chest, change the chest object to a jellybean.
                if (StatusItemWizardCards(GetStatusItem(class'StatusItemBronzeCards')).IsOwnedByHarry(classWC.default.id) ||
                    StatusItemWizardCards(GetStatusItem(class'StatusItemSilverCards')).IsOwnedByHarry(classWC.default.id) ||
                    StatusItemWizardCards(GetStatusItem(class'StatusItemGoldCards')).IsOwnedByHarry(classWC.default.id))
                {
                    smParent.playerHarry.ClientMessage("Replacing wizard card in chest because Harry already has it: " $classWC);       
				    Chest.EjectedObjects[i] = class'Jellybean';
                }
            }
		}		
	}

	// Wizard cards that are placed directly in the world.
	foreach AllActors(class'WizardCardIcon', WCard)
	{
        // If Harry already has the card, delete it from the world.
        if (StatusItemWizardCards(GetStatusItem(class'StatusItemBronzeCards')).IsOwnedByHarry(WCard.Id) ||
            StatusItemWizardCards(GetStatusItem(class'StatusItemBronzeCards')).IsOwnedByHarry(WCard.Id) ||
            StatusItemWizardCards(GetStatusItem(class'StatusItemBronzeCards')).IsOwnedByHarry(WCard.Id))
           
        {
            smParent.playerHarry.ClientMessage("Deleting wizard card from level because Harry already has it: " $WCard);
            WCard.Destroy();
        }
	}
}

function bool GetGameStateTokenIndex(string strGameState, out int nIdx)
{
	local string strTest;

	for (nIdx=0; nIdx<smParent.playerHarry.TotalGameStateTokens; nIdx++)
	{
		strTest = smParent.playerHarry.GetGameStateMasterListToken(nIdx);
		//smParent.playerHarry.ClientMessage("GetGameStateTokenIndex " $strTest);
		if (strTest == strGameState)
			return (true);
	}

	nIdx = 0;
	return (false);
}

function bool HasCardGameStatePassed(string strCardGameState)
{
	local int nCurrStateTokenIdx;
	local int nCardStateTokenIdx;

	if (GetGameStateTokenIndex(smParent.playerHarry.CurrentGameState, nCurrStateTokenIdx))
	{
		if (GetGameStateTokenIndex(strCardGameState, nCardStateTokenIdx))
		{
			if (nCardStateTokenIdx <= nCurrStateTokenIdx)
			{
				smParent.playerHarry.ClientMessage("Game state passed " $strCardGameState);
				return (true);
			}
		}
		else
			smParent.playerHarry.ClientMessage("ERROR getting wizard card game state token idx " $strCardGameState);
	}

	//smParent.playerHarry.ClientMessage("Game state not happened yet " $strCardGameState);
	return (false);
}


// Debugging function for showing card data of all cards encountered so far.
function ShowCardData()
{
	StatusItemWizardCards(GetStatusItem(class'StatusItemBronzeCards')).ShowCardData();
	StatusItemWizardCards(GetStatusItem(class'StatusItemSilverCards')).ShowCardData();
	StatusItemWizardCards(GetStatusItem(class'StatusItemGoldCards')).ShowCardData();
}

// Save off which group the last obtained card was in.
function SetLastObtainedCardItem(StatusItemWizardCards siWC)
{
	if (siWC.isa('StatusItemBronzeCards'))
		LastObtainedCardType = CardType_Bronze;
	else if (siWC.isa('StatusItemSilverCards'))
		LastObtainedCardType = CardType_Silver;
	else if (siWC.isa('StatusItemGoldCards'))
		LastObtainedCardType = CardType_Gold;
}

function ECardType GetLastObtainedCardType()
{
	return (LastObtainedCardType);
}

// Last obtained card info needs to travel with Harry.  In Harry, this data is
// stored as an int instead of an enum.  
function SetLastObtainedCardTypeAsInt(int nLastCardType)
{
	switch (nLastCardType)
	{
	case (nCARDTYPE_BRONZE):
		LastObtainedCardType = CardType_Bronze;
		break;
	case (nCARDTYPE_SILVER):
		LastObtainedCardType = CardType_Silver;
		break;
	case (nCARDTYPE_GOLD):
		LastObtainedCardType = CardType_Gold;
		break;
	default :
		LastObtainedCardType = CardType_None;
		break;
	}
}

function int GetLastObtainedCardTypeAsInt()
{
	switch (LastObtainedCardType)
	{
	case (CardType_Bronze):	return (nCARDTYPE_BRONZE);
	case (CardType_Silver): return (nCARDTYPE_SILVER);
	case (CardType_Gold)  : return (nCARDTYPE_GOLD);
	case (CardType_None)  : return (nCARDTYPE_NONE);
	}
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
	bDisplayJustFirstItem=true
	MenuProps=Menu_Never
	LastObtainedCardType=CardType_None
}



class Characters extends HChar;

const strCUE_LURING_DONE       = "_LuringDone";
const strCUE_OUT_OF_STOCK_DONE = "_OutOfStockDone";

// Enumerate things characters can sell.
enum ESells
{
	Sells_Nimbus2001,     // Improves Harry's Quidditch play (may not use)
	Sells_QArmor,         // Adds protection in Quidditch (may not use)
	Sells_WBark,          // Wiggen tree bark potion ingredient
	Sells_FMucus,         // Flobberworm mucus potion ingredient
	Sells_BronzeCards,    // Bronze wizard cards 
	Sells_SilverCards,    // Silver wizard cards
	Sells_Duel,           // Could Duel
	Sells_Nothing         // Either doesn't ever sell anything, or ran out
};    

// If character can sell something, it is a vendor.  Enumerate the 
// the dialog sets that can be selected for a character to use.
enum EVendorDialog
{
	VDialog_FredWeasley,   // Fred's dialog
	VDialog_GeorgeWeasley, // George's dialog
	VDialog_GenericMale1, 
	VDialog_GenericMale2,      
	VDialog_GenericFemale1,	
	VDialog_GenericFemale2,        
	VDialog_DuelVendor,
	VDialog_None           // If Sells_Nothing
};

// Dialog all vendors have
struct TVendorDialog
{
	var string strLureId;
	var string strSellNimbusId;
	var string strSellQArmorId;
	var string strSellWBarkId;
	var string strSellFMucusId;
	var string strSellBronzeCardsId;
	var string strSellSilverCardsId;
	var string strRanOutOfBeansId;
    var string strNotEnoughBeansId;
	var string strDeclineId;
	var string strTransactionDoneId;
    var string strOutOfStockId;
	var string strSellDuelId;
    var string strNarratorInstrId;      // Narrator describes vendor interaction
    var string strHarryWhatYouGotId;    // Harry's "What do you have"
};

var(Vendor) ESells          CharacterSells;          // What this character sells
var(Vendor) EVendorDialog   VendorDialogSet;         // Vendor dialog set to use
var(Vendor) int             nPriceNimbus2001;        // # beans for Nimbus 2001
var(Vendor) int             nPriceQArmor;            // # beans for Quidditch armor
var(Vendor) int             nPriceWBark;             // # beans for wiggentree bark
var(Vendor) int             nPriceFMucus;            // # beans for flobber worm mucus
var(Vendor) int             nPriceBronzeCardsMin;    // # beans (range) for bronze cards
var(Vendor) int             nPriceBronzeCardsMax;
var(Vendor) int             nPriceSilverCardsMin;    // # beans (range) for silver cards
var(Vendor) int             nPriceSilverCardsMax;
var(Vendor) bool            bLuringEnabled;          // Character will try to lure Harry 
var(Vendor) int             nLureDistance;           // Lure Harry when this close
var(Vendor) int             nFMucusInventoryMin;
var(Vendor) int             nFMucusInventoryMax;
var(Vendor) int             nWBarkInventoryMin;
var(Vendor) int             nWBarkInventoryMax;
var(Vendor) int 			DuelRank;

var TriggerChangeLevel		DuelLevelTrigger;
var			int 			DuelBeans;
var         TVendorDialog   VendorDialog;            // Dialog to use for this character
var         name            nameVendorSavedState;    // State character was in before doing vendor interaction
var         bool            bInLureRange;            // True while Harry is within luring range
var         string          strCurrLureLine;
var         sound           soundCurrVendorPopup;
var         VendorManager   managerVendor;
var         string          strCueVendorGive;
var         Jellybean       VendorJellybean;
var         float           fLureTick;
var         int             nCurrIngrCount;
var         string          LastIngrCountUpdateState;
var         rotator         rSave;

// Persistence
var			name			PersistentState;		// Saved State for Persistence
var			name			PersistentLeadingActor;	// Saved LeadingActor for Persistence

//*************************************************************************************************
event PostBeginPlay()
{
	local Actor A;

    VendorInit();

	if( CutName == "" )
		CutName = string( Name );
	
	// Reset to our last persistent animation and state (if we have them)
	if( bPersistent )
	{
		// Only support characters that are in a LeadingActor state for persistence
		if( PersistentState != '' && 
			PersistentState == 'stateIdle' ||
			PersistentState == 'stateLeadingActorPause' ||
			PersistentState == 'stateLeadingActor'      )
		{
			// find our loaded "persistent" leading actor
			foreach AllActors( class'Actor', A )
			{
				if( A.name == PersistentLeadingActor )
				{
					LeadingActor = A;					
					break;
				}
			}
			
			log("*!* " $self $" P_LOADING: PersistentState: " $PersistentState $" for " $self);
			log("*!* " $self $" P_LOADING: PersistentLeadingActor: " $PersistentLeadingActor
					   $" LeadingActor: " $LeadingActor
					   $" AnimSequence: " $AnimSequence );
			
			// Set our inital state as the saved "persistent" state
			InitialState = PersistentState;
		}
	}

	// set dueling stuff, if has to
	if((DuelRank > 0) && (DuelRank <= 10)) 
	{
		CharacterSells = Sells_Duel;
		VendorDialogSet= VDialog_DuelVendor;
	}

    rSave = rotation;

	super.PostBeginPlay();
}

event OnResolveGameState()
{
    Super.OnResolveGameState();

	// If we are not in the current gamestate then make sure we're not in the 
    // special VendorIdle state (tossing jellybean).
	if( !bInCurrentGameState && IsInState('VendorIdle'))
	{
        GoToState('stateIdle');
	}
}

//*************************************************************************************************
function VendorInit()
{
	// Initialize dialog
	switch (VendorDialogSet)
	{
	case (VDialog_FredWeasley):
		VendorDialog.strLureId                        = "PC_Frd_Vendor_01";
		VendorDialog.strSellNimbusId                  = "PC_Frd_Vendor_37";
		VendorDialog.strSellQArmorId                  = "";				     // only George sells QArmor
		VendorDialog.strSellWBarkId                   = "PC_Frd_Vendor_74";
		VendorDialog.strSellFMucusId                  = "PC_Frd_Vendor_08";
		VendorDialog.strSellBronzeCardsId             = "PC_Frd_Vendor_32";
		VendorDialog.strSellSilverCardsId             = "PC_Frd_Vendor_47";
		VendorDialog.strRanOutOfBeansId               = "PC_Frd_Vendor_20";
        VendorDialog.strNotEnoughBeansId              = "PC_Frd_Vendor_56";
		VendorDialog.strDeclineId                     = "PC_Frd_Vendor_14";
		VendorDialog.strTransactionDoneId             = "PC_Frd_Vendor_26";
        VendorDialog.strOutOfStockId                  = "PC_Frd_Vendor_85";
		VendorDialog.strSellDuelId                    = "";
        VendorDialog.strNarratorInstrId               = "PC_Nar_Vendor_88";
        VendorDialog.strHarryWhatYouGotId             = "PC_Hry_Vendor_07";	
		break;
	case (VDialog_GeorgeWeasley):
		VendorDialog.strLureId                        = "PC_Grg_Vendor_02";
		VendorDialog.strSellNimbusId                  = "";                   // only Fred sells Nimbus2001
		VendorDialog.strSellQArmorId                  = "PC_Grg_Vendor_38";
		VendorDialog.strSellWBarkId                   = "PC_Grg_Vendor_48";
		VendorDialog.strSellFMucusId                  = "PC_Grg_Vendor_09";
		VendorDialog.strSellBronzeCardsId             = "PC_Grg_Vendor_32";
		VendorDialog.strSellSilverCardsId             = "PC_Grg_Vendor_75";
		VendorDialog.strRanOutOfBeansId               = "PC_Grg_Vendor_21";
        VendorDialog.strNotEnoughBeansId              = "PC_Grg_Vendor_57";
		VendorDialog.strDeclineId                     = "PC_Grg_Vendor_15";
		VendorDialog.strTransactionDoneId             = "PC_Grg_Vendor_27";
        VendorDialog.strOutOfStockId                  = "PC_Grg_Vendor_86";
		VendorDialog.strSellDuelId                    = "";
        VendorDialog.strNarratorInstrId               = "PC_Nar_Vendor_88";
        VendorDialog.strHarryWhatYouGotId             = "PC_Hry_Vendor_07";	
		break;
	case (VDialog_GenericMale1):
		VendorDialog.strLureId                        = "PC_Gv1_Vendor_03";
		VendorDialog.strSellNimbusId                  = "";                   // only Fred sells Nimbus2001
		VendorDialog.strSellQArmorId                  = "";				   // only George sells QArmor
		VendorDialog.strSellWBarkId                   = "PC_Gv1_Vendor_39";
		VendorDialog.strSellFMucusId                  = "PC_Gv1_Vendor_10";   
		VendorDialog.strSellBronzeCardsId             = "PC_Gv1_Vendor_33";
		VendorDialog.strSellSilverCardsId             = "PC_Gv1_Vendor_43";
		VendorDialog.strRanOutOfBeansId               = "PC_Gv1_Vendor_22";
        VendorDialog.strNotEnoughBeansId              = "PC_Gv1_Vendor_52";
		VendorDialog.strDeclineId                     = "PC_Gv1_Vendor_16";
		VendorDialog.strTransactionDoneId             = "PC_Gv1_Vendor_28";
        VendorDialog.strOutOfStockId                  = "PC_Gv1_Vendor_80";
		VendorDialog.strSellDuelId                    = "";
        VendorDialog.strNarratorInstrId               = "PC_Nar_Vendor_88";
        VendorDialog.strHarryWhatYouGotId             = "PC_Hry_Vendor_07";	
		break;
	case (VDialog_GenericMale2):
		VendorDialog.strLureId                        = "PC_Gv2_Vendor_04";
		VendorDialog.strSellNimbusId                  = "";                   // only Fred sells Nimbus2001
		VendorDialog.strSellQArmorId                  = "";				      // only George sells QArmor
		VendorDialog.strSellWBarkId                   = "PC_Gv2_Vendor_40";
		VendorDialog.strSellFMucusId                  = "PC_Gv2_Vendor_11";   
		VendorDialog.strSellBronzeCardsId             = "PC_Gv2_Vendor_34";
		VendorDialog.strSellSilverCardsId             = "PC_Gv2_Vendor_44";
		VendorDialog.strRanOutOfBeansId               = "PC_Gv2_Vendor_23";
        VendorDialog.strNotEnoughBeansId              = "PC_Gv2_Vendor_53";
		VendorDialog.strDeclineId                     = "PC_Gv2_Vendor_17";
		VendorDialog.strTransactionDoneId             = "PC_Gv2_Vendor_29";
        VendorDialog.strOutOfStockId                  = "PC_Gv2_Vendor_81";
    	VendorDialog.strSellDuelId                    = "";
        VendorDialog.strNarratorInstrId               = "PC_Nar_Vendor_88";
        VendorDialog.strHarryWhatYouGotId             = "PC_Hry_Vendor_07";	
		break;
	case (VDialog_GenericFemale1):
		VendorDialog.strLureId                        = "PC_Gv3_Vendor_05";
		VendorDialog.strSellNimbusId                  = "";                   // only Fred sells Nimbus2001
		VendorDialog.strSellQArmorId                  = "";				      // only George sells QArmor
		VendorDialog.strSellWBarkId                   = "PC_Gv3_Vendor_41";
		VendorDialog.strSellFMucusId                  = "PC_Gv3_Vendor_12";   
		VendorDialog.strSellBronzeCardsId             = "PC_Gv3_Vendor_35";
		VendorDialog.strSellSilverCardsId             = "PC_Gv3_Vendor_45";
		VendorDialog.strRanOutOfBeansId               = "PC_Gv3_Vendor_24";
        VendorDialog.strNotEnoughBeansId              = "PC_Gv3_Vendor_54";
		VendorDialog.strDeclineId                     = "PC_Gv3_Vendor_18";
		VendorDialog.strTransactionDoneId             = "PC_Gv3_Vendor_30";
        VendorDialog.strOutOfStockId                  = "PC_Gv3_Vendor_82";
		VendorDialog.strSellDuelId                    = "";
        VendorDialog.strNarratorInstrId               = "PC_Nar_Vendor_88";
        VendorDialog.strHarryWhatYouGotId             = "PC_Hry_Vendor_07";	
		break;
	case (VDialog_GenericFemale2):
		VendorDialog.strLureId                        = "PC_Gv4_Vendor_06";
		VendorDialog.strSellNimbusId                  = "";                   // only Fred sells Nimbus2001
		VendorDialog.strSellQArmorId                  = "";				      // only George sells QArmor
		VendorDialog.strSellWBarkId                   = "PC_Gv4_Vendor_42";
		VendorDialog.strSellFMucusId                  = "PC_Gv4_Vendor_13";
		VendorDialog.strSellBronzeCardsId             = "PC_Gv4_Vendor_36";
		VendorDialog.strSellSilverCardsId             = "PC_Gv4_Vendor_46";
		VendorDialog.strRanOutOfBeansId               = "PC_Gv4_Vendor_25";
        VendorDialog.strNotEnoughBeansId              = "PC_Gv4_Vendor_55";
		VendorDialog.strDeclineId                     = "PC_Gv4_Vendor_19";
		VendorDialog.strTransactionDoneId             = "PC_Gv4_Vendor_31";
        VendorDialog.strOutOfStockId                  = "PC_Gv4_Vendor_83";
		VendorDialog.strSellDuelId                    = "";
        VendorDialog.strNarratorInstrId               = "PC_Nar_Vendor_88";
        VendorDialog.strHarryWhatYouGotId             = "PC_Hry_Vendor_07";	
        break;

	default:
        if (CharacterSells != Sells_Nothing)
            Harry(level.PlayerHarryActor).ClientMessage("WARNING: Vendor's VendorDialogSet not assigned");
		break;
	}

    // Duelling vendors placed in the world don't seem to have their VendorDialogSet initialized.
    // Rather than change the maps, we can initialize based on IsVendorDuel().
    
    if (IsDuelVendor())
    {
		VendorDialog.strLureId                        = "";
		VendorDialog.strSellNimbusId                  = "";
		VendorDialog.strSellQArmorId                  = "";
		VendorDialog.strSellWBarkId                   = "";
		VendorDialog.strSellFMucusId                  = "";
		VendorDialog.strSellBronzeCardsId             = "";
		VendorDialog.strSellSilverCardsId             = "";
		VendorDialog.strRanOutOfBeansId               = "";
	    VendorDialog.strNotEnoughBeansId              = "WizardDuel_0005";
		VendorDialog.strDeclineId                     = "";
		VendorDialog.strTransactionDoneId             = "";
		VendorDialog.strOutOfStockId                  = "";
		VendorDialog.strSellDuelId                    = ""; //"WizardDuel_0004";
        VendorDialog.strNarratorInstrId	              = "WizardDuel_0006";  // "WizardDuel_0008";
        VendorDialog.strHarryWhatYouGotId             = "";
    }
}

function string GetVendorHarryInquiryId()
{
    return (VendorDialog.strHarryWhatYouGotId);
}

function string GetVendorInstructionId()
{
    return (VendorDialog.strNarratorInstrId);
}

function string GetVendorLureId()
{
    return (VendorDialog.strLureId);
}

function string GetVendorRanOutOfBeansId()
{
    return (VendorDialog.strRanOutOfBeansId);
}

function string GetVendorNotEnoughBeansId()
{
    return (VendorDialog.strNotEnoughBeansId);
}

function string GetVendorTransactionDoneId()
{
    return (VendorDialog.strTransactionDoneId);
}

function string GetVendorOutOfStockId()
{
    return (VendorDialog.strOutOfStockId);
}

function string GetVendorDeclineId()
{
    return (VendorDialog.strDeclineId);            
}

function string GetSellDialogId()
{
	local string strDialogId;

	switch (CharacterSells)
	{
	case (Sells_Nimbus2001)  : strDialogId = VendorDialog.strSellNimbusId;		break;
	case (Sells_QArmor)      : strDialogId = VendorDialog.strSellQArmorId;		break;
	case (Sells_WBark)       : strDialogId = VendorDialog.strSellWBarkId;		break;
	case (Sells_FMucus)      : strDialogId = VendorDialog.strSellFMucusId;		break;
	case (Sells_BronzeCards) : strDialogId = VendorDialog.strSellBronzeCardsId;	break;
	case (Sells_SilverCards) : strDialogId = VendorDialog.strSellSilverCardsId;	break;
	case (Sells_Duel)        : strDialogId = VendorDialog.strSellDuelId;        break;
	default                  : break;
	}

    //Harry(level.PlayerHarryActor).ClientMessage("GetSellDialogId " $CharacterSells $" " $strDialogId);
	return (strDialogId);
}

// Called from CutScene script.
function bool CutCommand(string command, optional string cue, optional bool bFastFlag)
{
	local string  sActualCommand;
	local string  sCutName;
	local actor   a;

    //Harry(level.PlayerHarryActor).ClientMessage("VendorCutCommand " $command $" " $cue);

	sActualCommand = ParseDelimitedString( command, " ", 1, false );

	if( sActualCommand ~= "SellVendorItem" )
	{
        strCueVendorGive = cue;
        GoToState('VendorGive');
		return (true);        
	}

    return  super.CutCommand(command, cue, bFastFlag);
}

// If Harry bumped into us and we're a vendor with something to sell
event Bump(actor other)
{
	if ((other == level.PlayerHarryActor) && (HaveSomethingToSell() || SellsSomethingButOutOfStock()))
    {
        if (managerVendor == None)
        {
            managerVendor = VendorManager(Fancyspawn(class'VendorManager'));
            managerVendor.SetVendor(self);
        }

        // Flag that we're already in lure range so vendor won't try to lure
        // until we're out of range.  (If player runs into Harry before vendor
        // gets a chance to lure, don't lure when done with interaction until
        // Harry goes out of range and comes back in.)
        bInLureRange = true;

    	InterruptOtherVendorPopup();

	    if (IsInVendorPopupState())
		    StopSound(soundCurrVendorPopup);
	    else
		    nameVendorSavedState = GetStateName();
        
        if (SellsSomethingButOutOfStock())
            GoToState('SayOutOfStockLine');
        else
        {
            managerVendor.DoEngageVendor(nameVendorSavedState);
            GoToState('VendorEngaged');
        }
    }
	else
    {
		super.bump(other);
    }
}

// Loop through all characters, if character is not self, let him know that a
// vendor is interrupting him.
function InterruptOtherVendorPopup()
{
	local Characters character;

	foreach AllActors(class'Characters', character)
	{
		if (character != self)
			character.OtherVendorInterruptedPopup();
	}
} 

// This gets called when some other vendor (not this guy) says a vendor popup line.
// If this guy is in a vendor popup state, cancenl him out of that and go to idle.
function OtherVendorInterruptedPopup()
{
	StopSound(soundCurrVendorPopup);
    if (IsInVendorPopupState())
    {
        DesiredRotation = rSave;
	    GoToState('VendorIdle');
    }
}

// Vendor is in "VendorPopupState" if he's in one of the vendor popupline states.
function bool IsInVendorPopupState()
{
    return (IsInState('SayVendorLureLine') || IsInState('SayOutOfStockLine'));
}

// Get price of what the vendor sells.
function int GetSellingPrice()
{
	switch (CharacterSells)
	{
	case (Sells_Nimbus2001)  : return (nPriceNimbus2001);
	case (Sells_QArmor)      : return (nPriceQArmor);
	case (Sells_WBark)       : return (nPriceWBark);
	case (Sells_FMucus)      : return (nPriceFMucus);
	case (Sells_BronzeCards) : return (RandRange(nPriceBronzeCardsMin, nPriceBronzeCardsMax));
	case (Sells_SilverCards) : return (RandRange(nPriceSilverCardsMin, nPriceSilverCardsMax));
	case (Sells_Duel)        : return (DuelBeans);
	default                  : return (0);
	}
}

// Returns true if this is a vendor AND not out of stock AND harry is not Goyle.
function bool HaveSomethingToSell()
{
    // Vendors don't sell when Harry looks like Goyle
    if (Harry(level.PlayerHarryActor).bIsGoyle)
        return (false);

    // Return true if vendor has something to sell.
	switch (CharacterSells)
	{
	case (Sells_Nimbus2001)  : return (!Harry(level.PlayerHarryActor).bHaveNimbus2001);
	case (Sells_QArmor)      : return (!Harry(level.PlayerHarryActor).bHaveQArmor);
	case (Sells_WBark)       : return (nCurrIngrCount > 0);
	case (Sells_FMucus)      : return (nCurrIngrCount > 0);
	case (Sells_BronzeCards) : return (IsWizardCardAvailable(class'StatusItemBronzeCards'));
	case (Sells_SilverCards) : return (IsWizardCardAvailable(class'StatusItemSilverCards'));
	case (Sells_Duel)        : return (IsDuelVendor());
	default                  : return (false);
	}
}

// Returns True if vendor sells something, but doesn't currently have any left.
function bool SellsSomethingButOutOfStock()
{
    return ((CharacterSells != Sells_Nothing) && !HaveSomethingToSell());
}

// True if there's a wizard card for the vendor to sell.
function bool IsWizardCardAvailable(class<StatusItemWizardCards> classCardItem)
{
	local int nTemp;

	return (GetAvailableWizardCardId(classCardItem, nTemp));
}

// Id of 1st wizard card that a vendor can sell.
function bool GetAvailableWizardCardId(class<StatusItemWizardCards> classCardItem, out int nId)
{
	local StatusManager          managerStatus;
	local StatusGroupWizardCards sgWC;
	local StatusItemWizardCards  siWC;

	managerStatus = Harry(level.PlayerHarryActor).managerStatus;
	sgWC = StatusGroupWizardCards(managerStatus.GetStatusGroup(class'StatusGroupWizardCards'));
	siWC = StatusItemWizardCards(sgWC.GetStatusItem(classCardItem));
	return (siWC.GetFirstVendorCardId(nId));
}

event Tick(float fDeltaTime)
{
	Super.Tick(fDeltaTime);

    // As gamestate changes, so does the vendor's inventory if they are selling
    // flobberworm mucus or wiggentree bark.
    if (CharacterSells == Sells_WBark ||
        CharacterSells == Sells_FMucus)
    {
        // Gamestate has changed since last check
        if (LastIngrCountUpdateState != level.PlayerHarryActor.CurrentGameState)
        {
            // Save off new game state
            LastIngrCountUpdateState = level.PlayerHarryActor.CurrentGameState;

            // Upate inventory counts
            if (CharacterSells == Sells_FMucus)
                nCurrIngrCount = RandRange(nFMucusInventoryMin, nFMucusInventoryMax);
            else if (CharacterSells == Sells_WBark)
                nCurrIngrCount = RandRange(nWBarkInventoryMin, nWBarkInventoryMax);

        }
    }

	// If character can lure
	if (bLuringEnabled && !bHidden)
	{
        // Only check for luring every quarter second or so.
        fLureTick += fDeltaTime;
        if (fLureTick >= 0.25)
        {
            fLureTick = 0.0;

		    // Check to see if in luring range
		    if (VSize2D(level.PlayerHarryActor.location - Location) <= nLureDistance)
		    {
			    // If not already in luring range and no cutscene or popups are playing
			    // try to lure Harry in.
			    if (!bInLureRange &&
   				    HaveSomethingToSell() &&
                    Harry(level.PlayerHarryActor).LineOfSightTo(self) &&
               	    Harry(level.PlayerHarryActor).InFrontOfHarry(self) &&                    
				    !HPHud(Harry(Level.PlayerHarryActor).myHud).IsCutSceneOrPopupInProgress())
			    {
				    // Flag that we're within lure range and have thus handled luring.
				    bInLureRange = true;

				    // Save off state and play lure popup line
				    nameVendorSavedState = GetStateName();
				    GoToState('SayVendorLureLine');
			    }
		    }
		    else
            {
			    bInLureRange = false;
            }
        }
	}
}

// If Harry gets captured while we're luring.
function OnHarryCaptured()
{
   	if (IsInVendorPopupState())
    {
        DesiredRotation = rSave;
    	StopSound(soundCurrVendorPopup);
        GoToState('VendorIdle');
    }
}

// Return the requested Weasley twin.
function Characters GetWeasleyTwin(name nameWeasley)
{
    // There may be multiple Fred's or George's in a level.  So find all of the twin
    // we want and then return the nearest instance of him.

	local Characters Weasley;                  // Instance found
    local Characters NearestWeasley;           // Closest instance so far
    local float      fDistToWeasley;           // Distance to current instance
    local float      fDistToNearestWeasley;    // Closest distance so far

    // Loop thru all instances of the requested twin.
	foreach AllActors(class'Characters', Weasley)
	{
        // If found him
		if (Weasley.IsA(nameWeasley))
        {
            // See how close to Harry he is
            fDistToWeasley = VSize2D(level.PlayerHarryActor.location - Weasley.Location);

            // If first instance found, it's the nearest so far
            if (NearestWeasley == None)
            {
                fDistToNearestWeasley = fDistToWeasley;
                NearestWeasley = Weasley;
            }

            // If found a closer instance
            else if (fDistToWeasley < fDistToNearestWeasley)
            {
                fDistToNearestWeasley = fDistToWeasley;
                NearestWeasley = Weasley;
            }    
        }
	}

    return (NearestWeasley);
}

// Spawn sold object.
function MakePurchase()
{
	local StatusManager         managerStatus;
	local StatusItemWizardCards siWizardCards;
	local int                   nWizardCardId;
	local class<actor>          classSpawn;
	local actor                 aSpawnedObject;
	local vector                vSpawnLoc;
	local rotator               r;
	local vector                vTargetDir;
	local float                 fYawChange;

	switch (CharacterSells)
	{
	case (Sells_Nimbus2001)  : 
		Harry(level.PlayerHarryActor).bHaveNimbus2001 = true;
        classSpawn = class'VendorNimbusBroom';
		break;
	case (Sells_QArmor)      :
		Harry(level.PlayerHarryActor).bHaveQArmor = true;
        classSpawn = class'QArmor';
		break;
	case (Sells_WBark)       : 
		classSpawn = class'WiggentreeBark';		
        --nCurrIngrCount;
		break;
	case (Sells_FMucus)      : 
		classSpawn = class'FlobberwormMucus';	
        --nCurrIngrCount;
		break;
	case (Sells_BronzeCards) : 
        managerStatus = Harry(level.PlayerHarryActor).managerStatus;
		siWizardCards = StatusItemWizardCards(managerStatus.GetStatusItem(class'StatusGroupWizardCards', class'StatusItemBronzeCards'));
		siWizardCards.GetFirstVendorCardIdAndClass(nWizardCardId, classSpawn);
		break;  
	case (Sells_SilverCards) : 
        managerStatus = Harry(level.PlayerHarryActor).managerStatus;
		siWizardCards = StatusItemWizardCards(managerStatus.GetStatusItem(class'StatusGroupWizardCards', class'StatusItemSilverCards'));
		siWizardCards.GetFirstVendorCardIdAndClass(nWizardCardId, classSpawn);
		break;  
	case (Sells_Duel) : 
		break;  
	default                  :                                          
		break;
	}

	if (classSpawn != None)
	{
		// Spawn from character's waist.  Flash a little in front of waist.
		vSpawnLoc = Location;
		vSpawnLoc.z += (CollisionHeight / 4);
		vSpawnLoc += normal(playerHarry.location - location) * (CollisionRadius + 10);
		vSpawnLoc.z += (CollisionHeight / 4);
		spawn(class'Spawn_flash_2',,,vSpawnLoc,rot(0,0,0));
		aSpawnedObject = FancySpawn(classSpawn,,,vSpawnLoc);

		// Calculate direction 45 - 100 degrees to one side of Harry
		r = rotator(playerHarry.location - location);
		fYawChange = RandRange(8192,18205);

		// Randomly pick which side of Harry
		if ((Rand(2)) == 0)
			fYawChange = -fYawChange;
		r.yaw += fYawChange;

		// Direction item will spawn out from character
		vTargetDir = normal(vector(r));

		// Direction is 45-100 degrees to either side of Harry.  Distance
		// from us is 100 to 400.
		aSpawnedObject.Velocity = vTargetDir * RandRange(100,400);

		// Object is in falling state
		aSpawnedObject.SetPhysics( PHYS_Falling );

		// If spawned a wizard card, update so vendor no longer owns the card
		if ((CharacterSells == Sells_BronzeCards) || (CharacterSells == Sells_SilverCards))
			siWizardCards.SetCardOwner(nWizardCardId, siWizardCards.ECardOwner.CardOwner_None);	

		// Play spawn sound appropriate to what was sold
    	switch (CharacterSells)
	    {
	        case (Sells_Nimbus2001)  : PlaySound( sound'HPSounds.Magic_sfx.vendor_spawn_quid');	    break;
	        case (Sells_QArmor)      : PlaySound( sound'HPSounds.Magic_sfx.vendor_spawn_quid');	    break;
	        case (Sells_WBark)       : PlaySound( sound'HPSounds.Magic_sfx.vendor_spawn_ingred');   break;
	        case (Sells_FMucus)      : PlaySound( sound'HPSounds.Magic_sfx.vendor_spawn_ingred');	break;
	        case (Sells_BronzeCards) : PlaySound( sound'HPSounds.Magic_sfx.vendor_spawn_WC');   	break;
	        case (Sells_SilverCards) : PlaySound( sound'HPSounds.Magic_sfx.vendor_spawn_WC');	    break;
	        case (Sells_Duel)        : PlaySound( sound'HPSounds.Magic_sfx.vendor_spawn_WC');	    break;	//???
	        default                  : PlaySound( sound'HPSounds.Magic_sfx.vendor_spawn_ingred');   break;
        }
	}
}

function name GetTalkAnimName()
{
	switch (Rand(3))
	{
	case (0) : return 'Talk_bothhands';
	case (1) : return 'talk_lhand';
	case (2) : return 'talk_lhand';
	}
}

state VendorEngaged
{
    ignores Bump;
}
    

state VendorGive
{
    ignores Bump;

begin:

    // Do vendor give animation then make purchase/spawn the item.
    PlayAnim('Vendor_Give', 1.4 , 0.4);
    Sleep(0.75);
    MakePurchase();
    FinishAnim();

    CutNotifyActor.CutCue(strCueVendorGive);
}

// Say a popup line.  When done, go back to idle state.
function SayPopupLine(string strDialogId, string strCue, name nameAnim)
{
    local string   strDialog;
    local float    fSoundLen;
  	local TimedCue tcue;

	// Get text string and sound that corresponds to dialog id.
	strDialog = Localize( "all", strDialogId, "HPdialog" );
		
	// Play the dialog sound
	soundCurrVendorPopup = Sound(DynamicLoadObject("AllDialog." $strDialogId, class'Sound'));	
	if (soundCurrVendorPopup != None )
	{
		fSoundLen = GetSoundDuration(soundCurrVendorPopup);
	    PlaySound(soundCurrVendorPopup, , , , 10000, , true);
	}
	else
		fSoundLen = (Len(strDialog)*0.01)+3.0;

	strDialog = HandleFacialExpression( strDialog, fSoundLen );

	Harry(level.PlayerHarryActor).MyHud.SetSubtitleText(strDialog, fSoundLen);
	
	// Setup cue if caller wants one
	tcue=spawn(class 'TimedCue');
	tcue.CutNotifyActor=Self;		          //Tell me when done. This is auto passed back to the CutNotifyActor if any.			
	tcue.SetupTimer(fSoundLen+0.5, strCue); //little extra time for slop

    PlayAnim(nameAnim, ,0.75, [Type]AT_Combine);
}

state SayVendorLureLine
{
	function CutCue(string strCue)
	{
        if (strCue ~= strCUE_LURING_DONE)
        {
            DesiredRotation = rSave;
    		GoToState(nameVendorSavedState);
        }
	}

    function BeginState()
    {
    	InterruptOtherVendorPopup();
    }

    function EndState()
    {
    }

begin: 
    // Save current rotation to restore when popup is one.
    rSave = Rotation;

    // Turn to Harry, wave and ask him if he wants to trade.
	TurnToward(level.PlayerHarryActor);
	if (IsA('GeorgeWeasley'))
		GetWeasleyTwin('FredWeasley').TurnToward(level.PlayerHarryActor);
	else if (IsA('FredWeasley'))
		GetWeasleyTwin('GeorgeWeasley').TurnToward(level.PlayerHarryActor);
	SayPopupLine(VendorDialog.strLureId, strCUE_LURING_DONE, 'Wave');

    // Keey turning toward Harry until the strCUE_LURING_DONE cue happens.
keep_facing_harry:

    // As soon as vendor is done with his wave, continue turning toward
    // Harry while in an idle animation.
    if (!IsAnimating())
    {
       	CurrIdleAnimName = GetCurrIdleAnimName();
        LoopAnim(CurrIdleAnimName, [TweenTime]0.4);
    }
	TurnToward(level.PlayerHarryActor);
	if (IsA('GeorgeWeasley'))
		GetWeasleyTwin('FredWeasley').TurnToward(level.PlayerHarryActor);
	else if (IsA('FredWeasley'))
		GetWeasleyTwin('GeorgeWeasley').TurnToward(level.PlayerHarryActor);

    goto 'keep_facing_harry';
}

state SayOutOfStockLine
{
ignores bump;

	function CutCue(string strCue)
	{
        if (strCue ~= strCUE_OUT_OF_STOCK_DONE)
        {
            DesiredRotation = rSave;
    		GoToState(nameVendorSavedState);
        }
	}

    function BeginState()
    {
    	InterruptOtherVendorPopup();
    }

    function EndState()
    {
    }

begin: 
    // Save off rotation to restore when popup is done.
    rSave = Rotation;

    // Turn to Harry and tell him out of stock.
	TurnToward(level.PlayerHarryActor);
	if (IsA('GeorgeWeasley'))
		GetWeasleyTwin('FredWeasley').TurnToward(level.PlayerHarryActor);
	else if (IsA('FredWeasley'))
		GetWeasleyTwin('GeorgeWeasley').TurnToward(level.PlayerHarryActor);
	SayPopupLine(VendorDialog.strOutOfStockId, strCUE_OUT_OF_STOCK_DONE, GetTalkAnimName());

    // Keep turning toward Harry until strCUE_OUT_0F_STOCK_DONE cue happens.
keep_facing_harry:
    // Switch to idle animation after initial gesture animation is done
    if (!IsAnimating())
    {
       	CurrIdleAnimName = GetCurrIdleAnimName();
        LoopAnim(CurrIdleAnimName, [TweenTime]0.4);
    }
	TurnToward(level.PlayerHarryActor);
	if (IsA('GeorgeWeasley'))
		GetWeasleyTwin('FredWeasley').TurnToward(level.PlayerHarryActor);
	else if (IsA('FredWeasley'))
		GetWeasleyTwin('GeorgeWeasley').TurnToward(level.PlayerHarryActor);
	goto 'keep_facing_harry';
}

state VendorIdle
{

    function BeginState()
    {        
        if (!bHidden && (CharacterSells != Sells_Nothing) && (!IsDuelVendor()))
        {
            
            VendorJellybean = Jellybean(FancySpawn(class'Jellybean',self,,,Rotation)); 
            VendorJellybean.bAlignBottom = false;
            VendorJellybean.bRotateToDesired = false;
            VendorJellybean.SetPhysics(PHYS_Falling);
            VendorJellybean.SetCollision( false, false, false );
       	    VendorJellybean.SetOwner(Self);
	        VendorJellybean.AttachToOwner('RightHand');
            VendorJellybean.DrawScale = 0.5;
        }
        else
            GoToState('stateIdle');
    }

    function EndState()
    {
        VendorJellybean.Destroy();
    }

begin:

    do
	{
		PlayAnim('Vendor_Idle', RandRange(0.75,1.3), 0.2);
		FinishAnim();
	}until(false);
}

auto state patrol
{
    function startup()
    {
        if (CharacterSells != Sells_Nothing)
            GoToState('VendorIdle');
        else 
            Super.startup();
    }
}

//////////////////////////////////////////////////////////////////////////////////////////
// DUELLING SELECTION STUFF
//////////////////////////////////////////////////////////////////////////////////////////

function bool IsDuelVendor()
{
	if(CharacterSells != Sells_Duel)
		return false;

	if(DuelRank <= 0)
		return false;

	if(DuelRank > 10)
		return false;

	return true;
}

function SetEverythingForTheDuel()
{
	local string DuelLevelName;
	local int nGameState;

	if(DuelRank <= 0)
		return;

	if(DuelRank > 10)
		return;

	if(DuelRank > playerHarry.DuelRankHarry)
		return;

	nGameState=playerHarry.ConvertGameStateToNumber();

	// no duelling kids for game state less then 80
	if(nGameState < 80)	
		return;

	// show duel vendor
	bHidden = false;

	// and set his/her collisions
	SetCollision(true, true, true);

	switch(DuelRank)
	{
		case 1:	 												// HA_Griffindor,	Intellect = 0.0
			DuelBeans =  10; 
			DuelLevelName = "Duel01";
			Mesh = SkeletalMesh'HPModels.skhp2_genmale1Mesh';
			Skin = none;
			break;
		case 2:													// HA_Ravenclaw,	Intellect = 0.2
			DuelBeans =  15; 
			DuelLevelName = "Duel02";
			Mesh = SkeletalMesh'HPModels.skhp2_genfemale1Mesh';
			Skin = Texture'HPModels.Skins.skhp2_genfemale1_6Tex0';
			break;
		case 3:													// HA_Hufflepuf,	Intellect = 0.3
			DuelBeans =  20;	
			DuelLevelName = "Duel03";
			Mesh = SkeletalMesh'HPModels.skhp2_genfemale1Mesh';
			Skin = Texture'HPModels.Skins.skhp2_genfemale1_4Tex0';
			break;
		case 4:													// HA_Slytherin,	Intellect = 0.4
			DuelBeans = 25;
			DuelLevelName = "Duel04";
			Mesh = SkeletalMesh'HPModels.skhp2_genmale1Mesh';
			Skin = Texture'HPModels.Skins.skhp2_genmale1_2Tex0';
			break;
		case 5:													// HA_Griffindor,	Intellect = 0.5
			DuelBeans = 35; 
			DuelLevelName = "Duel05";
			Mesh = SkeletalMesh'HPModels.skhp2_genmale1Mesh';
			Skin = none;
			break;
		case 6:													// HA_Slytherin,	Intellect = 0.6
			DuelBeans = 50; 
			DuelLevelName = "Duel06";
			Mesh = SkeletalMesh'HPModels.skhp2_genfemale1Mesh';
			Skin = Texture'HPModels.Skins.skhp2_genfemale1_3Tex0';
			break;
		case 7:													// HA_Ravenclaw,	Intellect = 0.7
			DuelBeans = 70; 
			DuelLevelName = "Duel07";
			Mesh = SkeletalMesh'HPModels.skhp2_genmale1Mesh';
			Skin = Texture'HPModels.Skins.skhp2_genmale1_7Tex0';
			break;
		case 8:													// HA_Slytherin,	Intellect = 0.8
			DuelBeans = 100; 
			DuelLevelName = "Duel08";
			Mesh = SkeletalMesh'HPModels.skhp2_genfemale1Mesh';
			Skin = Texture'HPModels.Skins.skhp2_genfemale1_2Tex0';
			break;
		case 9:													// HA_Griffindor,	Intellect = 0.9
			DuelBeans = 150; 
			DuelLevelName = "Duel09";
			Mesh = SkeletalMesh'HPModels.skhp2_genfemale1Mesh';
			Skin = Texture'HPModels.Skins.skhp2_genfemale1_0Tex0';
			break;
		case 10:												// HA_Slytherin,	Intellect = 0.8
			DuelBeans = 200; 
			DuelLevelName = "Duel10";
			Mesh = SkeletalMesh'HPModels.skhp2_genmale1Mesh';
			Skin = Texture'HPModels.Skins.skhp2_genmale1_3Tex0';
			break;
		default: 
			DuelBeans = 0; 
			DuelLevelName = "";
			break;
	}

	if(DuelLevelTrigger == none )
		DuelLevelTrigger = spawn(class'TriggerChangeLevel', , , , );

	if(DuelLevelTrigger != none )
	{
		DuelLevelTrigger.NewMapName = DuelLevelName;
		DuelLevelTrigger.SetCollision(false, false, false);
	}

// Now setup in VendorInit
/*
	VendorDialog.strLureId                        = "";
	VendorDialog.strSellNimbusId                  = "";
	VendorDialog.strSellQArmorId                  = "";
	VendorDialog.strSellWBarkId                   = "";
	VendorDialog.strSellFMucusId                  = "";
	VendorDialog.strSellBronzeCardsId             = "";
	VendorDialog.strSellSilverCardsId             = "";
	VendorDialog.strRanOutOfBeansId               = "";
	VendorDialog.strNotEnoughBeansId              = "WizardDuel_0005";
	VendorDialog.strDeclineId                     = "";
	VendorDialog.strTransactionDoneId             = "";
	VendorDialog.strOutOfStockId                  = "";
	VendorDialog.strSellDuelId                    = ""; //"WizardDuel_0004";

    strNarratorInstrId	= "WizardDuel_0006";  // "WizardDuel_0008";
    strHarryWhatYouGotId= "";
*/
}

//////////////////////////////////////////////////////////////////////////////////////////

defaultproperties
{
	CharacterSells=Sells_Nothing
	VendorDialogSet=VDialog_None
	nPriceNimbus2001=700
	nPriceQArmor=500
	nPriceWBark=20
	nPriceFMucus=10
	nPriceBronzeCardsMin=120
	nPriceBronzeCardsMax=230
	nPriceSilverCardsMin=200
	nPriceSilverCardsMax=300
	nLureDistance=300
    nFMucusInventoryMin=2
    nFMucusInventoryMax=5
    nWBarkInventoryMin=2
    nWBarkInventoryMax=5

	bDoEyeBlinks=true

	bCantStandOnMe=true
	bSnapToPatrolPoint=true
}
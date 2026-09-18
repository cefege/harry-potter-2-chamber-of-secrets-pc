
class VendorManager extends Actor;

// Hud texture names
const strVENDORBAR_LEFT        = "HP_Menu.Hud.VendorBarLeft";
const strVENDORBAR_RIGHT       = "HP_Menu.Hud.VendorBarRight";
const strVENDORBUTTON_GREY     = "HP_Menu.Hud.VendorButtonGrey";
const strVENDORBUTTON_OVER     = "HP_Menu.Hud.VendorButtonOver";
const strVENDORBUTTON_NORMAL   = "HP_Menu.Hud.VendorButtonNormal";
const strVENDORITEM_NIMBUS     = "HP_Menu.Hud.Nimbus2001";
const strVENDORITEM_QARMOR     = "HP_Menu.Hud.QuidditchArmor";
const strVENDORITEM_FMUCUS     = "HP_Menu.Hud.VendorFMucus";
const strVENDORITEM_WBARK      = "HP_Menu.Hud.VendorWBark";
const strVENDORITEM_SILVERCARD = "HP_Menu.Hud.VendorSilverCard";
const strVENDORITEM_BRONZECARD = "HP_Menu.Hud.VendorBronzeCard";

// Dialog database text
const strID_YES    = "Shared_Menu_0003";
const strID_NO     = "Shared_Menu_0004";
const strID_CANCEL = "Localized_kn_0003";

// Hud coordinates
const fVENDORBAR_W               = 348.0;
const fVENDORBAR_Y               =  50.0;
const fVENDORBAR_PRICE_X         =  35.0;
const fVENDORBAR_PRICE_Y         =  59.0;
const fVENDORBAR_BUTTON_W        = 106.0;
const fVENDORBAR_BUTTON_H        =  19.0;
const fVENDORBAR_YESBUTTON_X     = 226.0;
const fVENDORBAR_YESBUTTON_Y     =  16.0; //7.0;
const fVENDORBAR_NOBUTTON_X      = 226.0;
const fVENDORBAR_NOBUTTON_Y      =  43.0; //30.0;
const fVENDORBAR_CANCELBUTTON_X  = 226.0;
const fVENDORBAR_CANCELBUTTON_Y  =  53.0;
const fVENDORBAR_PURCHASE_ITEM_X = 152.0;
const fVENDORBAR_PURCHASE_ITEM_Y =   7.0;
const fVENDORBAR_BUTTON_TEXT_X   =  53.0;
const fVENDORBAR_BUTTON_TEXT_Y   =  12.0;

const strQUESTION_ANIM_PARAM     = " startanim=talk_question";
const strBOTHHANDS_ANIM_PARAM    = " startanim=talk_bothhands";
const strLEFTHAND_ANIM_PARAM     = " startanim=talk_lhand";
const strRIGHTHAND_ANIM_PARAM    = " startanim=talk_rhand";
const strVENDOR_WAIT_ANIM_PARAM  = " loopanim=vendor_idle2";
const strINDEFINITE_TEXT_PARAM   = " IndefiniteText";
const strTALK_COMMAND            = "TALK ";
const strSAY_COMMAND             = "SAY ";
const strFACE_HARRY_COMMAND      = "TURNTO harry";
const strCAPTURE_COMMAND         = "CAPTURE";
const strRELEASE_COMMAND         = "RELEASE";
const strFLYTO_COMMAND           = "FLYTO ";
const strTARGET_FLYTO_COMMAND    = "TARGET FLYTO ";

const strCUE_VENDOR_TURN_DONE    = "_VendorTurnDone";
const strCUE_CAMERA_IN_POSITION  = "_CameraInPosition";
const strCUE_ITEM_SOLD           = "_ItemSold";
const strCUE_HARRY_INQUIRY       = "_HarryInquiry";
const strCUE_IHAVE_X             = "_IHaveX";
const strCUE_INSTRUCTIONS        = "_Instructions";
const strCUE_TRANSACTION_DONE    = "_TransactionDone";
const strCUE_DECLINE_SALE        = "_DeclineSale";
const strCUE_NOT_ENOUGH_BEANS    = "_NotEnoughBeans";
const strCUE_RAN_OUT_OF_BEANS    = "_RanOutOfBeans";
const strCUE_OUT_OF_STOCK        = "_OutOfStock";

const strTEMP_VENDOR_CUT_NAME    = "TempVendorCutName";

var Characters      Vendor;
var name            nameVendorSavedState;     // State character was in before doing vendor interaction
var string          strVendorSavedCutName;
var int             nLastButtonYes;           // Previous value of VendorYesButton
var int             nLastButtonNo;            // Previous value of VendorNoButton 
var int             nItemsBoughtInCurrTransaction; // Items Harry bought in current "engagement"
var int             nCurrPrice;               // Price of item being bought
var StatusManager   managerStatus;            
var Canvas          VendorCanvas;             // save off the canvas for use when it's not passed in as a param
var int             i;                        // generic loop var

var texture         textureVendorBarLeft;
var texture         textureVendorBarRight;
var texture         textureVendorButtonGrey;
var texture         textureVendorButtonOver;
var texture         textureVendorButtonNormal;
var texture         textureItemToSell;

var string strYes;
var string strNo;
var string strCancel;

function SetVendor(Characters V)
{
    Vendor = V;
}

// Harry will pass player input along to us once vendor has been registered.
// Only care about it in specific states though.
event PlayerInput( float DeltaTime )
{
}

function DoEngageVendor(name nameSaveState)
{
    local StatusGroup sgJellyBeans;     // Status group for jellybeans hud 
    local string      strItemTexture;   // Name of "item to sell" texture

    // Save off vendor's state
    nameVendorSavedState = nameSaveState;

    // We need the vendor to have a cutname.  So give the vendor a temporary one
    // if he doesn't already have one.  His old cutname will be restored in
    // DisengageVendor().
    if (Vendor.CutName == "")
        strVendorSavedCutName = Vendor.CutName;
    Vendor.CutName = strTEMP_VENDOR_CUT_NAME;

	// Load vendor hud textures if not already loaded for this VendorManager.
	if (textureVendorBarLeft == None)
	{
        // Load generic textures
		textureVendorBarLeft      = texture(DynamicLoadObject(strVENDORBAR_LEFT, class'Texture'));
		textureVendorBarRight     = texture(DynamicLoadObject(strVENDORBAR_RIGHT, class'Texture'));
		textureVendorButtonGrey   = texture(DynamicLoadObject(strVENDORBUTTON_GREY, class'Texture'));
		textureVendorButtonOver   = texture(DynamicLoadObject(strVENDORBUTTON_OVER, class'Texture'));
        textureVendorButtonNormal = texture(DynamicLoadObject(strVENDORBUTTON_NORMAL, class'Texture'));

        // Get name of the texture for the item being sold
        switch (Vendor.CharacterSells)
        {
        case (Vendor.ESells.Sells_Nimbus2001):  strItemTexture = strVENDORITEM_NIMBUS;      break;
        case (Vendor.ESells.Sells_QArmor):      strItemTexture = strVENDORITEM_QARMOR;      break;
        case (Vendor.ESells.Sells_WBark):       strItemTexture = strVENDORITEM_WBARK;       break;
        case (Vendor.ESells.Sells_FMucus):      strItemTexture = strVENDORITEM_FMUCUS;      break;
        case (Vendor.ESells.Sells_BronzeCards): strItemTexture = strVENDORITEM_BRONZECARD;  break;
        case (Vendor.ESells.Sells_SilverCards): strItemTexture = strVENDORITEM_SILVERCARD;  break;
        default :
            break;
        }

        // Load texture for the item being sold
        textureItemToSell = texture(DynamicLoadObject(strItemTexture, class'Texture'));

	}

    // Get text for buttons
	strYes    = (Localize("All", strID_YES,    "HPMenu"));
    strNo     = (Localize("All", strID_NO,     "HPMenu"));
    strCancel = (Localize("All", strID_CANCEL, "HPMenu"));

	// Register vendor manager with the hud for vendor specific hud drawing
	HPHud(Harry(Level.PlayerHarryActor).myHud).RegisterVendorManager(self);

	// Setup Harry to be engaged with vendor
	Harry(Level.PlayerHarryActor).StartVendorEngagement(self);

	// Permanently show jellybean count while in transaction
    sgJellybeans = Harry(Level.PlayerHarryActor).managerStatus.GetStatusGroup(class'StatusGroupJellybeans');
	sgJellybeans.SetEffectTypeToPermanent();
    sgJellybeans.SetCutSceneRenderMode(true);

	// Go into cutscene mode
	level.playerHarryActor.CutNotifyActor = self;
    Harry(level.playerHarryActor).cam.CutNotifyActor = self;
    Vendor.CutNotifyActor = self;
	level.playerHarryActor.CutCommand(strCAPTURE_COMMAND);
    Harry(level.playerHarryActor).cam.CutCommand(strCAPTURE_COMMAND);
    Vendor.CutCommand(strCAPTURE_COMMAND);

	nItemsBoughtInCurrTransaction = 0;
	nCurrPrice = Vendor.GetSellingPrice();

    GoToState('EngageVendor');
}

function string GetAnimParam()
{
	switch (Rand(3))
	{
	case (0) : return strRIGHTHAND_ANIM_PARAM;
	case (1) : return strLEFTHAND_ANIM_PARAM;
	case (2) : return strBOTHHANDS_ANIM_PARAM;
	}
}

// The console code will draw the mouse and update it's positioning.  Set the
// appropriate flags to on or off.
function SetConsoleVendorBarFlags(bool bSet)
{
    hpConsole(level.playerHarryActor.player.console).bVendorBar = bSet;
    hpConsole(level.playerHarryActor.player.console).bUWindowActive = bSet;    
    hpConsole(level.playerHarryActor.player.console).Viewport.bShowWindowsMouse = bSet;
}

function DoCutTalk(actor actorTalk, string strDialogId, string strTalkAnimName, 
                   string strLoopAnimName, string strIndefiniteParam, string strCue)
{
    local string   strDialog;
    local TimedCue tcue;
    local float    fSoundLen;

    // If vendor has any dialog setup for the requested line (duelists won't always).
    if (strDialogId == "")
        CutCue(strCue);
    else
    {
        // Dialog for "duel" vendors didn't get in under the deadline so we use
        // text without sound.  So, if we have a normal vendor, the use regular cutcommands
        // (with sound).  If we have a "duel" vendor, we just show their dialog in text
        // and send out a cue after it's been up long enough.
		if(!Vendor.IsDuelVendor())
            actorTalk.CutCommand(strTALK_COMMAND $strDialogId $strTalkAnimName $strLoopAnimName $strIndefiniteParam, strCue);
        else
        {
            // Get the text and how long to show it.
        	strDialog = (Localize( "All", strDialogId,"HPMenu" ));
    		fSoundLen = (Len(strDialog)*0.01)+3.0;

            // If not "indefinite", show text for desired len.
            if (strIndefiniteParam == "")
            {
                // Send out cue to ourselves when text should go away
        	    tcue=spawn(class 'TimedCue');           
	            tcue.CutNotifyActor=Self;	
	            tcue.SetupTimer(fSoundLen+0.5, strCue); //little extra time for slop

                Harry(level.playerHarryActor).ClientMessage("Indefinite");

                // Show text.
                Harry(level.playerHarryActor).MyHud.SetSubtitleText(strDialog, fSoundLen);
            }

            // If we do want to leave the line up indefinitely, show the text and send
            // cue right away.
            else
            {
                Harry(level.playerHarryActor).MyHud.SetSubtitleText(strDialog, 0);
                CutCue(strCue);
            }
        }
    }
}

function DoNarratorInstructions()
{
    if (Vendor.GetVendorInstructionId() == "")
        CutCue(strCUE_INSTRUCTIONS);
    else
    {
        // If dueling vendor, line doesn't actually get said, the text is just displayed.  Go
        // thru DoCutTalk because it will handle duel vendor "no sound" cases.
        if(Vendor.IsDuelVendor())
            DoCutTalk(level.playerHarryActor, Vendor.GetVendorInstructionId(), "", "", strINDEFINITE_TEXT_PARAM, strCUE_INSTRUCTIONS);

        // For regular vendors, want to make sure we use the say command because this is coming from 
        // the narrator and there is no narrator on the screen.  If we force Harry to say the line,
        // Harry will do his hand gestures.
        else
            CutCommand(strSAY_COMMAND $Vendor.GetVendorInstructionId() $strINDEFINITE_TEXT_PARAM, strCUE_INSTRUCTIONS);
    }
}

function bool WantInstructions()
{
    return (Harry(level.PlayerHarryActor).bSaidVendorInstructions == false || Vendor.IsDuelVendor());
}

auto state Idle
{
}

state EngageVendor
{

	function CutCue(string cue)
	{
        local string strStayUpText;
        local float  fSoundLenDummy;

        //Harry(level.playerHarryActor).ClientMessage("cutcue " $cue);

        // Harry: "What have you got?"
		if (cue ~= strCUE_VENDOR_TURN_DONE)
        {
            Harry(level.playerHarryActor).cam.CutCommand(strFLYTO_COMMAND $Vendor.Cutname $" x=80 y=80");
            Harry(level.playerHarryActor).cam.CutCommand(strTARGET_FLYTO_COMMAND $Vendor.Cutname $" x=10 z=10", strCUE_CAMERA_IN_POSITION);
        }

        else if (cue ~= strCUE_CAMERA_IN_POSITION)
        {
            DoCutTalk(level.playerHarryActor, Vendor.GetVendorHarryInquiryId(), strQUESTION_ANIM_PARAM, "", "", strCUE_HARRY_INQUIRY);
        }

        // Vendor: "I have X"
        else if (cue ~= strCUE_HARRY_INQUIRY)
        {
	        if (Vendor.HaveSomethingToSell())
            {
                // If instructions are going to play, line stays up until done.  If don't want instructions,
                // line stays up indefinitely (and will go away when player presses Yes/No).
                if (WantInstructions())
                    DoCutTalk(Vendor, Vendor.GetSellDialogId(), GetAnimParam(), strVENDOR_WAIT_ANIM_PARAM, "" , strCUE_IHAVE_X);
                else
                    DoCutTalk(Vendor, Vendor.GetSellDialogId(), GetAnimParam(), strVENDOR_WAIT_ANIM_PARAM, strINDEFINITE_TEXT_PARAM, strCUE_IHAVE_X);
            }
            else
                DoCutTalk(Vendor, Vendor.GetVendorOutOfStockId(), GetAnimParam(), "", "", strCUE_OUT_OF_STOCK);
        }
         
        else if (cue ~= strCUE_OUT_OF_STOCK)
        {
            DoDisengageVendor();
        }

        // Depending on state of things, tell Harry he needs more beans, or give instructions 
        // or begin transaction.
        else if (cue ~= strCUE_IHAVE_X)
        {
            // Tell player how vendors work if they haven't already been told.  
            // Or, always tell player if they are duelling vendors (because we have no
            // sound for them and this might help to clear things up).
            if (WantInstructions())            
            {
                DoNarratorInstructions();
                Harry(level.PlayerHarryActor).bSaidVendorInstructions = true;
            }

            // Put up vendor hud and wait for player input
            else
                GoToState('VendorTransaction');
        }

        // Put up vendor hud and wait for player input
        else if (cue ~= strCUE_INSTRUCTIONS)
        {
            GoToState('VendorTransaction');
        }
 	}

begin:
	level.PlayerHarryActor.Acceleration = vect(0,0,0);
	level.PlayerHarryActor.Velocity *= vect(0,0,1);

	//Keep the turnto in 2d land so harry doesn't tilt.
	Harry(level.PlayerHarryActor).TurnTo( level.PlayerHarryActor.location + (Vendor.location-level.PlayerHarryActor.location)*vect(1,1,0) );

    Vendor.CutCommand(strFACE_HARRY_COMMAND, strCUE_VENDOR_TURN_DONE);

    // Vendor turns toward Harry.  If vendor is George or Fred, make sure the other
    // twin turns, too.
	if (Vendor.IsA('GeorgeWeasley'))
		(Vendor.GetWeasleyTwin('FredWeasley')).CutCommand(strFACE_HARRY_COMMAND);
	else if (Vendor.IsA('FredWeasley'))
		(Vendor.GetWeasleyTwin('GeorgeWeasley')).CutCommand(strFACE_HARRY_COMMAND);
}

state VendorTransaction
{
	function RenderHud(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		local StatusGroup sgJellybeans;

        sgJellybeans = Harry(Level.PlayerHarryActor).managerStatus.GetStatusGroup(class'StatusGroupJellybeans');
        sgJellybeans.RenderHudItemManager(canvas, bMenuMode, bFullCutMode, bHalfCutMode);
	    DrawVendorBar(canvas, true);
	}

	function CutCue(string cue)
	{
        //Harry(level.playerHarryActor).ClientMessage("cutcue " $cue);

		if (cue ~= strCUE_TRANSACTION_DONE ||
            cue ~= strCUE_DECLINE_SALE     ||
            cue ~= strCUE_OUT_OF_STOCK)
        {
            DoDisengageVendor();
        }
    }
    
	event PlayerInput( float fDeltaTime )
	{
		// Pressed yes button
        if ((Harry(level.PlayerHarryActor).bVendorReply == 1) && IsMouseOverVendorYes())
		{
            SetConsoleVendorBarFlags(false);

            // Vendor has run out of stuff to sell
            // Vendor: "Sorry, out of stock"
	        if (!Vendor.HaveSomethingToSell())	
            {
                DoCutTalk(Vendor, Vendor.GetVendorOutOfStockId(), GetAnimParam(), "", "", strCUE_OUT_OF_STOCK);
            }
		
	        // If Harry does not have enough beans to buy another item, go to not enough beans state
	        else if (!HarryHasEnoughBeans(nCurrPrice))
            {
                GoToState('NotEnoughBeans');
            }
            
            else
            {
        	    // Increment count of items that have been bought since Harry bumped into us.
	            nItemsBoughtInCurrTransaction++;

                // Vendor spawns the purchase
                GoToState('MakePurchase');
            }
		}	

		// Just pressed "no" key
		else if ((Harry(level.PlayerHarryActor).bVendorReply == 1) && IsMouseOverVendorNo())
		{
            SetConsoleVendorBarFlags(false);

			// If Harry bought at least 1 item, "pleasure doing business with you" then we're done
			if (nItemsBoughtInCurrTransaction > 0)
                DoCutTalk(Vendor, Vendor.GetVendorTransactionDoneId(), GetAnimParam(), "", "", strCUE_TRANSACTION_DONE);

			// Harry didn't buy anything, "suit yourself"
			else
                DoCutTalk(Vendor, Vendor.GetVendorDeclineId(), GetAnimParam(), "", "", strCUE_DECLINE_SALE);
		}
	}

    event BeginState()
    {
        SetConsoleVendorBarFlags(true);
    }

    event EndState()
    {
        SetConsoleVendorBarFlags(false);
    }

begin:

    Vendor.LoopAnim('Vendor_Idle2', RandRange(0.8, 1.2), 0.2);
}

state MakePurchase
{
	function RenderHud(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		local StatusGroup sgJellybeans;

		sgJellybeans = Harry(Level.PlayerHarryActor).managerStatus.GetStatusGroup(class'StatusGroupJellybeans');
		sgJellybeans.RenderHudItemManager(canvas, bMenuMode, bFullCutMode, bHalfCutMode);

        // Hide vendor bar during purchase because key press not recognized here.
		//DrawVendorBar(canvas, true, (fYesShowTime > 0), (fNoShowTime > 0));
	}

	function CutCue(string cue)
	{        
        //Harry(level.playerHarryActor).ClientMessage("cutcue " $cue);

        // Vendor done spawning the item (consider it sold)
		if (cue ~= strCUE_ITEM_SOLD)
        {
			// do not offer duel any more.
			if(Vendor.IsDuelVendor())
				Vendor.DuelRank = 0;

            // Vendor has run out of stuff to sell
            // Vendor: "Pleasure doing business" + "Sorry, out of stock"
	        if (!Vendor.HaveSomethingToSell())	
                DoCutTalk(Vendor, Vendor.GetVendorTransactionDoneId(), GetAnimParam(), "", "", strCUE_TRANSACTION_DONE);
			
            // Vendor offers to sell more.
            else            
                GoToState('VendorTransaction');
        }
        
        // Vendor: "I'm out of stock"
        else if (cue ~= strCUE_TRANSACTION_DONE)
            DoCutTalk(Vendor, Vendor.GetVendorOutOfStockId(), GetAnimParam(), "", "", strCUE_OUT_OF_STOCK);

        // Give control back to player
        else if (cue ~= strCUE_OUT_OF_STOCK)
            DoDisengageVendor();
    }

begin:
	managerStatus = Harry(level.PlayerHarryActor).managerStatus;

	for (i=0; i<nCurrPrice; i++)
	{
		managerStatus.IncrementCount(class'StatusGroupJellybeans', class'StatusItemJellybeans', -1);
		Sleep(0.01);					 
	}

	if(Vendor.IsDuelVendor())
	{
		if(Vendor.IsDuelVendor() && (Vendor.DuelLevelTrigger != none))
		{
			// In Duel Mode we have to know this info
			Harry(level.PlayerHarryActor).DuelRankOppon = Vendor.DuelRank;
			Harry(level.PlayerHarryActor).DuelRankBeans = Vendor.DuelBeans;

			Vendor.DuelLevelTrigger.ProcessTrigger();
		}
	}

    // Vendor spawns the item just bought
    Vendor.CutCommand("SellVendorItem"  $GetAnimParam(), strCUE_ITEM_SOLD);
}

state NotEnoughBeans
{
	function RenderHud(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		local StatusGroup sgJellybeans;

        sgJellybeans = Harry(Level.PlayerHarryActor).managerStatus.GetStatusGroup(class'StatusGroupJellybeans');
		sgJellybeans.RenderHudItemManager(canvas, bMenuMode, bFullCutMode, bHalfCutMode);

        // Don't draw vendor bar here afterall- don't think it seems right
		//DrawVendorBar(canvas, false);
	}

	function CutCue(string cue)
	{
		if (cue ~= strCUE_RAN_OUT_OF_BEANS||
            cue ~= strCUE_NOT_ENOUGH_BEANS)
            DoDisengageVendor();
    }

begin: 

    // If nothing has been bought and not enough beans
    // Vendor: "But you don't have enough beans"
	if (nItemsBoughtInCurrTransaction == 0)
        DoCutTalk(Vendor, Vendor.GetVendorNotEnoughBeansId(), GetAnimParam(), "", "", strCUE_NOT_ENOUGH_BEANS);

    // If some transactions were made before running out of beans
    // Vendor: "Sorry, you ran out of beans"
    else
        DoCutTalk(Vendor, Vendor.GetVendorRanOutOfBeansId(), GetAnimParam(), "", "", strCUE_RAN_OUT_OF_BEANS);
}

function DoDisengageVendor()
{
    local StatusGroup sgJellybeans;
    local Characters  WeasleyTwin;

    // Release everyone
    Harry(level.playerHarryActor).cam.CutCommand(strRELEASE_COMMAND);
    Harry(level.playerHarryActor).cam.CutNotifyActor = None;
	level.playerHarryActor.CutCommand(strRELEASE_COMMAND);
	level.playerHarryActor.CutNotifyActor = None;
    Vendor.CutCommand(strRELEASE_COMMAND);
	Vendor.CutNotifyActor = None;

    // Let Harry know he's done.
	Harry(level.PlayerHarryActor).EndVendorEngagement();

    // Restore jellybean hud
    sgJellybeans = Harry(Level.PlayerHarryActor).managerStatus.GetStatusGroup(class'StatusGroupJellybeans');
	sgJellybeans.SetEffectTypeToNormal();
    sgJellybeans.SetCutSceneRenderMode(true);

    // Return vendor to proper rotation and state
    Vendor.DesiredRotation = Vendor.rSave;
	Vendor.GoToState(nameVendorSavedState);

    // Restore original vendor cutname
    Vendor.CutName = strVendorSavedCutName;

    // If vendor is Fred or George, make sure twin is restored to original rotation.
	if (Vendor.IsA('GeorgeWeasley'))
    {
        WeasleyTwin = Vendor.GetWeasleyTwin('FredWeasley');
        WeasleyTwin.DesiredRotation = WeasleyTwin.rSave;
    }
	else if (Vendor.IsA('FredWeasley'))
    {
		WeasleyTwin = Vendor.GetWeasleyTwin('GeorgeWeasley');
        WeasleyTwin.DesiredRotation = WeasleyTwin.rSave;
    }

    // Manager goes into idle state
    GoToState('Idle');
}


function bool HarryHasEnoughBeans(int nPrice)
{
	local StatusItem siBeans;

	siBeans = Harry(level.PlayerHarryActor).managerStatus.GetStatusItem(class'StatusGroupJellybeans', class'StatusItemJellybeans');
	return (siBeans.nCount >= nPrice);
}

function RenderHud(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
{
}

function DrawVendorBar(Canvas canvas, bool bEnabled)
{
	local texture    textureYesButton, textureNoButton, textureCancelButton;
	local float      fBarX, fBarY;
	local string     strCurrPrice;
	local color      colorSave;
	local font       fontSave;
	local float      fXTextLen, fYTextLen;
	local float      fScaleFactor;
    local StatusItem siJellybeans;

    VendorCanvas = Canvas;

	// Save off canvas properties
	colorSave = Canvas.DrawColor;
	fontSave  = Canvas.Font;

	// Get scale for different screen resolutions.
	fScaleFactor = Canvas.GetHudScaleFactor();

    // If Yes/No allowed
	if (bEnabled)
    {
        if (IsMouseOverVendorYes())
        {
            textureYesButton    = textureVendorButtonOver;
            textureNoButton     = textureVendorButtonNormal;
            textureCancelButton = textureVendorButtonNormal;
        }   
        else if (IsMouseOverVendorNo())
        {
            textureYesButton    = textureVendorButtonNormal;
            textureNoButton     = textureVendorButtonOver;
            textureCancelButton = textureVendorButtonNormal;         
        }
        else if (IsMouseOverVendorCancel())
        {
            textureYesButton    = textureVendorButtonNormal;
            textureNoButton     = textureVendorButtonNormal;
            textureCancelButton = textureVendorButtonOver;            
        }
        else
        {
            textureYesButton    = textureVendorButtonNormal;
            textureNoButton     = textureVendorButtonNormal;
            textureCancelButton = textureVendorButtonNormal;
        }
    }

    // Grey out Yes/No
	else
    {
		textureYesButton    = textureVendorButtonGrey;
        textureNoButton     = textureVendorButtonGrey;
        textureCancelButton = textureVendorButtonNormal;
    }
    
	// Display bar
	fBarX = GetVendorBarX(Canvas);
	fBarY = GetVendorBarY(Canvas);
	canvas.SetPos(fBarX,fBarY);
	canvas.DrawIcon(textureVendorBarLeft, fScaleFactor);
    canvas.SetPos(fBarX + (256 * fScaleFactor), fBarY);
    canvas.DrawIcon(textureVendorBarRight, fScaleFactor);

    // Display item to sell
    canvas.SetPos(fBarX + (fVENDORBAR_PURCHASE_ITEM_X * fScaleFactor), fBarY + (fVENDORBAR_PURCHASE_ITEM_Y * fScaleFactor));
    canvas.DrawIcon(textureItemToSell, fScaleFactor);

	// Draw yes button
	canvas.SetPos(fBarX + (fVENDORBAR_YESBUTTON_X * fScaleFactor), fBarY + (fVENDORBAR_YESBUTTON_Y * fScaleFactor));
	canvas.DrawIcon(textureYesButton, fScaleFactor);

	// Draw no button
	canvas.SetPos(fBarX + (fVENDORBAR_NOBUTTON_X * fScaleFactor), fBarY + (fVENDORBAR_NOBUTTON_Y * fScaleFactor));
	canvas.DrawIcon(textureNoButton, fScaleFactor);

/*
    // Draw cancel button
	canvas.SetPos(fBarX + (fVENDORBAR_CANCELBUTTON_X * fScaleFactor), fBarY + (fVENDORBAR_CANCELBUTTON_Y * fScaleFactor));
	canvas.DrawIcon(textureCancelButton, fScaleFactor);
*/

    // Button text font and color
    Canvas.DrawColor.R = 206;
    Canvas.DrawColor.G = 200;
    Canvas.DrawColor.B = 190;

    if (Canvas.SizeX <= 512)
        Canvas.Font = baseConsole(Harry(level.playerHarryActor).player.console).LocalTinyFont;
	else if (Canvas.SizeX <= 640)
		Canvas.Font = baseConsole(Harry(level.playerHarryActor).player.console).LocalSmallFont;
	else
		Canvas.Font = baseConsole(Harry(level.playerHarryActor).player.console).LocalMedFont;

    // "Yes" button text
    Canvas.TextSize(strYes, fXTextLen, fYTextLen);
    Canvas.SetPos(fBarX + (fVENDORBAR_YESBUTTON_X * fScaleFactor) + (fVENDORBAR_BUTTON_TEXT_X * fScaleFactor) - (fXTextLen/2),
                  fBarY + (fVENDORBAR_YESBUTTON_Y * fScaleFactor) + (fVENDORBAR_BUTTON_TEXT_Y * fScaleFactor) - (fYTextLen/2));
    Canvas.DrawText(strYes, false);

    // "No" button text
    Canvas.TextSize(strNo, fXTextLen, fYTextLen);
    Canvas.SetPos(fBarX + (fVENDORBAR_NOBUTTON_X * fScaleFactor) + (fVENDORBAR_BUTTON_TEXT_X * fScaleFactor) - (fXTextLen/2),
                  fBarY + (fVENDORBAR_NOBUTTON_Y * fScaleFactor) + (fVENDORBAR_BUTTON_TEXT_Y * fScaleFactor) - (fYTextLen/2));
    Canvas.DrawText(strNo, false);

/*
    // "Cancel" button text
    Canvas.TextSize(strCancel, fXTextLen, fYTextLen);
    Canvas.SetPos(fBarX + (fVENDORBAR_CANCELBUTTON_X * fScaleFactor) + (fVENDORBAR_BUTTON_TEXT_X * fScaleFactor) - (fXTextLen/2),
                  fBarY + (fVENDORBAR_CANCELBUTTON_Y * fScaleFactor) + (fVENDORBAR_BUTTON_TEXT_Y * fScaleFactor) - (fYTextLen/2));
    Canvas.DrawText(strCancel, false);
*/

	// Set price text color and font
    siJellybeans = Harry(level.playerHarryActor).managerStatus.GetStatusItem(class'StatusGroupStars', 
			                                                                 class'StatusItemStars');
    Canvas.DrawColor = siJellybeans.GetCountColor();
	Canvas.Font = siJellybeans.GetCountFont(Canvas);

	// Draw price text
	strCurrPrice = string(nCurrPrice);
	Canvas.TextSize(strCurrPrice, fXTextLen, fYTextLen);
	Canvas.SetPos(fBarX + (fVENDORBAR_PRICE_X * fScaleFactor) - fXTextLen/2,
		          fBarY + (fVENDORBAR_PRICE_Y * fScaleFactor) - fYTextLen/2);
	Canvas.DrawText(strCurrPrice, false);

	// Restore canvas properties
	Canvas.DrawColor = colorSave;
	Canvas.Font      = fontSave;
}

function float GetVendorBarX(Canvas canvas)
{
    return (canvas.SizeX / 2) - (canvas.GetHudScaleFactor() * (fVENDORBAR_W / 2));
}

function float GetVendorBarY(Canvas canvas)
{
    return (canvas.GetHudScaleFactor() * fVENDORBAR_Y);
}

function bool IsMouseOverVendorYes()
{
    return (IsMouseOverVendorButton(fVENDORBAR_YESBUTTON_X, fVENDORBAR_YESBUTTON_Y, fVENDORBAR_BUTTON_W, fVENDORBAR_BUTTON_H));
}

function bool IsMouseOverVendorNo()
{
    return (IsMouseOverVendorButton(fVENDORBAR_NOBUTTON_X, fVENDORBAR_NOBUTTON_Y, fVENDORBAR_BUTTON_W, fVENDORBAR_BUTTON_H));
}

function bool IsMouseOverVendorCancel()
{
    return (IsMouseOverVendorButton(fVENDORBAR_CANCELBUTTON_X, fVENDORBAR_CANCELBUTTON_Y, fVENDORBAR_BUTTON_W, fVENDORBAR_BUTTON_H));
}

function bool IsMouseOverVendorButton(int nLeft, int nTop, int nWidth, int nHeight)
{
    local HPConsole hpCon;
    local int       nVendorMouseX;
    local int       nVendorMouseY;
    local float     fScaleFactor;

    hpCon = hpConsole(level.playerHarryActor.player.console);

    fScaleFactor = VendorCanvas.GetHudScaleFactor();

    nVendorMouseX = hpCon.MouseX * hpCon.Root.GUIScale;  // Console updates the mouse, but it updates with the
    nVendorMouseY = hpCon.MouseY * hpCon.Root.GUIScale;  // windowing scale that we don't need.

    nLeft   *= fScaleFactor;
    nTop    *= fScaleFactor;
    nWidth  *= fScaleFactor;
    nHeight *= fScaleFactor;

    nLeft += GetVendorBarX(VendorCanvas);
    nTop  += GetVendorBarY(VendorCanvas);

    //Harry(level.playerHarryActor).ClientMessage("IsMouseOver " $nVendorMouseX $" " $nVendorMouseY);
    //Harry(level.playerHarryActor).ClientMessage("IsMouseOver " $hpCon.MouseX $" " $hpCon.MouseY);
    //Harry(level.playerHarryActor).ClientMessage("Button " $nLeft $" " $nTop $" " $nWidth $" " $nHeight);
    //Harry(level.playerHarryActor).ClientMessage("VBar   " $GetVendorBarX(VendorCanvas) $" " $GetVendorBarY(VendorCanvas));

    if ((nVendorMouseX >= nLeft) && (nVendorMouseX <= (nLeft + nWidth)))
        return ((nVendorMouseY >= nTop) && (nVendorMouseY <= (nTop + nHeight)));
    else
        return (false);
}

event Tick(float fDeltaTime)
{
	Super.Tick(fDeltaTime);

    // Some flags we need for using the mouse with the vendor bar can get turned
    // off if user brings up the menu or some other UWindow.  If the vendor bar is
    // supposed to be on, make sure those flags are on.
    if (hpConsole(level.playerHarryActor.player.console).bVendorBar)
    {
        SetConsoleVendorBarFlags(true);
    }
}

defaultproperties
{
    DrawType=DT_None
    bHidden=true
}
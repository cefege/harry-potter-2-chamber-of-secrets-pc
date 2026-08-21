//===============================================================================
//  [CauldronMixing]
//
//  The mixing cauldron is the cauldron that Harry can use to make a 
//  Wiggenwell potion.  If Harry has at least one flobberworm mucus and one
//  wiggentree bark in his possession, a trigger sent to the cauldron will
//  cause Wiggenwell potions to be made (one for each flobberworm/wiggentree bark
//  set).
//
//  The mixing scene is designed such that Harry should be standing on "ground"
//  that is 16 collision units higher than the ground that the cauldron is
//  standing on in order for things to look right (his stirring animation relies
//  on this).  
//
//  A trigger should be setup on the step right next to the mixing cauldron.  
//  The trigger should be very close so that when Harry does his stirring 
//  animation, he's right over the cauldron.
//
//  Note:  The default collision properties for the cauldron are intentionally
//  different than normal collision properties.  The collision radius is small
//  on purpose so that Harry can bend into the cauldron.  The height is taller
//  than normal so that Harry can't accidentally jump on top of the cauldron.
//
//  When Harry is stirring, there is no control to move Harry, but the player
//  can move the camera around him and watch him stir.
//
//===============================================================================

class CauldronMixing extends HCauldron;

const TOP_OF_CAULDRON_OFFSET    = 25;
const strTEMP_CAULDRON_CUT_NAME = "TempMixingCauldronCutName";
const nPOTIONS_AVAILABLE_STATE  = 40;
const strCUE_CAULDRON_NOT_AVAIL_LINE = "_MixingCauldronsNotAvailableYet";

// Start mixing scene off a trigger or off a bump
enum EStartMixOn
{
    StartMixOn_Trigger,
    StartMixOn_Bump
};

// Type of particle effect to use on the cauldron.
enum ECauldronFX
{
	CauldronFX_Neutral,
	CauldronFX_Mixed
};

var StatusGroup sgPotionIngr;           // Potion ingredient status group for hud stuff
var StatusGroup sgPotions;              // Potion ingredient status group for hud stuff
var StatusItem  siWiggenBark;           // WBark status item for hud stuff
var StatusItem  siFlobberMucus;         // FMucus status item for hud stuff
var HProp       propTemp;               // Potion spawned out of cauldron
var vector		vFlobberHudLoc;         // Location of FMucus hud item
var vector      vWiggenHudLoc;          // Location of WBark hud item
var vector      vWWellPotionHudLoc;     // Potion location on hud
var vector      vTopOfCauldron;         // Location at center top of cauldron
var int         nPotionCount;           // Potions to spawn out of cauldron 
var int         i;                      // Generic loop var
var rotator     r;                      // Generic rotator var
var vector      vTargetDir;             // Direction to spawn potion out of cauldron
var float       fYawChange;             // Used to determine yaw for potions flying out of cauldron


var() EStartMixOn StartMixOn;           // Start mix scence from trigger event or bump
var() bool        bMixingEnabled;       // True if mixing is allowed (as long
                                        // as player has the right ingredients)

event PostBeginPlay()
{
    Super.PostBeginPlay();

	// Save off potion ingredient status objects.
	sgPotionIngr   = playerHarry.managerStatus.GetStatusGroup(class'StatusGroupPotionIngr');
    sgPotions      = playerHarry.managerStatus.GetStatusGroup(class'StatusGroupPotions');
	siWiggenBark   = sgPotionIngr.GetStatusItem(class'StatusItemWiggenBark');
	siFlobberMucus = sgPotionIngr.GetStatusItem(class'StatusItemFlobberMucus');
}

event Trigger( actor Other, pawn EventInstigator )
{
	// If can mix right now
	if (bMixingEnabled &&                       // mixing is allowed for the cauldron
        StartMixOn == StartMixOn_Trigger &&     // mixing set to start from trigger
        HaveWiggenPotionIngredients())          // have the right ingredients
	{
		// Start mixing the potion.
		GoToState('Mixing');
	}
}

event Bump(actor Other)
{
    local int nGameState;

	nGameState = playerHarry.ConvertGameStateToNumber();

	// Before game state 40, can't mix anything
	if (nGameState < nPOTIONS_AVAILABLE_STATE)	
    {
		GoToState('CauldronsNotAvailableYet');
    }

	// If can mix right now
	else if (bMixingEnabled &&                       // mixing is allowed for the cauldron
             StartMixOn == StartMixOn_Bump &&        // mixing set to start from bump
             HaveWiggenPotionIngredients())          // have the right ingredients
	{
		// Send out a trigger when bump.
		TriggerEvent( Event, none, none );

		// Start mixing the potion.
		GoToState('Mixing');
	}
}

// Called from CutScene script.
function bool CutCommand(string command, optional string cue, optional bool bFastFlag)
{
	local string  sActualCommand;
	local string  sCutName;
	local actor   a;
	
	sActualCommand = ParseDelimitedString( command, " ", 1, false );


	if( sActualCommand ~= "Capture" )
	{
		return (Super.CutCommand( Command, Cue, bFastFlag ));
	}

	else 
	if( sActualCommand ~= "Release" )
	{
        return (Super.CutCommand( Command, Cue, bFastFlag ));
	}

	else
	if( sActualCommand ~= "Enable" )
	{
        bMixingEnabled = true;          // Allow potion mixing
        CutNotifyActor.CutCue( cue );
		return (true);
	}

	else
	if( sActualCommand ~= "Disable" )
	{
        bMixingEnabled = false;         // Don't allow potion mixing
        CutNotifyActor.CutCue( cue );
		return (true);
	}

	else
		return Super.CutCommand( Command, Cue, bFastFlag );
}

// Setup the particle effect that should be attached to the cauldron.
function SetCauldronFX(ECauldronFX fx)
{
	local vector vOffset;

	// Out with the old effect
	KillAttachedParticleFX(0.0);

	// Set offset of particle effect to be on top of the cauldron.
	vOffset.X = 0;
	vOffset.Y = 0;
	vOffset.Z = TOP_OF_CAULDRON_OFFSET;
	attachedParticleOffset[0] = vOffset;

	// In with the new effect
	switch (fx)
	{
	case (CauldronFX_Neutral):
		attachedParticleClass[0]=Class'HPParticle.Cauldron_Neutral';
		break;
	case (CauldronFX_Mixed):
		attachedParticleClass[0]=Class'HPParticle.Cauldron_Mixed'; 
		break;
	default :
		log("ERROR: Invalid cauldron fx");
		break;
	}

	CreateAttachedParticleFX();
}

// Number of potions that can be made based on # of ingredients player has
function int GetNumPotionsToMake()
{
	return (min(siWiggenBark.nCount,siFlobberMucus.nCount));
}

// Return tru if have enough ingredients for at least one potion.
function bool HaveWiggenPotionIngredients()
{
	return ((siWiggenBark.nCount >= 1) && (siFlobberMucus.nCount >= 1));
}
			
//-----------------------------------------------------------------------------------
//  State Idle
//-----------------------------------------------------------------------------------
//
//  In this state, the cauldron is bubbling and waiting for Harry to come
//  make a potion.

auto state Idle
{
	// Setup particle effects on top of the cauldron.
	event BeginState()
	{
        Super.BeginState();
		SetCauldronFX(CauldronFX_Neutral);
	}
}

//-----------------------------------------------------------------------------------
//  State CauldronsNotAvailableYet
//-----------------------------------------------------------------------------------
//
//  Prior to gamestate 40, the player hasn't been introduced to mixing cauldrons and
//  thus cannot mix potions (even if they have ingredients).  Display a text message
//  if player bumps into a mixing cauldron before gamestate 40.

state CauldronsNotAvailableYet
{
ignores bump;

    // When "I can't use this yet" line is done, go back to idle state.
	function CutCue(string cue)
	{
		if (cue ~= strCUE_CAULDRON_NOT_AVAIL_LINE)
            GoToState('Idle');
    }

    // When state starts up, show "I can't use this yet" as text in cut area.
    // (Would have had sound too, but dialog didn't get added to game in time.)
    event BeginState()
    {
        local string   strDialog;
        local string   strDialogId;
        local float    fSoundLen;
        local TimedCue tcue;


        if (Rand(2) == 0)
            strDialogId = "Shared_Menu_0009";
        else
            strDialogId = "Shared_Menu_0010";

        // Get the text and how long to show it.
        strDialog = (Localize( "All", strDialogId,"HPMenu" ));
    	fSoundLen = (Len(strDialog)*0.01)+3.0;

        // Send out cue to ourselves when text should go away
   	    tcue=spawn(class 'TimedCue');           
        tcue.CutNotifyActor=Self;	
        tcue.SetupTimer(fSoundLen+0.5, strCUE_CAULDRON_NOT_AVAIL_LINE); 

        // Show text.
        Harry(level.playerHarryActor).MyHud.SetSubtitleText(strDialog, fSoundLen);
    }
}

//-----------------------------------------------------------------------------------
//  State Mixing
//-----------------------------------------------------------------------------------
//
//  State Mixing starts when trigger goes to the cauldron and Harry has one or more
//  flobberworm mucus and one or more wiggentreebark.

state Mixing
{
ignores bump;

begin:	
	// Set harry into potion mixing mode
	playerHarry.DoPotionMixingBegin();
	
/*
    if (CutName == "")
    CutName = strTEMP_CAULDRON_CUT_NAME;

    Harry(level.playerHarryActor).cam.CutCommand("Capture");
    playerHarry.cam.CutCommand("FLYTO " $Cutname $" x=50 z=50");
    playerHarry.cam.CutCommand("TARGET FLYTO " $Cutname $" x=10 z=10");

    sleep(0.5);

    if (CutName == strTEMP_CAULDRON_CUT_NAME)
        CutName = "";
*/

	// Bring up potion ingredient and potion hud items.
	sgPotionIngr.SetEffectTypeToPermanent();
	sgPotions.SetEffectTypeToPermanent();
    sgPotionIngr.SetCutSceneRenderMode(true);
    sgPotions.SetCutSceneRenderMode(true);

	// Let Harry get to the cauldron
	Sleep(0.5);

	// Fly all potion ingredients into the cauldron.
	vTopOfCauldron = Location;
	vTopOfCauldron.z += 40;//25;
	nPotionCount = GetNumPotionsToMake();
	for (i=0; i<nPotionCount; i++)
	{
		// Spawn new flobberworm mucus ingredient and fly it to the cauldron.
		vFlobberHudLoc = sgPotionIngr.GetItemLocation(class'StatusItemFlobberMucus', false);
		propTemp = HProp(FancySpawn(class'FlobberwormMucus',,,vFlobberHudLoc));		
        propTemp.fMinFlyToHudScale = 0.1;
		propTemp.DoDropOffProp(vTopOfCauldron, true);
		sleep(0.1);
				
		// Spawn new wiggentree bark ingredient and fly it to the cauldron.
		vWiggenHudLoc  = sgPotionIngr.GetItemLocation(class'StatusItemWiggenBark', false);
		propTemp = HProp(FancySpawn(class'WiggentreeBark',,,vFlobberHudLoc));		
        propTemp.fMinFlyToHudScale = 0.1;
		propTemp.DoDropOffProp(vTopOfCauldron, true);
		sleep(0.1);
	}

	// Harry stirs
	playerHarry.DoPotionMixingStir();

	// Stir for 1 second before adding potions to hud
	sleep(1.0);

	// Fly made potions out of the cauldron to the hud.
	for (i=0; i<nPotionCount; i++)
	{
		// Spawn new potion object at top of cauldron and fly it to the hud.
		vWWellPotionHudLoc = sgPotions.GetItemLocation(class'StatusItemWiggenWell', false);
		propTemp = HProp(FancySpawn(class'WWellCauldronBottle',,,vTopOfCauldron));	
        sleep(0.25);
        propTemp.fTotalFlyTime = 0.5;
        propTemp.fMinFlyToHudScale=0.8;
		propTemp.DoPickupProp();

        // Pause before throwing out another
		sleep(0.25);
	}

	// Voila.  All done.  Harry watches green smoke fly out.
	SetCauldronFX(CauldronFX_Mixed);
	Sleep(0.3);
	playerHarry.DoPotionMixingIdle();

/*
    Harry(level.playerHarryActor).cam.CutCommand("Release");
*/

	// Return control to Harry.
	playerHarry.DoPotionMixingEnd();

	// Let effect finish up.
	sleep(4.5);

	// Return hud items to normal.
    sgPotionIngr.SetCutSceneRenderModeToNormal();
    sgPotions.SetCutSceneRenderModeToNormal();
	sgPotionIngr.SetEffectTypeToNormal();
	sgPotions.SetEffectTypeToNormal();

	// Return cauldron to normal.
	GotoState('Idle');
}

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skCauldronTeacherMesh'

     // Make collision height much bigger than the actual so Harry cannot jump 
     // onto the cauldron.
     CollisionRadius=15  
     CollisionHeight=100 // actual is 20

     StartMixOn=StartMixOn_Trigger
     bMixingEnabled=true
}

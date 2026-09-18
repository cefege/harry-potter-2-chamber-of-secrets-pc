//=============================================================================
// Harry  -- hero character 
//=============================================================================
class Harry extends PlayerPawn;

//****************************************************************

var globalconfig	bool bAutoCenterCamera;		// Set this to false in the user.ini if you don't want the camera to autoCenter
var globalconfig	bool bMoveWhileCasting;		// Set this to true  in the user.ini if you want to move forward and backward while casting

var globalconfig	bool bAutoQuaff;			// Auto drink Potion

var globalconfig	float fDamageMultiplier_Easy;	// 
var globalconfig	float fDamageMultiplier_Medium;	//
var globalconfig	float fDamageMultiplier_Hard;	//


var(Sounds) sound 	drown;
var(Sounds) sound	breathagain;
var(Sounds) sound	HitSound3;
var(Sounds) sound	HitSound4;
var(Sounds) sound	Die2;
var(Sounds) sound	Die3;
var(Sounds) sound	Die4;
var(Sounds) sound	GaspSound;
var(Sounds) sound	UWHit1;
var(Sounds) sound	UWHit2;
var(Sounds) sound	LandGrunt;
var(Sounds) sound speech[20];
var(Display) const class<Decal> ShadowClass;

var bool bLastJumpAlt;

var bool b3DSound;

var globalconfig int AnnouncerVolume;
//var class<CriticalEventPlus> TimeMessageClass;

//var class<Actor> BossRef;

var string eaid;

const  NUM_HURT_SOUNDS = 15;
var sound   HurtSound[15];

// The following inputs are in addition to the standard inputs defined in PlayerPawn.uc.
// Broom-flying inputs are given their own input channels here so they can be remapped
// independently of standard walk-and-look inputs.

// Input Axes
var input float
	aBroomYaw, aBroomPitch,			// Intended for mapping to mouse
	aJoyBroomYaw, aJoyBroomPitch,	// Intended for mapping to analog joystick
	aSpellLessonX, aSpellLessonY;

// Input Buttons
var input byte
	bBroomYawLeft, bBroomYawRight,	// Intended for mapping to keyboard or digital gamepad
	bBroomPitchUp, bBroomPitchDown,
	bBroomBoost, bBroomBrake,
	bBroomAction,
	bDrinkWiggenwell,
	bSpellBallAction,
    bVendorReply,
	bDuelRictusempra,
	bDuelMimblewimble,
	bDuelExpelliarmus,
	bDuelCycleSpell,
    bSpellLessonLeft,
    bSpellLessonRight,
    bSpellLessonUp,
    bSpellLessonDown;

// Input Configuration
var globalconfig bool
	bInvertBroomPitch,				// Reverses direction of broom pitch inputs (both buttons and axes)
	bAllowBroomMouse;				// Enables use of mouse for broom flight control
	//bPlayWithKeyboard;              // Defaul:false play with mouse.   Set to true, remaps keys so playing with keyboard works better

var (movement)float turnRate;

//var target    rectarget;
var SpellCursor	SpellCursor;
var rotator     AimRotOffset;

var	bool	  bSkipKeyPressed;

var  travel int	  numHousePointsHarry;
var  int	  numHousePointsGryffindor;
var  int	  numHousePointsSlytherin;
var  int	  numHousePointsHufflepuff;
var  int	  numHousePointsRavenclaw;
var int           numLastHousePointsHarry;

var(HousePoints) const int maxPointsPerHouse;
var(HousePoints) const int HarryMultipleForGryffindor;

var(HarryTut) name TriggerToSendOnFirstBean;

var(HarryTut) bool bLockOutForward;
var(HarryTut) bool bLockOutBackward;
var(HarryTut) bool bLockOutStrafeLeft;
var(HarryTut) bool bLockOutStrafeRight;

var Director			Director;		// Current object in control of the mini-game or puzzle that Harry's playing
var Adv1TutManager		Adv1TutManager;	// When the current Director is this particular class, Harry makes direct calls to it

struct StatusSaveData
{
	var class<StatusGroup> classGroup;
	var class<StatusItem>  classItem;
	var int                nPotential;
	var int                nCount;
    var int                nMaxCount;
};

struct WCardSaveData
{
	var int nCardId;
	var int nCardOwner;
};

var travel StatusSaveData StatusSave[30];  
var travel WCardSaveData  BronzeCardSave[50];
var travel WCardSaveData  SilverCardSave[40];
var travel WCardSaveData  GoldCardSave[11];
var travel int              nLastCardTypeSave;

var StatusManager managerStatus;
var FEBook menuBook;	

var travel bool bHaveNimbus2001;
var travel bool bHaveQArmor;

// --- Spell Book
var travel class<baseSpell>	SpellBook[32];		// spells harry can use
const						MAX_NUM_SPELLS = 32;// MAX_NUM_SPELLS >= SPELL_NumSpells (in spell enum)
var bool					bNoSpellBookCheck;	// if bNoSpellBookCheck == true then harry can cast all spells

// --- Dueling vars
var bool				bDuelIsOver;		// by default this is false
var bool				bInDuelingMode;		// by default this is false
var bool				bSpellCyclePressed;	// save the state of the spellCycle button
var bool				bReboundingSpells;		// are we currently rebounding spells?
var HPawn				DuelOpponent;		// Harry's dueling opponent
var int					CurrentDuelSpell;	// Current spell
var class<baseSpell>	DuelSpells[3];		
var sound				DuelSpellSwitchSounds[3];		
const					NUM_DUEL_SPELLS = 3;
var	float				fTimeAfterHit;
var	int					nHealthSave;
var	float				fTimeAfterShield;

var travel int 			DuelRankHarry;
var travel int 			DuelRankOppon;
var travel int 			DuelRankBeans;

// sto: For debugging purposes, below
var globalconfig bool bDisableDialog;

const                   NUM_SPONGIFY_FX = 2;
var ParticleFX          SpongifyFX[2];

//**************************************************************************************************************
//**************************************************************************************************************
var bool clearMessages;

//Mirrored input values from Harry.  You need these cause somewhere along the way in Harry, they get zero'd out before you get to HarryPawn.
//var float aStrafe, aTurn, aLookup, aSideMove, aForward, aBaseX, aBaseY, aBaseZ,	aMouseX, aMouseY;
//var byte  bZoom, bRun, bLook, bDuck, bSnapLevel, bStrafe, bFire, bAltFire, bFreeLook;

// The following inputs are in addition to the standard inputs defined in PlayerPawn.uc.
// Broom-flying inputs are given their own input channels here so they can be remapped
// independently of standard walk-and-look inputs.

// Other new variables for Harry...
//var bool hidden;
var int stillDistance;
var int movingDistance;
var int hiddenDistance;
var actor bustedBy;
var vector targetOffset;
var bool bMovingBackwards;
var actor focusActor;
var HPawn foreachActor;
var bool  bVeryAfraid;		// if True harry will play his look_frantic anim instead of his normal fidgit

var int beansoundCount;

var float LessonScore;			// Used for spell lesson so HUD can easily get to value
var float LessonPass;
var int	  iLevelReached;
var int	  iLessonPoints;

var SpongifyPad	HitSpongifyPad;	// If we hit a spongifyPad then this refrence will be filled in

//var actor     ViewTarget;  This is better off left in PlayerPawn
//var target    rectarget;
//var CamTarget StandardTarget;
var bool      bExtendedTargetting;
var BaseCam   cam;
var bool      bHarryMovingNotAiming;  //Lets camera know that harry has his "movement key" pressed.

var bool      bIsAiming;           //Replaces entire state playerAiming, since you can now cast while moving around.
                                   // This variable is on during the entire cast process, from raising the arm, to when the arm is back down at harry's side (when TurnOffCastingVars() is called.)
var bool      bIsAimingWithCharge; //This is on from the beginning of the cast process, up until when the projectile is actually shot, and TurnOffSpellCursor() is called.
var bool      bLockedOnTarget;
var HChar     BossTarget;

var HPawn     HearHarryRecipient;

var bool      bCastFastSpells;     //Really just fast flipendo

var bool      bKeepStationary;     //This is a flag you can set to keep harry fixed. (He can still turn)

var bool      bFixedFaceDirection; //If this is true, only let harry face the direction set in vFixedFaceDirection.
var vector    vFixedFaceDirection;

var bool	  bStationary;      //This is more of a bIsStationary flag that you read.
var bool	  bTargettingError;

var float     fLargestAForward;

//If this is set to an actor, harry carries it, instead of the wand
// Use SetCarryingActor(actor a); to make harry carry something
var actor CarryingActor;
var actor ActorToCarry;  // This is the actor that harry is about to start carrying.
//var name  OptionalCarryBoneName; // optional bone name in harry to use as attach point.
//var name  OptionalPickupAnimName;// also, optional anim for harry to play

var cHarryAnimChannel   HarryAnimChannel;
var bool                bThrow;
var bool				bCanCast;
//var bool                bInstantCast;     //Mainly for when harry's using the sword

var bool                bHarryUsingSword; //Using and extra var makes things a bit safer

var EAnimType           HarryAnimType;

var name				SpongifyFallAnim;	// SpongifyFallAnim name
var name				AnimFalling;		// User can change Harry's falling anim (will reset on landing)

struct cHarryAnims
{
	var name Idle;
	var name Walk;
	var name Run;
	var name WalkBack;
	var name StrafeRight;
	var name StrafeLeft;
	var name Jump;
	var name Jump2;
	var name Fall;
	var name Land;
};

enum enumHarryAnimSet
{
	HARRY_ANIM_SET_MAIN,
	HARRY_ANIM_SET_ECTO,
	HARRY_ANIM_SET_SLEEPY,
	HARRY_ANIM_SET_SWORD,
	HARRY_ANIM_SET_WEB,
	HARRY_ANIM_SET_DUEL,
};


const NUM_HARRY_ANIM_SETS = 6;
var cHarryAnims         HarryAnims[6];
var enumHarryAnimSet    HarryAnimSet;
var float               LastAnimFrame; //Saves the last AnimFrame

var vector              vAdditionalAccel;

var bool                bTempKillHarry;

var bool                bAllowHarryToDie;  //Normally true, so after playing die anim, he dies.
                //Set to false (using KillHarry(false)) and he'll lie there and wait until some external agent sets it to true.

var bool bScreenRelativeMovement;
var int   ScreenRelativeMovementYaw; //This is the 2d direction you'd like to be running when in bScreenRelativeMovement mode

var bool bReverseInput;
var float fTimeWalking;  //How long harry's been walking

// yet more variables to do weird stuff
// underAttack bool and pointer are used to indicate that Harry is being attacked.
// intially this is used for spell selection purposes, but could be usefull for other things

var bool underAttack;
var actor attacker;
var vector lookhere;

var bool bClubDeath;
var bool bPitDeath;

var bool      bConstrainYaw; //Totally for troll chase, keep harry's yaw pointing down the x axis
var int       ConstrainYawVariance;

var name	LastState;
var int ShortCutNum;

var   float  _LastKeyPressTime;
var   int    _iCurrentStringChar;
var   string _CurrentString;

var bool	bInSneak;

var bool bIsCrouching;  //dead var, need to use the one in PlayerPawn if we want to have crouching.

var bool bIsTurning;
var bool bAnimTransition;

var int WaitingCount;

//********************************************************************************************************************

var byte    LastbLook;

var bool    bFallingMount;
var bool    bIsPickingUpWizardCard;

// AE:
var bool    bPlayedFallSound;

var float   fTimeToStop;

var bool	bOldStrafingState;
var float   fOldGroundSpeed;

var bool	bEndedWizCardPickup;

var	vector	ChessTargetLocation;

var float    fTimeInAir;  //try and measure how long you've been in PHYS_Falling
var EPhysics eLastPhysState;
var float    fFallingZ;   //try and save your z when you start falling.
var(Movement) float    GroundJumpSpeed;  //2d max speed harry can move while jumping/falling.  Makes it so current puzzles aren't broken.

// Mount vars.
var vector	MountDelta;
var actor	MountBase;

// Ectoplasma Support
var int		iEctoRefCount;
var float	GroundEctoSpeed;
var bool    bPlayedEctoKnockBack;  // <==-- This may not be generic enough.  If there's anywhere else that we want harry to do just one knockback, this should be made more generic.
var int     iEctoHurtSoundCount;   //counts up to 2 or 3 so that you dont play the sound as often
var bool	bEctoFlashed;

// Sleepy Animation Support
var int		iSleepyAnimTimer;
var int		iMaxSleepyAnim;
var float	fSleepySpeed;

// Web Animation Support  (when Harry is stuck in a spider web)
var int		iWebAnimRefCount;
//var int		iMaxWebAnim;
var float	fWebSpeed;

// Mixing potion variables
var float fStirSoundDuration;
var sound soundStirPotion;

var bool bE3DemoLockout;

var travel string PreviousLevelName;

var SpellLessonTrigger CurrSpellLesson;

var VendorManager      CurrVendorManager;

var float    FootOffsetZ; // probably a way to calculate this, right now it's about -34

var bool     bFraserMode; //More commonly known as God mode, more appropriately named though.

var bool	 bFinishPickBitOfGoyle;
var bool	 bIsGoyle;

// generic fidget / idle support
var	name	CurrFidgetAnimName;
var	name	CurrIdleAnimName;
var int		FidgetNums;
var int		IdleNums;

// Fall Damage Support
const		FALL_DAMAGE_DISTANCE = 512;
var float	fHighestZ;

// Contains text id for current objective
var travel string strObjectiveId;

// Flag set when House Points Ceremony happens in hub 9.  This flag is set through a
// cutscene call.  A cutquestion later needs it to know if the game can move onto
// transition E.
var travel bool bHub9CeremonyFlag;

//var cFacialAnimChannel  

var SpellSelector HudSpellSelector;

var bool          bDisplayedFirstErrorMessages;

var travel bool bSaidVendorInstructions;



//Quidditch stuff*****************************************************************************
var travel int curQuidMatchNum;
struct QuidGameResult
{
	var string opponent;
	var int myScore;
	var int opponentScore;
	var int housePoints;
	var bool bLocked;
	var bool bWon;

};
var travel QuidGameResult quidGameResults[6];
//Wizard dueling stuff************************************************************************
var travel int curWizardDuel;
var travel int curWizardDuelRank;
var travel int lastUnlockedDuelist;

//********************************************************************************************
//********************************************************************************************
event PreBeginPlay()
{
	// Initialize
	Super.PreBeginPlay();

/*
	//If collision values are true, spawn will fail cause it tries to spawn HarryPawn where Harry is standing.
	// Something somewhere will set these to true again, but we set them to false again in PlayerWalking::BeginState.
	SetCollision(false,false,false);

	foreach AllActors(class'HarryPawn', HarryPawn)
		break;

	if( HarryPawn == none )
	{
		HarryPawn = spawn( class'HarryPawn' );

		if( HarryPawn == none )
			ClientMessage("Error:  Couldn't make a HarryPawn puppet.");
	}
	else
	{
		ClientMessage("HarryPawn already in map");
	}
*/	

	// Find mini-game director, if any
	foreach AllActors( class'Director', Director )
		break;

	// Create the Spell Cursor
	SpellCursor = spawn(class'SpellCursor');

	// Create StatusManager
	if(managerStatus == None)
	{
		managerStatus = spawn(class'StatusManager');

		// Save off reference to harry in status manager
		managerStatus.playerHarry = self;

		// Create status items that need to exist at startup. Other status items
		// will get created as an attempt to access them is made.  
		managerStatus.CreateStartupItems();
	}

	// Make sure Harry has the default spells
	AddToSpellBook( class'spellFlipendo'		); // 1 - pushes things
	AddToSpellBook( class'spellLumos'			); // 2 - lights up your wand
	AddToSpellBook( class'spellAlohomora'		); // 3 - unlock doors, secret areas
	
	// Reset our NoSpellBookCheck
	bNoSpellBookCheck = false;
}

//*********************************************************************************************
function PostBeginPlay()
{
    local Pawn p;
	local actor a, a2;
	local Duellist d;

	local string animName;
	local int	i;
	local name	nm;


	Super.PostBeginPlay();

    //ClientMessage(self $" In PostBeginPlay*******************************");
    //log(self $" In PostBeginPlay*******************************");

	FidgetNums = 0;
	for ( i = 1; i <= 16; i++)
	{
		animName = "fidget_" $i;
		nm = StringToAnimName(animName);
		if(nm == '')
		{
			FidgetNums = i - 1;
			break;
		}
	}

	IdleNums = 0;
	for ( i = 1; i <= 16; i++)
	{
		animName = "idle_" $i;
		nm = StringToAnimName(animName);
		if(nm == '')
		{
			IdleNums = i - 1;
			break;
		}
	}

	bShowMenu=false;

	HUDType=class'HPHud';
	

	//If there's no camera, make one.
	ForEach AllActors( class'baseCam', cam )
		break;
	if( cam == none )
		cam = spawn( class'baseCam' );

	viewClass(class'baseCam', true);

  	// @PAB added new camera target
	// 	makeCamTarget();	

	//ForEach AllActors(class'actor', a)
	//	Log("******* Actor:"$a.name);

	b3DSound = bool(ConsoleCommand("get ini:Engine.Engine.AudioDevice Use3dHardware"));

	//iFireSeedCount = 0;

	// Harry gets a shadow, bigger than normal.
	Shadow = Spawn(ShadowClass,self);
	log( self$ " ShadowClass=" $ShadowClass$ " shadow=" $Shadow$ " tex=" $Shadow.Texture );

	// @PAB temp give a spell to Harry
	//	baseWand(weap).addSpell(Class'spellDud');
	//	baseWand(weap).addSpell(Class'spellflip');
	//	baseWand(weap).addSpell(Class'spellALOho');
	//	baseWand(weap).SelectSpell(Class'spellFlip');

	HarryAnimChannel = cHarryAnimChannel( CreateAnimChannel(class'cHarryAnimChannel', AT_Replace, 'bip01 spine1') );
	HarryAnimChannel.SetOwner( self );

	// Adding a timer to Harry. 
	// Looping timer with a duration of 1 sec. Leave this as is. 
	SetTimer(1.0, true);




    // Fill in status manager stuff from save game
    CopyAllStatusFromHarryToManager();
}

//********************************************************************************************
event Possess()
{
	// Called when the PlayerPawn Harry is possessed (attached) to a viewport (Player).
	Super.Possess();

	// Let director (if any) know that possession has occurred
	if ( Director != None )
		Director.OnPlayerPossessed();
}

//********************************************************************************************
function DisablePlayerInput()
{
	bIsCaptured=true;
	myHud.StartCutScene();
	bKeepStationary = true;
}

//********************************************************************************************
function EnablePlayerInput()
{
	bIsCaptured=false;
	myHud.EndCutScene();
	bKeepStationary = false;
}

//********************************************************************************************
function bool InputIsDisabled()
{
	return bKeepStationary;
}

//********************************************************************************************
//********************************************************************************************
function PlayPeevesHack()
{
//	local Ron ron;
//	ForEach AllActors( class'Ron', ron )
//	{
//		ron.PlaySound(sound'HPSounds.AllDialog.Pc_Her_Password_07');
//	}
}

//********************************************************************************************
//********************************************************************************************
// SPELL BOOK

function AddToSpellBook( class<baseSpell> spellClass )
{
	if( spellClass.default.spellType < MAX_NUM_SPELLS && 
		SpellBook[ spellClass.default.spellType ] == None )
	{
		// Add this spell
		SpellBook[ spellClass.default.spellType ] = spellClass;
	}
}

function bool IsInSpellBook( ESpellType eSpellType )
{
	if( eSpellType >= MAX_NUM_SPELLS )
		return false;

	if( bNoSpellBookCheck )
		return true;

	// See if we have the spell we wish to use in our spell list
	return SpellBook[eSpellType] != none;
}

function AddAllSpellsToSpellBook()
{
	// Add all spells to our wand
	AddToSpellBook( class'spellFlipendo'		); // 1 - pushes things
	AddToSpellBook( class'spellLumos'			); // 2 - lights up your wand
	AddToSpellBook( class'spellAlohomora'		); // 3 - unlock doors, secret areas
	AddToSpellBook( class'spellSkurge'			); // 4 - gets rid of slime
	AddToSpellBook( class'spellRictusempra'		); // 5 - same as punching someone in the gut (wizard duel spell)
	AddToSpellBook( class'spellDiffindo'		); // 6 - cuts things
	AddToSpellBook( class'spellSpongify'		); // 7 - makes things bouncy
	
	// --- Wizard Duel only
	AddToSpellBook( class'spellDuelRictusempra' );	// 8  - dueling version of rictusempra
	AddToSpellBook( class'spellDuelMimblewimble');	// 9  - causes opponent wizard to mumble words that don't make sense ( wizard duel only )
	AddToSpellBook( class'spellDuelExpelliarmus');	// 10 - spell rebound ( wizard duel only )	
}

function AddToSpellBookByString( string SpellName )
{
	switch(SpellName)
	{
		case "Flipendo":			AddToSpellBook( class'spellFlipendo'		); break;
		case "Lumos":				AddToSpellBook( class'spellLumos'			); break;
		case "Alohomora":			AddToSpellBook( class'spellAlohomora'		); break;
		case "Skurge":				AddToSpellBook( class'spellSkurge'			); break;
		case "Rictusempra":			AddToSpellBook( class'spellRictusempra'		); break;
		case "Diffindo":			AddToSpellBook( class'spellDiffindo'		); break;
		case "Spongify":			AddToSpellBook( class'spellSpongify'		); break;
		case "DuelRictusempra":		AddToSpellBook( class'spellDuelRictusempra' ); break;
		case "DuelMimblewimble":	AddToSpellBook( class'spellDuelMimblewimble'); break;
		case "DuelExpelliarmus":	AddToSpellBook( class'spellDuelExpelliarmus'); break;
	}
}

function ClearSpellBook()
{
	local int i;
	for(i=0; i < MAX_NUM_SPELLS-1; ++i)
		SpellBook[i] = None;
}

//********************************************************************************************
//********************************************************************************************
// DUELING MODE

function TurnOnDuelingMode( HPawn PawnOpponent )
{
	local Rotator r;
	local int nMaxHealth;

	bDuelIsOver	   = false;	
	bInDuelingMode = true;
	DuelOpponent   = PawnOpponent;
	cam.SetCameraMode( cam.ECamMode.CM_Dueling );
	cam.SetYaw(0);
	cam.SetPitch(-3500);
	cam.SetFOV(50);

	// turn Harry straight to the opponent
	r = Rotation;
	r.yaw = 0;
	SetRotation( r );
	DesiredRotation = r;

	// At beginning of duel, save off current health
	nHealthSave = managerStatus.GetHealthCount();

	// Then set health to the max
	nMaxHealth = managerStatus.GetHealthPotentialCount();
	managerStatus.SetHealthCount(nMaxHealth);

	// set opponent Health (and maxHealth) to harry's max health
	Duellist(DuelOpponent).nMaxHealth = nMaxHealth;
	DuelOpponent.Health	= nMaxHealth;

	HarryAnimSet = HARRY_ANIM_SET_DUEL;

	SpellCursor.bInvisibleCursor = true;

 	baseWand(weapon).StartGlowingWand( DuelSpells[ CurrentDuelSpell ] );
	
	if (HudSpellSelector == None)
        HudSpellSelector = SpellSelector(FancySpawn(class'SpellSelector'));

	Duellist(DuelOpponent).SetHealthBar();
}

function TurnOffDuelingMode()
{
	bInDuelingMode = false;

	Duellist(DuelOpponent).TurnOffSpellCursor();
	DuelOpponent.gotoState('stateIdle');

	// to turn health bar off
	DuelOpponent.Health	= 0;

	cam.SetCameraMode( cam.ECamMode.CM_Standard );
	cam.SetFOV(90);

	// At end of duel, restore original health
	managerStatus.SetHealthCount(nHealthSave);

	HarryAnimSet = HARRY_ANIM_SET_MAIN;

	SpellCursor.bInvisibleCursor = false;
	baseWand(weapon).StopGlowingWand();

    HudSpellSelector.Destroy();
    HudSpellSelector = None;
}

function HandleDuelPlayerInput()
{
	if( bDuelCycleSpell == 1 )
		bSpellCyclePressed = true;
	else if( bSpellCyclePressed )
	{
		bSpellCyclePressed = false;
		CurrentDuelSpell = CurrentDuelSpell + 1;
		if(CurrentDuelSpell > NUM_DUEL_SPELLS-1)
			CurrentDuelSpell = 0;

		baseWand(weapon).StartGlowingWand( DuelSpells[ CurrentDuelSpell ] );
		PlaySound(DuelSpellSwitchSounds[CurrentDuelSpell]);
	}
	else
	if( bDuelRictusempra == 1 )
	{
		CurrentDuelSpell = 0;
		baseWand(weapon).StartGlowingWand( DuelSpells[ CurrentDuelSpell ] );
	 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_switch2RIC' );
	}
	else 
	if( bDuelMimblewimble == 1 )
	{
		CurrentDuelSpell = 1;
		baseWand(weapon).StartGlowingWand( DuelSpells[ CurrentDuelSpell ] );
	 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_switch2MIM' );
	}
	else
	if( bDuelExpelliarmus == 1 )
	{
		CurrentDuelSpell = 2;
		baseWand(weapon).StartGlowingWand( DuelSpells[ CurrentDuelSpell ] );
	 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_switch2EXP' );
	}

    switch (CurrentDuelSpell)
    {
    case (0): HudSpellSelector.SetSelection(HudSpellSelector.ESpellSelection.SSelection_Rictusempra); break;
    case (1): HudSpellSelector.SetSelection(HudSpellSelector.ESpellSelection.SSelection_Mimblewimble); break;
    case (2): HudSpellSelector.SetSelection(HudSpellSelector.ESpellSelection.SSelection_Expelliarmus); break;
    default :
        log("ERROR: Unrecognized spell for hud spell selector");
        break;
    }
}

//********************************************************************************************
//********************************************************************************************
// ECTOPLASMA SUPPORT

function EctoRefAdd()
{
	if( iEctoRefCount == 0 )
	{		
		// set our ground speed to ecto speed and our animation set to ecto set
		GroundSpeed  = GroundEctoSpeed;
		HarryAnimSet = HARRY_ANIM_SET_ECTO;

		// Start Ecto looping Sound effect
		PlaySound( sound'HPSounds.Ch2Skurge.ecto_damage', //sound
				   SLOT_Interact,	//slot
				   0.75f,			//volume
				   ,				//no override
				   ,				//radius
				   ,				//pitch
				   ,				//disable3D
				   true );			//loop
	}
	iEctoRefCount++;
}

function EctoRefSub()
{
	if( iEctoRefCount > 0 )
	{
		if( iEctoRefCount == 1 )
		{
			// set our ground speed back to normal and our anim set to normal
			GroundSpeed  = GroundRunSpeed;
			HarryAnimSet = HARRY_ANIM_SET_MAIN;
			
			LeaveEcto();
			bEctoFlashed = false;

			// Stop ecto looping sound effect
			StopSound( Sound'HPSounds.Ch2Skurge.ecto_damage', SLOT_Interact);
		}
		iEctoRefCount--;
	}
}

//********************************************************************************************
//********************************************************************************************

function SleepyAnimTimerAdd(int t)
{
	if ( iSleepyAnimTimer == 0 )
	{
		GroundSpeed = fSleepySpeed;
		HarryAnimSet = HARRY_ANIM_SET_SLEEPY;
	}
	iSleepyAnimTimer += t;

	if ( iSleepyAnimTimer > iMaxSleepyAnim )
		iSleepyAnimTimer = iMaxSleepyAnim;
}

function SleepyAnimTimerSub()
{
	if ( iSleepyAnimTimer > 0 )
	{
		if ( iSleepyAnimTimer == 1 )
		{
			// set our ground speed back to normal and our anim set to normal
			GroundSpeed  = GroundRunSpeed;
			HarryAnimSet = HARRY_ANIM_SET_MAIN;
		}
		iSleepyAnimTimer--;
	}
}

function SetMaxSleepyAnim( float t )
{
	iMaxSleepyAnim = t;
}


function WebAnimRefCountAdd()
{
	if ( iWebAnimRefCount == 0 )
	{
		GroundSpeed = fWebSpeed;
//		HarryAnimSet = HARRY_ANIM_SET_WEB;
	}
	iWebAnimRefCount++;
}

function WebAnimRefCountSub()
{
	if ( iWebAnimRefCount > 0 )
	{
		if ( iWebAnimRefCount == 1 )
		{
			// set our ground speed back to normal and our anim set to normal
			GroundSpeed  = GroundRunSpeed;
//			HarryAnimSet = HARRY_ANIM_SET_MAIN;
			LeaveEcto();  // this is NOT ecto but I only want the knockback to play once
		}
		iWebAnimRefCount--;
	}
	else if ( iWebAnimRefCount < 0 )
	{
		iWebAnimRefCount = 0;
	}

}

//********************************************************************************************
function LeaveEcto()
{
	bPlayedEctoKnockBack = false;
}

//********************************************************************************************
/*
function Tick( float dtime )
{
	//Simulate PlayerTick here.
	PlayerTick( dtime );
}

//This one shouldn't get called
function PlayerTick( float dtime )
{
	ClientMessage("Global PlayerTick");
}

//Nor should this.
function ProcessMove ( float DeltaTime, vector newAccel, eDodgeDir DodgeMove, rotator DeltaRot)
{
	ClientMessage("Global.ProcessMove");
	Acceleration = newAccel;
}
*/

//called from HPConsole. Used for the console command DeleteClass
//This is only used for debugging.
function DestroyClass(string sinput)
{
local name cname;
local actor act;

	cname=name(sinput);
	ForEach AllActors( class'actor', act)
	{
		if(act.isa(cname))
		{
			ClientMessage("Destroying:"$act);
			act.destroy();
		}
	}
}
function ListGroups()
{
local name cname;
local actor act;

	ForEach AllActors( class'actor', act)
	{
		ClientMessage(act $" " $act.group);
	}
}

event PreClientTravel()
{

	local string ts;
	local int ti;


		//save current level name so the next level will know what smart start to use.
/*	ts=level.GetLocalURL();
	ts=Caps(ts);

		//remove .unr part.
	ti=instr(ts,".UNR");
	if(ti>-1)
		ts=left(ts,ti);
*/


	ti = InStr(level.LevelEnterText, ".");
	if (ti==-1)
		ts = level.LevelEnterText;
	else
		ts = Left(level.LevelEnterText, ti);
		
	log("############################# Level Name:" $level.LevelEnterText);

	PreviousLevelName=ts;
	level.playerHarryActor.clientMessage("Setting Previous Level Name:" $PreviousLevelName);
	log("############################# Setting Previous Level Name:" $PreviousLevelName);


	ClientMessage(self $"########################## In PreClientTravel PreviousLevelName:"$PreviousLevelName);
	log(self $"########################## In PreClientTravel PreviousLevelName:" $PreviousLevelName);

    ClearNonTravelStatus();
    CopyAllStatusFromManagerToHarry();

}


event TravelPostAccept()
{
	// Called after all inventory has been accepted from previous level
local SmartStart startPoint;
local Characters ch;
local weapon weap;

	super.TravelPostAccept();

	// Insure Harry as exactly one wand
	log( "weapon is"$weapon );
	if ( inventory==none )
	{
		weap=spawn( class'baseWand', [SpawnOwner] self );
		weap.BecomeItem();
		AddInventory( weap );
		weap.WeaponSet( self );
		weap.GiveAmmo( self );
		log( self$ " spawning weap " $weap );
	}
	else
	{
		log( "not spawning weap" );
	}

    // Restore status travel items.
    CopyAllStatusFromHarryToManager();

    // If Harry owns a card and it exists in the current level, remove it from the level.
    StatusGroupWizardCards(managerStatus.GetStatusGroup(class'StatusGroupWizardCards')).RemoveHarryOwnedCardsFromLevel();

	// Let director (if any) know that all travel items have been accepted
	if ( Director != None )
		Director.OnPlayerTravelPostAccept();

	ForEach AllActors( class'Characters', ch )
		ch.SetEverythingForTheDuel();


	
	//ClientMessage(self $"########################## In TravelPostAccept PreviousLevelName:"$PreviousLevelName);
	//log(self $"########################## In TravelPostAccept PreviousLevelName:" $PreviousLevelName);

	// Move harry to the startpoint that has the same name as the level we just came from.
	//
	if(PreviousLevelName!="")
	{
	ForEach AllActors( class'smartStart', startPoint )
		{
//		ClientMessage("*************** "$startPoint $" " $startPoint.PreviousLevelName);
		if(startPoint.PreviousLevelName!="" && startPoint.PreviousLevelName~=PreviousLevelName)
			{
			SetLocation(startPoint.Location);
			SetRotation(startPoint.Rotation);
			if(startPoint.bDoLevelSave)
				Harry(level.PlayerHarryActor).SaveGame(0);
			}
		}
	}


}

function CopyAllStatusFromHarryToManager()
{
    //ClientMessage(self $" In CopyAllStatusFromHarryToManager*******************************");

	CopyGenericStatusFromHarryToManager();
	CopyCardCardStatusFromHarryToManager();
}

// Repopulate all StatusGroups and StatusItems in StatusManager from data that was saved
// off before traveling.
function CopyGenericStatusFromHarryToManager()
{
	local StatusItem  siCurr;
	local int         nStatusIdx;

	// Repopulate status manager items from info we saved off before traveling or saving.
	for (nStatusIdx=0; nStatusIdx<ArrayCount(StatusSave); nStatusIdx++)
	{
		// Nothing more in our list.  Stop looping.
		if (StatusSave[nStatusIdx].classGroup == None)
			break;

		// GetStatusItem will get the status item if it exists or create a new one if not.
		// In either case, set the count and potential count for the desired status item.
		siCurr = managerStatus.GetStatusItem(StatusSave[nStatusIdx].classGroup, StatusSave[nStatusIdx].classItem); 
		siCurr.nCount = StatusSave[nStatusIdx].nCount;
		siCurr.nCurrCountPotential = StatusSave[nStatusIdx].nPotential;
        siCurr.nMaxCount = StatusSave[nStatusIdx].nMaxCount;
	}
}

// Repopulate the StatusGroupWizardCards and it's StatusItems from data that was saved off
// before travelling or saving.
function CopyCardCardStatusFromHarryToManager()
{
	local StatusGroupWizardCards sgCards;
	local StatusItemWizardCards  siCards;
	local int                    i;

	// Restore bronze card data.
	sgCards = StatusGroupWizardCards(managerStatus.GetStatusGroup(class'StatusGroupWizardCards'));
	siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemBronzeCards'));
	for (i=0; i<ArrayCount(BronzeCardSave); i++)
		siCards.SetCardData(i, BronzeCardSave[i].nCardId, BronzeCardSave[i].nCardOwner);

	// Restore silver card data.
	siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemSilverCards'));
	for (i=0; i<ArrayCount(SilverCardSave); i++)
		siCards.SetCardData(i, SilverCardSave[i].nCardId, SilverCardSave[i].nCardOwner);

	// Restore gold card data.
	siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemGoldCards'));
	for (i=0; i<ArrayCount(GoldCardSave); i++)
		siCards.SetCardData(i, GoldCardSave[i].nCardId, GoldCardSave[i].nCardOwner);

	// Restore last card group picked up
	sgCards.SetLastObtainedCardTypeAsInt(nLastCardTypeSave);

	// For debugging, display wizard card data to the console.
	//sgCards.ShowCardData();		
}


function int getnumHousePointsHarry ()
{
	return numHousePointsHarry;
}

function int getLastHousePointsHarry ()
{
	return numLastHousePointsHarry;
}

function int getNumHousePointsGryffindor ()
{
	return numHousePointsGryffindor;
}

function int getNumHousePointsSlytherin ()
{
	return numHousePointsSlytherin;
}

function int getNumHousePointsHufflePuff ()
{
	return numHousePointsHufflepuff;
}

function int getNumHousePointsRavenclaw ()
{
	return numHousePointsRavenclaw;
}

//********************************************************************************************
function SaveStateName()
{
	clientmessage("Harry state " $GetStateName() $" " $LastState);
	
	if (GetStateName() != 'PickingUpWizardCard')
	{
		LastState = GetStateName();
	}
}

//********************************************************************************************
function RestoreStateName()
{
	gotostate(LastState);
}

//********************************************************************************************
//This is called from AttachCarryActor...
function SetCarryingActor(actor a, optional name nameBone)
{
	if (nameBone == 'None')
		nameBone = 'WeaponRight';		// RightHand

	CarryingActor = a;

	if( CarryingActor != none )
	{
		// Only need to hide wand if carrying something in right hand.
		if (nameBone == 'WeaponRight')	// RightHand
			weapon.bHidden = true;

		//HarryAnimChannel.GotoStateHoldUpArm();
		HarryAnimType = AT_Combine;

		//if( HPawn(CarryingActor) != none )
		//	CarryingActor.GotoState( HPawn(CarryingActor).ObjectPickupState );
		
		// make sure there is no collision now that we are carrying this object
		CarryingActor.SetCollision( false, false, false );

		// affect our holding object so that we are now carrying it.
		CarryingActor.SetOwner( self );
		CarryingActor.AttachToOwner( nameBone );

		CarryingActor.bRotateToDesired = false;

		//If anyone else ever needs to be picked up, this call should be made more generic
		//if( spellFireCracker(a) != none )
		//	spellFireCracker(a).ActorPickedUp();
	}
	else
	{
		//Actually, dont allow this case:
		ClientMessage("******* Dont allow this case   SetCarryingActor *******");
		weapon.bHidden = false;
		//HarryAnimChannel.GotoStateIdle();
		//HarryAnimType = AT_Replace;
	}

	bThrow = false;
}

//********************************************************************************************
function bool InFrontOfHarry( actor a)
{
	local vector cdir, adir;
	local float cos, cdirsize, adirsize;

	// if too far, do nothing
	if(vsize(location - a.location) > 512)
		return false;

	// do not look for something below or above Harry
	if(abs(location.Z - a.location.Z) > 128)
		return false;

	cdir	= cam.vForward;
	cdir.Z	= 0;

	adir	= a.location - location;
	adir.Z	= 0;

	cdirsize= VSize2D(cdir);
	adirsize= VSize2D(adir);
	cos = (cdir dot adir) / (cdirsize * adirsize);

	if(cos > 0.5)		// a little bit less then 45 degrees
		return true;

	return false;
}


function actor FindClosestTargetPoint()
{
	local TargetPoint ClosestTP, CurrTP;
	local float	fClosestDist, fDist;

	ClosestTP = none;
	fClosestDist = 999999;
	foreach AllActors( class'TargetPoint', CurrTP )
	{
//		if( CurrTP.PlayerCanSeeMe() )
		if( InFrontOfHarry( CurrTP ) )

		{
 			fDist = vsize(CurrTP.location - location);
			if( fDist < fClosestDist )
			{
				ClosestTP = CurrTP;
				fClosestDist = fDist;
			}
		}
	}
 	
	return ClosestTP;
}

function actor AccurateThrowing(actor a)
{
	local actor target;
	if(	!HPawn(a).bAccurateThrowing )
		return none;

	target = FindClosestTargetPoint();

	return target;
}

function HarryAccurateThrowObject(actor a, actor target, bool bCollideActors, bool bCollideWorld )
{
	local vector vel;

	// set up the actor for throwing
	a.SetPhysics(PHYS_Falling);

	a.SetCollision( bCollideActors );
	a.bCollideWorld  = bCollideWorld;
	
	vel = ComputeTrajectoryByTime( a.location, target.location, 0.5 );

	a.Velocity = vel;

	a.GotoState( 'stateBeingThrown' );
}


//********************************************************************************************
//This is called from cHarryAnimChannel when it's time to throw the item.
function ThrowCarryingActor()
{
	local vector  v, v2;
	local rotator r;
	local actor   a, target;
	local float   ThrowVelocity;

	//Make sure everythings ok
	if( bThrow  &&  CarryingActor != none )//&& projectile(CarryingActor) != none )
	{
		bThrow = false;

		//May need a variable that sais what state to go to
		//if( spellFireCracker(CarryingActor) != none )
		//{
		//	CarryingActor.GotoState('Flying');
		//	spellFireCracker(CarryingActor).bExplodeOnContact = true;
		//	spellFireCracker(CarryingActor).target = none; //different target than actor::target
		//}

		a = CarryingActor;  //Save CarryingActor, cause DropCarryingActor will clear it.
		DropCarryingActor( true );  //true, do a latent drop, which lets the anim finish in the anim channel

		target = AccurateThrowing(a);
		if(target != none)
		{
			HarryAccurateThrowObject(a, target, true, true);
		}
		else
		{
			//r = Rotation;
			//r.pitch = 30 * 65536/360;
			//v = vector(r);
			v = normal( cam.vForward + vect(0, 0, 0.5) );

			if( HPawn(a) != none )
			{
				ThrowVelocity = HPawn(a).fThrowVelocity;
				a.GotoState( 'stateBeingThrown' );
			}
			else
			{
				ThrowVelocity = 400;
			}

			v *= ThrowVelocity;
			a.Velocity = v;

			//This is a hack, to try and figure out what's going on with the hovering wizard cracker.
			//if( spellFireCracker(CarryingActor) != none )
			//{
			//	spellFireCracker(CarryingActor).iKeepResettingVel = 2;
			//	spellFireCracker(CarryingActor).vInitialVelSave = v;
			//}
		}
	}
}

//********************************************************************************************
//This is called from cHarryAnimChannel when it's time to actually attach the pickup item
function AttachCarryActor(optional name nameBone)
{
	//Make sure the actor is still around
	if( ActorToCarry != none )
		SetCarryingActor( ActorToCarry, nameBone );
	else //Something happened, and the actor is gone.
		DropCarryingActor();
}

//********************************************************************************************
//This is the function you can call to start the process of harry picking up an actor.
function PickupActor( Actor Other )//, optional name nameBone, optional name PickupAnimName )
{
//	ClientMessage("Attempt Pickup:"$Other);

	if(    Physics == PHYS_Walking
	   &&  IsInState( 'PlayerWalking' )
	   &&  CarryingActor == none
	   &&  HPawn(Other) != none
	   &&  HPawn(Other).bObjectCanBePickedUp
	   &&  HarryAnimChannel.CanPickSomethingUp()
	  )
	{
		ClientMessage("Do Pickup");
		ActorToCarry = Other;
		//OptionalCarryBoneName = nameBone;
		//OptionalPickupAnimName = PickupAnimName;

		//HarryAnimChannel.PickUpObject( Other, false/*do normal pickup*/ );
		GotoState( 'statePickupItem' );
	}
}

//********************************************************************************************
//This is the function you try to call to make harry stop carrying something.
//bLatentDrop is true when harry throws the actor, this lets HarryAnimChannel finish it's anim.
function DropCarryingActor( optional bool bLatentDrop )
{
	//Uh, this may collide with harry.

	ClientMessage("** DropCarryingActor");

	if( CarryingActor != none )
	{
		CarryingActor.SetPhysics( PHYS_FALLING );
		CarryingActor.SetOwner( none );
		CarryingActor.Velocity = vect(0,0,125);
		CarryingActor.Instigator = self;
		CarryingActor.bRotateToDesired = true;
		CarryingActor.SetCollision(true, true, true); //<=-- problem with blockplayers?
		CarryingActor = none;
	}

	if( IsInState( 'statePickupItem' ) )
		GotoState( 'PlayerWalking' );

	if( !bLatentDrop )
	{
		HarryAnimChannel.GotoState( 'stateIdle' );
		HarryAnimType = AT_Replace;
	}

	weapon.bHidden = false;
}

//********************************************************************************************
state statePickupItem
{
	function BeginState()
	{
		Velocity *= vect(0,0,1);
		Acceleration *= vect(0,0,1);
	}

  Begin:
	CurrIdleAnimName = GetCurrIdleAnimName();
	PlayAnim( CurrIdleAnimName, [TweenTime]0.4, [Type]HarryAnimType );

	if( ActorToCarry != none )
		TurnTo( ActorToCarry.Location*vect(1,1,0) + Location*vect(0,0,1) );

	//We'll make the animchannel and harry play the same anim, so when they diverge there's no pop
	HarryAnimType = AT_Combine;
	HarryAnimChannel.GotoState( 'statePickupItem' );  //This will play the anim right away
	PlayAnim( 'Pickup', 1, 0.15, [Type]HarryAnimType );
	FinishAnim();
	Sleep( 0.5 );
	//You can now go back to state PlayerWalking
	GotoState('PlayerWalking');
}

//********************************************************************************************

function DoPotionMixingBegin()
{
    bKeepStationary = true;
	GotoState('statePotionMixingBegin');
}

function DoPotionMixingStir()
{
	GotoState('statePotionMixingStir');
	
}

function DoPotionMixingIdle()
{
	GotoState('statePotionMixingIdle');
}

function DoPotionMixingEnd()
{
    CutCue("MixingCauldronDone");
    bKeepStationary = false;
    GotoState('PlayerWalking');
}

function bool IsMixingPotion()
{
    return (IsInState('statePotionMixingBegin') ||
            IsInState('statePotionMixingStir') ||
            IsInState('statePotionMixingIdle'));
}

function CauldronMixing GetNearestMixingCauldron()
{
	local CauldronMixing cauldronTest;
	local CauldronMixing cauldronClosest;

	foreach AllActors(class'CauldronMixing', cauldronTest)
	{
		if (cauldronClosest == None)
			cauldronClosest = cauldronTest;
		else if (VSize2D(cauldronTest.Location - Location) < VSize2D(cauldronClosest.Location - Location))
			cauldronClosest = cauldronTest;
	}
	
	ClientMessage("closest " $cauldronClosest.name);
	return (cauldronClosest);
}

state statePotionMixingBegin
{
	function BeginState()
	{
		Velocity *= vect(0,0,1);
		Acceleration *= vect(0,0,1);
	}
begin:
	CurrIdleAnimName = GetCurrIdleAnimName();
	LoopAnim( CurrIdleAnimName, [TweenTime]0.4, [Type] HarryAnimType);
}

state statePotionMixingStir
{	
	function EndState()
	{
		StopSound(soundStirPotion, SLOT_Interact);
	}

Begin:

	// Make sure Harry is facing the mixing cauldron.
	TurnToward(GetNearestMixingCauldron());

	// Play Harry's stirring animation.
	LoopAnim('MixPotion' ,,, [Type]HarryAnimType);

	// Pick a cauldron stir sound.  Rumor has it that if you play a sound
	// in SLOT_Ambient, it will loop.  I couldn't hear anything if that
	// slot was selected.  So, for now, the sound is selected, we get
	// it's duration, play the sound then play again whent the duration
	// is up.
	switch Rand(4)
	{
	case (0) :
		soundStirPotion = sound'HPSounds.Magic_sfx.cauldron_stir_loop';
		break;
	case (1) :
		soundStirPotion = sound'HPSounds.Magic_sfx.cauldron_stir_loop2';
		break;
	case (2) :
		soundStirPotion = sound'HPSounds.Magic_sfx.cauldron_stir_loop3';
		break;
	case (3) :
		soundStirPotion = sound'HPSounds.Magic_sfx.cauldron_stir_loop4';
		break;
	}	
 
	fStirSoundDuration = GetSoundDuration(soundStirPotion);
Loop:
	PlaySound(soundStirPotion, SLOT_Interact);
	Sleep( fStirSoundDuration);
	Goto 'Loop';
}

state statePotionMixingIdle
{
begin:
	CurrIdleAnimName = GetCurrIdleAnimName();
	LoopAnim( CurrIdleAnimName, [TweenTime]0.4, [Type]HarryAnimType );
}

//********************************************************************************************
function GotoLocation( vector newLoc )
{
	//set correct height
	newLoc.z = newLoc.z + CollisionHeight;
	
	//set position of player
	SetLocation(newLoc);

	//Make it so you dont die when you land
	fHighestZ = location.z;
}

function GotoShortcut(int num)
{
local navShortcut sc;
local int count;
local vector newLoc;

	if( num < 0 )
		num = ShortCutNum++;
	count=0;
	foreach AllActors(class'navShortcut', sc)
	{
		if(count==num)
		{
				//calc new pos
			newLoc=sc.location;
				//set correct height
			newLoc.z=newLoc.z+CollisionHeight;
				//find the camera
			SetLocation(newLoc);
//			cam.SetLocation(newLoc + vect(0, 0, 150));
			fHighestZ = location.z;
		}

		count++;
	}

	if( ShortCutNum >= count )
		ShortCutNum = 0;
}


//********************************************************************************************
//AWRIGHT_111001_001
function int FindNearestSavePointID()
{
	local actor SavePointInstance;
	local int ReturnID;
	local string Str;
	local int SearchStrLen;

	ReturnID = -1;

	SearchStrLen = Len( "savepoint" );

	foreach RadiusActors( class'actor', SavePointInstance, 50, Location )
	{
		Log( "Found Actor In Radius = " $SavePointInstance.Name );

		if ( SavePointInstance.IsA('savepoint') )
		{
			Log( "Found Savepoint = " $SavePointInstance.Name );

			Str = Mid( SavePointInstance.Name, SearchStrLen );
			
			Log( "Found ID Str = " $Str );

			ReturnID = int( Str );
			break;
		}
	}

	return ReturnID;
}
//AWRIGHT_111001_001 - end

//********************************************************************************************
/*
function actor ExtendTarget()
{
	local int	defaultyaw, defaultpitch;
	local float	BestDist, TempDist;
	local actor BestTarget;
	local actor HitActor;
	local rotator checkAngle, bestAngle;
	local vector objectDir;
	
	checkangle = rotator(rectarget.location - location);
	defaultYaw = Rotation.yaw & 0xffff;
	defaultpitch = rectarget.TargetPitch & 0xffff;

	BestTarget = none;

	if (defaultyaw > 0x7fff)
	{
		defaultyaw = defaultyaw - 0x10000;
	}
	if (defaultpitch > 0x7fff)
	{
		defaultpitch = defaultpitch - 0x10000;
	}

	//BaseHUD(MyHUD).DebugValx = defaultyaw;
	//BaseHUD(MyHUD).DebugValy = defaultpitch;

	foreach VisibleActors( class 'ACTOR', hitactor)
	{
		if( HitActor.bprojtarget && PlayerPawn(HitActor) != Self && !HitActor.IsA('BaseCam'))
		{
			objectdir = normal(hitactor.location - location);
			checkAngle = rotator(objectdir);
			checkAngle.yaw = checkAngle.yaw & 0xffff;
			checkAngle.pitch = checkAngle.pitch & 0xffff;
			if (checkAngle.yaw > 0x7fff)
			{
				checkAngle.yaw = checkAngle.yaw - 0x10000;
			}
			if (checkAngle.pitch > 0x7fff)
			{
				checkAngle.pitch = checkAngle.pitch - 0x10000;
			}

			if(abs(checkAngle.yaw - defaultYaw) < 4000 && abs(checkAngle.pitch - defaultPitch) < 4000)
			{
				if( bestTarget == none)
				{
					BestDist = vsize(hitactor.location - location);
					bestTarget = hitactor;
					bestAngle = checkAngle;
				}
				else
				{	
					TempDist = vsize(hitactor.location - location);
					if (TempDist < BestDist)
					{
						BestDist = TempDist;
						bestTarget = hitActor;
						bestAngle = checkAngle;
					}
				}
			}
		}
	}

	if (BestDist > 512)
	{
		BestTarget = none;
	}
	//BaseHUD(MyHUD).DebugValx2 = bestAngle.yaw;
	//BaseHUD(MyHUD).DebugValy2 = bestAngle.pitch;

	return BestTarget;

}
*/


//********************************************************************************************
function KillHarry(bool bImmediateDeath)
{
	clientmessage("argghhh I'm Dead!!!!   in KillHarry");

	// If there is a mini-game director, let it know Harry's dying
	if ( Director != None )
		Director.OnPlayerDying();

	//If you're fighting a boss, stop the boss encounter
	if( baseBoss(BossTarget) != none )
		StopBossEncounter();

	//If you're fighting a boss, and he has a "I win" trigger, send that, and dont kill harry.
	if(   baseBoss(BossTarget) != none
	   && baseBoss(BossTarget).TrigEventWhenVictor != ''
	  )
	{
		baseBoss(BossTarget).SendVictoriousTrigger();
	}
	else
	{
		bAllowHarryToDie = bImmediateDeath;
		gotostate('stateDead');
	}

}

//********************************************************************************************
// a is the actor that clubs him
function KillHarryWithClub(bool bImmediateDeath, actor a)
{
	local int     yaw;
	local rotator r;

	bClubDeath = true;
	KillHarry( bImmediateDeath );

	//yaw = rotator(a.Location - Location).yaw;
	//yaw += 65536/4;

	yaw = a.Rotation.yaw;
	yaw += 65536/4;

	r = rotation;
	r.yaw = yaw;
	SetRotation( r );

	DesiredRotation = r;
}

//********************************************************************************************
//Returns distance to plane.  neg if looking away from plane

//********************************************************************************************
function Died(pawn Killer, name damageType, vector HitLocation)
{
	KillHarry( true );
}

//********************************************************************************************
state stateDead
{
	ignores Fire, AltFire, Tick;

	//Use this, cause it gets called when you call GotoState();
	function BeginState()
	{
		local float fAnimRate;

		// If there is a mini-game director, let it know Harry died
		if ( Director != None )
			Director.OnPlayersDeath();

		fAnimRate = 1.0;

		if( bClubDeath )
			fAnimRate = 1; //2; //3;

		//if( bPitDeath )
		//	fAnimRate = 0.65;

		//enable('tick');
		Velocity.x = 0;
		Velocity.y = 0;
		Acceleration = vect(0,0,0);

		playAnim('faint', fAnimRate, 0.2);

		//Harry's anim has already been started, so now set the frame his faint is on.
		//if( bClubDeath )
		//	AnimFrame = 42.0/93.0;//0.247;
	}

	//*******************************************************************************
	//Get an idea if harry is going to faint into a wall, and if so, move harry away from the wall the appropriate amount.
	function vector FindFaintLocation()
	{
		local float  d;
		local vector n;
		local vector v, vLast, vSave, vDest;
		local float  CheckDist;

		CheckDist = 70;  //70 seems to keep his head snug against the wall...
		d = CheckDist;
		//ClientMessage("********* Location:"$Location);
		//ClientMessage("*********        n:"$vector(rotation));

		vSave = Location;

		n = -vector(rotation);

		vDest = Location + n * CheckDist;

		v = Location;

		do
		{
			vLast = v;
			v += n*10;
			MoveSmooth( n*10 );

			if( Location != v )
			{
				v = vLast;
				break;
			}

			d -= 10;

		}until( d <= 0 );

		if( d <= 0 )
		{
			d = 0;
			v = vDest;
		}

		SetLocation( vSave );

		//ClientMessage("*********        d1:"$d);

		//If d is 0, then there's full room to fall down, return where we're at
		//Actually, lets make it at least 20, so he falls more in place.
		if( d < 20 )
			d = 20;
		//if( d == 0 )
		//	return Location;

		//No see if there's room to move away from the obstruction.  We need to move away only as far as we have to.
		// d is already set to the amount we need to move.
		n = -n;
		v = Location;

		do
		{
			vLast = v;
			v += n*10;
			MoveSmooth( n*10 );

			if( Location != v )
			{
				v = vLast;
				break;
			}


			//See if we can go down.  We dont want to fall off a ledge.
			MoveSmooth( vect(0,0,-20) );
			//ClientMessage("********** d:"$d$" trye zdelt");

			if( v.z - Location.z > 19 )
			{
				//ClientMessage("********** d:"$d$" zdelt was over 19");
				v = vLast;
				break;
			}

			SetLocation( v );


			d -= 10;

		}until( d <= 0 );
		//ClientMessage("*********        d2:"$d$" v:"$v);

		SetLocation( vSave );

		//v should now be where we're moving to...
		return v;
	}

  begin:

	//End of the project hack.  This would normally go to a special HarryDeadCam, but instead for the last two months it's
	// gone to CutState.  So, if you're fighting Voldemort, clear out the CutState vars.
//	if( !( BossTarget != none  &&  BossTarget.IsA('BossQuirrel') ) )
//	{
//		cam.PositionActor = none;
//		cam.DirectionActor = none;
//	}

//	cam.GotoState('CutState');

	RotationRate.yaw = 0;
	AccelRate=70;
	//GroundSpeed = GroundWalkSpeed;
	if( !bPitDeath )
	{
		Sleep( 0.666 );
		MoveTo( FindFaintLocation() );
	}
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	if( bPitDeath )
	{
		Sleep( 0.5 );
	}
	else
	{
		finishAnim();
		sleep( 0.5 );
	}

  loop:

	if( bAllowHarryToDie )
	{
		// Always load "save0.usa".
		ConsoleCommand("LoadGame 0");
		
		
		//I'm sure something else needs to happen here...
		//Level.Game.RestartGame();

		//		baseConsole(player.console).LoadSelectedSlot();
		//		Level.Game.RestartGame();

		/*		if( SaveGameExists() )
				{
					ConsoleCommand("open save9.usa");
				}
				else
				{
					Level.Game.RestartGame();
				}
		*/
		//		ClientTravel( "?load=0", TRAVEL_Absolute, false);

	}

	Sleep( 0.1 );

	goto 'loop';

}

//********************************************************************************************
state stateInactive
{
	ignores TakeDamage, Fire, AltFire, DoJump;
}

function StatusItem GetHealthStatusItem()
{
	return (managerStatus.GetStatusItem(class'StatusGroupHealth', class'StatusItemHealth'));
}

// Add Health points to Harry.
function AddHealth(int iHealth)
{
	local StatusItem siHealth;
	
	siHealth = GetHealthStatusItem();

	if (siHealth != None)
		siHealth.IncrementCount(iHealth);
	else
		log("Error getting health status item");
}

// Get current health points Harry has.
function int GetHealthCount()
{
	local StatusItem siHealth;
	
	siHealth = GetHealthStatusItem();

	if (siHealth != None)
		return siHealth.nCount;
	else
	{
		log("Error getting health status item");
		return (0);
	}
}

// Get health to current potential health ratio.
function float GetHealth()
{
	return (GetHealthStatusItem().GetCountToCurrPotentialRatio());
}

function AddGryffindorPoints(int iPoints)
{
	managerStatus.IncrementCount(class'StatusGroupHousePoints', 
		                         class'StatusItemGryffindorPts', 
				   			     iPoints);
}

function int JellyBeansCount()
{
	local StatusGroup sg;
	local int count;

	sg		= managerStatus.GetStatusGroup(class'StatusGroupJellybeans');
	count	= sg.GetStatusItem(class'StatusItemJellybeans').nCount;
	return count;
}

function int PotionsCount()
{
	local StatusGroup sg;
	local int count;

	sg		= managerStatus.GetStatusGroup(class'StatusGroupPotions');
	count	= sg.GetStatusItem(class'StatusItemWiggenWell').nCount;
	return count;
}

function managerStatus_PickupItem( HProp item )
{
	//Send off this trigger on the first bean you grab
	if( item.IsA('JellyBean')  &&  TriggerToSendOnFirstBean != '' )
	{
		ClientMessage("slkdjflsdkj sdlkfj sldkfj sldkj fsldkf jsdlkf jsdlfk j");
		TriggerEvent( TriggerToSendOnFirstBean, self, self );
		TriggerToSendOnFirstBean = '';
	}

	managerStatus.PickupItem( item );
}

function AddJellyBeansPoints(int iPoints)
{
ClientMessage("ajbp:"$iPoints);

	if(iPoints == 0)
		return;

	if((iPoints < 0) && (JellyBeansCount() == 0))
		return;

	managerStatus.IncrementCount(class'StatusGroupJellybeans', class'StatusItemJellybeans', iPoints);
}

function AddPotionsPoints(int iPoints)
{
	if(iPoints == 0)
		return;

	if((iPoints < 0) && (PotionsCount() == 0))
		return;

	managerStatus.IncrementCount(class'StatusGroupPotions', class'StatusItemWiggenWell', iPoints);
}

function forceHarryLook(actor other)
{
	focusActor=other;
	gotostate('lookatActor');
}


function forceHarrywing(actor other)
{
	focusActor=other;
	setphysics(phys_rotating);
	gotostate('wingspell');
}


function freeHarry()
{
	gotostate('playerWalking');
}

function MovementMode(bool bLockTarget)
{
	if (bLockTarget)
	{
		bStrafe = 1;
		bLook = 1;
		bLockedOnTarget = true;
	}
	else
	{
		bStrafe = 0;
		bLook = 0;
		bLockedOnTarget = false;
	}
}

state lookatActor
{
	ignores Fire, AltFire;
	begin:

	enable('tick');
	CurrIdleAnimName = GetCurrIdleAnimName();
	LoopAnim( CurrIdleAnimName );

	loop:
	//	turnto(focusActor.location);
	//	clientmessage("2focusactor is "$focusactor);

	sleep(0.1);
	goto 'loop';
}

state wingspell
{
	function tick (float deltaTime)
	{
	}

	exec function AltFire( optional float F )
	{
		if(!HProp(focusActor).lockSpell)
		{
			HProp(focusActor).bStopLevitating=true;
		}
	}
	
	function Fire( optional float F )
	{
		if(!HProp(focusActor).lockSpell)
		{
			HProp(focusActor).bStopLevitating=true;
		}
	}
	
	function endstate()
	{
		HProp(focusActor).bStopLevitating=true;
	}

begin:
	
	enable('tick');
	CurrIdleAnimName = GetCurrIdleAnimName();
	LoopAnim( CurrIdleAnimName );
	setphysics(phys_rotating);
	sleep(0.1);
	
loop:
	lookhere=focusActor.location;
	lookhere.z=location.z;
	turnto(lookhere);
	viewrotation=rotation;
	
	//	turnto(focusActor.location);
	if( bSkipKeyPressed )
		HProp(focusActor).bStopLevitating=true;

	playanim('cast');
//	rectarget.victim.eVulnerableToSpell=SPELL_WingSustain;
//	basewand(weapon).ChooseSpell(SPELL_WingSustain);
//	basewand(weapon).castspell(rectarget.victim);
	sleep(0.3);
	goto 'loop';

}

function KeyDownEvent( int Key )
{
	if(   Level.TimeSeconds - _LastKeyPressTime > 1.0
	   || _iCurrentStringChar > 20
	  )
	{
		_iCurrentStringChar = 0;
		_CurrentString = "";
	}

	_LastKeyPressTime = Level.TimeSeconds;

	//if( Key == Asc(Caps(Mid(_sCheatString, _iCurrentStringChar, 1))))

	_CurrentString = _CurrentString $ Chr(Key);
	_iCurrentStringChar++;

	if( _CurrentString ~= "HarryTriggerCheat" )
	{
		TriggerEvent('HarryCheat', self, self);
	}
	//else
	//if( _CurrentString ~= "HarryK" )
	//{
	//	if( IsInState('PlayerWalking') )
	//		HarryK(true);
	//}
	//else
	//if( _CurrentString ~= "HarrySuperK" )
	//{
	//	DoJump(0);
	//	velocity = (vector(Rotation) + vect(0,0,1)) * 650;
	//	HarryK(false);
	//	HarryK(false);
	//	HarryK(false);
	//	HarryK(false);
	//	HarryK(false);
	//	HarryK(true);
	//}
	//else
	//if( _CurrentString ~= "HarryKorWalk" )
	//{
	//	if( IsInState('PlayerWalking') )
	//		HarryK(false);
	//}
	else
	if( _CurrentString ~= "HarryDebugModeOn" )
	{
		TurnDebugModeOn();
	}
	else
	if( _CurrentString ~= "HarryGetsFullHealth" )
	{
		GetHealthStatusItem().SetCountToMaxPotential();
	}
	else
	if( _CurrentString ~= "HarrySuperJump" )
	{
		DoJump(0);
		velocity = (vector(Rotation) + vect(0,0,1)) * 800;
	}
	else
	if( _CurrentString ~= "HarryNormalJump" )
	{
		DoJump(0);
		velocity = (vector(Rotation) + vect(0,0,1)) * 500;
	}
	else
	if( _CurrentString ~= "HarrySword" )
	{
		ToggleUseSword();
	}
	else
	if( _CurrentString ~= "FraserIsGod" )
	{
		bFraserMode = !bFraserMode;
		if( bFraserMode )
			ClientMessage("Indeed, Fraser IS God...");
		else
			ClientMessage("Sad, Fraser is now NOT God.");
	}
	else
	if( _CurrentString ~= "BeatBoss" )
	{
		baseBoss(BossTarget).BeatBoss();
	}
	else
	if(	_CurrentString ~= "Quit" )
	{
	 	/*Console.Viewport.Actor.*/ConsoleCommand("exit");
	}

}

function SpawnAndAttach(name bone)
{
	local actor e;

	e = spawn(class'TorchFire03', [SpawnOwner] self);
	e.AttachToOwner(bone);
	//Log("********* bone:"$bone$"  AnimBone:"$e.AnimBone);
}

/*
function bool PlayNamedCutscene(name CutName)
{
local CutScene cut;
	foreach AllActors( class 'CutScene', cut, CutName )
		{
		cut.StartPlaying();
		return(true);
		}
	return(false);
}
*/

//****************************************************************************
// you can pass none for boss.  If in_vFixedFaceDirection is non zero, then harry will face that direction.
function StartBossEncounter( baseBoss   boss
                            ,bool       in_bHarryShouldLockOntoBoss
                            ,bool       in_bReverseInput
                            ,bool       in_bKeepHarryFixed
                            ,bool       in_bCanCast
                            ,vector     in_vFixedFaceDirection
                            ,ESpellType ForceSpellType
                            ,bool       in_bExtendedTargetting
                           )
{
	local  EnemyHealthManager   EHealth;

	BossTarget = boss;
	bLockedOnTarget = in_bHarryShouldLockOntoBoss;

	if(   in_vFixedFaceDirection.x != 0
	   || in_vFixedFaceDirection.y != 0
	   || in_vFixedFaceDirection.z != 0
	  )
	{
		bFixedFaceDirection = true;
		vFixedFaceDirection = normal(in_vFixedFaceDirection);
	}

	if( in_bHarryShouldLockOntoBoss )// ||  bFixedFaceDirection )
		bStrafe = 1;
	else
		bStrafe = 0;

	if( in_bReverseInput )
	{
		bReverseInput = true;

		//We'll make the assumption here that this is the troll chase.
		bConstrainYaw = true;
	}

	if( in_bKeepHarryFixed )
		bKeepStationary = true;

	bCanCast = in_bCanCast;
	bTargettingError = false;

	if( ForceSpellType != SPELL_None )
	{
		basewand(weapon).ChooseSpell( ForceSpellType, true );//SelectSpell( Class'spellflip' );
		basewand(weapon).bAutoSelectSpell = false;

		//All right, special hack case.  Sorry bout that.
		//if( ForceSpellType == SPELL_Flipendo )
		//{
		//	basewand(weapon).SelectSpell(class'spellFastFlip');
		//	bCastFastSpells = true;
		//}
	}

	bExtendedTargetting = in_bExtendedTargetting;
	if( bExtendedTargetting )
		SpellCursor.SetLOSDistance( 1000 );
	else
		SpellCursor.SetLOSDistance( 0 );


	//cam.SaveState();
	// then RestoreState() in StopBossEncounter

	if( bStrafe == 0 )//in_bHarryShouldLockOntoBoss )
	{
		//Tell the base cam not to use Strafing, cause if that's set, the camera calls
		// baseHarry.MovementMode(true), which turns on 		bStrafe = 1;		bLook = 1;	and  	bLockedOnTarget = true;
		// which we dont want.
		//		cam.bUseStrafing = false;
	}

	if( in_bHarryShouldLockOntoBoss  &&  boss != none )
	{
		//cam.GotoState( boss.GetCamState() ); //'BossState');
		if( !boss.SetCamMode() )
			cam.SetCameraMode( cam.ECamMode.CM_Boss );

		//cam.CameraOffset = boss.GetCameraOffset();
	}

	if( boss != none )
	{
		boss.StartBossEncounter();

		//Energy bars?
		EHealth = EnemyHealthManager( FancySpawn(class'EnemyHealthManager') );
		EHealth.Start( boss);
	}

}

//****************************************************************************
function StopBossEncounter()
{
//	cam.GotoState('StandardState');

	BossTarget = none;
	bLockedOnTarget = false;
	bFixedFaceDirection = false;
	bStrafe = 0;
	bKeepStationary = false;
	bReverseInput = false;
	bConstrainYaw = false;
ClientMessage("baseHarry.StopBossEncounter()");
	basewand(weapon).ChooseSpell( SPELL_None );//SelectSpell( Class'spellflip' );
	basewand(weapon).bAutoSelectSpell = true;
	bCanCast = true;
	bCastFastSpells = false;
	bTargettingError = true;
	bExtendedTargetting = false;
	SpellCursor.SetLOSDistance( 0 );

	GroundRunSpeed = default.GroundRunSpeed;
	GroundSpeed = GroundRunSpeed;

	cam.SetCameraMode( cam.ECamMode.CM_Standard );
}

//****************************************************************************
function name HarryAtMapMarker()
{
	local MenuMapLocationMarker   a;
	local name                    closestAtag;
	local float                   closestD;
	local float                   d;

	closestD = 1000000;

	ForEach AllActors(class'MenuMapLocationMarker', a)
	{
		d = VSize2d( a.location - Location );
		if(   d  <  CollisionRadius + a.CollisionRadius
		   && Location.z > a.location.z - a.CollisionHeight - 80
		   && Location.z < a.location.z + a.CollisionHeight + 80
		  )
		{
			if( d < closestD )
			{
				closestD = d;
				closestAtag = a.tag;
			}
		}
	}

	return closestAtag;
}

//****************************************************************************

// <EAUK> Function which updates the "inverted broom" state, then saves
// the configuration. This is used by FEOptionsPage, instead of
// updating the variable directly, to ensure that the value is saved.
function InvertBroomPitch( bool Value )
{
	bInvertBroomPitch = Value;
	SaveConfig();
}
//****************************************************************************

//********************************************************************************************************************
//********************************************************************************************************************
//********************************************************************************************************************
//********************************************************************************************************************
//********************************************************************************************************************
//********************************************************************************************************************
//********************************************************************************************************************
//********************************************************************************************************************
//********************************************************************************************************************


//Play a sound client side (so only client will hear it
simulated function ClientPlaySound(sound ASound, optional bool bInterrupt, optional bool bVolumeControl )
{	
	local actor SoundPlayer;
	local int Volume;

	if( b3DSound )
	{
		if ( bVolumeControl && (AnnouncerVolume == 0) )
			Volume = 0;
		else
			Volume = 1;
	}
	else if ( bVolumeControl )
		Volume = AnnouncerVolume;
	else
		Volume = 4;

	LastPlaySound = Level.TimeSeconds;	// so voice messages won't overlap

	if ( ViewTarget != None )
		SoundPlayer = ViewTarget;
	else
		SoundPlayer = self;

	if ( Volume == 0 )
		return;
	SoundPlayer.PlaySound(ASound, SLOT_None, 16.0, bInterrupt);
	if ( Volume == 1 )
		return;
	SoundPlayer.PlaySound(ASound, SLOT_Interface, 16.0, bInterrupt);
	if ( Volume == 2 )
		return;
	SoundPlayer.PlaySound(ASound, SLOT_Misc, 16.0, bInterrupt);
	if ( Volume == 3 )
		return;
	SoundPlayer.PlaySound(ASound, SLOT_Talk, 16.0, bInterrupt);
}


function DebugState()
{
//	BaseHUD(MyHUD).DebugString2 = string(GetStateName());
//	ClientMessage(string(GetStateName()));
//	log("Harry state is " $string(GetStateName()));

}

function TurnDebugModeOn()
{
	hpconsole(player.console).bDebugMode = true;
}

function PreSetMovement()
{
	bCanJump = true;
	bCanWalk = true;
	bCanSwim = true;
	bCanFly = false;
	bCanOpenDoors = true;
	bCanDoSpecial = true;
}

function HarryKnockBack()
{ 
 	PlaySound( HurtSound[ Rand(NUM_HURT_SOUNDS) ] );

 	//If we're carrying an item, toss it forward a bit.  At this point, we'll assume it's a firecracker
 	if( CarryingActor != none )
 		DropCarryingActor();

 	HarryAnimChannel.DoKnockBack();
 	Acceleration *= vect(0,0,1);
}

//*******************************************************************************
function TakeDamage( int Damage, Pawn instigatedBy, Vector hitlocation, 
							Vector momentum, name damageType)
{
	local Sound			snd;
	local bool			bHarryKilled;
	local bool			bPlayKnockBack;
	local bool			bPlayHurtSound;
	local float			fFlashScale;
	local StatusItem	siWiggenPotion;
	local bool			bFallDamage;
	local float			fDamageScaled;
	local int			FinalDamage;

	//cm("********* "$self$" ib:"$instigatedBy$" damateType:"$damageType);

	//Dont take damage from the item harry is carrying
	if( (CarryingActor != None) &&
		(CarryingActor == instigatedBy) )
		return;

	//I handle collision differently for the basil spell, so this is needed.
	// ** there is no basil spell anymore.
	//if(   HarryAnimChannel.IsInState( 'stateKnockBack' )
	//   && DamageType == 'BasiliskSpell'
	//  )
	//	return;

	bPlayHurtSound = true;
	
	
	// Alter damage acording to Difficulty
	fDamageScaled = Damage;
	switch( Difficulty )
	{
		case DifficultyEasy:	fDamageScaled = ( fDamageScaled * fDamageMultiplier_Easy  ); break;
		case DifficultyMedium:	fDamageScaled = ( fDamageScaled * fDamageMultiplier_Medium); break;
		case DifficultyHard:	fDamageScaled = ( fDamageScaled * fDamageMultiplier_Hard  ); break;
	}
	FinalDamage = fDamageScaled;


    // ******************** SCREEN FLASH *******************
	// Flash the screen so the user knows Harry is getting hit
	fFlashScale = FClamp(FinalDamage, 20, 60);
	
	switch( damageType )
	{
//		Commented out per EA
		// red/oarnge
//		case 'BasiliskSpell':
//		case 'exploded':
//			ClientFlash( -0.009375 * fFlashScale, fFlashScale * vect(16.41, 11.719, 4.6875));
//			break;
		
		// white
		case 'Falling':
			ClientFlash( -0.02 * fFlashScale, fFlashScale * vect(20, 20, 20));
			break;

		// green
		case 'ectoplasma':
			if( !bEctoFlashed )
			{
				bEctoFlashed = true; 
				ClientFlash( -0.02 * fFlashScale, fFlashScale * vect(9.375, 14.0625, 4.6875));
			}
			break;

		case 'PoisonCloud':
			ClientFlash( -0.01171875 * fFlashScale, fFlashScale * vect(7.8,11.718,11.718) );
			break;

		// violet/blue
//		case 'Pixie': 
//			ClientFlash(-0.390, vect(112.5,112.5,468.75));
//			break;

		// standard red
//		Commented out per EA
//		default:
//			ClientFlash( -0.019 * fFlashScale, fFlashScale * vect(26.5, 4.5, 4.5));
//			break;
	} // end SCREEN FLASH switch

	// *****************************************************

	//Check and see if harry has life potions left
	if( !HarryIsDead() )
	{
		//If we're carrying an item, toss it forward a bit.  At this point, we'll assume it's a firecracker
		if( CarryingActor != none )
			DropCarryingActor();

		bThrow = false;  //<==-- this variable needs to be got rid of.


		if( DamageType == 'ZonePain' || DamageType == 'pit' || DamageType == 'crushed' )
		{
			PlaySound( snd );

			bPitDeath = true;
			Damage = 1000;
			//gotostate ('hit_InstantPitDeath');
			bHidden = true;
		}
		else
		{
			//Save weather you took SIGNIFICANT fall damage
			if( DamageType == 'Falling'  &&  FinalDamage > 20 )
				bFallDamage = true;

			//playsound(sound'HPSounds.dlg_har.har_002');
			if( iEctoRefCount > 0 )
			{
				if( !bPlayedEctoKnockBack  ||  ++iEctoHurtSoundCount >= 6 )
					iEctoHurtSoundCount = 0;
				else
					bPlayHurtSound = false;
			}

			if( bPlayHurtSound )
				PlaySound( HurtSound[ Rand(NUM_HURT_SOUNDS) ] );

			// The result when you walk into a web. 
			// I am using the ecto stuff because it's the same effect NOT because it has anything to do 
			// with ecto
			if( iWebAnimRefCount > 0 )
			{
				if( !bPlayedEctoKnockBack  ||  ++iEctoHurtSoundCount >= 6 )
					iEctoHurtSoundCount = 0;
				else
					bPlayHurtSound = false;
			}

			if( bPlayHurtSound )
				PlaySound( HurtSound[ Rand(NUM_HURT_SOUNDS) ] );
			// end web

			bPlayKnockBack = true;

			//If we're in ecto, and we already played our ecto knockback, dont do it again.
			if( iEctoRefCount > 0 )
			{
				if( bPlayedEctoKnockBack )
					bPlayKnockBack = false;

				bPlayedEctoKnockBack = true;
			}

			// The result when you walk into a web. 
			// I am using the ecto stuff because it's the same effect NOT because it has anything to do 
			// with ecto
			//If we're in a web, and we already played our knockback, dont do it again.
			if( iEctoRefCount > 0 )
			{
				if( bPlayedEctoKnockBack )
					bPlayKnockBack = false;

				bPlayedEctoKnockBack = true;
			}
			// end web


			if( bPlayKnockBack )
				HarryAnimChannel.DoKnockBack();

			Acceleration *= vect(0,0,1);
		}
	}

	//super.takedamage(FinalDamage, instigatedBy,hitlocation, // Will change state to 'stateDead' if enough damage
	//						momentum, damageType);


	if( GetHealthCount() > 0  &&  !bFraserMode )
	{
		clientmessage("baseHarry: argghhh I'm HIT!!!!  "$FinalDamage $" Difficulty:" $Difficulty $" Type:"$DamageType$" State:"$GetStateName());
		AddHealth(-FinalDamage);

		if( GetHealthCount() <= 0.0 )
		{
			siWiggenPotion = managerStatus.GetStatusItem(class'StatusGroupPotions', class'StatusItemWiggenWell');

			//Try and do an AutoQuaff of one of our potions
			if( bAutoQuaff && !bFallDamage  &&  !bPitDeath  &&  siWiggenPotion.nCount >= 1 )
			{
				AddHealth(1);
				HarryAnimChannel.DoDrinkWiggenwell();
			}
			else		
			{
				bHarryKilled = true;
			}
		}

		if( BossTarget != none )
		{
			if( bHarryKilled )
				BossTarget.OnEvent( 'HarryWasKilled' );
			else
				BossTarget.OnEvent( 'HarryWasHurt' );
		}

		if( bHarryKilled )
			KillHarry(true);
	}
	else
	{
		clientmessage("baseHarry: argghhh I'm HIT!!!! (no damage) "$FinalDamage$" Type:"$DamageType$" State:"$GetStateName());
	}

}

//*******************************************************************************
exec function Summon( string ClassName )
{
	//local class<actor> NewClass;
	//if( instr(ClassName,".")==-1 )
	//	ClassName = "HPDecorations." $ ClassName;
	Summon( ClassName );
}

//*******************************************************************************
event Landed(vector HitNormal)
{
	GroundSpeed = GroundRunSpeed;
}

function Falling()
{
	local float s;

	//Limit his speed to 200 so puzzles arent' broken
	s = VSize2d( Velocity );
	if( s > GroundJumpSpeed )
		Velocity *= GroundJumpSpeed/s;

	//Change his ground speed so he cant go any faster then GroundJumpSpeed.  Will be restored when he lands.
	GroundSpeed = GroundJumpSpeed;
}

//*******************************************************************************
simulated function PlayFootStep()
{
	local sound step;
	local float decision;

	local Texture HitTexture;
	local int Flags;

	local sound Footstep1;
	local sound Footstep2;
	local sound Footstep3;

	local bool  bMakeNoise;

	if( FootRegion.Zone.bWaterZone )
	{
		PlaySound(WaterStep, SLOT_Interact, 1, false, 1000.0, 1.0);
		return;
	}

	if( iEctoRefCount > 0 )
	{
		Footstep1 = sound'HPSounds.FootSteps.Har_foot_ecto1';
		Footstep2 = sound'HPSounds.FootSteps.Har_foot_ecto2';
		Footstep3 = sound'HPSounds.FootSteps.Har_foot_ecto3';
	}
	else if( iWebAnimRefCount > 0 )
	{
		Footstep1 = sound'HPSounds.FootSteps.Har_foot_ecto1';
		Footstep2 = sound'HPSounds.FootSteps.Har_foot_ecto2';
		Footstep3 = sound'HPSounds.FootSteps.Har_foot_ecto3';
	}
	else
	{
		HitTexture = TraceTexture(Location + (vect(0,0,-128)), Location, Flags );

		Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_wood1';
		Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_wood2';
		Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_wood3';
		bMakeNoise = true;

		if(HitTexture!=None)	//cmp 10-17 log spam fix.
		{
			switch( HitTexture.FootstepSound )
			{
				case FOOTSTEP_Rug:
					Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_rug1';
					Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_rug2';
					Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_rug3';
					bMakeNoise = false;
					break;

				case FOOTSTEP_Wood:
					Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_wood1';
					Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_wood2';
					Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_wood3';
					break;

				case FOOTSTEP_Stone:
					Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_stone1';
					Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_stone2';
					Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_stone3';
					break;

				case FOOTSTEP_cave:
					Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_cave1';
					Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_cave2';
					Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_cave3';
					break;

				case FOOTSTEP_cloud:
					Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_cloud1';
					Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_cloud2';
					Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_cloud3';
					bMakeNoise = false;
					break;

				case FOOTSTEP_wet:
					Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_wet1';
					Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_wet2';
					Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_wet3';
					bMakeNoise = false;
					break;

				case FOOTSTEP_grass:
					Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_grass1';
					Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_grass2';
					Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_grass3';  //bad sound
					bMakeNoise = false;
					break;

				case FOOTSTEP_metal:
					Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_metal1';
					Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_metal2';
					Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_metal3';
					break;
			}
		}
	}

	decision = FRand();
	if ( decision < 0.34 )
		step = Footstep1;
	else if (decision < 0.67 )
		step = Footstep2;
	else
		step = Footstep3;

	PlaySound(step, SLOT_None,1, false, 1000.0, 0.9);

	if( bMakeNoise )
	{
		HearHarryRecipient.PawnHearHarryNoise();
		MakeNoise( 10 );
	}
}

//****************************************************************************************************
function PlayHit(float Damage, vector HitLocation, name damageType, vector Momentum)
{
/*	local float rnd;
	local Bubble1 bub;
	local bool bServerGuessWeapon;
	local class<DamageType> DamageClass;
	local vector BloodOffset, Mo;
	local int iDam;

	if ( (Damage <= 0) && (ReducedDamageType != 'All') )
		return;

	//DamageClass = class(damageType);
	if ( ReducedDamageType != 'All' ) //spawn some blood
	{
		if (damageType == 'Drowned')
		{
			bub = spawn(class 'Bubble1',,, Location 
				+ 0.7 * CollisionRadius * vector(ViewRotation) + 0.3 * EyeHeight * vect(0,0,1));
			if (bub != None)
				bub.DrawScale = FRand()*0.06+0.04; 
		}
		else if ( (damageType != 'Burned') && (damageType != 'Corroded') 
					&& (damageType != 'Fell') )
		{
			BloodOffset = 0.2 * CollisionRadius * Normal(HitLocation - Location);
			BloodOffset.Z = BloodOffset.Z * 0.5;
			if ( (DamageType == 'shot') || (DamageType == 'decapitated') || (DamageType == 'shredded') )
			{
				Mo = Momentum;
				if ( Mo.Z > 0 )
					Mo.Z *= 0.5;
				spawn(class 'UT_BloodHit',self,,hitLocation + BloodOffset, rotator(Mo));
			}
			else
				spawn(class 'UT_BloodBurst',self,,hitLocation + BloodOffset);
		}
	}	

	rnd = FClamp(Damage, 20, 60);
	if ( damageType == 'Burned' )
		ClientFlash( -0.009375 * rnd, rnd * vect(16.41, 11.719, 4.6875));
	else if ( damageType == 'Corroded' )
		ClientFlash( -0.01171875 * rnd, rnd * vect(9.375, 14.0625, 4.6875));
	else if ( damageType == 'Drowned' )
		ClientFlash(-0.390, vect(312.5,468.75,468.75));
	else 
		ClientFlash( -0.019 * rnd, rnd * vect(26.5, 4.5, 4.5));

	ShakeView(0.15 + 0.005 * Damage, Damage * 30, 0.3 * Damage); 
	PlayTakeHitSound(Damage, damageType, 1);
	bServerGuessWeapon = ( ((Weapon != None) && Weapon.bPointing) || (GetAnimGroup(AnimSequence) == 'Dodge') );
	iDam = Clamp(Damage,0,200);
	ClientPlayTakeHit(hitLocation - Location, iDam, bServerGuessWeapon ); 
	if ( !bServerGuessWeapon 
		&& ((Level.NetMode == NM_DedicatedServer) || (Level.NetMode == NM_ListenServer)) )
	{
		Enable('AnimEnd');
		BaseEyeHeight = Default.BaseEyeHeight;
		bAnimTransition = true;
		PlayTakeHit(0.1, hitLocation, Damage);
	}*/
}


function DoJump( optional float F )
{
	local float      TmpJumpZ;
	local vector     v;
	local float      s;

	if( bKeepStationary || bInDuelingMode )
		return;

	// do not jump, if corraled by mover, 
	// otherwise harry will lose his base and can jump from coralled mover
	if(bCorraledByMover)
		return;
	
	// if we are in ectoplasma then we cannot jump
	if( iEctoRefCount > 0 )
	{
		// play the ectoJump anim
		PlayAnim( HarryAnims[HarryAnimSet].Jump , [TweenTime]0.1, [Type] HarryAnimType );
		HarryAnimChannel.DoEctoJump();
		return;
	}
	else if ( iSleepyAnimTimer > 0 )
	{
		// Play the sleepy jump. 
		PlayAnim( HarryAnims[HarryAnimSet].Jump , [TweenTime]0.1, [Type] HarryAnimType );
		HarryAnimChannel.DoSleepyJump();
		return;
	}
//	else if ( iWebAnimRefCount > 0 )
//	{
//		// Play the web jump (actually this is the ecto jump but it's correct
//		PlayAnim( HarryAnims[HarryAnimSet].Jump , [TweenTime]0.1, [Type] HarryAnimType );
//		HarryAnimChannel.DoWebJump();
//		return;
//	}

	if( Physics == PHYS_Walking )
	{
		//Not any more, now you can jump and cast
		//StopAiming();

		//if ( !bUpdating )
		PlayOwnedSound(JumpSound, SLOT_Talk, 1.5, true, 1200, 1.0 );

		if ( (Level.Game != None) && (Level.Game.Difficulty > 0) )
			MakeNoise(0.1 * Level.Game.Difficulty);

		MountDelta = Location;
		if( VSize2D(Velocity) > 0 )
			PlayAnim( HarryAnims[HarryAnimSet].Jump2, [TweenTime]0.1, [Type] HarryAnimType);
		else
			PlayAnim( HarryAnims[HarryAnimSet].Jump , [TweenTime]0.1, [Type] HarryAnimType );
		/*
		switch( Rand(3) )
		{
			case 0:    PlaySound( sound'HPSounds.HAR_emotes.jump1' );     break;
			case 1:    PlaySound( sound'HPSounds.HAR_emotes.jump2' );     break;
			case 2:    PlaySound( sound'HPSounds.HAR_emotes.jump3' );     break;
		}
		*/

		//Limit his speed to 200 so puzzles arent' broken
		s = VSize2d( Velocity );
		if( s > GroundJumpSpeed )
			Velocity *= GroundJumpSpeed/s;

		//Change his ground speed so he cant go any faster then GroundJumpSpeed.  Will be restored when he lands.
		GroundSpeed = GroundJumpSpeed;
		TmpJumpZ = JumpZ;

		//Velocity.Z += TmpJumpZ;
		Velocity.Z = Velocity.Z * 0.2  +  TmpJumpZ; //Still use some of our old velocity, just not all...

		if ( (Base != Level) && (Base != None) )
			Velocity += Base.Velocity; 

		SetPhysics(PHYS_Falling);
	}
}

//-----------------------------------------------------------------------------
// Sound functions

//*****************************************************************************************
//This isn't done in Global.Landed() cause you dont want the sound when harry grabs a ledge
function PlayLandedSound()
{
	local sound   step;
	local float   decision;
	local Texture HitTexture;
	local int     Flags;
	local float   vol;

	if ( FootRegion.Zone.bWaterZone )
	{
		PlaySound(WaterStep, SLOT_Interact, 1, false, 1000.0, 1.0);
		return;
	}

	HitTexture = TraceTexture(Location + (vect(0,0,-128)), Location, Flags );

	step = Sound'HPSounds.FootSteps.HAR_Landing_stone';

	switch( HitTexture.FootstepSound )
	{
		case FOOTSTEP_Wood:
		//	step = Sound'HPSounds.FootSteps.HAR_Landing_wood';
			break;

		case FOOTSTEP_cloud:
		case FOOTSTEP_grass:
		case FOOTSTEP_Rug:
			step = Sound'HPSounds.FootSteps.HAR_Landing_rug';
			break;

		case FOOTSTEP_Stone:
		case FOOTSTEP_cave:
			step = Sound'HPSounds.FootSteps.HAR_Landing_stone';
			break;

		case FOOTSTEP_wet:
			step = Sound'HPSounds.FootSteps.HAR_Landing_wet';
			break;

		case FOOTSTEP_metal:
			step = Sound'HPSounds.FootSteps.HAR_Landing_metal';
			break;
	}

	if( fTimeInAir < 1.0 )
		vol = 0.3 * fTimeInAir;
	else
		vol = 0.3 + (fTimeInAir - 1.0) * 0.7/0.5;

	//if you fell, rather than jumped, make the sound louder
	if( Location.z < fFallingZ - 40 )
		vol *= 2;

//	PlaySound(step, SLOT_Interact, vol, false, 1000.0, 0.9);

	switch( Rand(5) )
	{
		case 0:    step = sound'HPSounds.Har_emotes.landing1';    break;
		case 1:    step = sound'HPSounds.Har_emotes.landing2';    break;
		case 2:    step = sound'HPSounds.Har_emotes.landing3';    break;
		case 3:    step = sound'HPSounds.Har_emotes.landing4';    break;
		case 4:    step = sound'HPSounds.Har_emotes.landing5';    break;
	}

	PlaySound(step, SLOT_Misc, vol, false, 1000.0);

	HearHarryRecipient.PawnHearHarryNoise();
	MakeNoise( 10 );
}

//*****************************************************************************************
//function Gasp()
//{
//	if ( PainTime < 2 )
//		PlaySound(GaspSound, SLOT_Talk, 2.0);
//	else
//		PlaySound(BreathAgain, SLOT_Talk, 2.0);
//}

//-----------------------------------------------------------------------------
// Animation functions

function PlayTurning()
{
	PlayAnim( HarryAnims[HarryAnimSet].StrafeLeft, [Type] HarryAnimType);
}

function TweenToRunning(float tweentime)
{
	local vector X,Y,Z, Dir;

	BaseEyeHeight = Default.BaseEyeHeight;

	if( !HarryAnimChannel.PlayHarryMovementAnims() )
		return;

	//LastAnimFrame

	//if (bIsWalking)
	//{
	//	TweenToWalking(0.1);
	//	return;
	//}
	GetAxes(Rotation, X,Y,Z);
	Dir = Normal(Acceleration);
	if ( (Dir Dot X < 0.75) && (Dir != vect(0,0,0)) )
	{
		// strafing or backing up
		if ( Dir Dot X < -0.75 )
		{
			LoopAnim( HarryAnims[HarryAnimSet].WalkBack, 0.9, tweentime, [Type]HarryAnimType );
			bMovingBackwards=true;
			//Velocity.X = Velocity.X / 2;
		}
		else if ( Dir Dot Y > 0 )
		{
			LoopAnim( HarryAnims[HarryAnimSet].StrafeRight, 0.9, tweentime, [Type]HarryAnimType );
			//Velocity.Y 
		}
		else
			LoopAnim( HarryAnims[HarryAnimSet].StrafeLeft, 0.9, tweentime, [Type]HarryAnimType );
	}
	else 
	{
		LoopAnim( HarryAnims[HarryAnimSet].Run, 0.9, tweentime, [Type]HarryAnimType );
		bMovingBackwards=false;
	 }
}

function PlayRunning()
{

	TweenToRunning( 0 );
/*
	local vector X,Y,Z, Dir;

	BaseEyeHeight = Default.BaseEyeHeight;

	// determine facing direction
	clearMessages=true;
	GetAxes(Rotation, X,Y,Z);
	Dir = Normal(Acceleration);
	if ( (Dir Dot X < 0.75) && (Dir != vect(0,0,0)) )
	{
		// strafing or backing up
		if ( Dir Dot X < -0.75 )
		{
			LoopAnim('runback', [Type] HarryAnimType);
			bMovingBackwards=true;
			Velocity.X = Velocity.X / 2;
		}
		else if ( Dir Dot Y > 0 )
		{
			LoopAnim('StrafeRight', [Type] HarryAnimType);
			clientmessage("strafe right");
		}
		else
		{
			LoopAnim('StrafeLeft', [Type] HarryAnimType);
		}
	}
	else 
	{
		LoopAnim('run', [Type] HarryAnimType);
		bMovingBackwards=false;
	}
*/	
}


function PlayinAir()
{
	loopAnim( AnimFalling, [TweenTime]0.4, [Type]HarryAnimType);

	ClientMessage(" animFalling = " $AnimFalling );
	
/*
	else
	{
		ClientMessage(" HarryAnims[HarryAnimSet].fall = " $HarryAnims[HarryAnimSet].fall );
		loopAnim(HarryAnims[HarryAnimSet].fall, [TweenTime]0.4, [Type]HarryAnimType);
	}
*/
}


function PlayDuck()
{
	BaseEyeHeight = 0;
	TweenAnim('SneakF', 0.25);
}

function PlayCrawling()
{
	//log("Play duck");
	BaseEyeHeight = 0;
	LoopAnim('SneakF');
}

function PlayIdle()
{
	if ( Mesh == None )
		return;

	CurrIdleAnimName = GetCurrIdleAnimName();
	LoopAnim( CurrIdleAnimName, 0.8, 0.25, , HarryAnimType );
}

function PlayWaiting()
{
	if ( Mesh == None )
		return;

	//keep idle from happening as soon as a level loads
	WaitingCount++;
	if(WaitingCount < 3)	//do breath first three times you get here.
	{
		CurrIdleAnimName = GetCurrIdleAnimName();
		LoopAnim( CurrIdleAnimName, 0.4 + 0.4 * FRand(), 0.25, , HarryAnimType );
		return;
	}

	if ( FRand() < 0.5)
	{
		CurrIdleAnimName = GetCurrIdleAnimName();
		LoopAnim( CurrIdleAnimName, 0.4 + 0.4 * FRand(), 0.25, , HarryAnimType );
	}
	else 
	{
		WaitingCount=0;
		CurrFidgetAnimName = GetCurrFidgetAnimName();
		if ( bVeryAfraid == true )
		{
			PlayAnim( 'look_frantic', 1.0, 0.2, [Type]HarryAnimType );
		}
		else
		{
			PlayAnim( CurrFidgetAnimName, 0.5 + 0.5 * FRand(), 0.3, HarryAnimType);
		}
	}
}

function TweenToWaiting(float tweentime)
{
	if( !HarryAnimChannel.PlayHarryMovementAnims() )
		return;

	// if it is playing fidget animation, do not interapt
	if(PlayingFidgetAnimation(AnimSequence))
		return;

	// if playing the look_frantic animation, do not interupt
	if ( AnimSequence == 'look_frantic' )
		return;

	CurrIdleAnimName = GetCurrIdleAnimName();
	LoopAnim( CurrIdleAnimName, [TweenTime]tweentime, [Type] HarryAnimType);
}

function Cast()
{
	local actor BestTarget;
	local actor HitActor;
	local rotator defaultAngle, checkAngle;
	local pawn hitPawn;
	local vector objectDir;
	local int bestYaw;
	local int tempYaw, defaultYaw;
	local float	BestDist, TempDist;

	if( fTimeAfterHit > 0 )
		return;

//	// do not defence himself in dueling mode, if are not charged enough
//	if( bInDuelingMode && (CurrentDuelSpell == 2) && (baseWand(weapon).ChargingLevel() < 0.05 * (1 + 3 * Duellist(DuelOpponent).Intellect)) )
//		return;

	defaultAngle = Rotation;
	defaultAngle.pitch = 0;
	defaultYaw = defaultAngle.yaw;
	defaultYaw = defaultYaw & 0xffff;

	if (defaultyaw > 0x7fff)
		defaultyaw = defaultyaw - 0x10000;

	bestTarget = none;
	
	//Also, if you're fighting a boss, make sure your target is the boss
	if( BossTarget != none )
		target = BossTarget;
	
	if( bInDuelingMode )
	{
		baseWand(weapon).CastSpell( DuelOpponent, , DuelSpells[CurrentDuelSpell] );
   		baseWand(weapon).LastCastedSpell.SetSpellDirection( SpellCursor.location - baseWand(weapon).LastCastedSpell.location );
	}
	else
	if( bHarryUsingSword ) //Otherwise if using sword mode, pass none so the weapon will just shoot.
	{
		baseWand(weapon).CastSpell( none,, class'spellSwordFire' );
	}
	else // If we're in normal cast mode, and we have a victim, shoot our spell at it!
	if( SpellCursor.IsLockedOn() )
	{
		baseWand(weapon).CastSpell( SpellCursor.aCurrentTarget, SpellCursor.vTargetOffset );
	}
	if( !baseWand(weapon).bAutoSelectSpell ) // or, if "autoselect spell" is off
	{
		baseWand(weapon).CastSpell( BossTarget, vect(0,0,0) );
		baseWand(weapon).LastCastedSpell.SeekSpeed *= 0.25;
	}
	else
	{
		//DEBUG
		ClientMessage("Harry Can't cast a spell... SpellCursor.IsLockedOn = " $SpellCursor.IsLockedOn()
						$" CurrentSpell = " $baseWand(weapon).CurrentSpell );
	}
	
	// Turn off castingVars
	TurnOffSpellCursor();
}


// The player wants to fire.
function Fire( optional float F )
{
	if( Weapon!=None && bJustFired  == false)
	{
		Weapon.bPointing = true;
		//PlayAnim('wave');
	}

	bJustFired = true;
}

// The player wants to alternate-fire.
exec function AltFire( optional float F )
{
	local vector v;
	local rotator r;

	// If Harry is frozen, disable firing

	//if (IsInState('HarryFrozen') || Physics == PHYS_Falling || baseHud(myhud).bCutSceneMode == true)
	//{
	//	return;
	//}

	//if( BossTarget != none && bCanCast == false)
	if( HarryAnimChannel.IsCarryingActor() )
	{
		//if( bThrow == false  &&  projectile(CarryingActor) != none  &&  IsInState('PlayerWalking') )
		if( bThrow == false  &&  IsInState('PlayerWalking') )
		{
			ClientMessage("Throw!");
			//Start throw animation, state PlayerWalking will monitor it from there
			HarryAnimChannel.GotoStateThrow();
			bThrow = true;

			//The animation channel will then call ThrowCarryingActor() when the time is right
		}
	}
	else
	{
		//DEBUG
//		ClientMessage("****** Harry bIsAiming:"$bIsAiming);

		if(   Weapon != None
		   //&& bJustAltFired == false
		   && CarryingActor == none   //you're not carrying an actor in your hand
		   //&& Physics == PHYS_Walking  No longer required
		   && !bIsAiming //!HarryAnimChannel.IsCasting()
		  )
		{
			//DEBUG
//			ClientMessage("Harry::AltFire");
			Weapon.bPointing = true;
			//	PlaySound(sound'spell1', SLOT_Interact, 2.2, false, 1000.0, 1.0);
			//	weapon.altfire(1);

			StartAiming( bHarryUsingSword );
		}
	}
}

//**************************************************************************************************
event Mount( vector Delta )
{
	//Drop any item you might be carrying
	DropCarryingActor();

	// Use Destination to store dest, as moveTo won't be called here.
	Destination = Location+Delta;
	MountBase = Base;
	if( Physics == PHYS_Falling )
	{
		bFallingMount = true;
		gotoState('FallingMount');
	}
	else
	{
		bFallingMount = false;
		gotoState('Mounting');
	}
}

state FallingMount
{
	ignores AltFire;

	event PlayerTick( float DeltaTime )
	{
		global.PlayerTick( DeltaTime );

		//problem where, if an elevator is moving upwards, and you jump and nick your toe so you 
		// do one of those "in the air falling mount" deals, the engine will make harry tilt forward
		// for some reason.  Just make sure his desiredrotation.pitch is zero.
		DesiredRotation.Pitch = 0;

		// Adjust destination by player and base movement.
		Destination.X += Location.X - OldLocation.X;
		Destination.Y += Location.Y - OldLocation.Y;
		if( Mover(MountBase) != none )
			Destination += MountBase.Location - MountBase.OldLocation;

		// Wait till we fall to catch point.
		if( Location.Z <= Destination.Z-82 )
		{
			// Move to exact height.
			Move( vec(0, 0, Destination.Z-82 - Location.Z) );
			gotoState('Mounting');
		}
	}

	function Mount(vector Delta)
	{
		// Update Dest.
		Destination = Location+Delta;
		MountBase = Base;
	}

	function Landed(vector HitNormal)
	{
		Global.Landed( HitNormal );
		PlaySound(Sound'HPSounds.HAR_emotes.landing5', SLOT_Interact,1, false, 1000.0, 0.9);
		gotoState('Mounting');
	}

	function BeginState()
	{
		DebugState();
		// Fall until we get to an exact mount height.
		// Start tweening to proper animation.
		playAnim('climb96end', [Rate] 0, [TweenTime] 0.5, [RootBone] 'move');
	}

begin:
	// Start turning here as well.
	//TurnTo( vec(Destination.X, Destination.Y, Location.Z) );
	DesiredRotation.Yaw = rotator( vec(Destination.X, Destination.Y, Location.Z) - Location ).yaw;
}

state Mounting
{
	ignores Mount, AltFire;

	function ProcessMove(float DeltaTime, vector NewAccel, eDodgeDir DodgeMove, rotator DeltaRot)
	{
		// Ignore acceleration.
		global.ProcessMove( DeltaTime, vect(0,0,0), DodgeMove, DeltaRot );
	}

	function BeginState()
	{
		DebugState();

		// Disable normal physics while performing mounting move.
		Velocity = vect(0,0,0);
		Acceleration = vect(0,0,0);
		SetPhysics(PHYS_Projectile);
		SetBase(MountBase);
	}

begin:
	// Finish turning.
	TurnTo( vec(Destination.X, Destination.Y, Location.Z) );
	//Ultra cheap fix, to make sure he has no pitch when he's done with the turnto.  If harry is falling while
	// he does the turnTo, he'll pitch onto his back.
	DesiredRotation.Pitch = 0;

	MountDelta = Destination - Location;

	// Subtract anim movement from delta.
	MountDelta -= vect(30,0,0) >> Rotation;

	if( bFallingMount )
	{
		MountDelta.Z -= 82;
		playAnim('climb96end', [RootBone] 'move');
		PlaySound( sound'HPSounds.HAR_emotes.pull_up3', , 0.5 );
	}
	else if( MountDelta.Z < 48 )
	{
		MountDelta.Z -= 32;
		playAnim('climb32', [Rate] 1.0, [RootBone] 'move');
		PlaySound( sound'HPSounds.HAR_emotes.EmotiveHarry5_b_pullup6', , 0.5 );
	}
	else if( MountDelta.Z < 80 )
	{
		MountDelta.Z -= 64;
		playAnim('climb64', [RootBone] 'move');
		PlaySound( sound'HPSounds.HAR_emotes.EmotiveHarry5_a_pullup5', , 0.5 );
	}
	else
	{
		MountDelta.Z -= 96;
		playAnim('climb96start', [RootBone] 'move');
	}

	gotoState('MountFinish');
}

state MountFinish
{
	ignores Mount, AltFire;

	function ProcessMove(float DeltaTime, vector NewAccel, eDodgeDir DodgeMove, rotator DeltaRot)
	{
		// Ignore acceleration.
		global.ProcessMove( DeltaTime, vect(0,0,0), DodgeMove, DeltaRot );
	}

	event PlayerTick( float DeltaTime )
	{
		local vector v;

		// Add additional movement for actual mount offset.
		v = MountDelta * DeltaTime*AnimRate;
		Move(v);
		ViewRotation = Rotation;
		//todo problem (most likely): global.PlayerTick( DeltaTime );
	}

	function BeginState()
	{
		DebugState();
		// Shrink collision for more lenient movement in world.
		SetCollisionSize( CollisionRadius*0.5, CollisionHeight*0.5, CollisionHeight*0.5 );
		PrePivot.Z -= CollisionHeight;
	}

	function EndState()
	{
		PrePivot.Z += CollisionHeight;
		SetCollisionSize( CollisionRadius*2, CollisionHeight*2, 0 );
	}

begin:

	if( AnimSequence == 'climb96start' )
	{
		// Split delta movement in 2.
		MountDelta *= 0.5;
		finishAnim();
		playAnim('climb96end', [RootBone] 'move');
		PlaySound( sound'HPSounds.HAR_emotes.EmotiveHarry5_a_pullup5', , 0.5 );
	}
	finishAnim();

	// Restore physics.
	SetPhysics(PHYS_Walking);

	// Play idle anim in case nothing's happening.
	CurrIdleAnimName = GetCurrIdleAnimName();
	PlayAnim( CurrIdleAnimName, 1.0, 0.2 );
	gotostate('PlayerWalking');
}

//********************************************************************************************
/*
state hit
{
	ignores DoJump, bump;

	function Landed(vector HitNormal)
	{
		clientMessage("landed: jump dist = " $VSize(Location-MountDelta) $ "   tia="$fTimeInAir);

		Global.Landed(HitNormal);

		PlayLandedSound();

//		playanim('land1');
		velocity *= 0;

//		log("PLOG Hitstate landed");

	}

	function EndState()
	{
//		log("PLOG end Hitstate");
	}

	function animEnd()
	{
		bPressedJump = false;
		gotostate('PlayerWalking');
	}

	begin:
		disable('AnimEnd');
		DebugState();
		bThrow = false;

//		log("PLOG Hitstate");

//		if(rectarget!=none)
//			rectarget.destroy();

		//If we're carrying an item, toss it forward a bit.  At this point, we'll assume it's
		// a firecracker
		if( CarryingActor != none )
			DropCarryingActor();

		//if( AnimSequence != 'knockback2' )
		//	playanim('knockback2',[RootBone] 'move');
		if( AnimSequence != 'knockback' )
			playanim('knockback',[RootBone] 'move');

	
//		sleep(0.5);
	
		enable('AnimEnd');
//		bPressedJump = false;
//		gotostate('PlayerWalking');

		Acceleration *= vect(0,0,1);
}
*/

//********************************************************************************************
state statePickBitOfGoyle
{
	begin:

	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	PlayAnim('PickBitOfGoyle', 1.0, 0.2);
	FinishAnim();

	bFinishPickBitOfGoyle = true;

	gotostate( 'PlayerWalking' );
}

//********************************************************************************************
/*
state hit_InstantPitDeath
{
	begin:
		disable('AnimEnd');
		DebugState();
		bThrow = false;

//		if(rectarget!=none)
//			rectarget.destroy();

		//If we're carrying an item, toss it forward a bit.  At this point, we'll assume it's
		// a firecracker
		if( CarryingActor != none )
			DropCarryingActor();

		bHidden = true;

}
*/
//********************************************************************************************
state ChessDeath
{


	begin:
		DebugState();
		KillHarry(true);
/*		PlaySound( HurtSound[ Rand(NUM_HURT_SOUNDS) ] );
		playanim('knockback2');
		finishanim();
	
//		sleep(1.0);
		goto 'begin';*/
	
}

//********************************************************************************************

function DoCelebrateBronzeCardSet()
{
	ClientMessage("DoCelebrateBronzeCardSet");
	GotoState('CelebrateBronzeCardSet');
}

state CelebrateBronzeCardSet
{
begin:
	// Setup camera to do a quick, rotate around Harry "cutscene"
	cam.SetCameraMode(cam.ECamMode.CM_CutScene);
	cam.CamTarget.SetAttachedTo(self);
	cam.SetSyncPosWithTarget(true);
	cam.SetSyncRotWithTarget(false);
	cam.SetZOffset(25);
	cam.SetPitch(-6000);
	cam.SetDistance(100);
	cam.SetRotStepYaw(-12288);

	// Play Harry celebrate animation and increse his health potential
	PlayAnim('Celebrate', 1.0, 0.2 );
	Sleep(1.5);
	PlaySound(Sound'HPSounds.Magic_sfx.health_boost1');
	GetHealthStatusItem().IncrementCountPotential(StatusItemHealth(GetHealthStatusItem()).nUnitsPerIcon);
	FinishAnim();

	// Return camera to normal and go to walking state.
	cam.SetCameraMode( cam.ECamMode.CM_Transition );
    while (cam.CameraMode == cam.ECamMode.CM_CutScene)
        sleep(0.1);
	GotoState('PlayerWalking');
}

//************************************************************************************************************************
function bump( actor Other )
{
	super.bump( Other );

	PickupActor( Other );
}

//************************************************************************************************************************
function StartAimSoundFX()
{
	// do not play aiming sfx when the spell is Expelliarmus
	if(	bInDuelingMode && (CurrentDuelSpell == 2) )
		return;

	PlaySound(sound'HPSounds.Magic_sfx.spell_aim', SLOT_Misc);
	if(	bInDuelingMode && (CurrentDuelSpell == 1) )
		PlaySound(sound'HPSounds.Magic_sfx.Dueling_MIM_buildup', SLOT_Interact);
	else
		PlaySound(sound'HPSounds.Magic_sfx.spell_loop_nl', SLOT_Interact);
}

//************************************************************************************************************************
function StopAimSoundFX()
{
	// Just in case this hasn't finished yet.
	StopSound(sound'HPSounds.Magic_sfx.spell_dud', SLOT_Misc);
	if(	bInDuelingMode && (CurrentDuelSpell == 1) )
		StopSound(sound'HPSounds.Magic_sfx.Dueling_MIM_buildup', SLOT_Interact);
	else
		StopSound(sound'HPSounds.Magic_sfx.spell_loop_nl', SLOT_Interact);
}

//************************************************************************************************************************
//Overriden version in PlayerWalking has functionality
//function HarryAnimChannel_AnimEnd()
//{
//}

//**************************************************************************************************
//Dont do anything unless you're in state PlayerWalking.  Same for StopAiming.
function StartAiming(bool in_bHarryUsingSword)
{
	//No code in here.  the PlayerWalking version will do it, which makes it only work when you're in PlayerWalking.  How nice.
}

//This function is normally called when harry's cast anim is done, or to completely cancel the cast process.
function StopAiming()
{
	//ClientMessage("StopAiming()");

	//Dont even bother trying to do this if you're carrying something.  You wont be aiming FOR SURE.
	if( CarryingActor == none )
	{
		HarryAnimChannel.GotoState( 'stateIdle' );
		HarryAnimType = AT_Replace;
		TurnOffCastingVars();
		TurnOffSpellCursor();
	}
}

//This is meant to be called when harry is completely done with the cast process, and his arm is back down at his side.
// Or, it can be called from StopAiming which can cancel the cast completely.
function TurnOffCastingVars()
{
	bIsAiming = false;
	bIsAimingWithCharge = false;
}

//This one is called when the projectile is shot, and harry has yet to finish his cast anim.
function TurnOffSpellCursor()
{
	//baseWand(weapon).StopCasting();
	bIsAimingWithCharge = false;
	
	baseWand(weapon).StopChargingSpell();
	SpellCursor.TurnTargetingOff();

	//if( bHarryUsingSword )
		GroundSpeed = GroundRunSpeed;
}

function TurnOnCastingVars(bool in_bHarryUsingSword)
{
	bIsAiming = true;
	bIsAimingWithCharge = true;

	if( bInDuelingMode )
	{
		if(	CurrentDuelSpell != 2 )
			baseWand(weapon).StartChargingSpell( true, in_bHarryUsingSword, DuelSpells[CurrentDuelSpell] );
		else
			baseWand(weapon).StartChargingSpell( false, in_bHarryUsingSword, DuelSpells[CurrentDuelSpell] );	// do not charge expelliarmus
	}
	else
		baseWand(weapon).StartChargingSpell( false, in_bHarryUsingSword );

	//if( bHarryUsingSword )
	//	GroundSpeed = GroundRunSpeed * 0.4;
}

//This sais if you are at all casting.
function bool PlayerIsAiming()
{
	return bIsAiming;
}

//This sais if you are casting at any point before the spell is shot.
function bool PlayerIsAimingWithCharge()
{
	return bIsAimingWithCharge;
}

//*************************************************************************************************
//Do this later in Pawn.cpp or pawn.uc
// Or at least from APawn::Tick, do a if bAutoFootSteps:ProcessFootSteps(dtime);
function PlayerTick(float dtime)
{
//log("DState = " $DuelOpponent.GetStateName() $" AState = " $Duellist(DuelOpponent).DuellistAnimChannel.GetStateName() $" DAnim = " $DuelOpponent.AnimSequence $" AAnim = " $Duellist(DuelOpponent).DuellistAnimChannel.AnimSequence $" AFrame = " $DuelOpponent.AnimFrame $" Z = " $Duellist(DuelOpponent).location.Z);

	if(	fTimeAfterShield > 0)
		fTimeAfterShield -= dtime;

	// in dueling mode reset GlowingWand after shield is about to die
	if(	bInDuelingMode && (fTimeAfterShield <= 0) && baseWand(weapon).fxChargeParticles.IsA('Exep_Shield')) 
	 	baseWand(weapon).StartGlowingWand( DuelSpells[ CurrentDuelSpell ] );

	if(	fTimeAfterHit > 0)
		fTimeAfterHit -= dtime;

	if( CurrentAnimHasFootStepSounds() )
	{
		//Note, this only supports AnimRate > 0
		//Special case for run, cause it has two cycles in it
		if( AnimSequence != 'run' )
		{
			if(   AnimFrame >= 0.5 && LastAnimFrame < 0.5
		       //|| AnimFrame >= 0.75 && LastAnimFrame < 0.75
			   || AnimFrame < LastAnimFrame //   &&    AnimFrame < 0.25 && LastAnimFrame < 0.75 // <=- the wrap case
		      )
				PlayFootStep();
		}
		else
		{
			if(   AnimFrame < LastAnimFrame
			   || AnimFrame >= 0.25 && LastAnimFrame < 0.25
			   || AnimFrame >= 0.5  && LastAnimFrame < 0.5
			   || AnimFrame >= 0.75 && LastAnimFrame < 0.75
			  )
				PlayFootStep();
		}
	}

	LastAnimFrame = AnimFrame;


	// Remember pre-shake.
	ViewRotation = baseCam(ViewTarget).rCurrRotation;
	ViewShake( dtime );
	if( ViewTarget != none )
	{
		// Apply the view shake delta to our camera actor.
		//newRotation = ViewTarget.Rotation + ViewRotation - newRotation;
		//newRotation.Roll = newRotation.Roll & 0xffff;
		//ViewTarget.SetRotation(newRotation);
		baseCam(ViewTarget).rExtraRotation = ViewRotation - baseCam(ViewTarget).rCurrRotation;
	}

	if( !bDisplayedFirstErrorMessages )
	{
		bDisplayedFirstErrorMessages = true;
		DisplayFirstErrorMessages();
	}
}

//************************************************************************************************************************
function DisplayFirstErrorMessages()
{
	local actor a, a2;

	if( hpconsole(player.console).bDebugMode )
		ForEach AllActors(class'actor', a)
			if( a.CutName != ""  &&  ( a.IsA('CutScene')  ||  a.IsA('CutCameraPos')  ||  a.IsA('CutMark') ) )
				ForEach AllActors(class'actor', a2)
					if( ( a2.IsA('CutScene')  ||  a2.IsA('CutCameraPos')  ||  a2.IsA('CutMark') ) && a2 != a  &&  a2.CutName ~= a.CutName )
						ClientMessage("**** ERROR: Actor:"$a2.name$" has same CutName as "$a.name$".  CutName:"$a.CutName);
}

//************************************************************************************************************************
function bool CurrentAnimHasFootStepSounds()
{
	switch( AnimSequence )
	{
		case 'run':
		case 'runback':
		case 'StrafeLeft':
		case 'StrafeRight':
		case 'walk':
		case 'ectowalk':
		case 'ectowalkback':
		case 'ectostraferight':
		case 'ectostrafeleft':

		case 'SwordRun':
		case 'SwordRunBack':
		case 'SwordStrafeRight':
		case 'SwordStrafeLeft':
			return true;
	}

	return false;
}

//************************************************************************************************************************
function bool PlayingIdleAnimation(name animseqname)
{
	local string animName;
	local int	i;
	local name	nm;

	for ( i = 1; i <= 16; i++)
	{
		animName = "idle_" $i;
		nm = StringToAnimName(animName);
		if(nm == animseqname)
			return true;
	}

	return false;
}

//************************************************************************************************************************
function bool PlayingFidgetAnimation(name animseqname)
{
	local string animName;
	local int	i;
	local name	nm;

	for ( i = 1; i <= 16; i++)
	{
		animName = "fidget_" $i;
		nm = StringToAnimName(animName);
		if(nm == animseqname)
			return true;
	}

	return false;
}

//************************************************************************************************************************
function name MyGetAnimGroup(name animseqname)
{
	if( PlayingIdleAnimation(animseqname) )
		return 'Waiting';

	if( PlayingFidgetAnimation(animseqname) )
		return 'Waiting';

	if ( AnimSequence == 'look_frantic' )
		return 'Waiting';

	return 'none';
}

//************************************************************************************************************************

function SetNewMesh()
{
	if(bIsGoyle && (mesh == SkeletalMesh'HPModels.skHarryMesh'))
	{
		mesh = SkeletalMesh'HPModels.skGoyleMesh';
		DrawScale = 1.15;
	}

	if(!bIsGoyle && (mesh == SkeletalMesh'HPModels.skGoyleMesh'))
	{
		mesh = SkeletalMesh'HPModels.skHarryMesh';
		DrawScale = 1.0;
	}
}

//************************************************************************************************************************

function SpawnParticles(class<ParticleFX> Particles)
{
	Spawn(Particles, , , location, rot(0,0,0));
}

//************************************************************************************************************************
function CreateSpongifyEffects()
{
	local int   i;

return;

	if( SpongifyFX[0] == none )
		{ SpongifyFX[0] = spawn( class'SpellVoldTrackingFX', self );       SpongifyFX[0].AttachToOwner( 'Bip01 R Foot' );
	if( SpongifyFX[1] == none )
		{ SpongifyFX[1] = spawn( class'SpellVoldTrackingFX', self );       SpongifyFX[1].AttachToOwner( 'Bip01 L Foot' ); }}

	for( i = 0; i < NUM_SPONGIFY_FX; i++ )
		SpongifyFX[i].bEmit = true;
}

//** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** ** 
function StopSpongifyEffects()
{
	local int   i;

	for( i = 0; i < NUM_SPONGIFY_FX; i++ )
		SpongifyFX[i].bEmit = false;
}

//************************************************************************************************************************
state PlayerWalking
{
ignores SeePlayer, HearNoise;//, Bump;

	// Director routing of events...
	// The director is informed of any touch, bump, or hit events in case they
	// could change the state of the mini-game or puzzle Harry is playing right
	// now.  Harry then performs his normal reaction to such events (if the
	// director hasn't intervened somehow).

	event Touch( Actor Other )
	{
		// Let the director (if any) know when Harry touches things
		if ( Director != None )
			Director.OnTouchEvent( Self, Other );

		Global.Touch( Other );
	}

	event UnTouch( Actor Other )
	{
		// Let the director (if any) know when Harry stops touching things
		if ( Director != None )
			Director.OnUnTouchEvent( Self, Other );

		Global.UnTouch( Other );
	}

	event Bump( Actor Other )
	{
		// Let the director (if any) know when Harry bumps things
		if ( Director != None )
			Director.OnBumpEvent( Self, Other );

		Global.Bump( Other );
	}

	event HitWall( vector HitNormal, Actor Wall )
	{
		// Let the director (if any) know when Harry hits things
		if ( Director != None )
			Director.OnHitEvent( Self );

		Global.HitWall( HitNormal, Wall );
	}

	
	function ZoneChange( ZoneInfo NewZone )
	{
		if (NewZone.bWaterZone)
		{
			setPhysics(PHYS_Swimming);
			GotoState('PlayerSwimming');
		}
	}

	function AnimEnd()
	{
		local name MyAnimGroup;

		bAnimTransition = false;

		bJustFired = false;
		bJustAltFired= false;

		if (Physics == PHYS_Walking)
		{
			MyAnimGroup = MyGetAnimGroup(AnimSequence);

			if ((Velocity.X * Velocity.X + Velocity.Y * Velocity.Y) < 1000)
			{
				if ( MyAnimGroup == 'Waiting' )
					PlayWaiting();
				else
				{
					bAnimTransition = true;
					TweenToWaiting(0.4);
				}
			}	
			else if (bIsWalking)
			{
				if ( (MyAnimGroup == 'Waiting') || (MyAnimGroup == 'Landing')   )
				{
					TweenToWalking(0.4);
					bAnimTransition = true;
				}
				else 
					PlayWalking();
			}
			else
			{
				if ( (MyAnimGroup == 'Waiting') || (MyAnimGroup == 'Landing')  )
				{
					bAnimTransition = true;
					TweenToRunning(0.4);
				}
				else
				{
					//ClientMessage("PlayerWalking:AnimEnd:PlayRunning");
					PlayRunning();
				}
			}
		}
		else
		{
			PlayInAir();
		}
	}

	//* * * * * * * * * * * * * * * * * * * * 
	//function HarryAnimChannel_AnimEnd()
	//{
	//	ClientMessage("HarryAnimChannel_AnimEnd().  AnimName="$HarryAnimChannel.AnimSequence);
	//
	//	if( bIsAiming  &&  HarryAnimChannel.AnimSequence == 'cast' )
	//		StopAiming();
	//}

	//* * * * * * * * * * * * * * * * * * * * 
	function StartAiming(bool in_bHarryUsingSword)
	{
		if( !bIsAiming  &&  CarryingActor == none )
		{
			TurnOnCastingVars( in_bHarryUsingSword );

			//rectarget.destroy();
			bJustFired = false;
			bJustAltFired =  false;
			hpconsole(player.console).bspaceReleased=false;
			hpconsole(player.console).bSpacePressed = false;
			
			if( in_bHarryUsingSword )
			{
				//need other sfx
				PlaySound(sound'HPSounds.Magic_sfx.sword_buildup', SLOT_Interact);
			}
			else
			{
				StartAimSoundFX();
				makeTarget();
			}

			//loopanim('wave', , 0.2); //Use the interp as the trans2wave
			//HarryAnimChannel.PlayAnim( 'wave', , 0.2 );
			HarryAnimChannel.GotoStateCasting( in_bHarryUsingSword );
			HarryAnimType = AT_Combine;
		}
	}

	//* * * * * * * * * * * * * * * * * * * * 
	function Landed(vector HitNormal)
	{
		local float fFallDistanceZ;
		local int   i;

		clientMessage("landed: jump dist = " $VSize(Location-MountDelta) $ "   tia="$fTimeInAir);
		
		Global.Landed(HitNormal);

		PlayLandedSound();

		playanim(HarryAnims[HarryAnimSet].land, [TweenTime]0.1, [Type] HarryAnimType);

		//log("PLOG PWalking landed");

		// Set our spell distance to the default if we landed from a spongify jump
		if( !bExtendedTargetting && AnimFalling == SpongifyFallAnim )
			SpellCursor.SetLOSDistance( 0 );
		
		// See if we laneded on spongify!
		for(i=0; i<ArrayCount(Touching); i++)
		{
			if( Touching[i].IsA('SpongifyPad') &&
				SpongifyPad(Touching[i]).IsEnabled() )
			{
				// We landed on a spongifyPad an
				HitSpongifyPad = SpongifyPad(Touching[i]);
				
				// Set our spell distance longer so we can easily target the next spongify pad
				// The spell distance will revert back to normal once we land from a spongify pad
				if( !bExtendedTargetting )
					SpellCursor.SetLOSDistance( 1024 );
			}
		}
		
		//if( AnimFalling == SpongifyFallAnim  &&  HitSpongifyPad == none )
			StopSpongifyEffects();

		// We didn't land on a spongify pad and we are not falling from a spongify pad bounce
		if( AnimFalling != SpongifyFallAnim && HitSpongifyPad == None )
		{
			// we are doing a regular fall animation
			ClientMessage("Z Fall Distance = " $(fHighestZ-location.z) $" TimeInAir = " $fTimeInAir
							$"ZHighest = " $fHighestZ $"ZLoc = " $location.z );
			
			fFallDistanceZ = (fHighestZ-location.z);

			// if we fell for a long distance then hurt harry
			if( fFallDistanceZ > FALL_DAMAGE_DISTANCE )
			{
				// The farther you fall the more damage you get
				if( fFallDistanceZ < FALL_DAMAGE_DISTANCE + 32 )	// 512 - 544
					TakeDamage(20, self, location,vec(0,0,0), 'Falling' );
				else
				if( fFallDistanceZ < FALL_DAMAGE_DISTANCE + 64 )	// 
					TakeDamage(30, self, location,vec(0,0,0), 'Falling' );
				else
				if( fFallDistanceZ < FALL_DAMAGE_DISTANCE + 256 )	// 
					TakeDamage(50, self, location,vec(0,0,0), 'Falling' );
				else
				if( fFallDistanceZ < FALL_DAMAGE_DISTANCE + 512 )	// 
					TakeDamage(100, self, location,vec(0,0,0), 'Falling' );
				else
				if( fFallDistanceZ < FALL_DAMAGE_DISTANCE + 1024 )	// 
					TakeDamage(200, self, location,vec(0,0,0), 'Falling' );
				else
				{													// 768 - 
					TakeDamage(99999, self, location,vec(0,0,0), 'Falling' );
				}
			}
		}
		
		// Reset our falling animation
		AnimFalling = HarryAnims[HarryAnimSet].fall;

		// Reset our highestZ position
		fHighestZ = default.fHighestZ;
	}
	
	//* * * * * * * * * * * * * * * * * * * * 
	event PlayerTick( float DeltaTime )
	{
		local actor a;
		local float d;
		local actor ca;

		Global.PlayerTick( DeltaTime );

		//if(	GetHealthCount() < 5 )
		//	DoDrinkWiggenwell();

		//		d = 1000000;
		//		ForEach AllActors(class'actor', a)
		//		{
		//			if( a == self )
		//				continue;
		//			//if( a.IsA('basewand') )
		//			//	continue;
		//			if( VSize( a.location - Location ) < 500 )
		//			{
		//				Log("*****:"$a$" a.h:"$a.bHidden$" dt:"$a.DrawType);
		//				d = VSize( a.location - Location );
		//				ca = a;
		//			}
		//		}
		//		ClientMessage("ca:"$ca);

		if( bTempKillHarry )// ||  lifePotions <= 0 )
		{
			bTempKillHarry = false;
			KillHarry(true);
		}

		//Weird problem, not sure what's causing it, but sometimes when you touch a painzone, but start your climb
		// you'll end up with no health, but not in the dying state.  This "safely" takes care of that.
		if( GetHealthCount() <= 0 )
		{
			KillHarry(true);
			return;
		}

		//if ( bUpdatePosition )   Might not be able to just remove this.
		//	ClientUpdatePosition();

		if( bIsAiming && HarryAnimChannel.IsInState('stateCasting') && bAltFire == 0 )
		{
			//HarryAnimChannel.

			//if( HarryAnimChannel.AnimSequence == 'castaim' )
			//{
				ClientMessage("LoopAim done");
				//PlaySound(sound'HPSounds.Magic_sfx.spell_loop_nl', [Volume]0);
				StopAimSoundFX();
				
				if( bInDuelingMode )
				{
					if(DuelSpells[CurrentDuelSpell] == class'spellDuelExpelliarmus')
					{
					 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_EXP_swoosh' );
						HarryAnimChannel.GotoState( 'stateDefenceCast' );
					}
					else
						HarryAnimChannel.GotoState( 'stateDuelingCast' );
				}
				else
				if(   SpellCursor.IsLockedOn() //If harry's locked on, he's in normal aim mode, so cast
				   || bHarryUsingSword && baseWand(weapon).SwordChargedUpEnough() //if using sword, and sword is charged up enough
				   || !baseWand(weapon).bAutoSelectSpell // if you're not in autoselect spell mode
				  )
				{
					HarryAnimChannel.GotoState( 'stateCast' );//PlayAnim('cast', 2.0, 0.1);
					if( bCastFastSpells )  //Old, may not be needed in HP2
					{
						AnimFrame = 0.09;
						AnimRate = 3;
					}
				}
				else
				{
					// We don't have a lock so lets stop casting
					HarryAnimChannel.GotoState( 'stateCancelCasting' );

					// Stop Aiming
					StopAiming();					
				}
			//}
		}

		//Try and save how long you've been falling, and what you're original height was when you started falling
		ProcessFalling( DeltaTime );

		PlayerMove(DeltaTime);

		if( CarryingActor != none )
		{
			//r = weaponRot;
			//v = vect(0,0,1);
			//v = v >> r;
			CarryingActor.setLocation( weaponLoc );//- vect(0,0,1 );
			CarryingActor.SetRotation( weaponRot );

			//Also, look for a spacebar throw
			if( hpconsole(player.console).bSpacePressed )
			{
				hpconsole(player.console).bSpacePressed = false;
				AltFire(0);
			}
		}
		
		// If we landed on a spongify pad then bounce harry
		if( HitSpongifyPad != None && HitSpongifyPad.IsEnabled() )
		{
			DoJump(0);
			HitSpongifyPad.OnBounce( self );
			AnimFalling = SpongifyFallAnim;
			PlayinAir();
			cam.SetPitch(-8000);
			HitSpongifyPad = None;
			CreateSpongifyEffects();
		}
		
		// HP2 cam
		if( cam.IsInState('StateStandardCam') )//|| cam.IsInState('StateBossCam') )
		{
			// Force our desired Yaw to what the camera's yaw is, in this way harry will
			// always "lookAt" what the camera is looking at.
			DesiredRotation.Yaw = cam.rotation.Yaw & 0xFFFF;

			//SetRotation( DesiredRotation );
		}
	}
	
	//Try and save how long you've been falling, and what you're original height was when you started falling
	//When you land, it uses this info to set the sound volume.
	function ProcessFalling( float DeltaTime )
	{
		local float fLastTimeInAir;
		
		if( Physics == PHYS_Falling )
		{
			if( eLastPhysState != PHYS_Falling )
			{
				fFallingZ = Location.z;
				fHighestZ = location.z;
			}
			else // Save the highest z location for falling damage
			if( fHighestZ < location.z )
			{
				fHighestZ = location.z;
			}
			
			fLastTimeInAir = fTimeInAir;
			fTimeInAir += DeltaTime;

			if( !bPlayedFallSound  &&  fTimeInAir > 1.2 )
			{
				bPlayedFallSound = true;

				if( AnimFalling != SpongifyFallAnim )
					PlaySound( sound'HPSounds.HAR_emotes.falldeep2' );
			}

			if( fLastTimeInAir <= 0.35   &&   fTimeInAir > 0.35 )
				PlayInAir();
		}
		else
		{
			bPlayedFallSound = false;
			fTimeInAir = 0;
		}

		eLastPhysState = Physics;
	}

	function PlayerMove( float DeltaTime )
	{
		local vector X,Y,Z, NewAccel;
		local EDodgeDir OldDodge;
		local eDodgeDir DodgeMove;
		local rotator OldRotation;
		local rotator CamRot;
		local float Speed2D;
		local bool	bSaveJump;
		local name AnimGroupName;

		//if( PotCam(ViewTarget) != none )
		//	GetAxes(ViewTarget.Rotation,X,Y,Z);
		//else
		//ClientMessage("*1 aStrafe:"$aStrafe$" aTurn:"$aTurn);
		if( bReverseInput )  //Right now, just for troll chase.
		{
			aForward = abs(aForward * 2);
			aTurn = -aTurn;
			aStrafe = -aStrafe;
		}

		aForward  *= 0.08;

		if( Physics == PHYS_Falling  ||  bLockedOnTarget  ||  bFixedFaceDirection )
		{
			aStrafe   *= 0.08;
			aTurn = 0;
		}
		else
		{
			//aStrafe = 0;  //you now use strafe
			aStrafe   *= 0.08;
			aTurn    *= 0.24;
		}

		aLookup   *= 0;			// make harry steady (no pitching with look up)
		aSideMove *= 0.1;

		if( Adv1TutManager != none )
		{
			if( aForward > 0 )
				Adv1TutManager.ForwardPushed();
			if( aForward < 0 )
				Adv1TutManager.BackwardPushed();
			if( aStrafe  < 0 )
				Adv1TutManager.StrafeLeftPushed();
			if( aStrafe  > 0 )
				Adv1TutManager.StrafeRightPushed();
		}

		if( bKeepStationary )
		{
			aForward = 0;
			aStrafe = 0;
		}

		//Or, look for harry tutorial stuff
		if( bLockOutForward && aForward   > 0  ||  bLockOutBackward && aForward   < 0 )
			aForward = 0;
		if( bLockOutStrafeLeft && aStrafe < 0  ||  bLockOutStrafeRight && aStrafe > 0 )
			aStrafe = 0;

		//ClientMessage("aTurn:"$aTurn$" aStrafe:"$aStrafe$" aForward:"$aForward);
		// Update acceleration.
		if( bLockedOnTarget  ||  bFixedFaceDirection )
		{
			//if( aForward < 0 )
			//	aForward *= 2;

			//ClientMessage("aForward:" @ aForward @ "   aStrafe:" @ aStrafe);
			//NewAccel = aForward*X + aStrafe*Y;
			//ProcessAccel messes with aForward and aStrafe to get a final NewAccel
			//ClientMessage("*2 aStrafe:"$aStrafe$" aTurn:"$aTurn);
			NewAccel = ProcessAccel();
		}
		else
		{
			GetAxes(Rotation,X,Y,Z);

			// Update acceleration.
			if( bScreenRelativeMovement )
			{
				GetAxes(cam.Rotation,X,Y,Z);
				//NewAccel = vect(1,0,0)*aForward + vect(0,1,0)*aSideMove; 
				NewAccel = aForward*X + aSideMove*Y; 

				//If there's no acceleration, do nothing, otherwise harry snaps back to X
				if( NewAccel != vect(0,0,0) )
				{
					CamRot = cam.Rotation;
					CamRot.Pitch = 0;
					//NewAccel = NewAccel << CamRot;
					ScreenRelativeMovementYaw = (Rotator(NewAccel)).Yaw;
					//ClientMessage( " " @ ScreenRelativeMovementYaw );
				}
			}
			else
			{
				NewAccel = aForward*X + aStrafe*Y; 
				if(	bInDuelingMode )
					NewAccel *= 1000000;	// we want move Harry with his max velocity
			}
		}

		//If harry's using the sword, get the speed/charge modifier from the baseWand, and set our ground speed
		if( bHarryUsingSword )
			GroundSpeed = GroundRunSpeed * (1.0 - 0.9*baseWand(Weapon).ChargingLevel());

		//If you're moving, and your not in aiming mode, 
		// let the camera know it should try and vertically go back to center.
		if( aForward != 0  &&  !bIsAiming && !SpellCursor.IsLockedOn() )
			bHarryMovingNotAiming = true;
		else
			bHarryMovingNotAiming = false;

		NewAccel.Z = 0;
		// Check for Dodge move
		
		AnimGroupName = GetAnimGroup(AnimSequence);		

		// Update rotation.
		OldRotation = Rotation;

		//When you're locked onto a target, or facing down a fixed direction, other rotation code is performed
		//if( !(bLockedOnTarget || bFixedFaceDirection) ) // &&  target != none )
		//	UpdateRotation(DeltaTime, 1);

		ProcessMove(DeltaTime, NewAccel, DodgeMove, OldRotation - Rotation);
		//ClientMessage("Accel:"$Acceleration$" Vel:"$Velocity);
		


		// HP2 cam
		if( cam.IsInState('StateStandardCam') )//|| cam.IsInState('StateBossCam') )
		{
			// Force our desired Yaw to what the camera's yaw is, in this way harry will
			// always "lookAt" what the camera is looking at.
			DesiredRotation.Yaw = cam.rotation.Yaw & 0xFFFF;
			//SetRotation( DesiredRotation );
		
			if( bHarryMovingNotAiming && bAutoCenterCamera && !bInDuelingMode )
			{
				// If we are falling from a spongify then don't AutoCenterCamera
				if( AnimFalling != SpongifyFallAnim )
					cam.SetPitch(-1500);
			}		
		}
	}

	function ProcessMove(float DeltaTime, vector NewAccel, eDodgeDir DodgeMove, rotator DeltaRot)	
	{
		local vector OldAccel;
		local float  Speed;

		OldAccel = Acceleration;
		Acceleration = NewAccel;
		bIsTurning = ( Abs(DeltaRot.Yaw/DeltaTime) > 5000 );

		if(bJustAltFired || bJustFired)
		{
			Velocity = vect(0,0,0);
			return;
		}

		if ( bPressedJump )
		{
//			ClientMessage("Jump pressed");
			DoJump();			// jumping
			bPressedJump = false;
		}

		if ( (Physics == PHYS_Walking)  )
		{
			Speed = VSize2d( Velocity );

			if(   (!bAnimTransition || (AnimFrame > 0))
			   && !( AnimSequence == HarryAnims[HarryAnimSet].Land && (Speed < 5 || VSize2D(acceleration)==0) )  //You need to NOT be (landing and not-moving)    //(GetAnimGroup(AnimSequence) != 'Landing') )
			  )
			{
				//ClientMessage("AnimSequence:"$AnimSequence$" AnimGroup:"$GetAnimGroup(AnimSequence)$" Speed:"$Speed);

				if( Speed > 5 )
					fTimeWalking += DeltaTime;
				else
					fTimeWalking = 0;

				if(   Acceleration != vect(0,0,0)
				   && Speed > 1 //you need a little bit of motion 
				   //&& (    bMovingBackwards   && Speed > 30
				   //    || !bMovingBackwards   && Speed > 65
				   //    ||  fTimeWalking > 0.5 && Speed > 15
				   //   )
				  )
				{
						bAnimTransition = true;
						TweenToRunning(0.4);
				}
			 	else
			 	{
						bAnimTransition = true;
						TweenToWaiting(0.4);
				}
			}
		}
	}

	function BeginState()
	{
		DebugState();

		//log("PLOG PWalking Entered");
		if ( Mesh == None )
			SetMesh();
		WalkBob = vect(0,0,0);
		DodgeDir = DODGE_None;
		bIsCrouching = false;
		bIsTurning = false;
		bPressedJump = false;
		if (Physics != PHYS_Falling)
			SetPhysics(PHYS_Walking);
		if ( !IsAnimating() )
			PlayWaiting();

		foreach allActors(class'BaseCam', cam)
			break;
	}
	
	function EndState()
	{
		//log("PLOG PWalking Exited");
		WalkBob = vect(0,0,0);
		bIsCrouching = false;
		StopAiming();

		Acceleration = vect(0,0,0);
		Velocity =     vect(0,0,0);
		CurrIdleAnimName = GetCurrIdleAnimName();
		LoopAnim( CurrIdleAnimName, [TweenTime]0.4, [Type] HarryAnimType);
	}
}


//**********************************************************************************************
//This is called from 'PlayerWalking' and 'PlayerAim'
//function UpdateRotation(float DeltaTime, float maxPitch)
//{
//	local rotator newRotation;
//	local float   YawVal;
//	local int     RotDist;
//	local float   FastRotRate;
//
//	if( Physics == PHYS_Falling )
//		return;
//
//	FastRotRate = 70000;
//
//	DesiredRotation = ViewRotation;
//	ViewRotation.Pitch += 32.0 * DeltaTime * aLookup;
//	ViewRotation.Pitch = ViewRotation.Pitch & 65535;
//
//	if ((ViewRotation.Pitch > 18000) && (ViewRotation.Pitch < 49152))
//	{
//		if (aLookup > 0) 
//			ViewRotation.Pitch = 18000;
//		else
//			ViewRotation.Pitch = 49152;
//	}
//
//	if(aTurn>turnRate)
//		aTurn=turnRate;
//	else
//	if(aTurn<-turnRate)
//		aTurn=-turnRate;
//	
//	//Special rot for when you're in state 'PlayerWalking' and bScreenRelativeMovement
//	if( IsInState('PlayerWalking') && bScreenRelativeMovement )
//	{
//		//Need to zip on round to ScreenRelativeMovementYaw
//		// ( or just snap to the direction )
//		RotDist = (ScreenRelativeMovementYaw - ViewRotation.yaw)%65536;
//		
//		//force the turn
//		//YawVal = RotDist;
//
//		if( RotDist < 32768 )
//		{
//			YawVal = FastRotRate * DeltaTime;
//			if( YawVal > RotDist )
//				YawVal = RotDist;
//		}
//		else
//		{
//			RotDist = 65536 - RotDist;
//			YawVal = FastRotRate * DeltaTime;
//			if( YawVal > RotDist )
//				YawVal = RotDist;
//			YawVal = -YawVal;
//		}
//	}
//	else
//	{
//		//When you're in state 'PlayerAim', and you're in "circle around the boss" (bLockedOnTarget) mode, this function gets called,
//		// and then overridden, with an absolute rot set towards the Boss.
//
//		if(Acceleration == vect(0,0,0))
//			YawVal=32.0 * DeltaTime * aTurn;
//		else
//			YawVal=24.0 * DeltaTime * aTurn;
//	}
//
//	//If bConstrainYaw is set and you're not turning, this tries to turn you back towards the x axis
//	if( bConstrainYaw  &&  yawVal == 0 )
//	{
//		ViewRotation.Yaw = ViewRotation.Yaw & 65535;
//
//		RotDist = 5500.0*deltaTime;
//		
//		if( ViewRotation.Yaw < 32767 )
//			yawVal = -min(  RotDist,         ViewRotation.Yaw );
//		else
//			yawVal =  min(  RotDist, 65536 - ViewRotation.Yaw );
//	}
//
//	ViewRotation.Yaw += yawVal;
//
//	//This is specially for keeping harry pointing down a specified yaw, with a certain amount of variance.  For now, it's hard coded down the x axis.
//	if( bConstrainYaw )
//	{
//		ViewRotation.Yaw = ViewRotation.Yaw & 65535;
//
//		if( ViewRotation.Yaw <  32767  &&  ViewRotation.Yaw > ConstrainYawVariance )
//			ViewRotation.Yaw = ConstrainYawVariance;
//		else
//		if( ViewRotation.Yaw >= 32767  &&  ViewRotation.Yaw < 65536 - ConstrainYawVariance )
//			ViewRotation.Yaw = 65536 - ConstrainYawVariance;
//	}
//
//	// Remember pre-shake.
//	//newRotation = ViewRotation;
//	//ViewShake(deltaTime);
//	//ClientMessage("Shake Diff:"$newRotation.yaw-ViewRotation.yaw$" "$newRotation.pitch-ViewRotation.pitch);
//	//if( ViewTarget != none )
//	//{
//	//	// Apply the view shake delta to our camera actor.
//	//	//newRotation = ViewTarget.Rotation + ViewRotation - newRotation;
//	//	//newRotation.Roll = newRotation.Roll & 0xffff;
//	//	//ViewTarget.SetRotation(newRotation);
//	//	baseCam(ViewTarget).rExtraRotation = ViewRotation - newRotation;
//	//}
//
//	newRotation = Rotation;
//	newRotation.Yaw = ViewRotation.Yaw;
//	//newRotation.Pitch = ViewRotation.Pitch;
//
//	//if ( (newRotation.Pitch > maxPitch * RotationRate.Pitch) && (newRotation.Pitch < 65536 - maxPitch * RotationRate.Pitch) )
//	//{
//	//	if (ViewRotation.Pitch < 32768) 
//	//		newRotation.Pitch = maxPitch * RotationRate.Pitch;
//	//	else
//	//		newRotation.Pitch = 65536 - maxPitch * RotationRate.Pitch;
//	//}
//	setRotation(newRotation);
//}

//**********************************************************************************************
function UpdateRotationToTarget()
{
	local rotator  r;
	local vector	v;

	local vector	TargetLoc;

	if( bFixedFaceDirection )
	{
		//ViewRotation.Yaw = ViewRotation.Yaw & 65535;
		//RotDist = 5500.0*deltaTime;
		//if( ViewRotation.Yaw < 32767 )
		//	yawVal = -min(  RotDist,         ViewRotation.Yaw );
		//else
		//	yawVal =  min(  RotDist, 65536 - ViewRotation.Yaw );
		//ViewRotation.Yaw += yawVal;
		r = Rotator( vFixedFaceDirection );
		r.pitch = 0;
		r.roll = 0;
		SetRotation( r );
		ViewRotation = r;
	}
	else
	if( BossTarget != none )
	{
		if( baseBoss(BossTarget) != none )
			TargetLoc = baseBoss(BossTarget).GetHarryFaceLocation();
		else
			TargetLoc = BossTarget.Location;

		/*
		if(   (StandardTarget.Location != TargetLoc && aStrafe != 0)
		   || (BossRailMove(BossTarget) != none)
		  )
		{
			v = Normal(Location - TargetLoc) * 10;
			v = StandardTarget.Location - v;

			if (VSize(v) < 10)
				v = TargetLoc;

			StandardTarget.SetLocation(v);
		}
		*/
		r = rotator(TargetLoc - Location);//StandardTarget.Location - Location);
		r.pitch = Rotation.pitch;
		//In Eli's new camera, the cam takes care of this...
		//SetRotation( r );
		ViewRotation = r;
		DesiredRotation = r;
	}
	//else
	//{
	////		TargetLoc = rectarget.location;
	//
	//	r = rotator(StandardTarget.Location - Location);
	//	r.pitch = Rotation.pitch;
	//	SetRotation( r );
	//	ViewRotation = r;
	//}
}

//**********************************************************************************************
function vector ProcessAccel()
{
	local float d;
	local vector v;
	local vector X, Y, Z;
	local vector x2, y2;
	local float xMag, yMag;
	local BossRailMove  b;
	local vector n;//, side;

	//Safety hack:  Keep track of the highest aForward we've ever seen
	if( aForward > fLargestAForward )
		fLargestAForward = aForward;

	//Point harry at our foe
	UpdateRotationToTarget();

	if( baseBoss(BossTarget) != none )
		GetAxes( rotator(baseBoss(BossTarget).GetHarryMovementCenter() - Location), X, Y, Z);
	else
		GetAxes(Rotation,X,Y,Z);

	//Boss Fighting: case: Boss on rail, both inside a 'rect'
	// If you're locked on a target, and it's a boss who you're circling around
	if(   bLockedOnTarget
	   && BossRailMove(BossTarget) != none
	  )
	{
		b = BossRailMove(BossTarget);

		//Push away from our foe as we move
		//xMag = aForward - (fLargestAForward*0.40);
		xMag = aForward;
		yMag = aStrafe;

		//If you're strafeing, push away
		xMag -= abs(aStrafe) * 0.25;

		v = xMag*X + yMag*Y;

		//Look at the top of BossRailMove for what these are
		//left
		v = KeepPawnInsidePlane(v, b.v1, -b.n2);
		//top
		v = KeepPawnInsidePlane(v, b.v1,  b.n1);
		//right
		v = KeepPawnInsidePlane(v, b.v2,  b.n2);
		//bottom
		v = KeepPawnInsidePlane(v, b.v4, -b.n1);

		//Keep accel consistent
		if( (v.x!=0 || v.y!=0) && fLargestAForward != 0 )
			v = Normal( v ) * fLargestAForward;
	}
	else
	{
		v = aForward*X + aStrafe*Y;
	}

	v += vAdditionalAccel;
	vAdditionalAccel = vect(0,0,0);

	return v;
}

//**********************************************************************************************
//Takes your current vAccel, and modifies it to keep you on the back side of the supplied plane.
// Returns a new accel that will keep you there.
function vector KeepPawnInsidePlane(vector vAccel, vector vPlanePoint, vector vPlaneNormal)
{
	local float d;
	local vector x2, y2;
	local float xMag, yMag;

	//vPlaneNormal = -b.n2;

	d = (Location - vPlanePoint) dot vPlaneNormal;

	if( d > 0 )
	{
		//Get forward and side vectors
		x2 = -vPlaneNormal;
		y2 = vect(0,0,1)  cross  x2;

		//Get accel vector componants relative to the box
		xMag = (vAccel  dot  x2);
		yMag = (vAccel  dot  y2);
		
		//Scale x vector by how much you're out of the box, if you're moving away from the box
		if( xMag < 0 )
			xMag = d * 10;  //10, arbitrary scaler to make you move more quickly inwards, the further you are away.
	
		vAccel = x2*xMag + y2*yMag;
	}

	return vAccel;
}

//**********************************************************************************************
state GameEnded
{
	ignores SeePlayer, HearNoise, KilledBy, Bump, HitWall, HeadZoneChange, FootZoneChange, ZoneChange, Falling, TakeDamage, PainTimer, Died;
}

function rotator AdjustAim(float projSpeed, vector projStart, int aimerror, bool bLeadTarget, bool bWarnTarget)
{
	local vector FireDir;
	local actor BestTarget;
	local actor HitActor;
	local rotator defaultAngle,checkAngle;
	local pawn hitPawn;
	local vector objectDir;
	local int bestYaw;
	local int tempYaw, defaultYaw;
	local float bestZ;

	defaultAngle = Rotation;
	defaultAngle.pitch=0;
	defaultYaw = defaultAngle.yaw;
	defaultYaw = defaultYaw & 0xffff;

	fireDir = vector(defaultAngle);
	fireDir = normal(fireDir);
	bestTarget = none;

//	if (rectarget.victim == none)
//	{
		foreach VisibleActors( class 'ACTOR', hitactor)
		{
			if( HitActor.bprojtarget && PlayerPawn(HitActor) != Self && !HitActor.IsA('BaseCam'))
			{
			
				objectdir=normal(hitactor.location-projstart);
				//fireDir.z=objectdir.z;
				checkAngle=rotator(objectdir);

				if(bestTarget==none)
				{
					bestYaw = checkAngle.yaw;
					bestYaw = bestYaw & 0xffff;
					bestTarget = hitactor;
					bestZ = objectdir.z;
				}
				else
				{	
					tempYaw = checkangle.yaw;
					tempYaw = tempYaw & 0xffff;
					if( abs(tempYaw - defaultYaw) < abs(bestYaw - defaultYaw))
					{
						bestYaw = tempYaw;
						bestTarget = hitActor;
						bestZ = objectdir.z;
					}
				}
			}
			
		}
/*	}
	else
	{
		bestTarget = rectarget.victim;
		objectdir = normal(rectarget.victim.location - projstart);
		checkAngle = rotator(objectdir);
		bestYaw = checkAngle.yaw;
		bestYaw = bestYaw & 0xffff;
		bestZ = objectdir.z;
	}
*/
	if(bestTarget != none)
	{
		//tempYaw=defaultAngle.yaw;
		//tempYaw=tempYaw&0xffff;

		if(abs(bestYaw - defaultYaw) < 8000)
		{
			fireDir.z=bestZ;
		}
	}

	defaultAngle = rotator(fireDir);

	return defaultAngle;
}

//**************************************************************************************************************
function float TurnWhileStrafingMult()
{
	//This should be made more generic
	if( Basilisk(BossTarget) != none )
		return 0.01  +  0.14 * (1 - baseWand(Weapon).ChargingLevel());

	return 0;
}

//**************************************************************************************************************
function vector GetSwordFireTargetLoc()
{
	//ClientMessage("***** offset:"$BossTarget.CentreOffset$" r_offset:"$BossTarget.CentreOffset >> BossTarget.Rotation);
	
	//If first half of battle, shoot at the head, otherwise return nothin' so wand uses the cam's rotation.
	//if( !Basilisk(BossTarget).bDidFirstBattle )
	//	return BossTarget.Location  +  (BossTarget.CentreOffset >> BossTarget.Rotation);
	//else
	//	return vect(0,0,0);

	return baseBoss(BossTarget).GetTargetLocation();
}

//**************************************************************************************************************
// Note: This timer duration is 1. Please don't change it unless you Know that it won't break 
// anything else.
function timer()
{
	SleepyAnimTimerSub();

}

//function DialogResponse(int dialogNum)
//{
//		local sound step;
//		step=speech[dialogNum];
//		PlaySound(step, SLOT_Talk,1.0, false, 1000.0, 0.9);
//}

function nailed(actor caller,out int status)
{
	if(bustedby==none)
	{
		bustedby=caller;
		status=1;
		gotostate('waitfordeath');
	}
	else
	{
		status=0;
	}
}

function displaydemoMessage()
{
	//basehud(myHud).ShowPopup(class'demoLetter');
}

state waitForDeath
{
	begin:
		DebugState();

	loop:
		if(abs(vsize(location-bustedby.location))<150)
		{
			moveto(location);
			CurrFidgetAnimName = GetCurrFidgetAnimName();
			playAnim(CurrFidgetAnimName, 1.0, 0.2);
		}
		else
		{
			moveToward(bustedby);
			loopAnim( HarryAnims[HarryAnimSet].Run );
		}

	//	sleep (0.3);
		goto 'loop';
}

//********************************************************************************************
function name GetCurrIdleAnimName()
{
	local string animName;
	local int	index;
	local name	nm;

	// in dueling mode, play just one idle animation
	if(bInDuelingMode)
		IdleNums = 0;
	
	if(IdleNums == 0)
		return HarryAnims[HarryAnimSet].Idle;

	// sleepy and sword have its own idle animations
	if((HarryAnimSet == HARRY_ANIM_SET_SLEEPY) || (HarryAnimSet == HARRY_ANIM_SET_SWORD) )
		return HarryAnims[HarryAnimSet].Idle;
	
	// HARRY_ANIM_SET_MAIN = 0, HARRY_ANIM_SET_ECTO = 1, HARRY_ANIM_SET_WEB = 4,
	index = 1 + Rand(IdleNums);
	animName = "idle_" $index;
	nm = StringToAnimName(animName);

	return nm;
}

//********************************************************************************************
function name GetCurrFidgetAnimName()
{
	local string animName;
	local int	index;
	local name	nm;

	// in dueling mode, do not play any fidget animation
	if(bInDuelingMode)
		FidgetNums = 0;
	
	if(FidgetNums == 0)
		return GetCurrIdleAnimName();
	
	index = 1 + Rand(FidgetNums);
	animName = "fidget_" $index;
	nm = StringToAnimName(animName);

	return nm;
}

//********************************************************************************************
function ReceiveIconMessage(Texture icon,string message,float duration)
{
	baseHUD(myHud).ReceiveIconMessage(icon,message,duration);
}

//********************************************************************************************
function bool HarryIsDead()
{
	return (GetHealthCount() <= 0);
}

//********************************************************************************************
/*
state PlayerWalking
{
ignores SeePlayer, HearNoise, Bump;

	event PlayerTick( float DeltaTime )
	{
		SaveInputVars();
	}

	function SaveInputVars()
	{
		HarryPawn.aStrafe = aStrafe;
		HarryPawn.aTurn = aTurn;
		HarryPawn.aLookup = aLookup;
		HarryPawn.aSideMove = aSideMove;
		HarryPawn.aForward = aForward;
		HarryPawn.aBaseX = aBaseX;
		HarryPawn.aBaseY = aBaseY;
		HarryPawn.aBaseZ = aBaseZ;
		HarryPawn.aMouseX = aMouseX;
		HarryPawn.aMouseY = aMouseY;

		HarryPawn.bZoom = bZoom;
		HarryPawn.bRun = bRun;
		HarryPawn.bLook = bLook;
		HarryPawn.bDuck = bDuck;
		HarryPawn.bSnapLevel = bSnapLevel;
		HarryPawn.bStrafe = bStrafe;
		HarryPawn.bFire = bFire;
		HarryPawn.bAltFire = bAltFire;
		HarryPawn.bFreeLook = bFreeLook;
	}

	function BeginState()
	{
		SetCollision(false,false,false);

		foreach allActors(class'BaseCam', cam)
			break;
	}
	
	function EndState()
	{
		//log("PLOG PWalking Exited");
		WalkBob = vect(0,0,0);
		bIsCrouching = false;
	}
}
*/

function startmenu()
{
	hpconsole(player.console).bQuickKeyEnable = false;
	hpconsole(player.console).LaunchUWindow();
}

state exittoMenu
{
	begin:
		Level.Game.RestartGame();
		gotostate('harryfrozen');
}

state harryfrozen
{
	ignores Fire, AltFire, ZoneChange, AnimEnd, Landed, PlayerTick, SeePlayer, HearNoise, Bump;

	function BeginState()
	{
	}
	
	function EndState()
	{
	}
}

//*----------------------------------------------------------------------------
//this override of PlayerCalcView allows the use of a third person camera
event PlayerCalcView(out actor ViewActor, out vector CameraLocation, out rotator CameraRotation )
{
	local Pawn PTarget;

	if ( ViewTarget != None )
	{
		ViewActor = ViewTarget;

		// HOLY CRAP I FINALLY FOUND IT!!!! THIS HARDCODED OFFSET WAS CAUSING ME SOOO MUCH PAIN!!!!!
		// 
		CameraLocation = ViewTarget.Location;// + vect(0,0,60); // <- damn this offset!!!!!!!!!!!
		CameraRotation = ViewTarget.Rotation;
	}
}

function makeTarget()
{
	local vector tloc;
	local vector targetoffset;

	targetOffset.y=0;
	targetOffset.x=50;
	targetOffset.z=0;

	tloc=targetOffset>>viewrotation;
	tloc=tloc+location;

	
	if(SpellCursor == none)
	{
		gotostate('playerwalking');
		clientmessage("failed targetspawn");
		hpconsole(player.console).bspaceReleased=true;
		hpconsole(player.console).bSpacePressed=false;
	}
	else
	{
		SpellCursor.TurnTargetingOn();
	}
}

//****************************************************************************************************************
function StartSpellLearning(SpellLessonTrigger SpellLesson)
{
	CurrSpellLesson = SpellLesson;
	GoToState('SpellLearning');
}

function EndSpellLearning()
{
	CurrSpellLesson = None;
	GotoState('PlayerWalking');
}

state SpellLearning
{
	// Harry be motionless here.
	ignores ProcessMove, AltFire;

	event PlayerInput( float DeltaTime )
	{
		Super.PlayerInput( DeltaTime );

		CurrSpellLesson.PlayerInput(DeltaTime);
	}
}

function StartVendorEngagement(VendorManager VManager)
{
	CurrVendorManager = VManager;

    bKeepStationary = true;

	//Want to stop harry from moving, but putting these here wont do anything, cause this is called from Bump,
	// when eventBump goes back to the c code, it must do more physics and reset his velocity.  You need to do this
	// later when the state has been changed and a tick has gone by.  So the code is now in state 'EngageVendor' in
	// VendorManager, and that seems to fix the problem.
	//	Acceleration = vect(0,0,0);
	//	Velocity *= vect(0,0,1);
}

function EndVendorEngagement()
{
	CurrVendorManager = None;
    bKeepStationary = false;
}

function bool IsEngagedWithVendor()
{
    return (CurrVendorManager != None);
}

//****************************************************************************************************************

//****************************************************************************************************************
function Add60HousePointsToGryffindor()
{
	numHousePointsHarry += 60;
	numHousePointsGryffindor += 60;
	numLastHousePointsHarry = 60;

	// just in case of error somewhere
	if(numHousePointsSlytherin >= numHousePointsGryffindor)
		numHousePointsSlytherin = numHousePointsGryffindor -1;
}


//************************************************************************************
function AddHousePoints (int num)
{
	local int temp;
	local float ftemp;

	numLastHousePointsHarry = num;

	numHousePointsHarry += num;

//	if(baseHud(myHud).PointItem!=None)
//		baseHud(myHud).PointItem.Show();

	// Set Gryffindor
	numHousePointsGryffindor = numHousePointsHarry;

	// Set Slytherin
		// Note Slytherin must always be ahead of Gryffindor,
		// but less than 60 points ...

	if (numHousePointsGryffindor<58)
		temp = numHousePointsGryffindor + Rand(numHousePointsGryffindor) + 1;
	else
		temp = numHousePointsGryffindor + Rand(58) + 1;
	if (numHousePointsSlytherin < temp)
		numHousePointsSlytherin = temp;

	// Set Hufflepuff
	ftemp = float(numHousePointsGryffindor) * (0.5 + Frand()*0.2);
	if (numHousePointsHufflepuff < ftemp)
		numHousePointsHufflepuff = ftemp;

	// Set Ravenclaw
	ftemp = float(numHousePointsGryffindor) * (0.7 + Frand()*0.2);
	if (numHousePointsRavenclaw < ftemp)
		numHousePointsRavenclaw = ftemp;

	log ("###### House Points");
	log ("added"@numLastHousePointsHarry);
	log ("Harry total"@numHousePointsHarry);
	log ("Gryffindor"@numHousePointsGryffindor);
	log ("Slytherin"@numHousePointsSlytherin);
	log ("Hufflepuff"@numHousePointsHufflepuff);
	log ("Ravenclaw"@numHousePointsRavenclaw);
}


//****************************************

/*
function int getNumCards ()
{
	local int iCard, num;

	for (iCard = 0; iCard < ArrayCount(WizardCards); iCard ++)
		if (WizardCards[iCard].Owner == CardOwner_Harry)
			num++;
			
	return num;
}


// debugging function
function giveAllCards ()
{
	local int iCard;
	for (iCard = 0; iCard < ArrayCount(WizardCards); iCard ++)
			WizardCards[iCard].Owner = CardOwner_Harry;
}

*/	
//**************************************************************************************************************
state stateCutIdle
{
	function BeginState()
	{
		Acceleration = vect(0,0,0);
		Velocity =     vect(0,0,0);
		CurrIdleAnimName = GetCurrIdleAnimName();
		LoopAnim( CurrIdleAnimName, 1.0, 0.2);
	}
}

//**************************************************************************************************************
function bool CutQuestion(string question)
{
	local ChallengeScoreManager managerChallenge;
	local bool		bAnswer;

	CutErrorString="";	//clear error string.

	// If there is a mini-game director, give it first chance at answering the
	// question; if it reports that it didn't answer it, then let Harry give it
	// a try.
	if ( Director != None )
	{
		bAnswer = Director.CutQuestion( question );
		if ( !(CutErrorString ~= "Unanswered") )
			return bAnswer;
	
		CutErrorString="";	// clear error string and let Harry try
	}

		//see if it is a question about the game state.
	question=caps(question);
	if(instr(question,"GSTATE")>-1)
	{
		cm("CutQuestion about game state:"$question $" CurrentGameState is:" $currentGameState);
		if(CurrentGameState~=question)
			return(true);
		else
			return(false);
	}


//sample question
	if(question~="EnoughBeans")
	{
//		if(beans greater than whatever)
			return true;
//		else
//			return false;
	}
	else if (question ~= "IsGryffindorAhead" ||
		     question ~= "IsSlytherinAhead"  ||
			 question ~= "IsHufflepuffAhead" ||
			 question ~= "IsRavenclawAhead")
	{
		return (managerStatus.GetStatusGroup(class'StatusGroupHousePoints').CutQuestion(question));
	}

	else if (question ~= "ChallengeIsFirstTime" ||
			 question ~= "ChallengePreviouslyBeaten" ||
			 question ~= "ChallengePreviouslyMastered" ||
			 question ~= "ChallengeWorseThanBefore"    ||
			 question ~= "ChallengeJustWonFirstTime"   ||
			 question ~= "ChallengeJustMastered"       ||
			 question ~= "ChallengeMissedStars"       ||
			 question ~= "ChallengeNewBestScore")
	{
		// Get level's challenge score manager.  A level should either have
		// 0 or 1 ChallengeScoreManagers.
		foreach AllActors(class'ChallengeScoreManager', managerChallenge )
			break;

		if (managerChallenge != None)
			return (managerChallenge.CutQuestion(question));		
		else
			return Super.CutQuestion(question);
	}
    else if (question ~= "ReadyForTransitionE")
    {        
        return (bHub9CeremonyFlag == true &&
                managerStatus.GetStatusItem(class'StatusGroupPolyIngr',class'StatusItemBoomslang').nCount > 0 &&
                managerStatus.GetStatusItem(class'StatusGroupPolyIngr',class'StatusItemBicorn').nCount > 0);
    }
    else if (question ~= "HaveAllSilverCards")
    {
        return (managerStatus.GetStatusItem(class'StatusGroupWizardCards',class'StatusItemSilverCards').nCount >= 40);
    }
	else
		return Super.CutQuestion(question);
}
	

//**************************************************************************************************************
function bool CutCommand(string command,optional string cue,optional bool bFastFlag)
{
	local string  sActualCommand;
	local string  sCutName;
	local actor   a;
	local string sSayText;
	local string sSayTextID;
    local Characters CurrCharacter;


	ClientMessage(self$" CutCommand:" $command $" Cue:" $cue);

	sActualCommand = ParseDelimitedString( command, " ", 1, false );

	if( sActualCommand ~= "Capture" )
	{
        // Vendor characters need to bail out of their mini cutscenes
        // if Harry is captured while they are luring.
		foreach AllActors( class'Characters', CurrCharacter )
			CurrCharacter.OnHarryCaptured();

		bIsCaptured=true;
		myHud.StartCutScene();

		foreach AllActors( class'HPawn', foreachActor )
			foreachActor.PlayerCutCapture();
		
		GotoState( 'stateCutIdle' );
		return true;
	}
	else
	if( sActualCommand ~= "Release" )
	{
		myHud.EndCutScene();

		DestroyControllers();

		foreach AllActors( class'HPawn', foreachActor )
			foreachActor.PlayerCutRelease();

		bIsCaptured=false;
		GotoState( 'playerWalking' );
		RotationRate = default.RotationRate;
		return true;
	}
	else
	if( sActualCommand ~= "ToggleUseSword" )
	{
		ToggleUseSword();
		CutCue( cue );
		return true;
	}
	else
	if( sActualCommand ~= "ChangeGameState" )
	{
		sActualCommand = ParseDelimitedString( command, " ", 2, false );
		
		if( !SetGameState( sActualCommand ) )
		{
			CutErrorString = "!E!R!R!O!R! GameState " $sActualCommand $" is not a valid GameState in the *GameStateMasterList*!!!";
			CutCue( cue );
			return false;
		}
		
		CutCue( cue );
		return true;
	}
	else
	if( sActualCommand ~= "HideWeapon" )
	{
		weapon.bHidden = true;
		CutCue( cue );
		return true;
	}
	else
	if( sActualCommand ~= "ShowWeapon" )
	{
		weapon.bHidden = false;
		CutCue( cue );
		return true;
	}
    else
    if (sActualCommand ~= "SetHub9CeremonyFlag")
    {
        bHub9CeremonyFlag = true;
        CutCue(cue);
        return true;
    }
    else
    if (sActualCommand ~= "GiveHermioneBicorn")
    {
        managerStatus.AddBicorn(-1);
        CutCue(cue);
        return true;
    }
    else
    if (sActualCommand ~= "GiveHermioneBoomslang")
    {
        managerStatus.AddBoomslang(-1);
        CutCue(cue);
        return true;
    }
	else
    if (sActualCommand ~= "RunCredits")
    {
		menuBook = HPConsole(player.console).MenuBook;
		if (menuBook != None)
		{
			menuBook.RunTheCredits();
	        CutCue(cue);
		    return true;
	    }
	}

        
	return  super.CutCommand(command, cue, bFastFlag);
}

//********************************************************************************************
function ToggleUseSword()
{
	bHarryUsingSword = !bHarryUsingSword;

	if( bHarryUsingSword )
		HarryAnimSet = HARRY_ANIM_SET_SWORD;
	else
		HarryAnimSet = HARRY_ANIM_SET_MAIN;

	//bInstantCast = !bInstantCast;

	baseWand(Weapon).ToggleUseSword();
}

//********************************************************************************************


function bool MoveWhileCasting()
{
	return bMoveWhileCasting;
}

event PlayerInput( float DeltaTime )
{
	if ( bE3DemoLockout ) 
	{
		if ( myHud.MainMenu != None )
			myHud.MainMenu.MenuTick( DeltaTime );
		// clear inputs
		bEdgeForward = false;
		bEdgeBack = false;
		bEdgeLeft = false;
		bEdgeRight = false;
		bWasForward = false;
		bWasBack = false;
		bWasLeft = false;
		bWasRight = false;
		aStrafe = 0;
		aTurn = 0;
		aForward = 0;
		aLookUp = 0;
		return;
	}

	if( bInDuelingMode )
		bStrafe = 1;
	
	Super.PlayerInput( DeltaTime );
	
	// If we are in dueling mode and we are not charging a spell
	if( bInDuelingMode && !(baseWand(weapon).ChargingLevel() > 0) )
		HandleDuelPlayerInput();
	else
	if (bDrinkWiggenWell == 1)
		DoDrinkWiggenWell();

	if (CurrVendorManager != None)
		CurrVendorManager.PlayerInput(DeltaTime);
}

//********************************************************************************************
function DoDrinkWiggenwell()
{
	local StatusItem  siWiggenPotion;
	local StatusGroup sgPotions;

    // If Harry's already at max health, don't drink
    if (managerStatus.GetHealthCount() == managerStatus.GetHealthPotentialCount())
        return;

	// If Harry's currently drinking a potion, wait until he's done.
	if (HarryAnimChannel.IsInState('stateDrinkWiggenwell'))
		return;

	//Dont do it if the player is aiming.
	if( PlayerIsAiming() )
		return;

	// If have a potion to drink, do it.
	siWiggenPotion = managerStatus.GetStatusItem(class'StatusGroupPotions', 
		                                         class'StatusItemWiggenWell');

	// If have a potion to drink, drink it.
	if (siWiggenPotion.nCount >= 1)
		HarryAnimChannel.DoDrinkWiggenwell();
}

//****************************************************************************************************************

function CopyAllStatusFromManagerToHarry()
{
    //ClientMessage(self $" In CopyAllStatusFromManagerToHarry*******************************");
    //log(self $" In CopyAllStatusFromManagerToHarry*******************************");

	CopyGenericStatusFromManagerToHarry();
    CopyCardStatusFromManagerToHarry();
}

// Save off StatusManager data into an array that travels/goes with save games.
function CopyGenericStatusFromManagerToHarry()
{
	local StatusGroup            sgLoop;
	local StatusItem             siLoop;
	local int                    nStatusIdx;

	// Setup StatusSave array with status item information to carry
	// onto next level or into restored game
	nStatusIdx = 0;
	for (sgLoop=managerStatus.sgList; sgLoop!=None; sgLoop=sgLoop.sgNext)
	{
		for (siLoop=sgLoop.siList; siLoop!=None; siLoop=siLoop.siNext)
		{
			// If list isn't big enough to hold everything, throw out a message.
			if (nStatusIdx >= ArrayCount(StatusSave))
			{
				ClientMessage("ERROR:  Need to increase StatusSaveSize");
				break;
			}

			// Save off necessary status information into our array that will travel/save.
			else
			{
				StatusSave[nStatusIdx].classGroup   = sgLoop.class;
				StatusSave[nStatusIdx].classItem    = siLoop.class;
				StatusSave[nStatusIdx].nPotential   = siLoop.nCurrCountPotential;
				StatusSave[nStatusIdx].nCount       = siLoop.nCount;
                StatusSave[nStatusIdx].nMaxCount    = siLoop.nMaxCount;

				nStatusIdx++;
			}
		}
	}

	// Clear out unused portion of StatusSave array.
	for (nStatusIdx=nStatusIdx; nStatusIdx < ArrayCount(StatusSave); nStatusIdx++)
	{
		StatusSave[nStatusIdx].classGroup = None;
		StatusSave[nStatusIdx].classItem  = None;
		StatusSave[nStatusIdx].nPotential = 0;
		StatusSave[nStatusIdx].nCount     = 0;
        StatusSave[nStatusIdx].nMaxCount  = 0;
	}

}

// Save off wizard card data in arrays that travel/save to save game.
function CopyCardStatusFromManagerToHarry()
{
	local StatusGroupWizardCards sgCards;
	local StatusItemWizardCards  siCards;
	local int                    i;
	local int                    nId;
	local int                    nOwner;

	// If there are any wizard cards left in the current level that can be sold by vendors,
	// flag them as being owned by vndors.
	sgCards = StatusGroupWizardCards(managerStatus.GetStatusGroup(class'StatusGroupWizardCards'));
	sgCards.AssignVendorCards();
	//sgCards.ShowCardData();   // for debugging

	// Save bronze card data in array that travels/goes to save game.
	siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemBronzeCards'));
	for (i=0; i<ArrayCount(BronzeCardSave); i++)
	{
		siCards.GetCardData(i, nId, nOwner);
		BronzeCardSave[i].nCardId    = nId;
		BronzeCardSave[i].nCardOwner = nOwner;
	}

	// Save silver card data in array that travels/goes to save game.
	siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemSilverCards'));
	for (i=0; i<ArrayCount(SilverCardSave); i++)
	{
		siCards.GetCardData(i, nId, nOwner);
		SilverCardSave[i].nCardId    = nId;
		SilverCardSave[i].nCardOwner = nOwner;
	}

	// Save gold card data in array that travels/goes to save gaem.
	siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemGoldCards'));
	for (i=0; i<ArrayCount(GoldCardSave); i++)
	{
		siCards.GetCardData(i, nId, nOwner);
		GoldCardSave[i].nCardId    = nId;
		GoldCardSave[i].nCardOwner = nOwner;
	}

	// Save off last card type picked up
	nLastCardTypeSave = sgCards.GetLastObtainedCardTypeAsInt();
}

function ClearNonTravelStatus()
{
	local StatusGroup            sgLoop;
	local StatusItem             siLoop;
	local int                    nStatusIdx;

	// Setup StatusSave array with status item information to carry
	// onto next level or into restored game
	nStatusIdx = 0;
	for (sgLoop=managerStatus.sgList; sgLoop!=None; sgLoop=sgLoop.sgNext)
	{
		for (siLoop=sgLoop.siList; siLoop!=None; siLoop=siLoop.siNext)
		{
            if (!siLoop.bTravelStatus)
            {
                siLoop.nCount = 0;
                siLoop.nMaxCount = 0;
                siLoop.nCurrCountPotential = 0;
            }
        }
    }
}


// --- Handle Spell Incantation Sound
function HandleSpellIncantationSound( ESpellType SpellType )
{
	local String SpellIncantation;
	local Sound  SpellSound;

	switch( SpellType )
	{
		case SPELL_Alohomora:
			switch( Rand(3) )
			{
				case 0:	SpellIncantation = "PC_Hry_SpellCast_01a";	break;
				case 1:	SpellIncantation = "PC_Hry_SpellCast_01b";	break;
				case 2:	SpellIncantation = "PC_Hry_SpellCast_01c";	break;
			}
			SpellSound = sound'HPSounds.Magic_sfx.cast_Alohomora';
			break;
		
		case SPELL_Flipendo:
			switch( Rand(3) )
			{
				case 0:	SpellIncantation = "PC_Hry_SpellCast_02a";	break;
				case 1:	SpellIncantation = "PC_Hry_SpellCast_02b";	break;
				case 2:	SpellIncantation = "PC_Hry_SpellCast_02c";	break;
			}
			SpellSound = sound'HPSounds.Magic_sfx.cast_Flipendo';
			break;

		case SPELL_Lumos:
			switch( Rand(3) )
			{
				case 0:	SpellIncantation = "PC_Hry_SpellCast_03a";	break;
				case 1:	SpellIncantation = "PC_Hry_SpellCast_03b";	break;
				case 2:	SpellIncantation = "PC_Hry_SpellCast_03c";	break;
			}
			SpellSound = sound'HPSounds.Magic_sfx.cast_Lumos';
			break;

		case SPELL_Skurge:
			switch( Rand(3) )
			{
				case 0:	SpellIncantation = "PC_Hry_SpellCast_04a";	break;
				case 1:	SpellIncantation = "PC_Hry_SpellCast_04b";	break;
				case 2:	SpellIncantation = "PC_Hry_SpellCast_04c";	break;
			}
			SpellSound = sound'HPSounds.Magic_sfx.cast_Skurge';
			break;

		case SPELL_Diffindo:
			switch( Rand(3) )
			{
				case 0:	SpellIncantation = "PC_Hry_SpellCast_05a";	break;
				case 1:	SpellIncantation = "PC_Hry_SpellCast_05b";	break;
				case 2:	SpellIncantation = "PC_Hry_SpellCast_05c";	break;
			}
			SpellSound = sound'HPSounds.Magic_sfx.cast_Diffindo';
			break;

		case SPELL_Rictusempra:
		case SPELL_DuelRictusempra:
			switch( Rand(3) )
			{
				case 0:	SpellIncantation = "PC_Hry_SpellCast_06a";	break;
				case 1:	SpellIncantation = "PC_Hry_SpellCast_06b";	break;
				case 2:	SpellIncantation = "PC_Hry_SpellCast_06c";	break;
			}
			SpellSound = sound'HPSounds.Magic_sfx.cast_Rictusempra';
			break;

		case SPELL_Spongify:
			switch( Rand(3) )
			{
				case 0:	SpellIncantation = "PC_Hry_SpellCast_07a";	break;
				case 1:	SpellIncantation = "PC_Hry_SpellCast_07b";	break;
				case 2:	SpellIncantation = "PC_Hry_SpellCast_07c";	break;
			}
			SpellSound = sound'HPSounds.Magic_sfx.cast_Spongify';
			break;

		case SPELL_DuelMimblewimble:
			switch( Rand(3) )
			{
				case 0:	SpellIncantation = "PC_Hry_SpellCast_08a";	break;
				case 1:	SpellIncantation = "PC_Hry_SpellCast_08b";	break;
				case 2:	SpellIncantation = "PC_Hry_SpellCast_08c";	break;
			}
			SpellSound = sound'HPSounds.Magic_sfx.cast_Mimblewimble';
			break;
			
		case SPELL_DuelExpelliarmus:
			switch( Rand(3) )
			{
				case 0:	SpellIncantation = "PC_Hry_SpellCast_09a";	break;
				case 1:	SpellIncantation = "PC_Hry_SpellCast_09b";	break;
				case 2:	SpellIncantation = "PC_Hry_SpellCast_09c";	break;
			}

			// do not play Expelliarmus sound here, decide what to play later
			SpellIncantation = "";	//	SpellIncantation = "PC_Hry_SpellCast_09a";	break;

			// do not play Expelliarmus sound here, decide what to play later
			SpellSound = none;			//	SpellSound = sound'HPSounds.Magic_sfx.cast_Expelliarmus';
			break;
	}
	
	// Play Incantation Sound
	if( SpellIncantation != "" )
		PlaySound( Sound(DynamicLoadObject("AllDialog."$SpellIncantation, class'Sound')), SLOT_Talk, , true);
	
	// Play Spell SoundFX
	if( SpellSound != None )
		PlaySound( SpellSound, SLOT_None, , true);
}

// --- Handle Dueling spell functions
function CheckIfHarryLostDuel()
{
	if( (managerStatus.GetHealthCount() <= 0) && !bDuelIsOver )
	{
		bDuelIsOver = true;
		UpdateDuelingRanks(false);

		Duellist(DuelOpponent).SayComment( DC_DuelLose, Duellist(DuelOpponent).eHouse, true );

		Duellist(DuelOpponent).SentEvent(Duellist(DuelOpponent).LostEventName);
	}
	else
		Duellist(DuelOpponent).SayComment( DC_DuelOpp, Duellist(DuelOpponent).eHouse, true );
}

function bool HandleSpellDuelRictusempra( optional baseSpell spell, optional vector vHitLocation )
{	
	local float fTimeAfterHitNew;
	local String SpellIncantation;

	if(	bDuelIsOver )
		return false;

	// See if we are currently rebounding spells
	if( bReboundingSpells )
	{	
		switch( Rand(3) )
		{
			case 0:	SpellIncantation = "PC_Hry_SpellCast_09a";	break;
			case 1:	SpellIncantation = "PC_Hry_SpellCast_09b";	break;
			case 2:	SpellIncantation = "PC_Hry_SpellCast_09c";	break;
		}
	
		// Play Incantation Sound
		PlaySound( Sound(DynamicLoadObject("AllDialog."$SpellIncantation, class'Sound')), SLOT_Talk, , true);
	 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_EXP_smack', Slot_Misc );

		// send this spell back at the caster
		spell.Reflect( self, FMin( 5,    spell.SpellCharge + ( 5 - spell.SpellCharge ) * 0.10f ), 
							 FMin( 1000, spell.Speed       + ( 1000 - spell.Speed  )   * 0.25f ));
		baseWand(weapon).FlashChargeParticles( class'Exep_Shield' );
		fTimeAfterShield = 1;

		return false;
	}

	AddHealth(-Duellist(DuelOpponent).DeltaHealth(true, 0, spell.SpellCharge));

	CheckIfHarryLostDuel();

 	PlaySound( HurtSound[ Rand(NUM_HURT_SOUNDS) ] );
// 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_harry_gets_hit', Slot_Misc );

	HarryAnimChannel.DoReactRictusempra();

	// if opponent is smarter, hit more
	fTimeAfterHitNew = 1.0 * (1 + Duellist(DuelOpponent).Intellect);
	if( fTimeAfterHitNew > fTimeAfterHit)
		fTimeAfterHit = fTimeAfterHitNew;

	return true; // valid hit
}

function bool HandleSpellDuelMimblewimble( optional baseSpell spell, optional vector vHitLocation )
{		
	local String SpellIncantation;

	if(	bDuelIsOver )
		return false;

	// See if we are currently rebounding spells
	if( bReboundingSpells )
	{
		switch( Rand(3) )
		{
			case 0:	SpellIncantation = "PC_Hry_SpellCast_09a";	break;
			case 1:	SpellIncantation = "PC_Hry_SpellCast_09b";	break;
			case 2:	SpellIncantation = "PC_Hry_SpellCast_09c";	break;
		}
	
		// Play Incantation Sound
		PlaySound( Sound(DynamicLoadObject("AllDialog."$SpellIncantation, class'Sound')), SLOT_Talk, , true);
	 	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_EXP_smack', Slot_Misc );

		// send this spell back at the caster
		spell.Reflect( self, FMin( 5,    spell.SpellCharge + ( 5 - spell.SpellCharge ) * 0.10f ), 
							 FMin( 1000, spell.Speed       + ( 1000 - spell.Speed  )   * 0.25f ));

		baseWand(weapon).FlashChargeParticles( class'Exep_Shield' );
		fTimeAfterShield = 1;
		return false;
	}

	PlaySound( Sound'HPSounds.Magic_sfx.Dueling_MIM_hit');

	if(Rand(2) == 0)
	{
		AddHealth(-Duellist(DuelOpponent).DeltaHealth(true, 1, spell.SpellCharge));
		PlaySound( Sound'HPSounds.Magic_sfx.Dueling_MIM_self_damage', Slot_Misc);
	}
	else
		PlaySound( Sound'HPSounds.Magic_sfx.Dueling_MIM_self_lucky', Slot_Misc);

	CheckIfHarryLostDuel();

	HarryAnimChannel.DoReactMimbleWimble();

	// if opponent is smarter, hit more
	fTimeAfterHit = 2.0 + Duellist(DuelOpponent).Intellect;

	return true; // valid hit
}

function bool HandleSpellDuelExpelliarmus( optional baseSpell spell, optional vector vHitLocation )
{
	return false; // *in-valid* hit
}

// ---

// Assign objective text id 
function SetObjectiveTextId(string strId)
{
    strObjectiveId = strId;
}

// If any entry in strObjectiveId array is non-empty, then have objective text.
function bool HaveObjectiveText()
{
	return (strObjectiveId != "");
}

//Returns an int with the current game state in a number format
function int ConvertGameStateToNumber()
{
local string num;

	num=right(currentGameState,3);

	ClientMessage("*********Converting gameState: " $currentGameState $" - " $num);

	return(int(num));
}
	//Update wizard rankings upon dueling win.
function UpdateDuelingRanks(bool bWon)
{
	if(bWon)
	{
        // Beans count is now updated in the "post-duel" cutscenes.
		//managerStatus.IncrementCount(class'StatusGroupJellybeans', class'StatusItemJellybeans', 2 * DuelRankBeans);

		if(DuelRankHarry == DuelRankOppon)
			DuelRankHarry++;
  
		if(curWizardDuel==lastUnlockedDuelist)
		{
			curWizardDuelRank--;
			if(curWizardDuelRank<0)
				curWizardDuelRank=0;

	//		PreSwitchPage();
		}
	}
}

// Overriddent from PlayerPawn .
function SaveGame(int nSlot)
{
    //ClientMessage(self $" In Harry::OnSaveGame*******************************");
    //log(self $" In Harry::OnSaveGame*******************************");
    
    // Copy status stuff to Harry so it gets saved off with Harry.
    CopyAllStatusFromManagerToHarry();

    // Call parent save game.
    Super.SaveGame(nSlot);
}

defaultproperties
{
	ShadowClass=Class'HGame.HarryShadow'
	eaid="xa37dd45ffe100bfffcc9753aabac325f07cb3fa231144fe2e33ae4783feead2b8a73ff021fac326df0ef9753ab9cdf6573ddff0312fab0b0ff39779eaff312x"
	HurtSound(0)=Sound'HPSounds.Har_Emotes.ouch1'
	HurtSound(1)=Sound'HPSounds.Har_Emotes.ouch2'
	HurtSound(2)=Sound'HPSounds.Har_Emotes.ouch3'
	HurtSound(3)=Sound'HPSounds.Har_Emotes.ouch4'
	HurtSound(4)=Sound'HPSounds.Har_Emotes.ouch5'
	HurtSound(5)=Sound'HPSounds.Har_Emotes.ouch6'
	HurtSound(6)=Sound'HPSounds.Har_Emotes.ouch7'
	HurtSound(7)=Sound'HPSounds.Har_Emotes.ouch8'
	HurtSound(8)=Sound'HPSounds.Har_Emotes.ouch9'
	HurtSound(9)=Sound'HPSounds.Har_Emotes.ouch10'
	HurtSound(10)=Sound'HPSounds.Har_Emotes.ouch11'
	HurtSound(11)=Sound'HPSounds.Har_Emotes.ouch12'
	HurtSound(12)=Sound'HPSounds.Har_Emotes.ouch13'
	HurtSound(13)=Sound'HPSounds.Har_Emotes.oof1'
	HurtSound(14)=Sound'HPSounds.Har_Emotes.oof2'
	turnRate=1000
	maxPointsPerHouse=150
	HarryMultipleForGryffindor=3
	bTargettingError=True
	bCanCast=True
	AnimFalling='fall'
     HarryAnims(0)=(Idle=Idle,Walk=Walk,run=run,WalkBack=runback,straferight=straferight,strafeleft=strafeleft,Jump=Jump,jump2=jump2,fall=fall,land=land)
     HarryAnims(1)=(Idle=Idle,Walk=ectowalk,run=ectowalk,WalkBack=ectowalkback,straferight=ectostraferight,strafeleft=ectostrafeleft,Jump=ectojump,jump2=jump2,fall=fall,land=land)
	 HarryAnims(2)=(Idle=IdleSleepy,Walk=sleepywalk,run=sleepywalk,WalkBack=sleepywalkback,straferight=sleepystraferight,strafeleft=sleepystrafeleft,Jump=sleepyjump,jump2=jump2,fall=fall,land=land)
     HarryAnims(3)=(Idle=SwordIdle,Walk=Walk,run=SwordRun,WalkBack=SwordRunBack,straferight=SwordStrafeRight,strafeleft=SwordStrafeLeft,Jump=SwordJump,jump2=SwordJump2,fall=SwordFall,land=SwordLand)
	 HarryAnims(4)=(Idle=Idle,Walk=webmove,run=webmove,WalkBack=webmove,straferight=webmove,strafeleft=webmove,Jump=ectojump,jump2=ectojump,fall=fall,land=land)
	 HarryAnims(5)=(Idle=Duel_Idle,Walk=duel_run,run=duel_run,WalkBack=duel_runback,straferight=duel_strafe_right,strafeleft=duel_strafe_left,Jump=none,jump2=none,fall=none,land=none)
//SwordFall
//SwordLand
//SwordFidget
//SwordCast

	RotationRate=(Pitch=20000,Yaw=70000,Roll=3072)

	bAllowHarryToDie=True
	ConstrainYawVariance=5500
	GroundJumpSpeed=200
	GroundEctoSpeed=50
	iMaxSleepyAnim=6
	fSleepySpeed=50
	fWebSpeed=145
	DesiredSpeed=1
	GroundSpeed=210
	AirSpeed=400
	AccelRate=1024
	JumpZ=245
	MaxMountHeight=96.5
	AirControl=0.25
	BaseEyeHeight=40.75
	EyeHeight=40.75
	MenuName="Harry"
	DrawType=DT_Mesh
	Mesh=SkeletalMesh'HPModels.skharryMesh'
	AmbientGlow=65
	CollisionRadius=15
	CollisionHeight=42
	FootOffsetZ=-34
	Mass=100
	Buoyancy=118.8

	DuelSpells(0)=class'spellDuelRictusempra'
	DuelSpells(1)=class'spellDuelMimblewimble'
	DuelSpells(2)=class'spellDuelExpelliarmus'

	DuelSpellSwitchSounds(0)=Sound'HPSounds.Magic_sfx.Dueling_switch2RIC'
	DuelSpellSwitchSounds(1)=Sound'HPSounds.Magic_sfx.Dueling_switch2MIM'
	DuelSpellSwitchSounds(2)=Sound'HPSounds.Magic_sfx.Dueling_switch2EXP'

	DuelRankHarry=1	// could duel just first guy in the beginning !
	
	CurrFidgetAnimName=none
	CurrIdleAnimName=none
	FidgetNums=0
	IdleNums=0
	bVeryAfraid=False

	SpongifyFallAnim=spongify
	fHighestZ=-999999.0f

	bAutoCenterCamera=true

	bIsGoyle=false
	bDoEyeBlinks=true

	quidGameResults(0)=(bLocked=true,opponent="Hufflepuff");
	quidGameResults(1)=(bLocked=true,opponent="Ravenclaw");
	quidGameResults(2)=(bLocked=true,opponent="Slytherin");
	quidGameResults(3)=(bLocked=true,opponent="Hufflepuff");
	quidGameResults(4)=(bLocked=true,opponent="Ravenclaw");
	quidGameResults(5)=(bLocked=true,opponent="Slytherin");

	curWizardDuelRank=9
	lastUnlockedDuelist=8
	
	bAutoQuaff=true
	fDamageMultiplier_Easy=1.0f
	fDamageMultiplier_Medium=1.2f
	fDamageMultiplier_Hard=1.5f

	bNoSpellBookCheck=false
}

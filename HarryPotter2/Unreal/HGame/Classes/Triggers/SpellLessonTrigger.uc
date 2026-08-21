//=============================================================================
// SpellLessonTrigger
//
// The SpellLessonTrigger and its helper objects, SpellLessonInterpolationPoint,
// SpellLessonWand and SpellLessonShape are used to play a spell lesson in the 
// game.
//
// SpellLessonTrigger is activated from an event sent out by an intro cutscene.
// When complete, SpellLessonTrigger sends out an event that an end cutscene
// will respond to.
//
// During a spell lesson, the SpellLessonTrigger object controls playing dialog
// describing the game, handles the player interaction, and gives feedback to 
// the player on how they are doing.
//
// There are a few steps to getting a spell lesson setup:
//
//		1) Add SpellLessonShape object.  Put it where you want it, assign the
//      appropriate spell shape texture to MultiSkins[0], adjust its DrawScale
//      as appropriate.
//      2) Add as many SpellLessonInterpolationPoint objects as needed to 
//      make the form of the shape and enough for for hitpoint arrow placement.
//      Note, some or all of these IP points can be hitpoints.  Take into 
//      consideration where you'll want hitpoints to be located when placing 
//      the IP points.
//      3) Depending on the orientation of the SpellLessonShape, one of the
//      3D coordinates should be the same for all interpolation points.  For
//      example, if the shape is parallel to the X axis, all interpolation points
//      should have the same X location value.  In addition, the StartControlPoint
//      and EndControlPoint values should be 0 for the value corresponding to
//      the parallel axis (0 for the X value in this case).
//      3) Make sure all SpellLessonInterpolationPoint objects have the same
//      tag name (which should be different than any other IP points in the
//      level).
//      4) Set the Position property for eacy point.
//      5) Make sure that the interpolation points are a little in front of
//      the shape and all on the same plane. 
//      6) Most likely, you want the bFaceMoveDirection for each interpolation
//      point to be false (SpellLessonInterpolationPoint was originally going
//      to set this property to default to false, but that seemed to cause
//      crashing problems when adding them in the Editor.)  
//      8) Position a cutcamera in front of the shape.
//      9) Add a SpellLessonTrigger object and fill in its "SpellLessonTrigger" 
//      properties.
//      10) Set the Event\Tag property to match the what the intro cutscene calls.
//      11) Set the Event\Event property to the event you want to send out
//      when the lesson is done.  The end cutscene will trigger off of this.
// 
//=============================================================================

class SpellLessonTrigger extends Trigger;

// For hud display positioning
const nHIT_INDICATOR_W                  = 76;
const nHIT_INDICATOR_H                  = 93;
const nHIT_INDICATOR_MID_X              = 37;
const nHIT_INDICATOR_PLAYER_MID_Y       = 14;
const nHIT_INDICATOR_PAR_MID_Y          = 92;

// Distance wand must be within to hit a hitpoint
const fHITPOINT_RANGE = 15.0;
const fHITPOINT_MISS_RANGE = 18.0;

const strRICTU_DIFF_GAME_MUSIC   = "sm_bur_playful_01_loopedit";
const strSKURGE_SPONG_GAME_MUSIC = "sm_bur_playful_01V2";

const strWON_MUSIC        = "sm_bur_PlayfulReward_01";
const strFAIL_MUSIC       = "sm_bur_PlayfulFail_01";
const fWON_MUSIC_LEN      = 7.5;
const fFAIL_MUSIC_LEN     = 9.0;
const fEND_MUSIC_FADE_OUT = 0.5;

// Camera modes for spell lesson.
enum ECameraSettings
{
	CS_GameShape,
	CS_BackToNormal
};

// Corresponding lesson shape- used to determine what dialog to play.
enum ELessonShape
{
	LessonShape_Rictusempra,
	LessonShape_Skurge,
	LessonShape_Diffindo,
	LessonShape_Spongify
};

// Key pressed during the lesson.
enum ELessonKey
{
    LessonKey_Up,
    LessonKey_Down,
    LessonKey_Left,
    LessonKey_Right
};

// Level designer needs to fill these in.
const nNUM_LEVELS = 3;               // Number of levels for each lesson
var() int          nLevel;
var() ELessonShape LessonShape;      // Shape enum- used to determine dialog
var() name         nameShape;		 // Object name of shape object for lesson
var() name         nameBackground;   // Ojbect name of background behind shape
var() name         nameCutCam;       // Object name of CutCameraPos object for lesson
var() name         nameSplinePath;   // Tag for all interpolation points in lesson
var() name         nameIPStart;		 // Object name of first interpolation point
var() name         nameIPEnd;		 // Object name of last interpolation point
var() float        fWandSpeed;       // Speed wand travels around shape
var() int          nMaxLoops[3];     // # loops player has to get it right

// Spell lesson objects that were placed in the world by the designer (they will
// be located using the name properties entered by the desinger).
var SpellLessonWand    GameWand;     // Wand that traces the shape
var SpellLessonShape   GameShape;    // Shape we're tracing
//var LessonBackground   GameBkgrd;    // Background behind shape
var CutCameraPos       GameCutCam;   // Camera postion while tracing shape
var InterpolationPoint GameIPStart;  // First interpolation point
var InterpolationPoint GameIPEnd;    // Last interpolation point
var Characters         Professor;    // Professor that teaches the class
var Harry              playerHarry;  // Harry
var baseCam            cam;          // Camera Harry uses
var int                nKeyNeedsReset[4]; // Array elem true if corresponding key
                                          // needs to be reset before hitting another.  
                                          // Note: You'd think this should be an array of
                                          // bools, but the compiler doesn't allow it
// For fading shape in.
//var bool     bShapeFullyVisible;	 // true when shape becomes fully visible

// Sounds
var sound soundWand;		   // Sound wand makes while moving

// For Hud Indicators
var int     nHitPointHits;     // # hit by player in current level
var int     nHitPointsInLevel; // Total hitpoints available in current level
var int     nHitPointsPassed;  // # of hitpoints that have passed by (regardless of hit)

// Number of times the wand has gone around the current level.
var int   nCurrTimesAround;

// Dialog strings
var string strIntroDialogId;	     // Clear your mind completely, Harry...
var string strTutorial1aId;          // See the wand? ... 
var string strTutorial1bId;          // When the wand passes over the arrows
var string strTutorial1cId;          // This exercise has 3 levels...
var string strEncourageDoingGood[2]; // You're as good as me!
var string strEncourageAny[2];       // Concentrate!
var string strTryAgain[3];           // Try again!
var string strTryOneLastTime[2];     // Try again for last time
var string strLevelCompleteId;       // Well done, Harry, you've advanced to the next level
var string str5PointsAwardedId;      // 5 points to Gryffindor!
var string str10PointsAwardedId;     // 10 points to Gryffindor!
var string str15PointsAwardedId;     // 15 points to Gryffindor!
var string strAllLevelsCompleteId;   // Well done, you've completed all levels
var string strMoveOnAnywayId;        // Alright, Harry, you've done enough.
var string strRound1Id;              // Round displayed before start of each level
var string strRound2Id;              // Round displayed before start of each level
var string strRound3Id;              // Round displayed before start of each level

// "Round X" text displayed in middle of screen
var string strCurrRound;		

// Generic looping var
var int    i;

// Housepoint hud related
var StatusGroup  sgHousePts;
var StatusItem   siGryffPts;
var int          nAddHousePts;
var sound        soundAddHousePts;

// Time to sleep during a dialog line or music sequence
var float        fDialogSleep;
var float        fMusicSleep;

// Keep track of last multiple active arrows that we passed (array needs to
// be big enough to handle overlapping arrows or arrows on corners)
var SpellLessonInterpolationPoint IPPassedArrow[3];

// Player hasn't successfully completed, but we're going to let them move on
// anyway.
var bool  bForceFinish;

// On trigger activation.
event Activate(actor Other, pawn EventInstigator)
{
	// Don't call parent activate- don't want event sent out as soon
	// as the trigger activates.  We send out an event when the lesson is done.
	//Super.Activate(Other, EventInstigator);

	// Initialize dialog strings.
	InitDialogStrings();

	// Find lesson related actors that have been placed in the level.
	FindLessonActors();

	// Initialize rotation of hitpoints
	InitHitPointRotation();

	// Harry goes into spell learning state
	playerHarry.StartSpellLearning(self);
	
	// Put camera into cutscene mode
	playerHarry.cam.SetCameraMode(playerHarry.cam.ECamMode.CM_CutScene);

	// Let hud know we're here now
	HPHud(playerHarry.myHud).RegisterSpellLesson(self);

	// Professor gives the intro.
	GoToState('GameIntro');	
}

event Tick(float DeltaTime)
{
    local float fDist;

    // If player made no attempt at last active hitpoint and now out of range,
    // it's now considered missed.  (We keep track of the last 2 to handle
    // cases where arrows overlap.)
    for (i=0; i<ArrayCount(IPPassedArrow); i++)
    {
        if (IPPassedArrow[i] != None)
        {
            // If last arrow is still active
            if (IPPassedArrow[i].IsActive(nLevel))
            {
                fDist = VSize(IPPassedArrow[i].Location - GameWand.Location);

                // Out of range now- missed.
		        if (fDist > fHITPOINT_RANGE)
		        {
			        IPPassedArrow[i].OnPlayerMissed(nLevel);
                    IPPassedArrow[i] = None;
                }
            }
            else
                IPPassedArrow[i] = None;
        }
    }
}

// Harry will pass player input along to us once the lesson has started.
// Only care about it in specific states where the lesson is in 
// progress though.
event PlayerInput( float DeltaTime )
{
}

// Initialize dialog based on shape lesson we're doing.
function InitDialogStrings()
{
	switch (LessonShape)
	{
	case (LessonShape_Rictusempra): 
        strIntroDialogId            = "PC_Gil_DADARictaTeach_09";
        strTutorial1aId             = "PC_Gil_DADARictaTeach_08";
        strTutorial1bId             = "PC_Gil_DADARictaTeach_19";
        strTutorial1cId             = "PC_Gil_DADARictaTeach_20";
        strEncourageDoingGood[0]    = "PC_Gil_DADARictaTeach_23";
        strEncourageDoingGood[1]    = "PC_Gil_DADARictaTeach_24";
        strEncourageAny[0]          = "";
        strEncourageAny[1]          = "";
        strTryAgain[0]              = "PC_Gil_DADARictaTeach_28";
        strTryAgain[1]              = "PC_Gil_DADARictaTeach_29";
        strTryAgain[2]              = "";
        strTryOneLastTime[0]        = "PC_Gil_DADARictaTeach_30";
        strTryOneLastTime[1]        = "";
        strLevelCompleteId          = "PC_Gil_DADARictaTeach_00";
        str5PointsAwardedId         = "PC_Gil_DADARictaTeach_01";
        str10PointsAwardedId        = "PC_Gil_DADARictaTeach_02";
        str15PointsAwardedId        = "PC_Gil_DADARictaTeach_04";
        strAllLevelsCompleteId      = "PC_Gil_DADARictaTeach_12";
        strMoveOnAnywayId           = "PC_Gil_DADARictaTeach_13";
        strRound1Id                 = "PC_Gil_DADARictaTeach_14";
        strRound2Id                 = "PC_Gil_DADARictaTeach_15";
        strRound3Id                 = "PC_Gil_DADARictaTeach_16";
		break;
	case (LessonShape_Skurge) :
		strIntroDialogId            = "PC_Flt_CharmSkurgTeach_02";
		strTutorial1aId             = "PC_Flt_CharmSkurgTeach_08";
		strTutorial1bId             = "PC_Flt_CharmSkurgTeach_09";
		strTutorial1cId             = "PC_Flt_CharmSkurgTeach_22";
        strEncourageDoingGood[0]    = "PC_Flt_CharmSkurgTeach_07";
        strEncourageDoingGood[1]    = "";
        strEncourageAny[0]          = "PC_Flt_CharmSkurgTeach_06";
        strEncourageAny[1]          = "";
        strTryAgain[0]              = "PC_Flt_CharmSkurgTeach_04";
        strTryAgain[1]              = "PC_Flt_CharmSkurgTeach_05";
        strTryAgain[2]              = "";
        strTryOneLastTime[0]        = "";
        strTryOneLastTime[1]        = "";
        strLevelCompleteId          = "PC_Flt_CharmSkurgTeach_15";
		str5PointsAwardedId         = "PC_Flt_CharmSkurgTeach_14";
		str10PointsAwardedId        = "PC_Flt_CharmSkurgTeach_13";
		str15PointsAwardedId        = "PC_Flt_CharmSkurgTeach_12";
        strAllLevelsCompleteId      = "PC_Flt_CharmSkurgTeach_23";
        strMoveOnAnywayId           = "PC_Flt_CharmSkurgTeach_26";
		strRound1Id                 = "PC_Flt_CharmSkurgTeach_17";
		strRound2Id                 = "PC_Flt_CharmSkurgTeach_18";
		strRound3Id                 = "PC_Flt_CharmSkurgTeach_19";
		break;
	case (LessonShape_Diffindo) :
		strIntroDialogId            = "PC_Spr_HerbDiffTeach_03";
		strTutorial1aId             = "PC_Spr_HerbDiffTeach_13";
		strTutorial1bId             = "PC_Spr_HerbDiffTeach_11";
		strTutorial1cId             = "PC_Spr_HerbDiffTeach_12";
        strEncourageDoingGood[0]    = "PC_Spr_HerbDiffTeach_06";
        strEncourageDoingGood[1]    = "";
        strEncourageAny[0]          = "PC_Spr_HerbDiffTeach_04";
        strEncourageAny[1]          = "PC_Spr_HerbDiffTeach_07";
        strTryAgain[0]              = "PC_Spr_HerbDiffTeach_29";
        strTryAgain[1]              = "PC_Spr_HerbDiffTeach_31";
        strTryAgain[2]              = "PC_Spr_HerbDiffTeach_33";
        strTryOneLastTime[0]        = "PC_Spr_HerbDiffTeach_30";
        strTryOneLastTime[1]        = "PC_Spr_HerbDiffTeach_32";
        strLevelCompleteId          = "PC_Spr_HerbDiffTeach_16";
		str5PointsAwardedId         = "PC_Spr_HerbDiffTeach_17";
		str10PointsAwardedId        = "PC_Spr_HerbDiffTeach_09";
		str15PointsAwardedId        = "PC_Spr_HerbDiffTeach_10";
        strAllLevelsCompleteId      = "PC_Spr_HerbDiffTeach_25";
        strMoveOnAnywayId           = "PC_Spr_HerbDiffTeach_28";
		strRound1Id                 = "PC_Spr_HerbDiffTeach_20";
		strRound2Id                 = "PC_Spr_HerbDiffTeach_21";
		strRound3Id                 = "PC_Spr_HerbDiffTeach_22";
		break;
	case (LessonShape_Spongify) :
		strIntroDialogId            = "PC_Gil_SpongeTeach_03";
		strTutorial1aId             = "PC_Gil_SpongeTeach_17";
		strTutorial1bId             = "PC_Gil_SpongeTeach_11";
		strTutorial1cId             = "PC_Gil_SpongeTeach_12";
        strEncourageDoingGood[0]    = "PC_Gil_SpongeTeach_30";
        strEncourageDoingGood[1]    = "";
        strEncourageAny[0]          = "";
        strEncourageAny[1]          = "";
        strTryAgain[0]              = "PC_Gil_SpongeTeach_32";
        strTryAgain[1]              = "PC_Gil_SpongeTeach_33";
        strTryAgain[2]              = "PC_Gil_SpongeTeach_34";
        strTryOneLastTime[0]        = "PC_Gil_SpongeTeach_35";
        strTryOneLastTime[1]        = "";
        strLevelCompleteId          = "PC_Gil_SpongeTeach_29";
		str5PointsAwardedId         = "PC_Gil_SpongeTeach_21";
		str10PointsAwardedId        = "PC_Gil_SpongeTeach_22";
		str15PointsAwardedId        = "PC_Gil_SpongeTeach_23";
        strAllLevelsCompleteId      = "PC_Gil_SpongeTeach_25";
        strMoveOnAnywayId           = "PC_Gil_SpongeTeach_28";
		strRound1Id                 = "PC_Gil_SpongeTeach_02";
		strRound2Id                 = "PC_Gil_SpongeTeach_04";
		strRound3Id                 = "PC_Gil_SpongeTeach_14";

		break;
	default :
		// LessonShape property not setup properly.  Display error message,
		// and use Rictusempra dialog.
		playerHarry.ClientMessage("Lesson shape not initialized.  Using Rictusempra dialog.");
        strIntroDialogId            = "PC_Gil_DADARictaTeach_09";
        strTutorial1aId             = "PC_Gil_DADARictaTeach_08";
        strTutorial1bId             = "PC_Gil_DADARictaTeach_19";
        strTutorial1cId             = "PC_Gil_DADARictaTeach_20";
        strEncourageDoingGood[0]    = "PC_Gil_DADARictaTeach_23";
        strEncourageDoingGood[1]    = "PC_Gil_DADARictaTeach_24";
        strEncourageAny[0]          = "";
        strEncourageAny[1]          = "";
        strTryAgain[0]              = "PC_Gil_DADARictaTeach_28";
        strTryAgain[1]              = "PC_Gil_DADARictaTeach_29";
        strTryAgain[2]              = "";
        strTryOneLastTime[0]        = "PC_Gil_DADARictaTeach_30";
        strTryOneLastTime[1]        = "";
        strLevelCompleteId          = "PC_Gil_DADARictaTeach_00";
        str5PointsAwardedId         = "PC_Gil_DADARictaTeach_01";
        str10PointsAwardedId        = "PC_Gil_DADARictaTeach_02";
        str15PointsAwardedId        = "PC_Gil_DADARictaTeach_04";
        strAllLevelsCompleteId      = "PC_Gil_DADARictaTeach_12";
        strMoveOnAnywayId           = "PC_Gil_DADARictaTeach_13";
        strRound1Id                 = "PC_Gil_DADARictaTeach_14";
        strRound2Id                 = "PC_Gil_DADARictaTeach_15";
        strRound3Id                 = "PC_Gil_DADARictaTeach_16";
		break;
	}
}

// Find objects related to the current spell lesson.
function FindLessonActors()
{
	// Find Harry.
	foreach AllActors( class'Harry', playerHarry )
	{
		if( playerHarry.bIsPlayer && playerHarry != Self )
			break;
	}

	cam = playerHarry.cam;

	// Use name properties entered by designer to locate the desired lesson objects.
	GameShape   = SpellLessonShape(GetLessonActor(class'SpellLessonShape', nameShape));
	GameIPStart = InterpolationPoint(GetLessonActor(class'InterpolationPoint', nameIPStart));
	GameIPEnd   = InterpolationPoint(GetLessonActor(class'InterpolationPoint', nameIPEnd));
	GameCutCam  = CutCameraPos(GetLessonActor(class'CutCameraPos', nameCutCam));
    //GameBkgrd   = LessonBackground(GetLessonActor(class'LessonBackground', nameBackground));

    // Find the lesson professor
    Professor = GetLessonProfessor();
}

// Get the lesson actor that corresponds to the class and object name passed in.
function Actor GetLessonActor(class<Actor> classActor, name nameActor)
{
	local Actor a;

	foreach AllActors(classActor, a)
	{
		if(a.name == nameActor)
			return (a);
	}

	playerHarry.ClientMessage("SpellLesson could not locate " $classActor $" " $nameActor);
	return (None);
}

function Characters GetLessonProfessor()
{
    local class<Actor> classProf;
    local Actor        Prof;
    local Actor        ClosestProf;
    local float        fDist;
    local float        fClosestDistSoFar;

	switch (LessonShape)
	{
	case (LessonShape_Rictusempra): 
        classProf = class'ProfLockhart';
		break;
	case (LessonShape_Skurge) :
        classProf = class'ProfFlitwick';
		break;
	case (LessonShape_Diffindo) :
        classProf = class'ProfSprout';
		break;
	case (LessonShape_Spongify) :
        classProf = class'ProfLockhart';
		break;
	default :
        return (None);
	}

    // Find all instances of the professor in the level.
	foreach AllActors(classProf, Prof)
	{
        // Distance from this prof to Harry
        fDist = VSize2D(Prof.Location - playerHarry.Location);

        // If first prof we found, then he's the closest
        if (ClosestProf == None)
        {
            ClosestProf = Prof;
            fClosestDistSoFar = fDist;
        }

        // This instance is closer than the previous closest
        else if (fDist < fClosestDistSoFar)
        {
            ClosestProf = Prof;
            fClosestDistSoFar = fDist;
        }
	}

    return (Characters(ClosestProf));
}

// Setup camera for desired spell lesson mode
function SetupCamera(ECameraSettings CameraSettings)
{
	// Update camera settings based on settings enum passed in
	switch (CameraSettings)
	{
	case (CS_GameShape):
		// If camera not in cutscene mode, change mode to cutscene
		if (cam.CameraMode != cam.ECamMode.CM_CutScene)
			cam.SetCameraMode(cam.ECamMode.CM_CutScene);

		cam.SetTargetActor(nameShape);
		cam.SetSyncPosWithTarget(false);
		
		break;

	case (CS_BackToNormal) :
		// If camera not in standard mode, change to standard
		if (cam.CameraMode != cam.ECamMode.CM_Standard)
			cam.SetCameraMode(cam.ECamMode.CM_Standard);

		// Restore camera properties
		cam.CamTarget.SetAttachedTo( playerHarry );

		break;

	default :
		playerHarry.ClientMessage("Unrecognized spell lesson camera setting");
		break;
	}
}

// Called by SpellLessonWand when wand has reached an interpolation point.
function bool WandAtInterpolationPoint( InterpolationPoint IPoint, InterpolationManager IManager )
{
	if (nameIPEnd != ''  &&  nameIPEnd == IPoint.Name )
    {
        GameWand.StopWand();
        nCurrTimesAround++;

        // If gone around the loop enough times
        if ((nCurrTimesAround >= nMaxLoops[nLevel]) ||
            (nHitpointsInLevel == nHitpointHits))
        {
		    GoToState('LevelOver');
        }

        // Go around again
        else
        {
            ResetForNextLoop();
            SayTryAgain((nCurrTimesAround == (nMaxLoops[nLevel] - 1)));
            GameWand.StartWand(fWandSpeed);
        }

        return (false);
    }
    else
    {
        // If this interpolation point is a hitpoint for the current level.
        if (SpellLessonInterpolationPoint(IPoint).IsInLevel(nLevel))
        {
            // Increment # of hitpoints that have gone by
            ++nHitPointsPassed;

            // If player hasn't hit it yet, save off for miss detection after the player
            // gets far enough away.  We keep an array of 2 of these to handle
            // cases where arrows might overlap.
            if (SpellLessonInterpolationPoint(IPoint).IsActive(nLevel))
            {
                for (i=0; i<ArrayCount(IPPassedArrow); i++)
                {
                    if (IPPassedArrow[i] == None)
                    {
                        IPPassedArrow[i] = SpellLessonInterpolationPoint(IPoint);
                        break;
                    }
                }
            }

            // Say some encouragement halfway through every other loop.
            if ((nCurrTimesAround % 2 != 0) && (nHitPointsPassed == (nHitPointsInLevel/2)))
                SayEncouragement();
        }

        return (true);
    }
}

// Make sure the rotation of each hitpoint matches the rotation of the shape.
function InitHitPointRotation()
{
	local SpellLessonInterpolationPoint IP;

	foreach AllActors(class'SpellLessonInterpolationPoint', IP)
	{
		if(IP.tag == GameIPStart.tag)
			IP.SetRotation(GameShape.rotation);
	}
}

// Hide all hitpoints..
function HideHitPoints()
{
	local SpellLessonInterpolationPoint IP;

	foreach AllActors(class'SpellLessonInterpolationPoint', IP)
	{
		if(IP.tag == GameIPStart.tag)
			IP.bHidden = true;
	}
}

// Reset hitpoints before each lesson level.
function ResetHitPoints()
{
	local SpellLessonInterpolationPoint IP;

    nHitpointsInLevel = 0;
    nHitpointHits     = 0;
    nHitPointsPassed  = 0;
	
	// Reset each IP and count up how many hitpoints there are for the current level.
	foreach AllActors(class'SpellLessonInterpolationPoint', IP)
	{
		if(IP.tag == GameIPStart.tag)
        {
			IP.Reset(nLevel);
            if (IP.IsInLevel(nLevel))
                nHitPointsInLevel++;
        }
	}
}

// Arrow key was pressed.  Handle hitpoints.
function DoArrowKeyPressed(ELessonKey LessonKey)
{
	local float fIPPrevDistance;
	local float fIPNextDistance;
	local float fIPClosestDistance;
	local SpellLessonInterpolationPoint IPTemp, IPPrev, IPNext, IPClosest;
    local float fSoundPitch;
    local int   nLastPosition;

    // Find the first active previous IP
    IPTemp = SpellLessonInterpolationPoint(GameWand.SplineManager.Dest.Prev);
    nLastPosition = IPTemp.Position;
    while (IPTemp != None)
    {
        if (IPTemp.Position > nLastPosition)
            break;
        if (IPTemp.IsActive(nLevel))
        {
            IPPrev = IPTemp;
            break;
        }
        else
        {
            nLastPosition = IPTemp.Position;
            IPTemp = SpellLessonInterpolationPoint(IPTemp.Prev);            
        }
    }

    // Find first active next IP
    IPTemp = SpellLessonInterpolationPoint(GameWand.SplineManager.Dest);
    nLastPosition = IPTemp.Position;
    while (IPTemp != None)
    {
        if (IPTemp.Position < nLastPosition)
            break;
        if (IPTemp.IsActive(nLevel))
        {
            IPNext = IPTemp;
            break;
        }
        else
        {   
            nLastPosition = IPTemp.Position;
            IPTemp = SpellLessonInterpolationPoint(IPTemp.Next);            
        }            
    }

    // If have both an active previous and next, use the previous.
    if ((IPPrev != None) && (IPNext != None))
    {
	    // Get distance from previous IPPoint to wand and from next IPPoint to wand.
	    fIPPrevDistance = VSize(IPPrev.Location - GameWand.Location);
	    // fIPNextDistance = VSize(IPNext.Location - GameWand.Location);

    	// Work with the IP point that's closer
	    //if (fIPPrevDistance < fIPNextDistance)
	    //{
		    fIPClosestDistance = fIPPrevDistance;
		    IPClosest = IPPrev;
	    //}
	    //else
	    //{
		//    fIPClosestDistance = fIPNextDistance;
		//    IPClosest = IPNext;
	    //}
    }

    // If only Previous is active.  Set it as closest.
    else if (IPPrev != None)
    {
        fIPClosestDistance = VSize(IPPrev.Location - GameWand.Location);
        IPClosest = IPPrev;
    }

    // If only Next is active.  Set it as closest.
    else if (IPNext != None)
    {
        fIPClosestDistance = VSize(IPNext.Location - GameWand.Location);
        IPClosest = IPNext;
    }

	// If have a closest IP.
	if (IPClosest != None)
	{
        // Check the arrow and range for hit/miss updates.
		if (KeyMatchesIPArrow(LessonKey, IPClosest) && fIPClosestDistance <= fHITPOINT_RANGE)
		{
			IPClosest.OnPlayerHit(nLevel);
			++nHitPointHits;
		}
		else
        {
            // Hit wrong key while in range of the arrow.
            if (fIPClosestDistance <= fHITPOINT_RANGE)
                IPClosest.OnPlayerMissed(nLevel);
                
            // Play miss if player is not on the hitpoint, but is very close.
            // For example, if player is holding the key down at the start, don't give the
            // miss until the get near the hitpoint (but still far enough away to be a miss).
            else if ((fIPClosestDistance > fHITPOINT_RANGE) && (fIPClosestDistance < fHITPOINT_MISS_RANGE))
                IPClosest.OnPlayerMissed(nLevel);			    
        }
	}
}

// Return true if the key passed in matches the IP's direction arrow.
function bool KeyMatchesIPArrow(ELessonKey LessonKey, SpellLessonInterpolationPoint IP)
{
    switch (LessonKey)
    {
    case (LessonKey_Up)    : return IP.IsDirectionArrowUp(nLevel);
    case (LessonKey_Down)  : return IP.IsDirectionArrowDown(nLevel);
    case (LessonKey_Left)  : return IP.IsDirectionArrowLeft(nLevel);
    case (LessonKey_Right) : return IP.IsDirectionArrowRight(nLevel);
    default :
        return (false);
    }
}

// RenderHudItems is called by HPHud once the spell lesson has been
// registered with it.  Only some states actually need to render the
// hud though.
function RenderHudItems(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
{
}

// Draw the spell lesson hud items to the hud canvas.
function DrawRoundText(Canvas canvas)
{
	local float  fScale;
	local font	 fontSave;
	local color	 colorSave;
	local color  colorText;
	local float	 fTextWidth, fTextHeight;

	fontSave  = canvas.font;
	colorSave = canvas.DrawColor;
	
	// Display "Round X" text in middle of screen.
	canvas.font = baseconsole(playerHarry.player.console).LocalBigFont;
	canvas.DrawColor.r = 255;
	canvas.DrawColor.g = 255;
	canvas.DrawColor.b = 0;

	canvas.TextSize(strCurrRound, fTextWidth, fTextHeight);
	canvas.SetPos((Canvas.SizeX/2) - (fTextWidth/2), (Canvas.SizeY/2) - (fTextHeight/2));
	canvas.DrawText(strCurrRound, false);

    // Restore canvas properties.
	canvas.Font      = fontSave;
	canvas.DrawColor = colorSave;
}

// Before a sequence of 1 or more SayLessonDialog calls, call this to bring in the
// cutscene borders.
function StartCutSequence()
{
	playerHarry.bIsCaptured = true;
	playerHarry.myHud.StartCutScene();
}

// After a sequence of SayLessonDialog calls, end with this to close cutscene borders.
function EndCutSequence()
{
	playerHarry.myHud.EndCutScene();
	playerHarry.bIsCaptured = false;
}

// Say the dialog corresponding to the id passed in.  Return how long sound
// is going to take.
function float SayLessonDialog(string strDialogID, bool bDisplayText)
{
	local string strDialog; 
	local sound  soundDialog;
	local float  fSoundLen;

	strDialog = Localize( "all", strDialogID,"HPdialog" );
	strDialog = HandleFacialExpression( strDialog, 0, true );

	soundDialog = Sound(DynamicLoadObject("AllDialog." $strDialogID, class'Sound'));
	
    // If the sound was loaded in, get the duration of it and play it.
	if(soundDialog != None )
	{
		fSoundLen = GetSoundDuration(soundDialog);
        if (Professor != None)
		    Professor.PlaySound(soundDialog, , , , 10000, , true);
        else
            Professor.PlaySound(soundDialog, , , , 10000, , true);
	}
    // Make up a duration if no sound
	else
    {
		fSoundLen = (Len(strDialog)*0.01)+3.0;
    }

    // Handle emotions
    if (Professor != None)
        strDialog = Professor.HandleFacialExpression( strDialog, fSoundLen);
    else
        strDialog = HandleFacialExpression( strDialog, fSoundLen);
    
    //playerHarry.ClientMessage("SayLessonDialog " $strDialog);

    // Show dialog in cut area for length of sound.
    if (bDisplayText)
	    playerHarry.MyHud.SetSubtitleText(strDialog, fSoundLen);

    // Return the sound length
	return (fSoundLen);
}

function SayTryAgain(bool bLastTimeAround)
{
    local int nTryLastTimeEntries;   // # "try again, last time" lines to pick from
    local int nTryAgainEntries;      // # "try again" lines to pick from
    local int i;                     // reused var

    // Some lines are geared toward saying "Try again for the last time".  So, if
    // we're on the last loop, try to use one of those (may or may not exist
    // for certain lessons).
    if (bLastTimeAround)
    {
        // Find out how many "try again, last time" lines exist for this lesson.
        for (i=0; i < ArrayCount(strTryOneLastTime); i++)
        {
            if (strTryOneLastTime[i] != "")
                ++nTryLastTimeEntries;
            else
                break;
        }
    }

    // If it's the last loop and there are 1 or more "try again, last time" entries,
    // play one of those lines (without text).
    if (bLastTimeAround && nTryLastTimeEntries > 0)
    {
        i = RandRange(0, (nTryLastTimeEntries - 1));
        SayLessonDialog(strTryOneLastTime[i], false);
    }            

    // It's either not the last loop or there are no "try again, last time" lines
    else
    {
        // Count how many "try again" lines there are to choose from
        for (i=0; i < ArrayCount(strTryAgain); i++)
        {
            if (strTryAgain[i] != "")
                ++nTryAgainEntries;
            else
                break;
        }        

        // If have 1 or more "try again" lines, play one
        if (nTryAgainEntries > 0)
        {
            i = RandRange(0, (nTryAgainEntries - 1));
            SayLessonDialog(strTryAgain[i], false);
        }            
    }
}

function SayEncouragement()
{
    local int  nDoingGoodEntries;       // # of possible doing good lines 
    local int  nEncourageAnyEntries;    // # of possible generic encourage lines
    local bool bDoingGood;              // True if we consider player to be doing good.
    local int  i;                       // reused var

    // If at least one hitpoint has passed.
    if (nHitPointsPassed > 1)
    {
        // Doing good means hitting all hitpoints or <all hitpoints - 1> so far.  The -1
        // is to cover cases where the last hitpoint has passed, the player hasn't hit
        // it yet, but is still within range and might hit it.
        if (nHitPointHits >= (nHitPointsPassed - 1))
            bDoingGood = true;
    }

    // Some encouragement lines are geared toward saying "Doing good".  So, if 
    // player is doing good, attempt to use one of those lines
    if (bDoingGood)
    {
        // Find out how many "doing good" lines exist for this lesson.
        for (i=0; i < ArrayCount(strEncourageDoingGood); i++)
        {
            if (strEncourageDoingGood[i] != "")
                ++nDoingGoodEntries;
            else
                break;
        }
    }

    // If player's doing good and there's one or more "doing good lines", 
    // play one of those lines (without text).
    if (bDoingGood && nDoingGoodEntries > 0)
    {
        i = RandRange(0, (nDoingGoodEntries - 1));
        SayLessonDialog(strEncourageDoingGood[i], false);
    }            

    // If the player is either not doing good or we don't have a doing good
    // line, try to find a generic encouragement line
    else
    {
        // Count how many encouragement lines there are to choose from
        for (i=0; i < ArrayCount(strEncourageAny); i++)
        {
            if (strEncourageAny[i] != "")
                ++nEncourageAnyEntries;
            else
                break;
        }        

        // If have 1 or more encourage lines, play one
        if (nEncourageAnyEntries > 0)
        {
            i = RandRange(0, (nEncourageAnyEntries - 1));
            SayLessonDialog(strEncourageAny[i], false);
        }            
    }
}

function ResetForNextLevel()
{
	ResetHitPoints();
    nCurrTimesAround = 0;
	GameWand.SetLocation(GameIPStart.Location);
    GameWand.bHidden = false;
}

function ResetForNextLoop()
{
	ResetHitPoints();
    GameWand.SetLocation(GameIPStart.Location);
}

// End the spell lesson
function EndLesson()
{
	// Hide the spell lesson stuff.
	GameWand.bHidden  = true;
	GameShape.bHidden = true;
    //GameBkgrd.bHidden = true;
	HideHitPoints();
	GameWand.Destroy();

    switch (LessonShape)
    {
    case (LessonShape_Rictusempra) : 
        playerHarry.AddToSpellBook( class'spellRictusempra'); 
        break;
    case (LessonShape_Skurge)      :
        playerHarry.AddToSpellBook( class'spellSkurge');
        break;
    case (LessonShape_Diffindo)    :
        playerHarry.AddToSpellBook( class'spellDiffindo');
        break;
    case (LessonShape_Spongify)    :
        playerHarry.AddToSpellBook( class'spellSpongify');
        break;
    default :
        log("ERROR: Unrecognized lesson shape.  Spell not added to wand " $LessonShape);
        break;
    }

	// We don't need to return the camera back to normal anymore. A cutscene
	// should be triggered at the end of each spell lesson now.  So, only
	// return the camera to normal if there is no event to send out.
	if (Event == 'None')
		SetupCamera(CS_BackToNormal);

	// Return Harry to normal
	playerHarry.EndSpellLearning();

	// Send out event (a cutscene should be setup to get this event).
	TriggerEvent( Event, none, none );

	// Back to just hanging around.
	GoToState('Idle');
}

function bool IsJoyPressed(ELessonKey LessonKey, float fAxisX, float fAxisY)
{
    local bool bRet;

    switch (LessonKey)
    {
    case (LessonKey_Up): 
        if (fAxisY > 0 && (Abs(fAxisY) > Abs(fAxisX)))
            bRet = true;
        break;
    case (LessonKey_Down):
        if (fAxisY < 0 && (Abs(fAxisY) > Abs(fAxisX)))
            bRet = true;
        break;
    case (LessonKey_Left):  
        if (fAxisX < 0 && (Abs(fAxisX) > Abs(fAxisY)))
            bRet = true;
        break;
    case (LessonKey_Right): 
        if (fAxisX > 0 && (Abs(fAxisX) > Abs(fAxisY)))
            bRet = true;
        break;
    default :
        bRet = false;
    }

    return (bRet);
}

//-----------------------------------------------------------------------------------
//  States
//-----------------------------------------------------------------------------------

//-----------------------------------------------------------------------------------
//  State Idle
//-----------------------------------------------------------------------------------
//
//  State before and after the lesson.  Sits there waiting for a Trigger event.

auto state Idle
{
	// If triggered, activate
	event Trigger(actor Other, pawn EventInstigator)
	{
		playerHarry.ClientMessage("Activate spelllesson trigger");
		Activate(Other, EventInstigator);
	}
}

//-----------------------------------------------------------------------------------
//  State GameIntro
//-----------------------------------------------------------------------------------
//
//  Show the shape and describe how things work.

state GameIntro
{

begin:
    // Show the shape
	GameShape.bHidden = false;
    //GameBkgrd.bHidden = false;

    // Set focus to the shape and fly over to it.
	SetupCamera(CS_GameShape);
	cam.DoFlyTo(GameCutCam.Location, cam.enumMoveType.MOVE_TYPE_EASE_TO, 3.0);
	sleep(3.0);

// Want the shape to fade in. Below was the attempt.  Does not work
/*
	GameShape.ScaleGlow = 0.1;
	while (GameShape.ScaleGlow < 1.0)
	{
		GameShape.ScaleGlow += 0.02;
		if (GameShape.ScaleGlow >= 1.0)
			GameShape.ScaleGlow = 1.0;
		playerHarry.ClientMessage("Glow " $GameShape.ScaleGlow);
		Sleep(0.1);
	}
*/

	// Begin professor's talk sequence.
    StartCutSequence();

    // Blah, blah, blah
	sleep(SayLessonDialog(strIntroDialogId, true));

	// Create and show the tracing wand.  Set speed to designer set property.
	GameWand = SpellLessonWand(FancySpawn(class'SpellLessonWand',,,GameIPStart.Location, GameShape.rotation));
	GameWand.SetParentLessonTrigger(self);

	// Professor describes wand and tracing.
	sleep(SayLessonDialog(strTutorial1aId, true));

    // Professor describes the hit arrows
    ResetHitPoints();	
	sleep(SayLessonDialog(strTutorial1bId, true));

    // 3 levels, must hit all arrows correctly
	sleep(SayLessonDialog(strTutorial1cId, true));

	// End cutsene stuff
    EndCutSequence();

    // Set up lesson variables and start the first level.
	ResetForNextLevel();
	GoToState('DeclareNewRound');
}

//-----------------------------------------------------------------------------------
//  State DeclareNewRound
//-----------------------------------------------------------------------------------

state DeclareNewRound
{
	// RenderHudItems.  Called by Hud every render cycle once SpellLesson
	// has been registered.
	function RenderHudItems(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
        if (strCurrRound != "")
		    DrawRoundText(canvas);
	}

	event BeginState()
	{
		strCurrRound = "";
	}

begin:
	switch (nLevel)
	{
	case (0) :
        strCurrRound = Localize( "all", strRound1Id ,"HPdialog" );
        strCurrRound = HandleFacialExpression( strCurrRound, 0, true);
        sleep(SayLessonDialog(strRound1Id, false));  
        break;
	case (1) :
        strCurrRound = Localize( "all", strRound2Id ,"HPdialog" );
        strCurrRound = HandleFacialExpression( strCurrRound, 0, true);
        sleep(SayLessonDialog(strRound2Id, false));  
        break;
	case (2) :
        strCurrRound = Localize( "all", strRound3Id ,"HPdialog" );
        strCurrRound = HandleFacialExpression( strCurrRound, 0, true);
        sleep(SayLessonDialog(strRound3Id, false));  
        break;
	default :
		playerHarry.ClientMessage("ERROR: Unknown spell lesson level");
		break;
	}

	GoToState('PlayGame');
}

//-----------------------------------------------------------------------------------
//  State PlayGame
//-----------------------------------------------------------------------------------
//
//  Do the lesson!

state PlayGame
{
	event PlayerInput( float fDeltaTime )
	{
		// Check state of up arrow.
		if (playerHarry.bSpellLessonUp != 0 || 
            IsJoyPressed(LessonKey_Up, playerHarry.aSpellLessonX, playerHarry.aSpellLessonY))
        {            
            if (nKeyNeedsReset[0] == 0)            // If up key doesn't need to be reset (release key),
			    DoArrowKeyPressed(LessonKey_Up);   // perform action for key press.
            nKeyNeedsReset[0] = 1;                 // Key will need to be released before performing 
        }                                          // action on another arrow.
        else
        {            
            nKeyNeedsReset[0] = 0;                 // Record that key has been released.
        }

        // Check state of down arrow.
        if (playerHarry.bSpellLessonDown != 0 || 
                 IsJoyPressed(LessonKey_Down, playerHarry.aSpellLessonX, playerHarry.aSpellLessonY))
        {
            if (nKeyNeedsReset[1] == 0)
                DoArrowKeyPressed(LessonKey_Down);
            nKeyNeedsReset[1] = 1;
        }
        else
        {
            nKeyNeedsReset[1] = 0;
        }

        // Check the state of the left arrow.
        if (playerHarry.bSpellLessonLeft != 0 || 
                 IsJoyPressed(LessonKey_Left, playerHarry.aSpellLessonX, playerHarry.aSpellLessonY))
        {
            if (nKeyNeedsReset[2] == 0)
                DoArrowKeyPressed(LessonKey_Left);
            nKeyNeedsReset[2] = 1;
        }
        else
        {
            nKeyNeedsReset[2] = 0;
        }


        // Check state of the right arrow
        if (playerHarry.bSpellLessonRight != 0 || 
                 IsJoyPressed(LessonKey_Right, playerHarry.aSpellLessonX, playerHarry.aSpellLessonY))
        {
            if (nKeyNeedsReset[3] == 0)
                DoArrowKeyPressed(LessonKey_Right);
            nKeyNeedsReset[3] = 1;
        }
        else
        {
            nKeyNeedsReset[3] = 0;
        }
	}

	event EndState()
	{
        playerHarry.StopAllMusic(0.5);
		GameWand.StopSound(soundWand, SLOT_Interact);		
	}

begin:	

    // Start background music.
    playerHarry.StopAllMusic(0);
	switch (LessonShape)
	{
	case (LessonShape_Rictusempra): 
    case (LessonShape_Diffindo):
        playerHarry.PlayMusic(strRICTU_DIFF_GAME_MUSIC, 0.5);
        break;
    case (LessonShape_Skurge):
    case (LessonShape_Spongify):
        playerHarry.PlayMusic(strSKURGE_SPONG_GAME_MUSIC, 0.5);
        break;
    default:
        break;
    }

	// Start the wand moving.
	GameWand.PlaySound(soundWand, SLOT_Interact, 1.0, true, TransientSoundRadius, 1.0, false, true);
	GameWand.StartWand(fWandSpeed);
}

state LevelOver
{
	// RenderHudItems.  Called by Hud every render cycle once SpellLesson
	// has been registered.
	function RenderHudItems(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		sgHousePts.RenderHudItemManager(canvas, bMenuMode, bFullCutMode, bHalfCutMode);
	}

	event BeginState()
	{
        // Setup housepoint group and item for use within this state.
		sgHousePts  = playerHarry.managerStatus.GetStatusGroup(class'StatusGroupHousepoints');
		siGryffPts = sgHousePts.GetStatusItem(class'StatusItemGryffindorPts');

        // Allow housepoints to draw during cutscene.
        sgHousePts.SetCutSceneRenderMode(true);
	}

    event EndState()
    {
        // Return housepoints to normal status of whether they should draw
        // or not during a cutscene.
        sgHousePts.SetCutSceneRenderModeToNormal();
    }

begin:
    // Hide the wand
    GameWand.bHidden = true;

	// Bring in cutscene borders.
    StartCutSequence();

	// If beat hit enough points
    if (nHitpointHits >= nHitpointsInLevel)
	{
		// Well done!  (now done with current level or entire lesson if on last level)
		if (nLevel < nNUM_LEVELS - 1)
			sleep(SayLessonDialog(strLevelCompleteId, true));
		else
        {
            // Start won music
            playerHarry.PlayMusic(strWON_MUSIC, 0.5);
            fMusicSleep = fWON_MUSIC_LEN;

            // Instructor congratulates Harry
            fDialogSleep = SayLessonDialog(strAllLevelsCompleteId, true);
        
            // If dialog longer than music
            if ((fMusicSleep + fEND_MUSIC_FADE_OUT) < fDialogSleep)
            {
                // Stop and fade the music after one time through
                sleep(fMusicSleep - fEND_MUSIC_FADE_OUT);
                playerHarry.StopAllMusic(fEND_MUSIC_FADE_OUT);

                // Let the dialog finish out
                sleep(fDialogSleep - (fMusicSleep - fEND_MUSIC_FADE_OUT));
            }

            // Music longer than dialog
            else
            {
                // Finish dialog out
                sleep(fDialogSleep);
                playerHarry.StopAllMusic(fEND_MUSIC_FADE_OUT);
                sleep(fEND_MUSIC_FADE_OUT);
            }
        }
	
		// X points awarded to Gryffindor!		
		sgHousePts.SetEffectTypeToPermanent();
		switch (nLevel)
		{
		case (0) :
			nAddHousePts = 5;
			fDialogSleep = SayLessonDialog(str5PointsAwardedId, true);
			break;
		case (1) :
			nAddHousePts = 10;
			fDialogSleep = SayLessonDialog(str10PointsAwardedId, true);
			break;
		case (2) :
			nAddHousePts = 15;
			fDialogSleep = SayLessonDialog(str15PointsAwardedId, true);
			break;
		default:
			fDialogSleep = 0;
			nAddHousePts = 0;
			break;
		}

		// Show the new housepoints	when half the dialog is finished and
		// keep the housepoints up until the other half is done.
		fDialogSleep /= 2.0;
		sleep(fDialogSleep);		
        playerHarry.PlaySound(soundAddHousePts, , , , , 10000, true);
    	while((nAddHousePts--) > 0)
	    {
    	    siGryffPts.IncrementCount(1);
		    Sleep(0.01);					 
        }
		playerHarry.StopSound(soundAddHousePts);
		sleep(fDialogSleep);
        EndCutSequence();

        ++nLevel;
		if (nLevel < nNUM_LEVELS)
		{
            // Setup hitpoints for new level
	    	ResetForNextLevel();

		    // Start next level
		    GoToState('DeclareNewRound');
        }

        else
            EndLesson();
	}

	// Player wasn't able to win the level, but they've done enough and we'll go on 
    // with the game.
	else
	{
        // Start fail music
        playerHarry.PlayMusic(strFAIL_MUSIC, 0.5);
        fMusicSleep = fFAIL_MUSIC_LEN;

        // Instructor says "move on anyway"
		fDialogSleep = SayLessonDialog(strMoveOnAnywayId, true);
        
        // If dialog longer than music
        if ((fMusicSleep + fEND_MUSIC_FADE_OUT) < fDialogSleep)
        {
            // Stop and fade the music after one time through
            sleep(fMusicSleep - fEND_MUSIC_FADE_OUT);
            playerHarry.StopAllMusic(fEND_MUSIC_FADE_OUT);

            // Let the dialog finish out
            sleep(fDialogSleep - (fMusicSleep - fEND_MUSIC_FADE_OUT));
        }

        // Music longer than dialog
        else
        {
            // Finish dialog out
            sleep(fDialogSleep);
            playerHarry.StopAllMusic(fEND_MUSIC_FADE_OUT);
            sleep(fEND_MUSIC_FADE_OUT);
        }

        EndCutSequence();
        EndLesson();		        
    }
}

defaultproperties
{
	bTriggerOnceOnly=true  
	soundWand=sound'HPSounds.Magic_sfx.spell_loop_nl'
	nLevel=0
	soundAddHousePts=sound'HPSounds.Menu_sfx.score_tally_up'
	InitialState=None
	bInitiallyActive=false
	bHidden=true
    fWandSpeed=100
    nMaxLoops(0)=5
    nMaxLoops(1)=4
    nMaxLoops(2)=3
}

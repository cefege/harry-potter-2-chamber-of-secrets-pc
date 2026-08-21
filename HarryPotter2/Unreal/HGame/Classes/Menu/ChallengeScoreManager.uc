//===============================================================================
//  ChallengeScoreManager
//
//  
//===============================================================================

class ChallengeScoreManager extends HudItemManager;

const DECREMENT_VALUE       = 1;       // Drop 1 point every second
const DECREMENT_SECONDS     = 1.0;         
const STAR_VALUE            = 200;     // Increase score by 100 when pick up a star

// Constants for "in progress" graphics.             
const strSCORE_ICON     = "HP_Menu.Hud.ChallengeScore";
const nSCORE_WIDTH      = 128;         // Width of score image (icon is 128x128, but image
                                       // may make use of that whole area.
const nCURR_SCORE_MIDX  = 31;          // Curr score text position (middle of text is here)
const nCURR_SCORE_MIDY  = 64;          
const nHIGH_SCORE_MIDX  = 91;		   // High score text position (middle of text)
const nHIGH_SCORE_MIDY  = 64;          

// Constants for graphics used in Tally phase
const strTALLY_SCORE_ICON     = "HP_Menu.Hud.BigChallengeScore";
const nTALLY_SCORE_WIDTH      = 128;   // Width of score image (icon is 128x128, but image
                                       // may make use of that whole area.
const nTALLY_SCORE_HEIGHT     = 128;    // Real height of tally score
const nTALLY_CURR_SCORE_MIDX  = 31;    // Curr score text position (middle of text is here)
const nTALLY_CURR_SCORE_MIDY  = 115;          
const nTALLY_HIGH_SCORE_MIDX  = 93;	   // High score text position (middle of text)
const nTALLY_HIGH_SCORE_MIDY  = 115; 

const strTALLY_POINTS_ICON    = "HP_Menu.Hud.BigHousepointsGryff";
const nTALLY_POINTS_WIDTH     = 128;   // Width of points image (icon is 128x128, but image
                                       // may make use of that whole area.
const nTALLY_POINTS_HEIGHT    = 128;   // Real height of points image
const nTALLY_POINTS_MIDX      = 65;    // Points text position (middle of text is here)
const nTALLY_POINTS_MIDY      = 87;          

const strBEANS_ICON           = "HP_Menu.Hud.BeanCounter";
const nBEANS_WIDTH            = 65;
const nBEANS_HEIGHT           = 76;

const strSTAR_ICON            = "HP_Menu.Hud.StarCounter";
const nSTAR_WIDTH             = 65;
const nSTAR_HEIGHT            = 76;

const nTALLY_ICONS_Y          = 20;    // Display Tally icons at this Y position

const nMASTER_BONUS_HPOINTS   = 50;    // If mastered, player gets 50 extra housepoints.

var Harry               playerHarry;            // Harry
var int                 nHighScore;             // High score for this level
var texture             textureScoreIcon;       // In progress score texture
var texture             textureTallyScoreIcon;  // Tally score texture
var texture             textureTallyPointsIcon; // Tally points texture
var texture             textureBeansIcon;       // Bean counter to display at end
var texture             textureStarIcon;        // Star counter to display at end
var int                 nCurrScore;             // Curr score for this challenge
var int                 nAwardGryffPoints;      // Points to award Gryffindor
var StatusItem          siGryffHousePoints;     // Reference to GryffindorStatusItem
var string              strTallyCue;            // For cutscene stuff
var sound               soundTally;             // Sound to play during tally.
var float               fTallySoundDuration;    // Lenghth of tally sound
var int                 nTallyPointsPerTick;    // Tally this many points each tick
var float               fTickDelta;             // Used to determine tally rate
var float               fTicksPerSec;           // Used to determine tally rate
var bool                bFirstTime;             // First time challenge has happened.
var bool                bMastered;              // Set after player has mastered challenge
var bool                bSentWarnTimeEvent;     // Sent out event that time is almost up

var(ChallengeManager) name nameChallengeLevel;  // Challenge level id
var(ChallengeManager) int  nStartScore;         // Player starts with this score
var(ChallengeManager) int  nMaxHousePoints;     // Max pts. can get for this challenge
var(ChallengeManager) int  nMaxScore;           // Score >= to this gives max points
var(ChallengeManager) name EventTimeUp;         // Event sent when time gets to 0,
                                                // but player has made to end in
                                                // previous attempts
var(ChallengeManager) name EventTimeUpRestart;  // Event sent when time gets to 0
                                                // & level MUST be restarted because
                                                // player has never made it to end
var(ChallengeManager) name EventRunningOutOfTime; // Event sent out when time almost up
var(ChallengeManager) int  nWarnTimeAlmostUp;   // When timer reaches this, 
                                                // EventRunningOutOfTime is sent out


//-----------------------------------------------------------------------------------
//  Functions
//-----------------------------------------------------------------------------------

// Dynamically load score texture when ChallengeScoreManager created.
event PostBeginPlay()
{
	Super.PostBeginPlay();

	// Load the score graphics.
	textureScoreIcon       = texture(DynamicLoadObject(strSCORE_ICON, class'Texture'));
	textureTallyScoreIcon  = texture(DynamicLoadObject(strTALLY_SCORE_ICON, class'Texture'));
	textureTallyPointsIcon = texture(DynamicLoadObject(strTALLY_POINTS_ICON, class'Texture'));
	textureStarIcon        = texture(DynamicLoadObject(strSTAR_ICON, class'Texture'));
	textureBeansIcon       = texture(DynamicLoadObject(strBEANS_ICON, class'Texture'));

	bSentWarnTimeEvent = false;
}

// Call to start the challenge.  This causes the challenge score graphic to appear and
// start counting down. 
function BeginChallenge()
{
	// Find and save off Harry.
	foreach AllActors(class'harry', playerHarry)
		break;

	// Register ourselves with the hud (the hud will pass us the canvas in PostRender).
	HPHud(playerHarry.myHud).RegisterChallengeManager(self);

	GoToState('ChallengeInProgress');
}

// End the challenge.
function EndChallenge()
{
    local HPawn foreachActor;

	// Once the challenge is over, allow no more damage from creatures.
    foreach AllActors( class'HPawn', foreachActor )
	    foreachActor.PlayerCutCapture();

    // Score values stay persistent, but do not display.
	GoToState('Idle');
}

// Start Tally process.
function TallyChallenge()
{
	GoToState('Tally');
}

// Called when ChallengeStar is picked up.  We should be in the ChallengeInProgress
// state when this function is called.  If not, somebody forgot to start the
// chanllenge.
function PickedUpStar()
{
	log("Error: Picked up star, but challenge has not been started");
}

// Final star calls this when picked up
function PickedUpFinalStar()
{
	EndChallenge();		
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
		return (true);
	}

	else 
	if( sActualCommand ~= "Release" )
	{
		return (true);
	}

	else
	if( sActualCommand ~= "BeginChallenge" )
	{
		BeginChallenge();
		CutNotifyActor.CutCue( cue );
		return (true);
	}
	else
	if( sActualCommand ~= "EndChallenge" )
	{
		EndChallenge();
		CutNotifyActor.CutCue( cue );
		return (true);
	}
	else
	if( sActualCommand ~= "TallyChallenge" )
	{
		// Save off cue so we can notify when tally is done.
		strTallyCue = cue;
		
		// Start tallying
		TallyChallenge();
		return (true);
	}

	else
		return (false);
}

// CutQuestion.  "If"  called from Cutscene script.
//
// Intended to be called at the beginning of a challenge level:
//
//		ChallengeIsFirstTime         - First time level has been encountered
//		ChallengePreviouslyBeaten    - Harry has previously made it to the end before time up
//		ChallengePreviouslyMastered  - Harry has previously mastered the level
//
// Intended to be called at the end of challenge level before tally occurs:
//
//		ChallengeWorseThanBefore     - Harry just did worse (or equal) to his previous best
//		ChallengeJustWonFirstTime    - Harry completed before time ran out for first time
//		ChallengeJustMastered        - Harry just mastered for first time
//		ChalllengeMissedStars        - Harry didn't pick up all stars
//		ChallengeNewBestScore        - Harry beat his previous best score
//		ChallengePreviouslyMastered  - Harry has previously mastered the level
//
// Note: ChallengePreviouslyMastered is a valid question either at the beginning of
//       the challenge level or at the end of the challenge before the tally.
function bool CutQuestion(string question)
{
	local StatusItem siStars;

	CutErrorString="";	//clear error string.

	if (question ~= "ChallengeIsFirstTime")
		return (bFirstTime);
	else if (question ~= "ChallengePreviouslyBeaten")
		return (!bFirstTime && (nHighScore > 0) && !bMastered);
	else if (question ~= "ChallengePreviouslyMastered")
		return (bMastered);
	else if (question ~= "ChallengeWorseThanBefore")
		return (nCurrScore <= nHighScore);
	else if (question ~= "ChallengeJustWonFirstTime")
		return ((nHighScore == 0) && (nCurrScore > 0));
	else if (question ~= "ChallengeJustMastered")
		return ((nHighScore < nMaxScore) && (nCurrScore >= nMaxScore));
	else if (question ~= "ChallengeMissedStars")
	{
		siStars = playerHarry.managerStatus.GetStatusItem(class'StatusGroupStars', 
		                                                   class'StatusItemStars');

		if (siStars != None)
		{
			// If no stars have been picked up, count and nMaxCount will be 0.
			// Make sure account for that case.
			if ((siStars.nCount == 0)   && (siStars.nMaxCount == 0))
				return (true);
			else
				return (siStars.nCount != siStars.nMaxCount);
		}
		else
			return (true);
	}
	else if (question ~= "ChallengeNewBestScore")
		return ((nCurrScore > nHighScore) && (nHighScore > 0));
	else
		return Super.CutQuestion(question);
}

// States override this function to get position of the score graphic.  When
// the challenge is in progress, the score is at the top of the screen.
// When the challenge is over and the score is being tallied, the score
// is in the middle of the screen.
function GetScorePosition(Canvas canvas, out int nIconX, out int nIconY)
{
	// Throw up error if this function is called.  It should be overridden
	// by each state that needs access to the score position.
	log("ERROR: states need to override GetScorePosition()");
}

// Get score hud icon placement for challenge in progress.  Place the score at 
// the top of the screen and in the middle.
function GetInProgressScorePosition(Canvas canvas, out int nIconX, out int nIconY)
{
	local float fScaleFactor;

	fScaleFactor = GetScaleFactor(canvas);

	nIconX = canvas.SizeX/2 - ((nSCORE_WIDTH/2) * fScaleFactor);
	nIconY = 5 * fScaleFactor;
}

// Get score hud icon placement for Tally mode.  Place score in the middle
// of the screen.
function GetTallyScorePosition(Canvas canvas, out int nIconX, out int nIconY)
{
	local float fScaleFactor;

	fScaleFactor = GetScaleFactor(canvas);

	nIconX = canvas.SizeX/2 - ((nTALLY_SCORE_WIDTH/2) * fScaleFactor);
	//nIconY = canvas.SizeY/2 - ((nTALLY_SCORE_HEIGHT/2) * fScaleFactor);
	nIconY = nTALLY_ICONS_Y * fScaleFactor;
}

// Get mid position for curr score text.  States should override.
function GetCurrScoreTextXY(out int nMidX, out int nMidY)
{
	log("ERROR: states need to override GetCurrScoreTextXY()");
}

// Get mid position for high score text.  States should override.
function GetHighScoreTextXY(out int nMidX, out int nMidY)
{
	log("ERROR: states need to override GetCurrScoreTextXY()");
}

// States override this function to get the score texture that should be used.
function texture GetScoreTexture()
{
	log("ERROR: states need to override GetScoreTexture()");
}

// Draw the current score.  This function is used for displaying both the
// in progress score and the tally version of the score.
function DrawScore(Canvas canvas, bool bMenuMode)
{
	local float  fScaleFactor;
	local int    nIconX, nIconY;
	local int    nMidX, nMidY;
	local color  colorSave;		// Save off original canvas draw color
	local font   fontSave;
	local string strCurrScore;
	local string strPrevHighScore;
	local float  nXTextLen, nYTextLen;

	// Don't draw challenge score while at menu
	if (bMenuMode)
		return;

	// Get scale for different screen resolutions.
	fScaleFactor = GetScaleFactor(canvas);

	// Place the challenge score graphic
	GetScorePosition(canvas, nIconX, nIconY);
	canvas.SetPos(nIconX,nIconY);
	canvas.DrawIcon(GetScoreTexture(), fScaleFactor);

	nXTextLen     = 0;
	nYTextLen     = 0;

	// Save off canvas properties
	colorSave = Canvas.DrawColor;
	fontSave  = Canvas.Font;

	// Build score strings
	strCurrScore     = string(nCurrScore); 
	strPrevHighScore = string(nHighScore);

    // Text will be offwhite
    Canvas.DrawColor.r = 206;
    Canvas.DrawColor.g = 200;
    Canvas.DrawColor.b = 190;

    // Get fot to use
    Canvas.Font = GetScoreFont(Canvas);

	// Place and draw curr score
	Canvas.TextSize(strCurrScore, nXTextLen, nYTextLen);
	GetCurrScoreTextXY(nMidX, nMidY);
	Canvas.SetPos(nIconX + (nMidX * fScaleFactor) - nXTextLen/2,
		          nIconY + (nMidY * fScaleFactor) - nYTextLen/2);
	Canvas.DrawText(strCurrScore, false);

	// Place and draw previous high score
	Canvas.TextSize(strPrevHighScore, nXTextLen, nYTextLen);
	GetHighScoreTextXY(nMidX, nMidY);
	Canvas.SetPos(nIconX + (nMidX * fScaleFactor) - nXTextLen/2,
		          nIconY + (nMidY * fScaleFactor) - nYTextLen/2);
	Canvas.DrawText(strPrevHighScore, false);

	// Restore canvas properties
	Canvas.DrawColor = colorSave;
	Canvas.Font      = fontSave;
}

function font GetScoreFont(Canvas canvas)
{
    local font fontRet;

    if (Canvas.SizeX <= 512)
		fontRet = baseConsole(playerHarry.player.console).LocalTinyFont;
	else if (Canvas.SizeX <= 640)
        fontRet = baseConsole(playerHarry.player.console).LocalSmallFont;
	else
		fontRet = baseConsole(playerHarry.player.console).LocalMedFont;

    return (fontRet);
}

// To calculate the housepoints for a given score, we use the following
// formula(s) created by Michael Lankerovich.  The idea is that we want
// housepoints to go up slightly for low score values.  For higher scores,
// the rate of increase of score to housepoints should increase.
// 
// The formulas below were based on a MaxHousepoint:MaxScore values of
// somewhere around 200:1260.  For each challenge, the max housepoint and the
// max score values may vary some, but it is assumed that the values
// will be somewhere around the test ratio of 200:1260.
//
// In the graph below, A is the MaxScore a player can get (they can get
// slightly higher, but MaxScore is the score where the maximum points
// will be awarded). B is the MaxHousepoints the player can get. For
// a score less than or equal to 1/3 A, the housepoint ratio to score goes 
// up slightly.  For a score greater than 1/3 A and less than 2/3 A, the
// housepoint to score ratio is slightly higher.  And, for a score 
// greater than 2/3 A and less than A, the housepoint to score ratio is
// even higher.
//         _______________________________X
//      B |                               |
//		 _|_                              |
//		  |                               |
//		 _|_                              |   Connect the X's to follow
//		  |                               |   the score:housepoint "curve"
//	  B/2 |_____________________X_________| 
//        |						|         |
//		 _|_					|         |
//		  |						|         |
//	  B/6 |________X____________|_________|
//        |        |            |         | 
//		  |________|____________|_________|
//       0        A/3        2A/3        A
//
//  
//   y = B/2A x;			if 0    <  x <= A/3
//   y = B/A x - B/6;		if A/3  <  x <= 2A/3
//   y = 3B/2A x - 1/2B;	if 2/3A <  x <= A

function int GetHousePointsFromScore(int nScore)
{
	local int A; 
	local int B;

	A = nMaxScore;
	B = nMaxHousePoints;

	if (nScore <= 0)
		return (0);
	else if (nScore < A/3)
		return ( (B*nScore) / (2*A) );
	else if (nScore < ((2*A) / 3))
		return ( ((B*nScore) / A) - (B/6));
	else if (nScore < A)
		return ( ((3*B*nScore) / (2*A)) - (B/2) );
	else
	{
		// Player got better than the amount we require for
		// maximum house points.
		return (B);
	}
}

//-----------------------------------------------------------------------------------
//  States
//-----------------------------------------------------------------------------------

//-----------------------------------------------------------------------------------
//  State Idle
//-----------------------------------------------------------------------------------

auto state Idle
{
}

//-----------------------------------------------------------------------------------
//  State ChallengeInProgress
//-----------------------------------------------------------------------------------
//
//  While the challenge is in progress, continuously count down the score.  When
//  a star is picked up, bump up the score.  When the final star is picked up, 
//  end the challenge.  
state ChallengeInProgress
{
	function Timer()
	{
		// Decerement score.
		if (nCurrScore > 0 && (baseHud(playerharry.myHud).bCutSceneMode == false))
			nCurrScore -= DECREMENT_VALUE;

		// If score reached 0, time up.
		if (nCurrScore <= 0)
		{
			nCurrScore = 0;

			// If haven't made it through level yet, must restart
			if (nHighScore == 0)
			{
				if (EventTimeUpRestart != 'None')
					TriggerEvent(EventTimeUpRestart, self, none);
			}

			// Time is up, but player has made it through level before
			else
			{
				if (EventTimeUp != 'None')
					TriggerEvent(EventTimeUp, self, none);
			}
		}

		// See if it's time to send out event warning that time is almost up.
		else if ((nWarnTimeAlmostUp != 0) && (nWarnTimeAlmostUp >= nCurrScore) && !bSentWarnTimeEvent)
		{
			bSentWarnTimeEvent = true;
			TriggerEvent(EventRunningOutOfTime, self, none);
		}
	}

	function PickedUpStar()
	{
		nCurrScore += STAR_VALUE;
	}

	// Get position of score icon for in progress state.
	function GetScorePosition(Canvas canvas, out int nIconX, out int nIconY)
	{
		GetInProgressScorePosition(canvas, nIconX, nIconY);
	}

	// Get score icon texture that is used for in progress state.
	function texture GetScoreTexture()
	{
		return (textureScoreIcon);
	}

	// Get mid position for curr score text. 
	function GetCurrScoreTextXY(out int nMidX, out int nMidY)
	{
		nMidX = nCURR_SCORE_MIDX;
		nMidY = nCURR_SCORE_MIDY;
	}

	// Get mid position for high score text. 
	function GetHighScoreTextXY(out int nMidX, out int nMidY)
	{
		nMidX = nHIGH_SCORE_MIDX;
		nMidY = nHIGH_SCORE_MIDY;		
	}

	// Called by HPHud.  Draw the Challenge Score.  Housepoints hud icon will
	// be rendered by StatusGroupHousepoints.
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		// Don't draw when cutscene is going on.
		if (!bFullCutMode)
			DrawScore(canvas, bMenuMode);
	}

	// State Intializations
	function BeginState()
	{
		nCurrScore = nStartScore;
		SetTimer(DECREMENT_SECONDS, true);
	}
}

//-----------------------------------------------------------------------------------
//  State Tally
//-----------------------------------------------------------------------------------
//
//  Tally the player's housepoints based on score.

state Tally
{
	// At each tick, bump up the player's previous high score (shown on the bottom)
	// until it reaches the player's current score (shown on the top).  Exit to 
	// the PostTallyHold state when the previous score has reached the current.
	function Tick(float fDeltaTime)
	{
		if (fTickDelta > 0.0)
		{
			if (nHighScore < nCurrScore)
			{
				// Increment high score
				nHighScore += nTallyPointsPerTick;
				if (nHighScore > nCurrScore)
					nHighScore = nCurrScore;
			}
			else
				GoToState('PostTallyHold');
		}
		else
			fTickDelta = fDeltaTime;
	}

	// Draw current tally status
	function RenderHudItemManager(Canvas canvas, bool bMenuMode,  bool bFullCutMode, bool bHalfCutMode)
	{
		// When tallying, draw even when cutscene is in progress because the tally
		// happens during a cutscene!
		DrawScore(canvas, bMenuMode);
	}

	// Get position of score icon for tally state.
	function GetScorePosition(Canvas canvas, out int nIconX, out int nIconY)
	{
		GetTallyScorePosition(canvas, nIconX, nIconY);
	}

	// Get score icon texture that is used for in tally state.
	function texture GetScoreTexture()
	{
		return (textureTallyScoreIcon);
	}

	// Get mid position for curr score text.
	function GetCurrScoreTextXY(out int nMidX, out int nMidY)
	{
		nMidX = nTALLY_CURR_SCORE_MIDX;
		nMidY = nTALLY_CURR_SCORE_MIDY;
	}

	// Get mid position for high score text.
	function GetHighScoreTextXY(out int nMidX, out int nMidY)
	{
		nMidX = nTALLY_HIGH_SCORE_MIDX;
		nMidY = nTALLY_HIGH_SCORE_MIDY;		
	}

	// Initialize for the tally process
	function BeginState()
	{
		local StatusGroup sgHousePoints;
		local float       fTallyPointsPerSec;

		fTickDelta = 0.0;

		// If didn't do better than last time no tally to perform- bail to the hold state.
		if (nCurrScore < nHighScore)
		{
			nAwardGryffPoints = 0;
			GoToState('PostTallyHold');
		}
		else
		{
			// Figure out how many the player's score is worth
			nAwardGryffPoints = GetHousepointsFromScore(nCurrScore);

			// Subtract off points received from previous performances.
			nAwardGryffPoints -= GetHousepointsFromScore(nHighScore);

            // If mastered this time for the first time, Extra points get awarded.
            if (!bMastered && (nCurrScore >= nMaxScore))
                nAwardGryffPoints += nMASTER_BONUS_HPOINTS;
		}
	}

	function EndState()
	{
		// Stop tally sound.
		StopSound(soundTally, SLOT_Interact);

		// After tally has completed, flag that the challenge has been done at least once.
		bFirstTime = false;

        // Record that we've now mastered.
        if (!bMastered && (nHighScore >= nMaxScore))
            bMastered = true;
	}

begin:
	// Calculate how many points to add per tick.
	while (fTickDelta <= 0.0)
		sleep(0.1);

	fTicksPerSec = 1.0 / fTickDelta;
	nTallyPointsPerTick = (nCurrScore - nHighScore) / (3.0 * fTicksPerSec);
	if (nTallyPointsPerTick < 1)
		nTallyPointsPerTick = 1;


	fTallySoundDuration = GetSoundDuration(soundTally);
loop:
	// Loop tally sound until leave the state (playing the sound from Harry because
	// the ChallengeScoreManager object may be a ways away and out of hearing range).
	playerHarry.PlaySound(soundTally, SLOT_Interact);
	Sleep(fTallySoundDuration);
	goto 'Loop';
}

//-----------------------------------------------------------------------------------
//  State PostTallyHold
//-----------------------------------------------------------------------------------
//
// After tally is done, leave the display up for a little while.

state PostTallyHold
{
	// When timer's up, go back to idle state.
	function Timer()
	{
		GoToState('PostTallyHoldPoints');
	}

	// Begin state- start the hold timer.
	function BeginState()
	{
		SetTimer(4.0, false);
	}

	// Get position of score icon for tally state.
	function GetScorePosition(Canvas canvas, out int nIconX, out int nIconY)
	{
		GetTallyScorePosition(canvas, nIconX, nIconY);
	}

	// Get score icon texture that is used for in tally State.
	function texture GetScoreTexture()
	{
		return (textureTallyScoreIcon);
	}

	// Get mid position for curr score text.
	function GetCurrScoreTextXY(out int nMidX, out int nMidY)
	{
		nMidX = nTALLY_CURR_SCORE_MIDX;
		nMidY = nTALLY_CURR_SCORE_MIDY;
	}

	// Get mid position for high score text.
	function GetHighScoreTextXY(out int nMidX, out int nMidY)
	{
		nMidX = nTALLY_HIGH_SCORE_MIDX;
		nMidY = nTALLY_HIGH_SCORE_MIDY;		
	}

	// Called by HPHud.  Draw the Challenge Score.  
	function RenderHudItemManager(Canvas canvas, bool bMenuMode,  bool bFullCutMode, bool bHalfCutMode)
	{
		// When tallying, draw even when cutscene is in progress because the tally
		// happens during a cutscene!
		DrawScore(canvas, bMenuMode);
	}
}

//-----------------------------------------------------------------------------------
//  State PostTallyHoldPoints
//-----------------------------------------------------------------------------------
//
// After tally is done and has been held for a little while, show the points that
// have been gained.

state PostTallyHoldPoints
{
	// When timer's up, go back to idle state.
	function Timer()
	{
		GoToState('Idle');
	}

	// Begin state- start the hold timer.
	function BeginState()
	{
		SetTimer(5.0, false);
	}

	// Called by HPHud.  Draw the Challenge Score.  
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		local string     strPoints;                    // # of points string
		local string     strBeans;                     // # of beans string
		local string     strStars;                     // # of stars string
		local int        nNumBeans;                    // # of beans
		local int        nNumStars;                    // # of stars 
		local float      fScaleFactor;                 
		local int        nPointsIconX, nPointsIconY;   // points icon position
		local int        nBeansIconX, nBeansIconY;     // beans icon position
		local int        nStarIconX, nStarIconY;       // star icon position
		local color      colorSave;		// Save off original canvas draw color
		local font       fontSave;
		local float      nXTextLen, nYTextLen;
        local StatusItem siHudItem;

		// Don't draw challenge score while at menu
		if (bMenuMode)
			return;

		// When tallying, draw even when cutscene is in progress because the tally
		// happens during a cutscene!

		// Get scale for different screen resolutions.
		fScaleFactor = GetScaleFactor(canvas);

		// Display points graphic horizontally in the middle and near the top.
		nPointsIconX = canvas.SizeX/2 - ((nTALLY_POINTS_WIDTH/2) * fScaleFactor);
		nPointsIconY = nTALLY_ICONS_Y * fScaleFactor;
		canvas.SetPos(nPointsIconX,nPointsIconY);
		canvas.DrawIcon(textureTallyPointsIcon, fScaleFactor);

		// Display bean counter graphic to the left of the points and centered
		// vertically to the points icon
		nBeansIconX = nPointsIconX - ((nBEANS_WIDTH + 30) * fScaleFactor);
		nBeansIconY = nPointsIconY + 
			         ((nTALLY_POINTS_HEIGHT/2) * fScaleFactor)  - 
					 ((nBEANS_HEIGHT/2) * fScaleFactor);
		canvas.SetPos(nBeansIconX,nBeansIconY);
		canvas.DrawIcon(textureBeansIcon, fScaleFactor);

		// Display star counter graphic to the right of the points and centered
		// vertically to the points icon.
		nStarIconX = nPointsIconX + ((nTALLY_POINTS_WIDTH + 30) * fScaleFactor);
		nStarIconY = nPointsIconY + 
			         ((nTALLY_POINTS_HEIGHT/2) * fScaleFactor)  - 
					 ((nSTAR_HEIGHT/2) * fScaleFactor);
		canvas.SetPos(nStarIconX,nStarIconY);
		canvas.DrawIcon(textureStarIcon, fScaleFactor);

		nXTextLen     = 0;
		nYTextLen     = 0;

		// Save off canvas properties
		colorSave = Canvas.DrawColor;
		fontSave  = Canvas.Font;

		// Get number of beans and stars
		nNumBeans = playerHarry.managerStatus.GetStatusItem(class'StatusGroupJellyBeans',
			                                                class'StatusItemJellyBeans').nCount;
		nNumStars = playerHarry.managerStatus.GetStatusItem(class'StatusGroupStars', 
			                                                class'StatusItemStars').nCount;

		// Build number string s
		strPoints = string(nAwardGryffPoints); 
		strBeans  = string(nNumBeans);
		strStars  = string(nNumStars);

		// Set draw color to black
		Canvas.DrawColor.r = 0;
		Canvas.DrawColor.g = 0;
		Canvas.DrawColor.b = 0;

		// Set font 
		Canvas.Font = baseConsole(playerHarry.player.console).LocalMedFont;

		// Place and draw points
		Canvas.TextSize(strPoints, nXTextLen, nYTextLen);
		Canvas.SetPos(nPointsIconX + (nTALLY_POINTS_MIDX * fScaleFactor) - nXTextLen/2,
			          nPointsIconY + (nTALLY_POINTS_MIDY * fScaleFactor) - nYTextLen/2);
		Canvas.DrawText(strPoints, false);

        // Place and draw bean count
        siHudItem = playerHarry.managerStatus.GetStatusItem(class'StatusGroupJellyBeans',
			                                                class'StatusItemJellyBeans');
        Canvas.DrawColor = siHudItem.GetCountColor();	
        Canvas.Font = siHudItem.GetCountFont(Canvas);
		Canvas.TextSize(strBeans, nXTextLen, nYTextLen);
		Canvas.SetPos(nBeansIconX + (siHudItem.nCountMiddleX * fScaleFactor) - nXTextLen/2,
			          nBeansIconY + (siHudItem.nCountMiddleY * fScaleFactor) - nYTextLen/2);
		Canvas.DrawText(strBeans, false);

		// Place and draw star count
        siHudItem = playerHarry.managerStatus.GetStatusItem(class'StatusGroupStars', 
			                                                class'StatusItemStars');
        Canvas.DrawColor = siHudItem.GetCountColor();
        Canvas.Font = siHudItem.GetCountFont(Canvas);
		Canvas.TextSize(strStars, nXTextLen, nYTextLen);
		Canvas.SetPos(nStarIconX + (siHudItem.nCountMiddleX * fScaleFactor) - nXTextLen/2,
			          nStarIconY + (siHudItem.nCountMiddleY * fScaleFactor) - nYTextLen/2);
		Canvas.DrawText(strStars, false);

		// Restore canvas properties
		Canvas.DrawColor = colorSave;
		Canvas.Font      = fontSave;
	}

	function EndState()
	{
		siGryffHousePoints = playerHarry.managerStatus.GetStatusItem(class'StatusGroupHousePoints',
										                             class'StatusItemGryffindorPts');
		siGryffHousePoints.IncrementCount(nAwardGryffPoints);

		CutNotifyActor.CutCue(strTallyCue);
	}
}

defaultproperties
{
	DrawType=DT_Sprite							// For editor drawing
	bHidden=true                                // Displays in editor, but not game
	nCurrScore=0
	nStartScore=1000
	nMaxHousePoints=200
	nMaxScore=1260
	CutName="ChallengeScoreManager"
	soundTally=sound'HPSounds.Menu_sfx.score_tally_up'
	bPersistent=true							
	bFirstTime=true
	EventTimeUp=ChallengeTimeUp
	EventTimeUpRestart=ChallengeTimeUpRestart
	EventRunningOutOfTime=ChallengeRunningOutOfTime
	nWarnTimeAlmostUp=100
}


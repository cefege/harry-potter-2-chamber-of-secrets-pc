//===============================================================================
//  QuidScoreManager
//
//  Handles display of the Quidditch score and tally display after a Quidditch
//  match ends.
//
//  The QuidScoreManager is designed to be used as follows:
//
//  1) Spawn the QuidScoreManager from the QuidditchDirector.  This can be
//     done in the startup of the Quidditch director or right before the score
//     needs to first display.
//  2) Call SetOpponent() from the QuidditchDirector.  This will setup the correct
//     score texture.
//  3) Call StartQuidScore() from the QuidditchDirector to start the display of the
//     score.
//  4) As the Quidditch score changes, the QuidditchDirector can use the score
//     functions to update the hud display:  SetGryffindorScore(), 
//     SetOpponentScore(), IncrementGryffindorScore(), DecrementGryffindorScore().
//  5) If the score ever needs to be hidden (will automtically be hid during a
//     cutscene), PauseQuidScore() and ResumeQuidScore() can be used.
//  6) Call TallyQuidScore() directly from the QuidditchDirector or by using the
//     TallyQuidScore cutscene command to convert the Quidditch score to
//     housepoints (along with a visual tally display).
//  7) After the tally is complete, the score display will no longer appear.
//  8) QuidditchDirector should destroy the QuidScoreManager on shutdown of
//     the QuidditchDirector itself.
//
//  If you want to call TallyQuidScore() from a cutscene instead of the
//  Quidditch director, here's how you'd do it:
//
//      Capture QuidScoreManager
//      QuidScoreManager TallyQuidScore
//      Release QuidScoreManager
//
//  (Note: The cue passed into TallyQuidScore is sent back once the tally 
//         process completes.)
//
//===============================================================================

class QuidScoreManager extends HudItemManager;

// Constants for "in progress" graphics.             
const strSCORE_ICON     = "HP_Menu.Hud.ChallengeScore";
const nSCORE_WIDTH      = 128;         // Width of score image (icon is 128x128, but image
                                       // may make use of that whole area.
const nGRYFFINDOR_SCORE_MIDX  = 32;    // Gryffindor's score text position (middle of text is here)
const nGRYFFINDOR_SCORE_MIDY  = 62;          
const nOPPONENT_SCORE_MIDX    = 98;	   // Opponent's score text position
const nOPPONENT_SCORE_MIDY    = 62;          

// Constants for graphics used in Tally phase
const strTALLY_POINTS_ICON    = "HP_Menu.Hud.BigHousepointsGryff";
const nTALLY_POINTS_WIDTH     = 128;   // Width of points image (icon is 128x128, but image
                                       // may make use of that whole area.
const nTALLY_POINTS_HEIGHT    = 128;   // Real height of points image
const nTALLY_POINTS_MIDX      = 65;    // Points text position (middle of text is here)
const nTALLY_POINTS_MIDY      = 87;          

// Screen location of tally
const nTALLY_DISPLAY_Y        = 20;

enum OpponentHouse	                   // Same order as QuidditchDirector
{                                    
	Opponent_Gryffindor,
	Opponent_Ravenclaw,
	Opponent_Hufflepuff,
    Opponent_Slytherin,
};

var Harry               playerHarry;            // Harry
var int                 nGryffindorScore;       // Gryffindor's current score
var int                 nOpponentScore;         // Opponent's current score
var texture             textureScoreIcon;       // In progress score texture
var texture             textureTallyPointsIcon; // Tally points texture shown at end
var int                 nAwardGryffPoints;      // Points to award Gryffindor
var StatusItem          siGryffHousePoints;     // Reference to GryffindorStatusItem
var string              strTallyCue;            // For cutscene stuff
var sound               soundTally;             // Sound to play during tally.
var float               fTallySoundDuration;    // Lenghth of tally sound
var int                 nTallyPointsPerTick;    // Tally this many points each tick
var float               fTickDelta;             // Used to determine tally rate
var float               fTicksPerSec;           // Used to determine tally rate
var OpponentHouse       Opponent;               // Used to determine opponent score texture

//-----------------------------------------------------------------------------------
//  Functions
//-----------------------------------------------------------------------------------

// Dynamically load score texture when QuidScoreManager created.
event PostBeginPlay()
{
	Super.PostBeginPlay();

	// Load the score graphics.
	textureScoreIcon       = texture(DynamicLoadObject(strSCORE_ICON, class'Texture'));
	textureTallyPointsIcon = texture(DynamicLoadObject(strTALLY_POINTS_ICON, class'Texture'));
}

// This timer keeps going off until we have access to the hud (so we can register with it).
function Timer()
{
	// Find and save off Harry.
	foreach AllActors(class'harry', playerHarry)
		break;

    if (playerHarry.myHud != None)
    {
	    // Register ourselves with the hud (the hud will pass us the canvas in PostRender).
	    HPHud(playerHarry.myHud).RegisterQuidScoreManager(self);

        SetTimer(0.0, false);
    }
}

// Call to start the score display. 
function StartQuidScore()
{
    // Will be registered with hud within the timer.  We do the registering within the timer so
    // we can keep retrying until the player's hud has been created.
    SetTimer(0.2, true);

    nGryffindorScore = 0;
    nOpponentScore   = 0;
	GoToState('QuidScoreDisplay');
}

// Pause the display.
function PauseQuidScore()
{
	// Score values stay persistent, but do not display.
	GoToState('Idle');
}

// Resume the display
function ResumeQuidScore()
{
    GoToState('QuidScoreDisplay');
}

// Start Tally process.
function TallyQuidScore()
{
	GoToState('Tally');
}

// Set Gryffindor's opponent.  Currently, we don't use this for anything, but the intent
// is that we'll use different score textures for different opponents (final textures
// have not been created yet).
function SetOpponent(OpponentHouse NewOpponent)
{
    Opponent = NewOpponent;
}

// Set Gryffindor's score to the score passed in.
function SetGryffindorScore(int nScore)
{
    nGryffindorScore = nScore;

    // Make sure score never dips below zero
    if (nGryffindorScore < 0)
        nGryffindorScore = 0;
}

// Set opponent's score to the score passed in
function SetOpponentScore(int nScore)
{
    nOpponentScore = nScore;

    // Make sure score never dips below zero
    if (nOpponentScore < 0)
        nOpponentScore = 0;
}

// Increment (or decrement if you want) Gryffindor's score
function IncrementGryffindorScore(int nScore)
{
    SetGryffindorScore(nGryffindorScore + nScore);
}

// Increment (or decrement if you want) Opponent's score
function IncrementOpponentScore(int nScore)
{
    SetOpponentScore(nOpponentScore + nScore);
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
	if( sActualCommand ~= "StartQuidScore" )
	{
		StartQuidScore();
		CutNotifyActor.CutCue( cue );
		return (true);
	}
	else
	if( sActualCommand ~= "TallyQuidScore" )
	{
		// Save off cue so we can notify when tally is done.
		strTallyCue = cue;
		
		// Start tallying
		TallyQuidScore();
		return (true);
	}

	else
		return (false);
}

// Draw the current score.  This function is used for displaying both the
// in progress score and the tally version of the score.
function DrawQuidScore(Canvas canvas)
{
	local float  fScaleFactor;
	local int    nIconX, nIconY;
	local int    nMidX, nMidY;
	local color  colorSave;		// Save off original canvas draw color
	local font   fontSave;
	local string strGryffScore;
	local string strOpponentScore;
	local float  nXTextLen, nYTextLen;

	// Save off canvas properties
	colorSave = Canvas.DrawColor;
	fontSave  = Canvas.Font;

	// Get scale for different screen resolutions.
	fScaleFactor = GetScaleFactor(canvas);

    // Place in upper right corner
	nIconX = canvas.SizeX - (nSCORE_WIDTH * fScaleFactor) - (5 * fScaleFactor);
	nIconY = 5 * fScaleFactor;
	canvas.SetPos(nIconX,nIconY);
	canvas.DrawIcon(textureScoreIcon, fScaleFactor);

	nXTextLen     = 0;
	nYTextLen     = 0;

	// Build score strings
	strGryffScore     = string(nGryffindorScore); 
	strOpponentScore  = string(nOpponentScore);

	// Set draw color to off-white
	Canvas.DrawColor.r = 206;
	Canvas.DrawColor.g = 200;
	Canvas.DrawColor.b = 190;

	// Set font 
	Canvas.Font = baseConsole(playerHarry.player.console).LocalMedFont;

	// Place and draw Gryffindor score
	Canvas.TextSize(strGryffScore, nXTextLen, nYTextLen);
	Canvas.SetPos(nIconX + (nGRYFFINDOR_SCORE_MIDX * fScaleFactor) - nXTextLen/2,
		          nIconY + (nGRYFFINDOR_SCORE_MIDY * fScaleFactor) - nYTextLen/2);
	Canvas.DrawText(strGryffScore, false);

	// Place and draw opponent score
	Canvas.TextSize(strOpponentScore, nXTextLen, nYTextLen);
	Canvas.SetPos(nIconX + (nOPPONENT_SCORE_MIDX * fScaleFactor) - nXTextLen/2,
		          nIconY + (nOPPONENT_SCORE_MIDY * fScaleFactor) - nYTextLen/2);
	Canvas.DrawText(strOpponentScore, false);

	// Restore canvas properties
	Canvas.DrawColor = colorSave;
	Canvas.Font      = fontSave;
}

// Draw big housepoints icon with current housepoints
function DrawTallyHousepoints(Canvas canvas)
{
	local string strPoints;                    // # of points string
	local float  fScaleFactor;                 
	local int    nPointsIconX, nPointsIconY;   // points icon position
	local color  colorSave;		// Save off original canvas draw color
	local font   fontSave;
	local float  nXTextLen, nYTextLen;

	// Save off canvas properties
	colorSave = Canvas.DrawColor;
	fontSave  = Canvas.Font;

	// Get scale for different screen resolutions.
	fScaleFactor = GetScaleFactor(canvas);

	// Display points graphic horizontally in the middle and near the top.
	nPointsIconX = canvas.SizeX/2 - ((nTALLY_POINTS_WIDTH/2) * fScaleFactor);
	nPointsIconY = nTALLY_DISPLAY_Y * fScaleFactor;
	canvas.SetPos(nPointsIconX,nPointsIconY);
	canvas.DrawIcon(textureTallyPointsIcon, fScaleFactor);

	nXTextLen     = 0;
	nYTextLen     = 0;

	// Build number string s
	strPoints = string(siGryffHousePoints.nCount); 

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

	// Restore canvas properties
	Canvas.DrawColor = colorSave;
	Canvas.Font      = fontSave;
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
//  State QuidScoreDisplay
//-----------------------------------------------------------------------------------
//
//  While in this state, the score is displayed during the Quidditch match.

state QuidScoreDisplay
{
	// Called by HPHud.  
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		// Don't draw when at menu or when cutscene is going on.
		if (!bFullCutMode && !bMenuMode)
			DrawQuidScore(canvas);
	}
}

//-----------------------------------------------------------------------------------
//  State Tally
//-----------------------------------------------------------------------------------
//
//  Tally the player's housepoints based on score.

state Tally
{
	// At each tick, increment the player's house points.  Exit to the PostTallyHold
    // state when all points have been awarded.
	function Tick(float fDeltaTime)
	{
        // If Tick has already been called once, do the tally process.
		if (fTickDelta > 0.0)
		{
            // If still have more points to award.
			if (nAwardGryffPoints > 0)
			{
                // If have more than nTallyPointsPerTick left to award
                if (nTallyPointsPerTick <= nAwardGryffPoints)
                {
                    // Update Gryff status item with new points
                    siGryffHousePoints.IncrementCount(nTallyPointsPerTick);

                    // Points remaining to award
                    nAwardGryffPoints -= nTallyPointsPerTick;
                }
                // Award all remaining points if have fewer than nTallyPointsPerTick left
                else
                {
                    // Update Gryff status item with new points
                    siGryffHousePoints.IncrementCount(nAwardGryffPoints);

                    // All points have been awarded
                    nAwardGryffPoints = 0;
                }
			}
			else
				GoToState('PostTallyHold');
		}

        // First time tick has been called in this state- save off the delta time.
        // This value is used in "begin:" to figure out the tally rate.
		else
			fTickDelta = fDeltaTime;
	}

	// While tallying housepoints, display the score
	function RenderHudItemManager(Canvas canvas, bool bMenuMode,  bool bFullCutMode, bool bHalfCutMode)
	{
		// When tallying, don't draw if at menu, but draw during cutscene is in progress 
        // because the tally happens during a cutscene!
        if (!bMenuMode)
        {
		    DrawQuidScore(canvas);

            DrawTallyHousepoints(canvas);
        }
	}

	// Initialize for the tally process
	function BeginState()
	{
		local StatusGroup sgHousePoints;
		local float       fTallyPointsPerSec;

		fTickDelta = 0.0;

		// If Gryffindor didn't win, no housepoints to add- bail to the hold state.
		if (nGryffindorScore < nOpponentScore)
		{
			nAwardGryffPoints = 0;
			GoToState('PostTallyHold');
		}

        // Gryffindor gets however many points they were ahead by.
		else
			nAwardGryffPoints = nGryffindorScore - nOpponentScore;

        // Get Gryffindor house points status item.
		siGryffHousePoints = playerHarry.managerStatus.GetStatusItem(class'StatusGroupHousePoints',
										                             class'StatusItemGryffindorPts');		
	}

	function EndState()
	{
		// Stop tally sound.
		StopSound(soundTally, SLOT_Interact);
	}

begin:
	// fTickDelta gets set in Tick().  Wait here until fTickDelta has been set.
    // Once set, we know how much time there is between ticks and can then
    // calculate the rate at which points should be added.
	while (fTickDelta <= 0.0)
		sleep(0.1);

    // Figure out how many points we need to increment per tick in order to have
    // the tally complete in about 3 seconds.
	fTicksPerSec = 1.0 / fTickDelta;
    if ((nAwardGryffPoints / (3.0 * fTicksPerSec)) > 1)
        nTallyPointsPerTick = nAwardGryffPoints / (3.0 * fTicksPerSec);
    else
		nTallyPointsPerTick = 1;

    playerHarry.clientmessage("start tally " $nAwardGryffPoints $" " $nTallyPointsPerTick $ " " $fTickDelta $" " $fTicksPerSec);

	fTallySoundDuration = GetSoundDuration(soundTally);
loop:
	// Loop tally sound until leave the state (playing the sound from Harry because
	// the QuidScoreManager object may be a ways away and out of hearing range).
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
		GoToState('Idle');
	}

	// Called by HPHud.  Draw the Challenge Score.  
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		// When tallying, don't draw if at menu, but draw during cutscene is in progress 
        // because the tally happens during a cutscene!
        if (!bMenuMode)
        {
		    DrawQuidScore(canvas);

            DrawTallyHousepoints(canvas);
        }
	}

	// Begin state- start the hold timer.
	function BeginState()
	{
		SetTimer(4.0, false);
	}

    // End state, send back cue to cutscene
    function EndState()
    {
		CutNotifyActor.CutCue(strTallyCue);

       	// Unregister ourselves with the hud (the hud will pass us the canvas in PostRender).
    	HPHud(playerHarry.myHud).RegisterQuidScoreManager(None);
    }
}

defaultproperties
{
	DrawType=DT_Sprite							// For editor drawing
	bHidden=true                                // Displays in editor, but not game
	CutName="QuidScoreManager"
	soundTally=sound'HPSounds.Menu_sfx.score_tally_up'
    Opponent=Opponent_Slytherin
}


//===============================================================================
//  [MuggleMeterManager] 
//
//  The Muggle Observation Meter is a graphic with a bar that rises when the
//  player is close to muggles and lowers when muggles are out of range.
//  The muggle meter itself does not detect muggles, it just updates the 
//  observation meter based on weighted values passed in.  In addition, 
//  there is an eye on the slider graphic that opens as the bar gets higher.
//
//  To use the MuggleMeterManager:
//
//     1) Place a MuggleMeterManager object in the desired level and then
//        a "foreach AllActors" can be used to find the MuggleMeterManager
//        in code (like from a director).  Alternatively, an object like
//        a director can spawn the MuggleMeterManager.
//     2) Start display of the meter by calling BeginDetection()
//     3) Make the slider bar rise at a constant rate by calling
//        MugglesInRange(nWeight).  A weight of 1 is the slowest.  There is
//        no code limited ceiling to the weight, but you'll probably want to
//        use values between 1 and approximately 5.
//     4) Make the slider bar lower at a constant rate by calling
//        MugglesOutOfRange(nWeight).
//
//  When the slider bar is at the top, whatever event is in the 
//  MuggleMeterManager's Events\Event property will be sent out.  The meter
//  will also stop displaying.
//
//  The Muggle Observation Meter can be forced to go away by calling EndDetection.
//  If EndDetection is called, no event will be sent out.
//
//  If a MuggleMeterManager object is placed in a level and bDisplayAtLevelLoad 
//  property is set to true, BeginDetection() will be called automatically when 
//  the level loads.  At this point, the meter will be displayed with the 
//  slider bar at its lowest position (no muggles).  The slider bar will remain 
//  at its lowest position until the  MugglesInRange()/MugglesOutOfRange() 
//  functions are used.
// 
//===============================================================================

class MuggleMeterManager extends HudItemManager;

// Names of muggle meter textures
const strEYE1      = "HP_Menu.Hud.MuggleEye1";
const strEYE2      = "HP_Menu.Hud.MuggleEye2";
const strEYE3      = "HP_Menu.Hud.MuggleEye3";
const strEYE4      = "HP_Menu.Hud.MuggleEye4";
const strBAR_EMPTY = "HP_Menu.Hud.MuggleBarEmpty";
const strBAR_FULL  = "HP_Menu.Hud.MuggleBarFull";

// Muggle meter graphic constants
const fBAR_W = 31.0;	// Width of muggle bar
const fBAR_H = 128.0;   // Height ar muggle bar
const fBAR_Y = 8.0;     // Display muggle meter at this vertical pos

const fEYE_W                  = 64.0;
const fEYE_POINTER_YOFFSET    = 45.0;
const fBAR_FULL_EXTRA         = 14.0;

const fMETER_X = 25;
const fMETER_Y = 25;

// Update meter ever tenth of a second.
const fUPDATE_RATE = 0.1;

// Good ol' Harry
var Harry   playerHarry;

// Muggle meter textures- get loaded dynamically
var texture textureEye1;
var texture textureEye2;
var texture textureEye3;
var texture textureEye4;
var texture textureBarEmpty;
var texture textureBarFull;

// Current eye texture to display-- is assigned one of the textures above as
// the meter moves.
var texture textureCurrEye;

// Need to know when harry's accessible so we can register with the hud.
var bool bHarryAvailable;

// How much to move the bar at a time
var float fMovement;

// Current fullness of bar
var float fBarFullAmount;

// Elapsed time since last time meter was updated
var float fTimeSinceLastUpdate;

// Level accessible properties
var() bool bDisplayAtLevelLoad;

event Tick(float fDelta)
{
	if (!bHarryAvailable)
	{
		// Harry has been created for the level.  We can now register with the hud
		if (Level.PlayerHarryActor != None)
		{
			playerHarry = Harry(Level.PlayerHarryActor);
			bHarryAvailable = true;
			if (bDisplayAtLevelLoad)
				BeginDetection();
		}
	}
}

// Dynamically load score texture when ChallengeScoreManager created.
event PostBeginPlay()
{
	Super.PostBeginPlay();

	// Load the score graphics.
	textureEye1     = texture(DynamicLoadObject(strEYE1     , class'Texture'));
	textureEye2     = texture(DynamicLoadObject(strEYE2     , class'Texture'));
	textureEye3     = texture(DynamicLoadObject(strEYE3     , class'Texture'));
	textureEye4     = texture(DynamicLoadObject(strEYE4     , class'Texture'));
	textureBarEmpty = texture(DynamicLoadObject(strBAR_EMPTY, class'Texture'));
	textureBarFull  = texture(DynamicLoadObject(strBAR_FULL , class'Texture'));
}

// Start display and muggle detection.
function BeginDetection()
{
	HPHud(playerHarry.myHud).RegisterMuggleMeter(self);

	GoToState('DetectMuggles');
}

// Stop display and end mugge detection.
function EndDetection()
{
	HPHud(playerHarry.myHud).RegisterMuggleMeter(None);

	GoToState('Idle');
}

// Call when player is getting close to muggles.
function MugglesInRange(int nWeight)
{
	// nMovement is how many units to move the cloud bar each time.
	fMovement = nWeight;
}

// Call when player moves away from muggles.
function MugglesOutOfRange(int nWeight)
{
	// nMovement is how many units to move the cloud bar each time.
	fMovement = -nWeight;
}

//-----------------------------------------------------------------------------------
//  States
//-----------------------------------------------------------------------------------

//-----------------------------------------------------------------------------------
//  State Idle
//-----------------------------------------------------------------------------------

auto state idle
{
}

//-----------------------------------------------------------------------------------
//  State DetectMuggles
//-----------------------------------------------------------------------------------
//
//  Meter display is updated while in this state.

state DetectMuggles
{
	function Tick(float fDelta)
	{
		local float fPercentFull;

		// If cutscene is playing, no calculations.
		if (baseHud(playerharry.myHud).bCutSceneMode == true)
			return;

		// Figure out if we need to update the meter yet. If not time yet, bail out.
		fTimeSinceLastUpdate += fDelta;
		if (fTimeSinceLastUpdate < fUPDATE_RATE)
			return;

		// Time to update.  Reset the update time.
		fTimeSinceLastUpdate = 0.0;

		// Upate amount of the bar that's full
		fBarFullAmount += fMovement;

		// Don't dip below the bottom of the bar
		if (fBarFullAmount < 0)
			fBarFullAmount = 0;

		// If reached the top of the bar		
		if (fBarFullAmount >= fBAR_H)
		{
			// Let anyone who cares know that the time is up.
			TriggerEvent(Event, none, none );

			// Stop display of muggle meter
			EndDetection();
		}

		// Set the current eye slider to use based on how much of the bar is used up.
		else
		{
			fPercentFull = fBarFullAmount / fBAR_H;
			if (fPercentFull < 0.25)
				textureCurrEye = textureEye1;
			else if (fPercentFull < 0.50)
				textureCurrEye = textureEye2;
			else if (fPercentFull < 0.75)
				textureCurrEye = textureEye3;
			else
				textureCurrEye = textureEye4;
		}
	}

	// RenderHudItemManager.  Called by Hud every render cycle.  
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		local float fScaleFactor;		// amount to scale based on screen res
		local float fBarEmptyX;         // place empty bar here
		local float fBarEmptyY;
		local float fBarFullX;
		local float fBarFullY;
		local float fEyeX;
		local float fEyeY;
		local float fBarFullH;
		local float fBottomOfBar;

		// Get scale for different screen resolutions.
		fScaleFactor = GetScaleFactor(canvas);

		// Display all of the empty bar
		fBarEmptyX = (fMETER_X * fScaleFactor);
		fBarEmptyY = (fMETER_Y * fScaleFactor);
		canvas.SetPos(fBarEmptyX,fBarEmptyY);
		canvas.DrawIcon(textureBarEmpty, fScaleFactor);

		// Calculate bottom position of bar
		fBottomOfBar = fBarEmptyY + (fBAR_H * fScaleFactor);

		// Display proper portion of the full bar
		fBarFullH = fBarFullAmount + fBAR_FULL_EXTRA;
		if (fBarFullH >= fBAR_H)			    // displaying all of the bar
		{
			canvas.SetPos(fBarEmptyX,fBarEmptyY);
			canvas.DrawIcon(textureBarFull, fScaleFactor);
		}
		else                                    // displaying only part of the bar
		{
			canvas.SetPos(fBarEmptyX, fBottomOfBar - (fBarFullH * fScaleFactor));
			canvas.DrawTile(textureBarFull, 
							textureBarFull.USize * fScaleFactor,
							fBarFullH * fScaleFactor,
							0,
							fBAR_H - fBarFullH,
							textureBarFull.USize,
							fBarFullH);
		}

		// Display the eye slider
		fEyeX = fBarEmptyX - (((fEYE_W - fBAR_W) / 2) * fScaleFactor);
		fEyeY = fBottomOfBar - (fBarFullAmount * fScaleFactor) - (fEYE_POINTER_YOFFSET * fScaleFactor);
		canvas.SetPos(fEyeX, fEyeY);
		canvas.DrawIcon(textureCurrEye, fScaleFactor);
	}

	event BeginState()
	{
		textureCurrEye = textureEye1;   // start with eye closed
		fBarFullAmount = 0;             // bar starts at bottom
	}

begin:
	// For testing
/*
	MugglesInRange(1);
	sleep(9.0);
	MugglesOutOfRange(1);
	sleep(8.0);
	MugglesInRange(2);
*/
}


defaultproperties
{
	DrawType=DT_Sprite							// For editor drawing
	bHidden=true                                // Displays in editor, but not game
}


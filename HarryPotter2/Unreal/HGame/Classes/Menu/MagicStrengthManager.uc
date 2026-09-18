//===============================================================================
//  [MagicStrengthManager] 
//
//  Magic strength is represented by a bar and a slider moving up and down the
//  bar.  The higher the slider, the more magic strength remaining.
//
//  To use the MagicStrengthManager:
//
//  1) Either place a MagicStrengthManager object in the level or spawn
//     the object from script.  If you want an event to be sent out when
//     the strength is gone, setup the Events\Event property.
//  2) The display of the magic strength bar will start when StartMagicStrength
//     is called.  (This gets called automatically if the object is placed in
//     the level and bDisplayAtLevelLoad is set to true).
//  3) To take away strength, call UseUpStrength(int nPercent).  For example, 
//     UseUpStrength(5) would use up 5% of the total possible strength.  
//  4) Strength is automatically recovered at a rate setup by the constant
//     fRECOVER_RATE.
//  5) When the remaining strength gets down to 0, the event in Events\Event
//     will be sent out.
// 
//===============================================================================

class MagicStrengthManager extends HudItemManager;

// Names of muggle meter textures
const strSLIDER    = "HP_Menu.Hud.MagicStrengthSlider";
const strBAR_EMPTY = "HP_Menu.Hud.MagicStrengthEmpty";
const strBAR_FULL  = "HP_Menu.Hud.MagicStrengthFull";

// Constants for graphic placement
const fBAR_W = 36.0;	// Width of magic strength bar
const fBAR_H = 128.0;   // Height of magic strength bar
const fBAR_X = 25.0;    // Display magic strength bar at this horizontal pos
const fBAR_Y = 25.0;    // Display magic strength bar at this vertical pos
const fSLIDER_W               = 128; // Slider centered on 128 unit canvas
const fSLIDER_POINTER_YOFFSET = 66;  // Bottom of slider graphic

// Total strength units
const fTOTAL_STRENGTH  = 120; // Total strength is number of bar units that we
                              // use-- don't use all because lightningbolt
                              // slider isn't flat along the bottom

// Recover 1 unit every 10th of a second (1.0 / 10)
const fRECOVER_RATE = 0.1;  

// Strength units left.
var float   fRemainingStrength;

// Textures- get loaded dynamically
var texture textureSlider;
var texture textureBarEmpty;
var texture textureBarFull;

// Need to know when harry's accessible so we can register with the hud.
var bool bHarryAvailable;

// Elapsed time since last time strength was recovered
var float fTimeSinceLastRecover;

// Good ol' Harry
var Harry   playerHarry;

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
				StartMagicStrength();
		}
	}
}

// Dynamically load textures when manager created.
event PostBeginPlay()
{
	Super.PostBeginPlay();

	// Load the score graphics.
	textureSlider     = texture(DynamicLoadObject(strSLIDER    , class'Texture'));
	textureBarEmpty   = texture(DynamicLoadObject(strBAR_EMPTY , class'Texture'));
	textureBarFull    = texture(DynamicLoadObject(strBAR_FULL  , class'Texture'));
}

// Start display.
function StartMagicStrength()
{
	HPHud(playerHarry.myHud).RegisterMagicStrength(self);

	GoToState('DisplayStrength');
}

// Stop display.
function EndMagicStrength()
{
	HPHud(playerHarry.myHud).RegisterMagicStrength(None);

	GoToState('Idle');
}

// Call to lower magic strength by nPercent of the total possible strength.
function UseUpStrength(int nPercent)
{
	fRemainingStrength -= (fTOTAL_STRENGTH * (float(nPercent) / 100.0));
	playerHarry.ClientMessage("remaining strength " $fRemainingStrength);
	if (fRemainingStrength < 0)
		fRemainingStrength = 0;
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
//  State DisplayStrength
//-----------------------------------------------------------------------------------
//
//  Strength display is updated while in this state.

state DisplayStrength
{
	function Tick(float fDelta)
	{
		local float fPercentFull;

		// If cutscene is playing, no calculations.
		if (baseHud(playerharry.myHud).bCutSceneMode == true)
			return;

		// If ran out of strength, send out event
		if (fRemainingStrength <= 0)
		{
			// Let anyone who cares know that the time is up.
			TriggerEvent(Event, none, none );

			// Stop display of strength
			EndMagicStrength();
		}

		// If it's time to recover strength
		fTimeSinceLastRecover += fDelta;
		if (fTimeSinceLastRecover >= fRECOVER_RATE)
		{
			// Time to update.  Reset the update time.
			fTimeSinceLastRecover = 0.0;

			// Upate amount of the bar that's full.  Recover at least 1 unit.  May recover
			// more if recover rate is very small and our last tick was greater
			// than fRECOVER_RATE seconds ago.
			fRemainingStrength += (1 + fTimeSinceLastRecover - fRECOVER_RATE);
			if (fRemainingStrength > fTOTAL_STRENGTH)
				fRemainingStrength = fTOTAL_STRENGTH;
		}
	}

	// RenderHudItemManager.  Called by Hud every render cycle.  
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		local float fScaleFactor;		// amount to scale based on screen res
		local float fBarScaledX;         // place bar here
		local float fBarScaledY;
		local float fSliderX;			// place slider here
		local float fSliderY;
		local float fBarEmptyH;			// Height of empty bar

		// Get scale for different screen resolutions.
		fScaleFactor = GetScaleFactor(canvas);

		// Display all of the full bar
		fBarScaledX = (fBAR_X * fScaleFactor);
		fBarScaledY = (fBAR_Y * fScaleFactor);
		canvas.SetPos(fBarScaledX,fBarScaledY);
		canvas.DrawIcon(textureBarFull, fScaleFactor);

		fBarEmptyH = fTOTAL_STRENGTH - fRemainingStrength;
		canvas.SetPos(fBarScaledX, fBarScaledY);
		canvas.DrawTile(textureBarEmpty,
						textureBarEmpty.USize * fScaleFactor,
						fBarEmptyH * fScaleFactor,
						0,
						0,
						textureBarEmpty.USize,
						fBarEmptyH);

		// Display the slider
		fSliderX = fBarScaledX - (((fSLIDER_W - fBAR_W) / 2) * fScaleFactor);
		fSliderY = (fBAR_Y + fBarEmptyH - fSLIDER_POINTER_YOFFSET) * fScaleFactor;
		canvas.SetPos(fSliderX, fSliderY);
		canvas.DrawIcon(textureSlider, fScaleFactor);
	}

	event BeginState()
	{
		fRemainingStrength = fTOTAL_STRENGTH;
	}

begin:
/*
	// For testing
	playerHarry.ClientMessage("45");
	UseUpStrength(45);
	sleep(3.0);

	playerHarry.ClientMessage("85");
	UseUpStrength(40);
	sleep(10.0);

	UseUpStrength(60);
	sleep(1.0);
	UseUpStrength(60);
*/
}


defaultproperties
{
	DrawType=DT_Sprite							// For editor drawing
	bHidden=true                                // Displays in editor, but not game
	fRemainingStrength=fTOTAL_STRENGTH
}


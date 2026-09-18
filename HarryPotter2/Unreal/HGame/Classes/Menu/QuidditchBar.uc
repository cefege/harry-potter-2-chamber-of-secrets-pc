//===============================================================================
//  [QuidditchBar] 
//
//  The QuidditchBar handles the drawing of Quidditch progress bar.
//
//  To use the QuidditchBar
//
//  1) Spawn a QuiddtichBar object from script.
//  2) Call object.Show(true) to show display.
//  3) Call SetProgress(nPercent) to set the percentage "full" to display.
//     (0 to 100 - starts at 0 by default).
//  4) Call object.Show(false) to hide display.
//
//  5) Note:  SetProgress has 2 optional parameters bShowRed and 
//     fFadeRedOutSeconds.  These parameters cause the bar to be tinted
//     red and fade back to normal over the specified seconds.
//
//  Example of starting up QuidditchBar from script:
//
//      var QuidditchBar QBar;
//
//     	QBar = QuidditchBar(FancySpawn(class'QuidditchBar'));
//		QBar.Show(true);
// 
//===============================================================================

class QuidditchBar extends HudItemManager;

// Names of QuidditchBar textures
const strBAR_EMPTY    = "HP_Menu.Hud.QuidditchBarEmpty";   // empty bar
const strBAR_PURPLE   = "HP_Menu.Hud.QuidditchBarPurple";  // full bar

const fBAR_W          = 117.0;   // width of actual bar part of graphic
const fBAR_H          =  20.0;   // height of actual bar part of graphic
const fBAR_START_X    =   5.0;   // bar offset within graphic
const fBAR_START_Y    =  48.0;

const fSCREEN_OVER_FROM_RIGHT_X =  132.0; // Display Y units from right edge
const fSCREEN_UP_FROM_BOTTOM_Y  =   80.0; // Display at Y units up from bottom

var texture textureBarEmpty;     // Empty bar graphic
var texture textureBarPurple;    // Full slider
var bool    bRegisteredWithHud;  // True when hud knows about us
var int     nPercentFull;        // Percent full to display (0 - 100)

var float   fFadeRedTotalSecs;   // Tint bar red for this long
var float   fFadeRedCurrSecs;    // Seconds that red tint has been applied


// Dynamically load textures when manager created.
event PostBeginPlay()
{
	Super.PostBeginPlay();

	// Load emptybar
	textureBarEmpty  = texture(DynamicLoadObject(strBAR_EMPTY, class'Texture'));
    textureBarPurple = texture(DynamicLoadObject(strBAR_PURPLE, class'Texture'));
}

// Show/Hide
function Show(bool bShow)
{   
    if (bShow)
	    GoToState('DisplayQBar');
    else
        GoToState('Idle');
}

// Set percentage full to display (0 - 100).  Can optionally have the bar flash
// red and then fade out for a specified period of time.
function SetProgress(int nPercentFullIn, optional bool bShowRed, optional float fFadeRedOutSeconds)
{
    Clamp(nPercentFullIn, 0, 100);  // Force percent to be from 0 to 100
    nPercentFull = nPercentFullIn;  // Save off new percent setting

    if (bShowRed)
    {
        fFadeRedTotalSecs = fFadeRedOutSeconds;
        fFadeRedCurrSecs  = 0;
    }
}

// Guarantee we're not registered with the hud when destroyed.
event Destroyed()
{
    HPHud(Harry(Level.PlayerHarryActor).myHud).RegisterQuidditchBar(None);
    Super.Destroyed();
}

// Get draw color for bar (may be tinted red)
function color GetBarDrawColor()
{
    local color colorRet;
    local float fGreenAndBlue;

    // If not fading, all colors are in ful force
    if (fFadeRedTotalSecs == 0)
    {
        colorRet.R = 255;
        colorRet.G = 255;
        colorRet.B = 255;
    }

    // If fading
    else
    {
        // Red always in full force
        colorRet.R = 255;

        // Green and Blue gradually go from 0 to 255
        fGreenAndBlue = 255 * (fFadeRedCurrSecs/fFadeRedTotalSecs);
        colorRet.G = fGreenAndBlue;
        colorRet.B = fGreenAndBlue;
    }

    return (colorRet);
}

//-----------------------------------------------------------------------------------
//  States
//-----------------------------------------------------------------------------------

//-----------------------------------------------------------------------------------
//  State Idle
//-----------------------------------------------------------------------------------
//
//  State when bar is not displaying.  No calculations performed and no drawing.

auto state idle
{
}

//-----------------------------------------------------------------------------------
//  State DisplayQBar
//-----------------------------------------------------------------------------------
//
//  Quidditch progress bar is updated while in this state.

state DisplayQBar
{
	event Tick(float fDelta)
	{
        // Keep trying to register until the hud becomes available.  This is so
        // caller can setup the bar in PreBeginPlay if they want and the bar will
        // still get registered with the hud.
		if (!bRegisteredWithHud)
		{
			if (level.PlayerHarryActor.myHud != None)
			{
				// Set things up so the hud will call us.
				HPHud(Harry(level.PlayerHarryActor).myHud).RegisterQuidditchBar(self);
				bRegisteredWithHud = true;
			}
		}

        // While fFadeRedTotalSecs > 0, we want to perform red fade handling
        if (fFadeRedTotalSecs > 0)
        {
            // fFadeRedCurrSecs counts up until it reaches fFadeRedTotalSecs. When
            // fFadeRedCurrSecs reaches fFadeRedTotalSecs, the fade is done and
            // both vars get reset to indicate no fading in progress.
            if (fFadeRedCurrSecs >= fFadeRedTotalSecs)
            {
                fFadeRedCurrSecs  = 0;
                fFadeRedTotalSecs = 0;
            }

            // If fFadeRedCurrSecs has not yet reached fFadeRedTotalSecs, increase
            // its time by fDelta.  Make sure time does not go beyond fFadeRedTotalSecs.
            else
            {
                fFadeRedCurrSecs += fDelta;
                fFadeRedCurrSecs = fClamp(fFadeRedCurrSecs, 0, fFadeRedTotalSecs);
            }
        }
	}

	// RenderHudItemManager.  Called by Hud every render cycle.  
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		local float fScaleFactor;		// Amount to scale based on screen res
		local float fIconX;             // Place bar here
		local float fIconY; 
		local float fFullRatio;         // How much to fill up (0 - 1.0)
		local float fSegmentWidth;      // Portion of full bar to show
        local color colorSave;          // Original canvas drawcolor

        colorSave = Canvas.DrawColor;    
    
		// Get scale for different screen resolutions.
		fScaleFactor = GetScaleFactor(canvas);

		// Display all of the empty bar at left-bottom of screen.
		fIconX = canvas.SizeX - (fScaleFactor * fSCREEN_OVER_FROM_RIGHT_X);
		fIconY = canvas.SizeY - (fScaleFactor * fSCREEN_UP_FROM_BOTTOM_Y);
		canvas.SetPos(fIconX,fIconY);
		canvas.DrawIcon(textureBarEmpty, fScaleFactor);

		// Get enemy health (0 to 1.0)
        fFullRatio = float(nPercentFull) / 100.0;
		fFullRatio = fclamp(fFullRatio, 0, 1.0);

        // Get full bar color (may be tinted red if requested by last SetProgress() call).
        Canvas.DrawColor = GetBarDrawColor();

        // Draw the full part of the bar.
		fSegmentWidth = fFullRatio * (fBAR_W);
		Canvas.SetPos(fIconX + (fBAR_START_X * fScaleFactor), fIconY + (fBAR_START_Y * fScaleFactor));
		Canvas.DrawTile(textureBarPurple,
						fSegmentWidth * fScaleFactor,
			            textureBarPurple.VSize * fScaleFactor,
						0,
						0,
						fSegmentWidth,
						textureBarPurple.VSize);

        // Restore canvas draw color.
        canvas.DrawColor = colorSave;
	}

    // When not displayed, unregister from hud
    event EndState()
    {
        bRegisteredWithHud = false;
        HPHud(Harry(Level.PlayerHarryActor).myHud).RegisterQuidditchBar(None);
    }
}


defaultproperties
{
	DrawType=DT_Sprite							
	bHidden=true
    nPercentFull=0
}


//===============================================================================
//  [StatusItemHealth] 
//===============================================================================

class StatusItemHealth extends StatusItem;

const TOP_OFFSET     = 2;		// actual health indication starts here
const BOTTOM_OFFSET  = 11;      // actual health indication starts at icon height - this

const nHEALTH_X = 5;            // Icon displayed at 5,5
const nHEALTH_Y = 5;
const nSPACE_BETWEEN_ROWS = 4;  // 4 pixels separate health rows

// Change point constants (lose or gain health)
const CHANGE_BASE_HOLD     = 0.2;   // For 1 change point, hold red/green for this many secs
const CHANGE_PERPOINT_HOLD = 0.055; // After the first point, hold for this many secs
                                    // per point (so change of 70 means hold ~4 secs)

const CHANGE_BASE_FADE     = 0.2;   // For 1 change point, fade out for this long after hold
const CHANGE_PERPOINT_FADE = 0.026; // After the first point, fade for this many secs
                                    // per point (so change of 70 means fade ~2 secs)

var texture textureHealthBorder;
var texture textureHealthOrangeInside;
var texture textureHealthBlack;

var int     nCurrChange;		// Change in heath that just incurred
var float   fTotalFadeTime;     // Fade out time for change
var float   fCurrFadeTime;      // Current time while fading change out
var int     nUnitsPerIcon;
var int     nMaxIcons;

// Load some stuff at object initilization.
event PreBeginPlay()
{
	local string strHealthBorder;
	local string strHealthGreenInside;
	local string strHealthRedInside;
	local string strHealthOrangeInside;
    local string strHealthBlack;

	// Status item will handle loading textureHudIcon based on the strHudIcon we
	// set in the default properties.
	Super.PreBeginPlay();

	// Health uses more than one icon, load those here.
	strHealthBorder          = "HP_Menu.Hud.HealthBorder";
    strHealthBlack           = "HP_Menu.Hud.HealthBlack";
	strHealthOrangeInside    = "HP_Menu.Hud.HealthOrangeInside";

	textureHealthBorder       = texture(DynamicLoadObject(strHealthBorder, class'Texture'));
    textureHealthBlack        = texture(DynamicLoadObject(strHealthBlack, class'Texture'));
	textureHealthOrangeInside = texture(DynamicLoadObject(strHealthOrangeInside, class'Texture'));
}

// Add/subtract health
function IncrementCount(int nNum)
{
	Super.IncrementCount(nNum);

	// Save off the change in health
	nCurrChange = nNum;
	GoToState('HoldChange');
}

function IncrementCountPotential(int nNum)
{
	Super.IncrementCountPotential(nNum);

    // When get new potential, filler' up
    if (nNum > 0)
        nCount = nCurrCountPotential;
	//nCurrChange = nNum;   // commented out so don't see green when get new health potential
	//ToState('HoldChange');
}

// Return canvas color to use when drawing in the potential area.  Default is white-
// states that want to tint will override.
function color GetHealthDrawColor()
{
	local color colorReturn;

	colorReturn.r = 255;
	colorReturn.g = 255;
	colorReturn.b = 255;

	return (colorReturn);
}

function color GetChangeInHealthDrawColor()
{
	local color colorReturn;

	colorReturn.r = 0;
	colorReturn.g = 0;
	colorReturn.b = 0;

	return (colorReturn);
}

// Draw health status
function DrawItem(Canvas canvas, int nCurrX, int nCurrY, float fScaleFactor)
{
	local int     nX, nY;								// Draw position
	local float   fSegmentHeight;                       // Segment of icon height
	local float   fSegmentStartAt;                      // Offset for a segment
	local int     nTotalOffsets;                        // Area not part of calculations
	local texture textureEmpty;                         // texture to use for potential area
	local color   colorSave;                            // original canvas color
    local byte    byStyleSave;
    local int     i;
    local int     nNumHealthIcons;
    local int     nRemainingCount;
    local float   fFillRatio;
    local float   fCurrBarBeforeEffect;
    local float   fCurrBarAfterEffect;
		
	// Intialize everything
	nX              = 5 * fScaleFactor;			      // Will draw at (5,5) taking
	nY              = 5 * fScaleFactor;               // scale factor into account
	fSegmentHeight  = 0.0;							  // Clear segment height
	fSegmentStartAt = 0.0;                            // Clear start at
	nTotalOffsets = TOP_OFFSET + BOTTOM_OFFSET;       // Total vertical not part of calcs
    
    nNumHealthIcons = nCurrCountPotential/nUnitsPerIcon;
    if ((nCurrCountPotential % nUnitsPerIcon) > 0)
        ++nNumHealthIcons;
    nRemainingCount = nCount;
    
    // Save off canvas props
    byStyleSave = canvas.Style;
    colorSave   = canvas.DrawColor;

    for (i=0; i<nNumHealthIcons; i++)
    {
        if (i < 3)
            nY = nHEALTH_X * fScaleFactor;
        else
            nY = (nHEALTH_Y + nActualIconH + nSPACE_BETWEEN_ROWS) * fScaleFactor;

        if (i == 3)
            nX = nHEALTH_X * fScaleFactor;

        if (nRemainingCount >= nUnitsPerIcon)
            fFillRatio = 1.0;
        else
            fFillRatio = float(nRemainingCount)/float(nUnitsPerIcon);        

        nRemainingCount -= nUnitsPerIcon;
        if (nRemainingCount < 0)
            nRemainingCount = 0;

        canvas.DrawColor.R = 255;
        canvas.DrawColor.G = 255;
        canvas.DrawColor.B = 255;

        // Draw complete translucent black health
        canvas.Style = 3;
        Canvas.SetPos(nX, nY);
        Canvas.DrawIcon(textureHealthBlack,fScaleFactor);

        // Draw border
        canvas.Style = byStyleSave;
        Canvas.SetPos(nX, nY);
        Canvas.DrawIcon(textureHealthBorder,fScaleFactor);

        // If fading in or out, tint the "orange inside" for the health bars being affected.
        if (!IsInState('NormalDisplay'))
        {
            //fCurrBarBeforeEffect  = float(nCount - nCurrChange)/float(nUnitsPerIcon);
            fCurrBarAfterEffect = float(nCount)/float(nUnitsPerIcon);            
            //if ((fCurrBarBeforeEffect >= i) && (fCurrBarBeforeEffect <= (i+1)))
            //    canvas.DrawColor = GetHealthDrawColor();
            if ((fCurrBarAfterEffect >= i) && (fCurrBarAfterEffect <= (i+1)))
                canvas.DrawColor = GetHealthDrawColor();
        }

	    // Draw the health segment with the "orange inside" texture (draw from bottom 
	    // icon up to the desired height).
        if (fFillRatio > 0)
        {
	        fSegmentHeight = fFillRatio * (textureHudIcon.VSize - nTotalOffsets);
	        fSegmentStartAt = textureHudIcon.VSize - BOTTOM_OFFSET - fSegmentHeight;
	        fSegmentHeight  += BOTTOM_OFFSET;
            Canvas.SetPos(nX, nY + (fSegmentStartAt*fScaleFactor));
	        Canvas.DrawTile(textureHealthOrangeInside, 
		                    textureHudIcon.USize * fScaleFactor,  // scale x to this
			        	    fSegmentHeight * fScaleFactor,        // scale y to this
				        	0, 
					        fSegmentStartAt, 
					        textureHudIcon.USize,				  // draw all of width
					        fSegmentHeight);                      // draw this much of height
        }

        nX += (nActualIconW * fScaleFactor);
    }

	// Restore canvas settings
	canvas.DrawColor = colorSave;
}

//-----------------------------------------------------------------------------------
//  States
//-----------------------------------------------------------------------------------

//-----------------------------------------------------------------------------------
//  State NormalDisplay
//-----------------------------------------------------------------------------------
//
//  Default display state for health bar (non-change state).

auto state NormalDisplay
{
	function BeginState()
	{
		nCurrChange = 0;
	}
}

//-----------------------------------------------------------------------------------
//  State HoldChange
//-----------------------------------------------------------------------------------
//
//  Hold the display in a change state (show potential health area as red if loss
//  and green if there is a gain).

state HoldChange
{
	// When hold time is up, go to state that will fade the red back to normal.
	function Timer()
	{
		GoToState('FadeChangeOut');
	}

	// While in HoldChange state, the potential area of the healthbar should be 
	// all red if losing health or green if gaining health.
	function color GetHealthDrawColor()
	{
		local color colorReturn;

		// Getting damage, use red.
		if (nCurrChange < 0)
		{
			colorReturn.r = 255;
			colorReturn.g = 0;
		}

		// Gaining health, use green.
		else
		{
			colorReturn.r = 0;
			colorReturn.g = 255;
		}

		// No blue if gaining or losing health.
		colorReturn.b = 0;

		return (colorReturn);
	}

	function color GetChangeInHealthDrawColor()
	{
		local color colorReturn;

		colorReturn.r = 255;
		colorReturn.g = 255;
		colorReturn.b = 255;

		return (colorReturn);
	}

	// Get time to stay in this date.  Length of hold depends on change received.
	function float GetHoldChangeTime()
	{
        local float fHoldTime;
		// Hold CHANGE_BASE_HOLD seconds for the first point and then 
        // CHANGE_PERPOINT_HOLD for each change point after that.
		fHoldTime = ((Abs(nCurrChange) - 1) * CHANGE_PERPOINT_HOLD) + CHANGE_BASE_HOLD;

        // Hold max of 5 seconds
        if (fHoldTime > 5.0)
            fHoldTime = 5.0;

        return (fHoldTime);
	}

	// When state starts up, begin a timer that will expire when we should
	// leave this state.
	function BeginState()
	{
		log("Hold time " $GetHoldChangeTime());
		SetTimer(GetHoldChangeTime(), false);
	}
}

//-----------------------------------------------------------------------------------
//  State FadeChangeOut
//-----------------------------------------------------------------------------------
//
//  Fade out of the change state.  Go from full red/green back to normal colors.

state FadeChangeOut
{
	// Every tick fade out a little more.  When done with fade out, go to 
	// NormalDisplay state.
	event Tick(float fDelta)
	{
		local float fBlueGreen;
	
		// If time has not expired yet.
		if (fCurrFadeTime <= fTotalFadeTime)
		{
			// Increment time.
			fCurrFadeTime += fDelta;

			// If time has expired, go to NormalDisplay State.
			if (fCurrFadeTime >= fTotalFadeTime)
			{
				fCurrFadeTime = fTotalFadeTime;
				GoToState('NormalDisplay');
			}
		}
	}

	// Get canvas red tint color based on how much we've faded out.
	function color GetHealthDrawColor()
	{
		local color colorReturn;
		local float fFade;

		// Get color fade value.
		fFade = 255 * (fCurrFadeTime / fTotalFadeTime);

		// If took damage, fade blue and green back up to full 255 value.
		if (nCurrChange <= 0)
		{
			colorReturn.r = 255;
			colorReturn.g = fFade;
			colorReturn.b = fFade;
		}

		// If gained health, fade red and blue back up to full 255 value.
		else
		{
			colorReturn.r = fFade;
			colorReturn.g = 255;
			colorReturn.b = fFade;
		}

		return (colorReturn);
	}

	function color GetChangeInHealthDrawColor()
	{
		local color colorReturn;
		local float fColor;

		// Go from 0 to 255 over time.
		fColor = 255 - (255 * (fCurrFadeTime / fTotalFadeTime));

		colorReturn.r = fColor;
		colorReturn.g = fColor;
		colorReturn.b = fColor;

		return (colorReturn);
	}

	// Get time that should be taken to fade back out.  Fade time
	// is relative to the change in health.
	function float GetFadeChangeTime()
	{
        local float fChangeTime;

		// Fade CHANGE_BASE_FADE seconds for the first point and then 
        // CHANGE_PERPOINT_FADE for each change point after that.
		fChangeTime = ((Abs(nCurrChange) - 1) * CHANGE_PERPOINT_FADE) + CHANGE_BASE_FADE;
        if (fChangeTime > 5.0)
            fChangeTime = 5.0;
        return (fChangeTime);
	}

	// When state begins, intialize fade times.
	function BeginState()
	{
		fCurrFadeTime       = 0;
		fTotalFadeTime = GetFadeChangeTime();
		log("Fade time " $fTotalFadeTime);
	}
}


defaultproperties
{
	strHudIcon="HP_Menu.Hud.HealthEmpty"
	bDisplayCount=true
	bDisplayMaxCount=true
    nMaxIcons=6
    nUnitsPerIcon=100
	nCount=100
	nMaxCount=600
	nCurrCountPotential=100
	nActualIconW=21			// Icon is 128x128, but image is only 36x120
	nActualIconH=58   
	strToolTipId="InGameMenu_0022"
}


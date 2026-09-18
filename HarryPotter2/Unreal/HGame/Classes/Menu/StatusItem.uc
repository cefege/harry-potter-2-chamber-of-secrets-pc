//===============================================================================
//  StatusItem
//
//  Contains data about a status related item that cam be displayed on 
//  the Hud.  See StatusManager and StatusGroup.
//
//===============================================================================

class StatusItem extends Actor abstract;

enum ECountColor
{
    CountColor_Black,
    CountColor_NearWhite,
    CountColor_White
};

var StatusItem  siNext;				// Next status item in the "list"
var StatusGroup sgParent;			// Parent group of this status item
var string      strHudIcon;         // Name of hud icon to dynamically load
var texture     textureHudIcon;     // Hud icon to use
var bool        bDisplayCount;		// true to display item count
var bool        bDisplayMaxCount;   // true to display max count
var bool        bMenuModeOnly;      // true to only display in menu mode
var int         nCount;             // Number of this item
var int         nMaxCount;          // Maximum number of this item
var int         nCurrCountPotential;// Number currently possible of this item
                                    // (nMaxCount stays constant- potential can
                                    // increase)
var int         nActualIconW;       // The actual icon image may not take up the
var int         nActualIconH;       // whole canvas it's on.  Specify actual size
                                    // or leave as 0 to use textureHudIcon V and USize
var int         nCountMiddleX;		// Middle of count area along x axis
var int         nCountMiddleY;      // Middle of count area along y axis
var string      strToolTipId;
var bool        bDisplayWhenCountZero; // Display the status item if count is zero
var bool        bTravelStatus;      // true if status counts should travel
var ECountColor CountColor;

event PreBeginPlay()
{
	if (strHudIcon != "")
		textureHudIcon = texture(DynamicLoadObject(strHudIcon, class'Texture'));
}

// Increment/Decrement count of status item by specified amount (no visual effect triggered).
function IncrementCount(int nNum)
{
	SetCount(nCount + nNum);
}

// Sets the count of the status item without triggering a visual effect.
function SetCount(int nNum)
{
	nCount = nNum;

	// Make sure count does not dip below 0
	if (nCount < 0)
		nCount = 0;

	// If a potential count is setup for the item, make sure don't
	// exceed that potential count.
	else if (nCurrCountPotential != 0)
	{
		if (nCount > nCurrCountPotential)
			nCount = nCurrCountPotential;
	}

	// If no potential count is setup, but a max count is, make sure
	// don't exceed the max count.
	else if (nMaxCount != 0)
	{
		if (nCount > nMaxCount)
			nCount = nMaxCount;
	}
}

function int GetCount()
{
	return (nCount);
}

function int GetPotentialCount()
{
	return (nCurrCountPotential);
}

// Increment/Decrement count potential of status item by specified amount.
function IncrementCountPotential(int nNum)
{
	nCurrCountPotential += nNum;

	// Make sure potential count doesn't dip below 0.
	if (nCurrCountPotential < 0)
		nCurrCountPotential = 0;

	// If a max count is define, make sure the potential does not exceed that.
	else if ((nMaxCount != 0) && (nCurrCountPotential > nMaxCount))
		nCurrCountPotential = nMaxCount;
}

// Set count to maximum potential.
function SetCountToMaxPotential()
{
	nCount = nCurrCountPotential;
}

// Potential count percent available
function float GetPotentialToMaxCountRatio()
{
	if (nMaxCount != 0)
		return (float(nCurrCountPotential) / float(nMaxCount));
	else
	{
		log("Divide by 0");
		return (0);
	}
}

// Count out of MaxCount
function float GetCountToMaxCountRatio()
{
	if (nMaxCount > 0)
		return (float(nCount) / float(nMaxCount));
	else
	{
		log("Divide by 0");
		return (0);
	}
}

function float GetCountToCurrPotentialRatio()
{
	return (float(nCount) / float(nCurrCountPotential));
}

function color GetCountColor()
{
    local color colorRet;

	// Set text draw color
    switch (CountColor)
    {
    case (CountColor_Black) :
	    colorRet.r = 0;
	    colorRet.g = 0;
	    colorRet.b = 0;
        break;
    case (CountColor_White) :
	    colorRet.r = 255;
	    colorRet.g = 255;
	    colorRet.b = 255;
        break;
    case (CountColor_NearWhite) :
	    colorRet.r = 206;
	    colorRet.g = 200;
	    colorRet.b = 190;
        break;
    default :
        break;
    }

    return (colorRet);
}

function font GetCountFont(Canvas canvas)
{
    local font fontRet;

    if (Canvas.SizeX <= 512)
        fontRet = baseConsole(sgParent.smParent.playerHarry.player.console).LocalTinyFont;
	else if (Canvas.SizeX <= 640)
		fontRet = baseConsole(sgParent.smParent.playerHarry.player.console).LocalSmallFont;
	else
		fontRet = baseConsole(sgParent.smParent.playerHarry.player.console).LocalMedFont;

    return (fontRet);
}

// Draw status item.
function DrawItem(Canvas canvas, int nCurrX, int nCurrY, float fScaleFactor)
{
	canvas.SetPos(nCurrX,nCurrY);
	canvas.DrawIcon(textureHudIcon, fScaleFactor);
	if (bDisplayCount)
		DrawCount(canvas, nCurrX, nCurrY, fScaleFactor);
}

// Draw StatusItem's count on top of the status item.
function DrawCount(canvas Canvas, int nCurrX, int nCurrY, float fScaleFactor)
{
	local color  colorSave;		// Save off original canvas draw color
	local font   fontSave;
	local string strCountDisplay;
	local float  nXTextLen, nYTextLen;

	nXTextLen     = 0;
	nYTextLen     = 0;

	// Save off canvas properties
	colorSave = Canvas.DrawColor;
	fontSave  = Canvas.Font;

	// Build count string
	strCountDisplay = string(nCount); 
	if (bDisplayMaxCount == true)
		strCountDisplay = strCountDisplay $"/" $string(nMaxCount);;

    Canvas.DrawColor = GetCountColor();

	// Set font based on resolution.
    Canvas.Font = GetCountFont(Canvas);

	// Calculate where to place text based on number of digits.
	Canvas.TextSize(strCountDisplay, nXTextLen, nYTextLen);
	Canvas.SetPos(nCurrX + (nCountMiddleX * fScaleFactor) - nXTextLen/2,
		          nCurrY + (nCountMiddleY * fScaleFactor) - nYTextLen/2);

	// Draw the count
	Canvas.DrawText(strCountDisplay, false);

	// Restore canvas properties
	Canvas.DrawColor = colorSave;
	Canvas.Font      = fontSave;
}

// Hud images may not take up the whole icon canvas that they are on.  For
// positioning purposes, information on the size of the image and not the
// size of the actual texture is needed.  If a derived status item sets
// nActualIconW to some value, that value will be returned as the width
// of the icon.  If the derived class does not override and leaves 
// nActualIconW at 0, this function will return the actual texture size.
function int GetHudIconUSize()
{
	if (nActualIconW == 0)
		return (textureHudIcon.USize);
	else
		return (nActualIconW);
}

// Return specified VSize or actual VSize of the texture if not specified.
function int GetHudIconVSize()
{
	if (nActualIconH == 0)
		return (textureHudIcon.VSize);
	else
		return (nActualIconH);
}

function string GetToolTip()
{
	return (Localize( "All", strToolTipId,"HPMenu" ));
}

defaultproperties
{
	DrawType=DT_None
	bHidden=true
	siNext=None
	bDisplayCount=false
	bDisplayMaxCount=false
	bMenuModeOnly=false
	nCount=0
	nMaxCount=0
	nCurrCountPotential=0
	nCountMiddleX=34
	nCountMiddleY=56
	nActualIconW=0			// Leave as 0 to use textureHudIcon.USize
	nActualIconH=0          // Leave as 0 to use textureHudIcon.VSize
    bDisplayWhenCountZero=true
    bTravelStatus=true
    CountColor=CountColor_Black
}


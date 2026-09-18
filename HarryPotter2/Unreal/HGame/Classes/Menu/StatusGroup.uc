//===============================================================================
//  StatusGroup
//
//  StatusGroup contains a list of StatusItems that should be drawn as a unit.
//  See StatusManager.
//  
//===============================================================================

class StatusGroup extends Actor abstract;

const BASE_RESOLUTION_X = 800.0;       // Icons are made to look good in
                                       // 800x600.  We'll scale when drawing icons
                                       // in other resolutions.
const FLY_TO_HUD_CAM_DIST = 150;

enum EEffectType
{
	ET_Fade,
	ET_Fly,
	ET_Permanent,
	ET_Menu
};

enum EMenuProps
{
	Menu_Always,
	Menu_IfEverHadAny,
	Menu_IfCurrentlyHaveAny,
	Menu_Never
};

// Internal
var StatusGroup   sgNext;		  	    // Next status group in the "list"
var StatusItem    siList;               // List of status items
var StatusManager smParent;			    // Parent status manager
var float         fCurrEffectInTime;    // Time elapsed for In effect
var float         fCurrEffectOutTime;   // Time elapsed for Out effect
var EEffectType   CurrEffectType;       // Current effect to apply either ET_Permanent
                                        // if in menu mode or GameEffectType
var bool          bDisplayJustFirstItem;// true to display only 1st in list

// Meant to be customized by derived classes.
var bool        bDisplayHorizontally;  // display items along X axis
var int         nSpaceBetweenIcons;    // Vertical or Horiz. pixels between items
var float       fTotalEffectInTime;    // Take this long for In effect; 0 if none
var float       fTotalHoldTime;        // Hold for this long; 0 ok if permanent
var float       fTotalEffectOutTime;   // Take this long for Out effect; 0 if none
var EEffectType GameEffectType;        // Effect that should be used for hud while 
                                       // game is in progress (as opposed to menu)
var bool         bDisplayOnMenu;
var EMenuProps   MenuProps;
var bool         bNormallyRenderInCutScene;
var bool         bCurrRenderInCutScene;

//-----------------------------------------------------------------------------------
// Override functions
//-----------------------------------------------------------------------------------
//
// Functions that derived classes should override.

// Position of group when in the hold state. (If flying in, the group will fly into
// this location.)  This function doesn't actually need to be overridden because
// it calls GetGroupFinalXY_2.
function GetGroupFinalXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, 
						 out int nOutX, out int nOutY)
{
	GetGroupFinalXY_2(bMenuMode, Canvas.SizeX, Canvas.SizeY, nIconWidth, nIconHeight, 
		              nOutX, nOutY);
}

// Position of group when in hold state (this version can be used when know the size
// of the canvas, but don't already have a canvas).  Should always override this
// function and calculate your own hold state position.
function GetGroupFinalXY_2(bool bMenuMode, int nCanvasSizeX, int nCanvasSizeY, 
						   int nIconWidth, int nIconHeight, 
		                   out int nOutX, out int nOutY)
{
	log("Error:  Derived StatusGroup objects should override GetGroupFinalXY_2");

	nOutX = 0;
	nOutY = 0;
}


// Postion that group should fly in from.  Only need to override if 
// GameEffectType == ET_Permanent.
function GetGroupFlyOriginXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, 
							 out int nOutX, out int nOutY)
{
	log("Error:  Derived StatusGroup objects should override GetGroupFlyOriginXY");

	nOutX = 0;
	nOutY = 0;
}

//-----------------------------------------------------------------------------------
//  StatusGroup specific functions
//-----------------------------------------------------------------------------------

// Permanently show the status group (until set back to normal).
function SetEffectTypeToPermanent()
{	
	CurrEffectType = ET_Permanent;		
	SetTimer(0.0, false);
	GoToState('Hold');
}

// Set effect type back to normal.
function SetEffectTypeToNormal()
{
	CurrEffectType = GameEffectType;
	GoToState('Idle');
}

function SetCutSceneRenderModeToNormal()
{
    bCurrRenderInCutScene = bNormallyRenderInCutScene;
}

function SetCutSceneRenderMode(bool bRenderInCutScenes)
{
    bCurrRenderInCutScene = bRenderInCutScenes;
}

// Find the StatusItem of classItem in this group.  If one does not already exist,
// it will be created and retuend.
function StatusItem GetStatusItem(class<StatusItem> classItem)
{
	local StatusItem siLoop;

	// If no StatusItems in the list yet, spawn a classItem Status item,
	// add it to the list and return it.
	if (siList == None)
	{
		siList          = spawn(classItem);
		siList.sgParent = self;
		return (siList);
	}

	// Look through status item list and return StatusItem of classItem if
	// it exists.  If there is no classItem in the list, create one, add
	// it to the list and return.
	for (siLoop=siList; siLoop!=None; siLoop=siLoop.siNext)
	{
		// If found the existing StatusItem, return it.
		if (siLoop.class == classItem)
			return (siLoop);

		// If we've reached the end of the StatusItem list and no classITem
		// object has been found, create one, add it to the list and return
		// it.  Note:  If classItem is not a valid class, no StatusItem
		// will be added to the list and the return will be None.
		if (siLoop.siNext == None)
		{
			// Spawn new classItem object
			siLoop.siNext   = spawn(classItem);
			siLoop.siNext.sgParent = self;

			// Return the newly spawned object (or None if the spawn failed).
			return (siLoop.siNext);
		}
	}

	log("Error: StatusGroup::GetStatusItem- should not get to here");
	return (None);
}

// Draw the current state of the group to the hud canvas.  Note:  This function
// is overridden (and is empty) in the idle state so that no rendering happens.  
// The implementation below gets called for all other states.
function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
{
	local StatusItem siLoop;
	local int        nCurrX;                // X position of current status item
	local int        nCurrY;                // Y position of current status item
	local bool       bFirstIconInList;      // True when curr status item is first
	local color      colorSave;             // Save off canvas draw color
	local float      fScaleFactor;          // Scale icons based on canvas dimensions

	// Initialize
	nCurrX           = 0;
	nCurrY           = 0;
	bFirstIconInList = true;
	fScaleFactor     = GetScaleFactor(canvas.SizeX);

	// If game we're at the ingame menu, just return.
	if (bMenuMode)
		return;

    // If not at menu and not supposed to render while in cutscene and in full cutscene mode, 
    // just return.
    if (!bMenuMode && !bCurrRenderInCutScene && bFullCutMode)
        return;

	// Some groups need to go into a different display mode while on the menu.  For
	// example, groups that usually only display when a new item is added need
	// to display permanently while the InGame menu is up.
	HandleMenuModeSwitching(bMenuMode);

	// Save off draw color
	colorSave = Canvas.DrawColor;
	
	// Set new draw color
	canvas.DrawColor = GetDrawColor();

	// Draw each StatusItem in our group
	for (siLoop=siList; siLoop!=None; siLoop=siLoop.siNext)
	{
		// Check to see if the current item should display.  Some items only display
		// in menu mode.
		if (bMenuMode || (!bMenuMode && (siLoop.bMenuModeOnly == false)))
		{
		    // If this is the first icon for this render cycle
		    if (bFirstIconInList == true)
		    {
			    // Calculate where we should start drawing the group.
			    GetGroupCurrXY(bMenuMode, Canvas, siLoop.GetHudIconUSize(), 
				               siLoop.GetHudIconVSize(), nCurrX, nCurrY);
			    bFirstIconInList = false;
		    }

            // Some items don't display when count is zero, (but we'll still put
            // everything in the proper position).
            if (siLoop.nCount > 0 ||
                (siLoop.nCount == 0 && siLoop.bDisplayWhenCountZero == true))
            {
			    // Draw current status item.
			    siLoop.DrawItem(Canvas, nCurrX, nCurrY, fScaleFactor);
            }

			// Calculate next StatusItem position.
			if (bDisplayHorizontally)
			    nCurrX += fScaleFactor * (siLoop.GetHudIconUSize() + nSpaceBetweenIcons);
			else
			    nCurrY += fScaleFactor * (siLoop.GetHudIconVSize() + nSpaceBetweenIcons);

			// If only ever want to display first status item, break first time thru.
			if (bDisplayJustFirstItem)
            {
                log("displayjustfirst");
			    break;
            }
		}	

	}

	// Restore draw color
	Canvas.DrawColor=colorSave;
}

function HandleMenuModeSwitching(bool bMenuMode)
{
	// Set curr effect type based on whether we're rending for the menu
	// or for the hud in the game.
	if (bMenuMode)                         // at menu
	{
		CurrEffectType = ET_Menu;
		if (!IsInState('Hold'))
			GoToState('Hold');
	}
	else if (CurrEffectType == ET_Menu)    // must have just returned from menu
	{
		CurrEffectType = GameEffectType;
		if (!IsInState('Idle') && CurrEffectType != ET_Permanent)
			GoToState('Idle');
	}
}

// Get screen position that StatusItem corresponding to classItem should display.
function GetItemPosition(class<StatusItem> classItem, bool bMenuMode, out int nOutX, out int nOutY, optional int nCanvasSizeX, optional int nCanvasSizeY)
{
	local StatusItem si;
	local StatusItem siLoop;
	local bool       bFirstInList;
	local float      fScaleFactor;     // Icons are scale based on canvas dimensions

	nOutX = 0;
	nOutY = 0;

	if (nCanvasSizeX == 0)
		nCanvasSizeX = smParent.nCanvasSizeX;
	if (nCanvasSizeY == 0)
		nCanvasSizeY = smParent.nCanvasSizeY;

	bFirstInList = true;
	fScaleFactor = GetScaleFactor(nCanvasSizeX);

	si = GetStatusItem(classItem);

	if (si == None)
		log("Error:  Could not get StatusItem " $classItem);

	for (siLoop=siList; siLoop!=None; siLoop=siLoop.siNext)
	{
	    // If this is the first guy in list
	    if (bFirstInList == true)
	    {
		    // Calculate where we should start drawing the group.
		    GetGroupCurrXY_2(bMenuMode, nCanvasSizeX, nCanvasSizeY, siLoop.GetHudIconUSize(), 
			                 siLoop.GetHudIconVSize(), nOutX, nOutY);
		    bFirstInList = false;
	    }

	    // If found the item we were looking for
	    if (siLoop.class == si.class)
		    break;

	    // Calculate next StatusItem position.
	    if (bDisplayHorizontally)
		    nOutX += fScaleFactor * (siLoop.GetHudIconUSize() + nSpaceBetweenIcons);
	    else
		    nOutY += fScaleFactor * (siLoop.GetHudIconVSize() + nSpaceBetweenIcons);

	    // If only ever want to display first status item, break first time thru.
	    if (bDisplayJustFirstItem)
		    break;
    }
}

// Get status item's position in 3D space.
function vector GetItemLocation(class<StatusItem> classItem, bool bMenuMode)
{
	local int    nHudX, nHudY;
	local int    nCanvasHalfX, nCanvasHalfY;
	local float  fXVal, fYVal;
	local vector vectReturn;

	// Get StatusItem coords in 2D.
	GetItemPosition(classItem, bMenuMode, nHudX, nHudY);

	// We need canvas/2 for a few calculations below.  Calc once here.
	nCanvasHalfX = smParent.nCanvasSizeX / 2;
	nCanvasHalfY = smParent.nCanvasSizeY / 2;

	// Calculate X and Y 3D space values in camera coords.  The X value can range 
	// from -1.0 on the left to 1.0 on the right.  The Y value can range from 1.0
	// on the top to -1.0 on the bottom.
	//
	//            -1.0    0       1.0
	//           -------------------
	//		1.0	|					|
	//			|					|
	//			|					|
	//			|					|
	//		 0	|					|
	//			|					|
	//			|					|
	//			|					|
	//      -1.0|					|
	//           -------------------

	// Calculate X value in camera coords.
	if (nHudX >= nCanvasHalfX)                          // StatusItem on right half
		fXVal = (nHudX - nCanvasHalfX) / float(nCanvasHalfX);
	else                                                // StatusItem on left half
		fXVal =  -(1 - (nHudX / float(nCanvasHalfX)));

	// Calcualte Y value in camera coords.
	if (nHudY <= nCanvasHalfY)							// StatusItem on top half
		fYVal = 1 - (nHudY / float(nCanvasHalfY));
	else												// StatusItem on bottom half
		fYVal = -( 1 - ((nHudY - nCanvasHalfY) / float(nCanvasHalfY)) );

	// Create the vector based on the components we've just calculated.
	vectReturn.x = fXVal;
	vectReturn.y = fYVal;
	vectReturn.z = FLY_TO_HUD_CAM_DIST;

	// Convert the coords from camera coords to world coords.
	vectReturn = smParent.playerHarry.CameraToWorld(vectReturn);

	// Return world coords of the StatusItem hud position in 3d.
	return (vectReturn);
}


// Get DrawColor to use for the current Render cycle.
function color GetDrawColor()
{
	local color colorReturn;         // color returned
	local int   nFadeValue;          // Value used for R,G and B components

	nFadeValue    = GetFadeValue();
	colorReturn.r = nFadeValue;
	colorReturn.g = nFadeValue;
	colorReturn.b = nFadeValue;

	return (colorReturn);
}

// Get group start position for the current render cycle.
function GetGroupCurrXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, 
						out int nX, out int nY)
{
	GetGroupFinalXY(bMenuMode, Canvas, nIconWidth, nIconHeight, nX, nY);
}

// Get group start position (knowing the canvas size, but not having the
// actual canvas).
function GetGroupCurrXY_2(bool bMenuMode, int nCanvasSizeX, int nCanvasSizeY, int nIconWidth, int nIconHeight,
						  out int nOutX, out int nOutY)
{
	GetGroupFinalXY_2(bMenuMode, nCanvasSizeX, nCanvasSizeY, nIconWidth, nIconHeight, 
		              nOutX, nOutY);
}


// Get fade value to use.  Default is white.  This function will be overridden in
// state that need to handle fade information.
function int GetFadeValue()
{
	return (255);
}

// Increment count for specified status item.
function bool IncrementCount(class<StatusItem> classItem, int nNum)
{
	local StatusItem siUpdate;

	// Get existing status item that is of class classItem.  (One
	// will be created if none already exists.)
	siUpdate = GetStatusItem(classItem);

	if (siUpdate != None)
	{
		// Bump the count.
		siUpdate.IncrementCount(nNum);

		// Pass along event that count has incremented. 
		OnCountIncremented();

		return (true);
	}

	return (false);
}

// Increment count potential for specified status item.
function bool IncrementCountPotential(class<StatusItem> classItem, int nNum)
{
	local StatusItem siUpdate;

	// Get existing status item that is of class classItem.  (One
	// will be created if none already exists.)
	siUpdate = GetStatusItem(classItem);

	if (siUpdate != None)
	{
		// Bump the count.
		siUpdate.IncrementCountPotential(nNum);

		// Pass along event that count has incremented. 
		OnCountIncremented();

		return (true);
	}

	return (false);
}

// Most states don't need to do anything when the count has been incremented.
// If a state needs to do something, it will override this function.
// For example, if the state is currently idle, we want to display the 
// group and visually show that the count has changed.
function OnCountIncremented()
{
}

// Given time elapsed and total desired time, calculate the ratio of
// elapsed time.
function float GetTimeRatio(float fCurrTime, float fTotalTime)
{
	// If time already expired, set ratio to 1
	if (fTotalTime < fCurrTime)
		return (1.0);

	// Time not expired, calculate ratio
	else
		return ((fTotalTime - fCurrTime) / fTotalTime);
}

// Calculate the fade value for the current render cycle.
function int CalcFadeValue(bool bIn, float fCurrTime, float fTotalTime)
{
	local float fFadeRatio;    // Fade ratio based on time
	local int   nFadeValue;    // Fade value returned

	// Get ratio to fade based on time passed.
	fFadeRatio = GetTimeRatio(fCurrTime, fTotalTime);

	// If fading in
	if (bIn == true)
	{
		// Start fade at 127 and go proportionally over time to 255
		nFadeValue = 127 + (128 * fFadeRatio);  

		// Verify the calculation is valid
		if (Clamp(nFadeValue, 127, 255) != nFadeValue)
		{
			log("Error:  Error in fade value calculation");
			nFadeValue = Clamp(nFadeValue, 127, 255);
		}
	}

	// Fading out
	else
	{
		// Start fade at 255 and go proportionally over time down to 128
		nFadeValue = 255 - (128 * fFadeRatio);  

		// Verify the calculation is valid.
		if (Clamp(nFadeValue, 127, 255) != nFadeValue)
		{
			log("Error:  Fade value should never be under 127");
			nFadeValue = Clamp(nFadeValue, 127, 255);
		}
	}

	return (nFadeValue);
}

// Calculate the FlyIn/Out position for the current render cycle.
function CalcFlyXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, bool bFlyIn, 
				   float fCurrTime, float fTotalTime, out int nX, out int nY)
{
	local float fFlyRatio;
	local int   nFinalX, nFinalY; 
	local int   nOriginFlyX, nOriginFlyY;
	local int   nOffsetX, nOffsetY;
	local bool  bFlyOriginFromTop, bFlyOriginFromLeft;

	GetGroupFinalXY(bMenuMode, Canvas, nIconWidth, nIconHeight, nFinalX, nFinalY);
	GetGroupFlyOriginXY(bMenuMode, Canvas, nIconWidth, nIconHeight, nOriginFlyX, nOriginFlyY);

	fFlyRatio = GetTimeRatio(fCurrTime, fTotalTime);

	bFlyOriginFromTop  = (nFinalY > nOriginFlyY);
	bFlyOriginFromLeft = (nFinalX > nOriginFlyX);

	if (bFlyOriginFromTop)
	{
		nOffsetY = (nFinalY - nOriginFlyY) * fFlyRatio;

		if (bFlyIn)
			nY = nOriginFlyY + nOffsetY;
		else
			nY = nFinalY - nOffsetY;
	}

	else
	{
		nOffsetY = (nOriginFlyY - nFinalY) * fFlyRatio;

		if (bFlyIn)
			nY = nOriginFlyY - nOffsetY;
		else
			nY = nFinalY + nOffsetY;

	}

	if (bFlyOriginFromLeft)
	{
		nOffsetX = (nFinalX - nOriginFlyX) * fFlyRatio;

		if (bFlyIn)
			nX = nOriginFlyX + nOffsetX;
		else
			nX = nFinalX - nOffsetX;
	}
	else
	{
		nOffsetX = (nOriginFlyX - nFinalX) * fFlyRatio;

		if (bFlyIn)
			nX = nOriginFlyX - nOffsetX;
		else
			nX = nFinalX + nOffsetX;
	}
}

function float GetScaleFactor(int nCanvasSizeX)
{
	local float fScale;

	fScale = nCanvasSizeX / BASE_RESOLUTION_X;
	return (fScale);
}

event PreBeginPlay()
{
	CurrEffectType = GameEffectType;
}

//-----------------------------------------------------------------------------------
//  States
//-----------------------------------------------------------------------------------

//-----------------------------------------------------------------------------------
//  State Idle
//-----------------------------------------------------------------------------------
//
//  During the idle state, nothing needs to be drawn or calculated.
//
//  Control enters into the idle state either by default when the status group
//  has been created or the state is re-entered after display of the status group
//  is complete and no longer needs to be displayed.
//
//  Conrol leaves the Idle state either right way because the status group should
//  be shown permanently or leaves when the status group needs to be displayed.

auto state Idle
{
	// Called after count for an item in the group has been incremented.  To give
	// feedback on the new item, control goes to the EffectIn state.
	function OnCountIncremented()
	{
		if ((CurrEffectType == ET_Permanent) || (CurrEffectType == ET_Menu))
			GoToState('Hold');
		else
			GoToState('EffectIn');
	}

	// Override RenderHudItemManager when in idle.  No rendering needs to happen in idle.	
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		if (bDisplayOnMenu)
			HandleMenuModeSwitching(bMenuMode);
	}

	// State Intializations
	function BeginState()
	{
		// If the status group is permanently shown, go right away to Hold state.
		if ((CurrEffectType == ET_Permanent) || (CurrEffectType == ET_Menu))
			GoToState('Hold');
	}
}

//-----------------------------------------------------------------------------------
//  State EffectIn
//-----------------------------------------------------------------------------------
//
//  The status group is in this state while an "in" effect is in progress.  Currently
//  the effect can either be FadeIn or FlyIn.

state EffectIn
{
	// Keep track of EffectIn time.  After elapsed, move onto the Hold state.	
	event tick(float fDeltaTime)
	{
		// If EffectIn should continue, update the time
		if (fCurrEffectInTime > 0.0)
		{
			fCurrEffectInTime -= fDeltaTime;
			if (fCurrEffectInTime < 0.0)
				fCurrEffectInTime = 0.0;
		}

		// EffectIn is complete, go onto the Hold state.
		else
			GoToState('Hold');
	}

	// Calculate the fade color component if we're doing a fade effect.  Otherwise
	// return white.
	function int GetFadeValue()
	{
		local float fFadeRatio;
		local int   nFadeValue;
		local int   nFadeStart;

		if (CurrEffectType == ET_Fade)
			return (CalcFadeValue(true, fCurrEffectInTime, fTotalEffectInTime));
		else
			return (255);
	}

	// Get the position that the status group should start at for the current 
	// render cycle.
	function GetGroupCurrXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, 
		                    out int nX, out int nY)
	{
		// If doing a fly effect
		if (CurrEffectType == ET_Fly)
			CalcFlyXY(bMenuMode, Canvas, nIconWidth, nIconHeight, true, fCurrEffectInTime, 
			          fTotalEffectInTime, nX, nY);

		// No fly effect so just the normal status group start position
		else
			GetGroupFinalXY(bMenuMode, Canvas, nIconWidth, nIconHeight, nX, nY);
	}

	// Start EffectIn time
	function BeginState()
	{
		fCurrEffectInTime = fTotalEffectInTime;
	}
}

//-----------------------------------------------------------------------------------
//  State Hold
//-----------------------------------------------------------------------------------
//
//  While in this state, the status group is displayed in it's normal state.  
//
//  If CurrEffectType is ET_Permanent, then we get to this state right away and never 
//  leave this state.  Otherwise, control enters here after the EffectIn state
//  is complete.
//
//  A Timer is used to determine when the hold time is up instead of the tick 
//  method that is used in other states.  For the other states, which have effects
//  associated with them, we use the delta tick time to figure out the current
//  properties of the effect.  For the hold state, the properties remain the same
//  throughout the whole effect so we can just use a timer.

state Hold
{
	event Timer()
	{
		SetTimer(0.0, false);
		GoToState('EffectOut');
	}

	// Called after count for an item in the group has been incremented.  If we're
	// in the hold state while a new item is picked up, we want to reset the hold
	// time.
	function OnCountIncremented()
	{
		// If not showing permanently.
		if ((CurrEffectType != ET_Permanent) || (CurrEffectType == ET_Menu))
		{
			SetTimer(0.0, false);
			SetTimer(fTotalHoldTime, false);
		}
	}

	// Intitialize Hold time
	function BeginState()
	{
		if ((CurrEffectType != ET_Permanent) || (CurrEffectType == ET_Menu))
			SetTimer(fTotalHoldTime, false);
	}
}

//-----------------------------------------------------------------------------------
//  State EffectOut
//-----------------------------------------------------------------------------------
//
//  The status group is in this state while an "out" effect is in progress.  Currently
//  the effect can either be FadeOut or FlyOut.

state EffectOut
{
	// Keep track of EffectOut time
	event tick(float fDeltaTime)
	{
		// If continuing with EffectOut, update the time.
		if (fCurrEffectOutTime > 0.0)
		{
			fCurrEffectOutTime -= fDeltaTime;
			if (fCurrEffectOutTime < 0.0)
				fCurrEffectOutTime = 0.0;
		}

		// EffectOut is done, go back to idle.
		else
			GoToState('Idle');
	}

	// Called after count for an item in the group has been incremented.  If we're
	// in the effect out state while a new item is picked up, we want to return
	// to the hold state so the new status gets displayed.
	function OnCountIncremented()
	{
		// If not showing permanently.
		if ((CurrEffectType != ET_Permanent) || (CurrEffectType == ET_Menu))
		{
			GoToState('Hold');
		}
	}

	// Calculate the fade color component if we're doing a fade effect.  Otherwise
	// return white.
	function int GetFadeValue()
	{
		local float fFadeRatio;
		local int   nFadeValue;
		local int   nFadeStart;

		if (CurrEffectType == ET_Fade)
			return (CalcFadeValue(false, fCurrEffectOutTime, fTotalEffectOutTime));
		else
			return (255);
	}

	// Get the position that the status group should start at for the current 
	// render cycle.
	function GetGroupCurrXY(bool bMenuMode, canvas Canvas, int nIconWidth, int nIconHeight, 
		                    out int nX, out int nY)
	{
		// If doing the fly effect
		if (CurrEffectType == ET_Fly)
			CalcFlyXY(bMenuMode, Canvas, nIconWidth, nIconHeight, false, fCurrEffectOutTime, 
			          fTotalEffectOutTime, nX, nY);

		// No fly effect, just get normal position
		else
			GetGroupFinalXY(bMenuMode, Canvas, nIconWidth, nIconHeight, nX, nY);
	}

	// Intialize EffectOut time
	function BeginState()
	{
		fCurrEffectOutTime = fTotalEffectOutTime;
	}
}

defaultproperties
{
	DrawType=DT_None
	bHidden=true
	sgNext=None
	siList=None
	bDisplayHorizontally=true
	nSpaceBetweenIcons=5
	fTotalEffectInTime=0.0
	fTotalHoldTime=0.0
	fTotalEffectOutTime=0.0
	fCurrEffectInTime=0.0
	fCurrEffectOutTime=0.0		
	bDisplayJustFirstItem=false	
	MenuProps=Menu_Always
    bNormallyRenderInCutScene=false
    bCurrRenderInCutScene=false
}


//===============================================================================
//  [CutSceneManager] 
//===============================================================================

class CutSceneManager extends HudItemManager;

const SLIDE_DIVISOR = 25;

var texture    textureBorder;      
var float      fCurrBorderHeight;
var bool       bResetBorderHeightToMax;
var string     strText;
var string	   strCutCommentText;	//user comment displayed durring cutscenes.

var bool  bBothBordersActive;
var bool  bPopupBorderActive;
var color colorCutTextBlue;

//-----------------------------------------------------------------------------------
//  Caller interface
//-----------------------------------------------------------------------------------

// Call to start a cut scene.  The screen will slide into letterbox and remain in
// that state until EndCutScene is called.
function StartCutScene()
{
	// Slide in (only if not already in an "in" state)
	if (!IsInState('SlideIn') && !IsInState('Hold'))
		GoToState('SlideIn');
}

// Call to end a cut scene.  Text will cease drawing and the screen will slide back
// to fullscreen mode.
function EndCutScene()
{
	strCutCommentText="";
	// Slide out (only if not already in an "out" state)
	if (!IsInState('SlideOut') && !IsInState('Idle'))
		GoToState('SlideOut');
}

// Change the text that should be drawn.  The text will remain on the screen for
// fSetTextDuration seconds.  If fSetTextDuration is 0, the text will stay up
// indefinitely and it is up to the caller to call ClearText when the text
// should go away.  SetText may be called immediately after StartCutScene-
// the CutSceneManager will delay showing the text until the slide into letterbox
// is complete.  If SetText is called again before the current text duration has 
// expired and SetText is called, the old text is discarded and the new text will 
// display for the duration passed in.
function SetText(string strSetText, float fSetTextDuration)
{
	// Clear timer if one is still going.
	SetTimer(0.0, false);

	// Save off the text string.
	strText = strSetText;

	// Set text timer.
	if (fSetTextDuration > 0)
		SetTimer(fSetTextDuration, false);

		//bring in boarders if not alreay in.
	StartCutScene();


}

// Remove text and remove cutscene borders.
function ClearText()
{
	strText = "";
	strCutCommentText="";

		//if harry isnt captured anymore remove the boarders.
	if(!Level.playerHarryActor.bIsCaptured)
		EndCutScene();
}
//cutscene comments are displayed in the top boarder.
function SetCutCommentText(string strText)
{
	strCutCommentText=strText;
}
//-----------------------------------------------------------------------------------
//  Internal functions
//-----------------------------------------------------------------------------------

// The timer is initialized in SetText.  When the timer goes off, the current
// text is cleared.
event Timer()
{
	strText = "";

		//if harry isnt captured anymore remove the boarders.
	if(!Level.playerHarryActor.bIsCaptured)
		EndCutScene();
}

// Called by state code when border needs to be drawn.
function DrawBorder(Canvas canvas)
{
		//only draw top boarder if harry is captured
	if(Level.playerHarryActor.bIsCaptured)
	{
		Canvas.SetPos(0, 0);
		canvas.DrawTile(textureBorder, canvas.SizeX, fCurrBorderHeight, 0, 0, 1, 1);
	}

	Canvas.SetPos(0, canvas.SizeY - fCurrBorderHeight);
	canvas.DrawTile(textureBorder,canvas.SizeX, fCurrBorderHeight, 0, 0, 1, 1);
}

// Set fCurrBorderHeight to desired height.  The default is to leave the height as
// is.  This function is overridden in states where the border height actually
// needs to change.
function SetCurrBorderHeight(Canvas canvas)
{
}

// Get the maximum height for the border.
function float GetMaxBorderHeight(Canvas canvas)
{
    return (GetMaxBorderHeightFromCanvasHeight(canvas.SizeY));
}

function float GetMaxBorderHeightFromCanvasHeight(int nCanvasSizeY)
{
    return (nCanvasSizeY / 8.0);
}

// Called by state code when text is to be drawn.
function DrawText(Canvas canvas)
{
	HPHud(level.PlayerHarryActor.myHud).DrawCutStyleText(Canvas,
		                                                 strText,
                                                         0,
		                                                 Canvas.SizeY - fCurrBorderHeight + 1,
		                                                 fCurrBorderHeight,
													     colorCutTextBlue);

	if(strCutCommentText!="")
		HPHud(level.PlayerHarryActor.myHud).DrawCutStyleText(Canvas,
		                                                 strCutCommentText,
                                                         0,
		                                                 5,	//down a bit from the top.
		                                                 fCurrBorderHeight,
													     colorCutTextBlue);

}

//-----------------------------------------------------------------------------------
//  States
//-----------------------------------------------------------------------------------

//-----------------------------------------------------------------------------------
//  State Idle
//-----------------------------------------------------------------------------------
//
// We're in this state before StartCutScene has been called or after 
// EndCutScene has been called.

auto state Idle
{
}

//-----------------------------------------------------------------------------------
//  State SlideIn
//-----------------------------------------------------------------------------------
//
//  This state is entered when StartCutScene is called.  While in this state, the
//  screen display slides into a letterbox.  After the slid in has completed, we
//  move onto the Hold state.

state SlideIn
{
	// When sliding in, we just draw the border.
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		// Draw the slide in its current state
		SetCurrBorderHeight(canvas);
		DrawBorder(canvas);

		// If done sliding in, goto hold state
		if (fCurrBorderHeight >= GetMaxBorderHeight(canvas))
			GoToState('Hold');
	}

	// While in the slide in state, border height increases in each render cycle.
	function SetCurrBorderHeight(Canvas canvas)
	{
		local float fMaxBorderHeight;
			
		fMaxBorderHeight = GetMaxBorderHeight(canvas);

		// If haven't reached the maximum border height yet, slide in some more.
		if (fCurrBorderHeight < fMaxBorderHeight)
			fCurrBorderHeight += (fMaxBorderHeight)/SLIDE_DIVISOR;

		// Make sure we don't increment past max border height
		if (fCurrBorderHeight > fMaxBorderHeight)
			fCurrBorderHeight = fMaxBorderHeight;
	}

	// State Intializations
	function BeginState()
	{
		if (Level.playerHarryActor.bIsCaptured)
			bBothBordersActive=true;
		else
			bPopupBorderActive=true;
		fCurrBorderHeight = 0;
	}
}

//-----------------------------------------------------------------------------------
//  State Hold
//-----------------------------------------------------------------------------------
//
//  This state is entered after SlideIn completes.  We remain in this state drawing
//  the cutscene border and text (if there is any) until control moves to the 
//  SlideOut state when EndCutScene is called.

state Hold
{
	// Draw the border and text while in the hold state.
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		DrawBorder(canvas);
		DrawText(canvas);
	}

}

//-----------------------------------------------------------------------------------
//  State SlideOut
//-----------------------------------------------------------------------------------
//
//  The SlideOut state is entered when EndCutScene is called.  We remain in this
//  state until the screen has completed its slide from letterbox back to full screen.
//  Once the slide is complete, control goes back to the idle state where no
//  border or text drawing occurs.

state SlideOut
{
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		// The first time we render in the SlideOut state, we need to set the max
		// border height.  It would be nice to do this in BeginState, but we need
		// the canvas.  So, we set a flag in Begin state, then do the calculation
		// the first time we have a canvas to work with.
		if (bResetBorderHeightToMax == true)
		{
			fCurrBorderHeight = GetMaxBorderHeight(canvas);
			bResetBorderHeightToMax = false;
		}

		// Draw current state of the slide out.
		SetCurrBorderHeight(canvas);
		DrawBorder(canvas);

		// If done sliding out, go back to idle state.
		if (fCurrBorderHeight <= 0)
		{
			bBothBordersActive=false;
			bPopupBorderActive=false;
			GoToState('Idle');
		}
	}

	// While in the slide out state, border height decreases in each render cycle.
	function SetCurrBorderHeight(Canvas canvas)
	{
		// If still sliding out, decrease border height.
		if(fCurrBorderHeight > 0)
			fCurrBorderHeight -= GetMaxBorderHeight(canvas)/SLIDE_DIVISOR;

		// Make sure we don't go below 0.
		if (fCurrBorderHeight < 0)
			fCurrBorderHeight = 0;
	}

	// State Intializations
	function BeginState()
	{
		bResetBorderHeightToMax = true;
	}

}

defaultproperties
{
	textureBorder=Texture'HGame.Icons.leftPanel'
	fCurrBorderHeight=0.0
	bResetBorderHeightToMax=false
	strText=""
	DrawType=DT_None
	colorCutTextBlue=(R=127,G=127,B=255)
}


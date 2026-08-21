//===============================================================================
//  GoldCardTimerManager
//  
//  Timer manager that uses graphics specific to gold wizard card room timer.
//
//===============================================================================

class GoldCardTimerManager extends CountdownTimerManager;

//-----------------------------------------------------------------------------------
//  Functions
//-----------------------------------------------------------------------------------

const strTIMER_EMPTY = "HP_Menu.Hud.GoldCardTimerEmpty";
const strFULL_BAR    = "HP_Menu.Hud.GoldCardTimerFull";

const fFULL_OFFSET_X = 58.0;
const fFULL_OFFSET_Y = 23.0;

const fTIMER_EMPTY_W = 205.0;
const fTIMER_EMPTY_H = 58.0;

const fFULL_BAR_W    = 118.0;

// Textures- get loaded dynamically
var texture textureTimerEmpty;
var texture textureFullBar;

// Dynamically load textures when manager created.
event PostBeginPlay()
{
	Super.PostBeginPlay();

	// Load the score graphics.
	textureTimerEmpty  = texture(DynamicLoadObject(strTIMER_EMPTY    , class'Texture'));
	textureFullBar     = texture(DynamicLoadObject(strFULL_BAR, class'Texture'));
}

function DrawCountdown(Canvas canvas)
{
	local int		Ox, Oy;
	local float     fScaleFactor;
	local float     fFullRatio;
	local float     fSegmentWidth;

	fScaleFactor = canvas.GetHudScaleFactor();

	// Draw the empty timer	
	Ox = Canvas.SizeX - (8 * fScaleFactor) - (fTIMER_EMPTY_W * fScaleFactor);
	Oy = Canvas.SizeY - (8 * fScaleFactor) - (fTIMER_EMPTY_H * fScaleFactor);
	Canvas.SetPos(Ox, Oy);
	Canvas.DrawIcon(textureTimerEmpty,1);

	fFullRatio = fCountdownTime / GetTimerDuration();
	fSegmentWidth = fFullRatio * fFULL_BAR_W;
	Canvas.SetPos(Ox + (fFULL_OFFSET_X * fScaleFactor), Oy + (fFULL_OFFSET_Y * fScaleFactor));
	Canvas.DrawTile(textureFullBar,
		            fSegmentWidth * fScaleFactor,
					textureFullBar.VSize * fScaleFactor,
					0,
					0,
					fSegmentWidth,
					textureFullBar.VSize);
	
	DrawTuningModeData(Canvas);
}

defaultproperties
{
	DrawType=DT_Sprite							// For editor drawing
	bHidden=true                                // Displays in editor, but not game
	CutName="GoldCardTimerManager"
}


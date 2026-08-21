//===============================================================================
//  [SpellSelector] 
//
//  The SpellSelector handles the hud representation of the spell selection
//  for dueling.
//
//  To use the SpellSelector
//
//  1) Spawn a SpellSelector object from script when dueling starts.
//  2) Call SelectSpell(ESpellSelection) to change which spell
//     shows up as selected.
//  3) Destroy the SpellSelector when dueling ends.
// 
//===============================================================================

class SpellSelector extends HudItemManager;

// Possible spell selections
enum ESpellSelection
{
	SSelection_Rictusempra,	
	SSelection_Mimblewimble,
    SSelection_Expelliarmus
};

// Names of enemy health textures
const strSPELL_RICTUSEMPRA      = "HP_Menu.Hud.SpellRictusempra";
const strSPELL_RICTUSEMPRA_SEL  = "HP_Menu.Hud.SpellRictusempraSelect";
const strSPELL_MIMBLEWIMBLE     = "HP_Menu.Hud.SpellMimblewimble";
const strSPELL_MIMBLEWIMBLE_SEL = "HP_Menu.Hud.SpellMimblewimbleSelect";
const strSPELL_EXPELLIARMUS     = "HP_Menu.Hud.SpellExpelliarmus";
const strSPELL_EXPELLIARMUS_SEL = "HP_Menu.Hud.SpellExpelliarmusSelect";

// Corresponding hotkey strings
const strRICTUSEMPRA_HOTKEY   = "1";
const strMIMBLEWIMBLE_HOTKEY  = "2";
const strEXPELLIARMUS_HOTKEY  = "3";

// Screen coords
const nSTART_X = 2;
const nSTART_Y = 200;
const nTEXT_OFFSET_X = 20;
const nTEXT_OFFSET_Y = 50;
const nSPACE_BETWEEN_ICONS = 4;

// Spell icon textures.
var texture textureSpellRictusempra;
var texture textureSpellRictusempraSel;
var texture textureSpellMimblewimble;
var texture textureSpellMimblewimbleSel;
var texture textureSpellExpelliarmus;
var texture textureSpellExpelliarmusSel;

var ESpellSelection CurrSelection;      // Curr spell selection to highlight

// Dynamically load textures when manager created.
event PostBeginPlay()
{
	Super.PostBeginPlay();

	// Load textures
	textureSpellRictusempra     = texture(DynamicLoadObject(strSPELL_RICTUSEMPRA,      class'Texture'));
    textureSpellRictusempraSel  = texture(DynamicLoadObject(strSPELL_RICTUSEMPRA_SEL,  class'Texture'));
    textureSpellMimblewimble    = texture(DynamicLoadObject(strSPELL_MIMBLEWIMBLE,     class'Texture'));
    textureSpellMimblewimbleSel = texture(DynamicLoadObject(strSPELL_MIMBLEWIMBLE_SEL, class'Texture'));
    textureSpellExpelliarmus    = texture(DynamicLoadObject(strSPELL_EXPELLIARMUS,     class'Texture'));
    textureSpellExpelliarmusSel = texture(DynamicLoadObject(strSPELL_EXPELLIARMUS_SEL, class'Texture'));

    // Start repeating timer.  Will keep trying to register the SpellSelector with 
    // the hud until we can.
    SetTimer(0.2, true);
}

// When object is destroyed, un-register from hud
event Destroyed()
{
    Harry(Level.PlayerHarryActor).ClientMessage("spellselector destroyed");
	HPHud(Harry(Level.PlayerHarryActor).myHud).RegisterSpellSelector(None);
    Super.Destroyed();
}

// Keep trying to register ourselves with the hud (in case hud is not available
// at startup of this object).  Once registered, cancel the timer.
event Timer()
{
	if (level.PlayerHarryActor.myHud != None)
	{
		// Set things up so the hud will call us.
		HPHud(Harry(level.PlayerHarryActor).myHud).RegisterSpellSelector(self);

        // Registered so can stop the timer.
        SetTimer(0.0, false);
	}
}

// Set the spell icon to highlight
function SetSelection(ESpellSelection SSelection)
{
    CurrSelection = SSelection;
}

// RenderHudItemManager.  Called by Hud every render cycle while this object is registered.
function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
{
	local float   fScaleFactor;    // amount to scale based on screen res
	local int     nIconX;          // place spell icon here
	local int     nIconY;
    local texture textureSpellIcon;

	// Get scale for different screen resolutions.
	fScaleFactor = GetScaleFactor(Canvas);

    // Place on left side about 1/3 down the screen
	nIconX = nSTART_X * fScaleFactor;
	nIconY = nSTART_Y * fScaleFactor;

    // Draw Rictusempra icon
    if (CurrSelection == SSelection_Rictusempra)
        textureSpellIcon = textureSpellRictusempraSel;
    else
        textureSpellIcon = textureSpellRictusempra;
    DrawSpellIcon(Canvas, fScaleFactor, textureSpellIcon, nIconX, nIconY, strRICTUSEMPRA_HOTKEY);
    nIconY += (textureSpellRictusempra.VSize + nSPACE_BETWEEN_ICONS) * fScaleFactor;

    // Draw Mimblewimble icon
    if (CurrSelection == SSelection_Mimblewimble)
        textureSpellIcon = textureSpellMimblewimbleSel;
    else
        textureSpellIcon = textureSpellMimblewimble;
    DrawSpellIcon(Canvas, fScaleFactor, textureSpellIcon, nIconX, nIconY, strMIMBLEWIMBLE_HOTKEY);
    nIconY += (textureSpellMimblewimble.VSize + nSPACE_BETWEEN_ICONS) * fScaleFactor;

    // Draw Expelliarmus icon
    if (CurrSelection == SSelection_Expelliarmus)
        textureSpellIcon = textureSpellExpelliarmusSel;
    else
        textureSpellIcon = textureSpellExpelliarmus;
    DrawSpellIcon(Canvas, fScaleFactor, textureSpellIcon, nIconX, nIconY, strEXPELLIARMUS_HOTKEY);
}

// Draw the specified spell icon and the highlight if the icon is for the current spell.
function DrawSpellIcon(Canvas canvas, float fScaleFactor, texture textureSpellIcon, int nIconX, int nIconY, 
                       string strHotKey)
{
    // Draw the spell icon
	canvas.SetPos(nIconX,nIconY);
	canvas.DrawIcon(textureSpellIcon, fScaleFactor);

    DrawHotKeyText(Canvas, nIconX, nIconY, strHotKey);
}

// Draw hotkey text on icon.
function DrawHotKeyText(Canvas canvas, int nIconX, int nIconY, string strHotKey)
{
    local float fScaleFactor;
    local font  fontSave;
    local color colorSave;
    local float fXTextLen, fYTextLen;
    local int   nXOffset, nYOffset;

	// Get scale for different screen resolutions.
	fScaleFactor = GetScaleFactor(Canvas);

	// Save off canvas properties
	colorSave = Canvas.DrawColor;
	fontSave  = Canvas.Font;

    // Text draw color is off-white
    Canvas.DrawColor.R = 206;
    Canvas.DrawColor.G = 200;
    Canvas.DrawColor.B = 190;

    if (Canvas.SizeX <= 512)
        Canvas.Font = baseConsole(level.PlayerHarryActor.player.console).LocalTinyFont;
	else if (Canvas.SizeX <= 640)
		Canvas.Font = baseConsole(level.PlayerHarryActor.player.console).LocalSmallFont;
	else
		Canvas.Font = baseConsole(level.PlayerHarryActor.player.console).LocalMedFont;


	Canvas.TextSize(strHotKey, fXTextLen, fYTextLen);
    nXOffset = (nTEXT_OFFSET_X * fScaleFactor) - fXTextLen/2;
    nYOffset = (nTEXT_OFFSET_Y * fScaleFactor) - fYTextLen/2;
    Canvas.SetPos(nIconX + nXOffset, nIconY + nYOffset);
    Canvas.DrawText(strHotKey, false);

    Canvas.DrawColor = colorSave;
    Canvas.Font      = fontSave;
}

defaultproperties
{
	DrawType=DT_Sprite							
	bHidden=true                                
    CurrSelection=SSelection_Rictusempra
}


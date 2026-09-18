//=============================================================================
// HPHud
//=============================================================================
class HPHud extends baseHUD;

#EXEC TEXTURE IMPORT NAME=frogIcon  FILE=TEXTURES\Menu\HUD\frogIcon.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=fullPotionIcon  FILE=TEXTURES\Menu\HUD\potion2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=halfPotionIcon  FILE=TEXTURES\Menu\HUD\potion1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=EnemyBarFull  FILE=TEXTURES\Menu\HUD\EnemyBarFull.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=EnemyBarEmpty  FILE=TEXTURES\Menu\HUD\EmptyBar.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=EnemyHead1  FILE=TEXTURES\Menu\HUD\EnemyHead1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=EnemyHead2  FILE=TEXTURES\Menu\HUD\EnemyHead2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=EnemyHead3  FILE=TEXTURES\Menu\HUD\EnemyHead3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=EnemyHead4  FILE=TEXTURES\Menu\HUD\EnemyHead4.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=EnemyHead5  FILE=TEXTURES\Menu\HUD\EnemyHead5.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=EnemyHead6  FILE=TEXTURES\Menu\HUD\EnemyHead6.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=MalfoyHead	FILE=TEXTURES\Menu\HUD\Malfoybar.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=VoldemortHead	FILE=TEXTURES\Menu\HUD\Voldemortbar.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=PeevesHead	FILE=TEXTURES\Menu\HUD\Peevesbar.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

//#EXEC TEXTURE IMPORT NAME=FluffyHead	FILE=TEXTURES\HUD\Fluffybar.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
//#EXEC TEXTURE IMPORT NAME=FluffyHeadEmpty   FILE=TEXTURES\HUD\FluffyEmpty.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FluffyHeadMAwake	FILE=TEXTURES\Menu\HUD\FluffybarMAwake.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FluffyHeadMAsleep	FILE=TEXTURES\Menu\HUD\FluffybarMAsleep.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

//#exec Font Import File=Textures\Lrgred.pcx Name=LargeRedFont

var CutSceneManager        managerCutScene;
var ChallengeScoreManager  managerChallenge;
var QuidScoreManager       managerQuidScore;
var MuggleMeterManager     managerMuggleMeter;
var SpellLessonTrigger     managerSpellLesson;
var MagicStrengthManager   managerMagicStrength;
var EnemyHealthManager     managerEnemyHealth;
var CountdownTimerManager  managerCountdownTimer;
var SpellSelector          managerSpellSelector;
var QuidditchBar           managerQuidditchBar;
var VendorManager          CurrVendorManager;
var HProp propArray[20];
var texture               textureMenuBkgrd;
var bool                  bHideStatus;

function StartCutScene()
{
	if (Harry(owner).bIsCaptured)
		bCutSceneMode = true;
	else
		bCutPopupMode = true;
	managerCutScene.StartCutScene();
}
function EndCutScene()
{
	managerCutScene.EndCutScene();
	bCutSceneMode = false;
	bCutPopupMode = false;
}
function SetSubtitleText(string text, float duration)
{
	managerCutScene.SetText(text, duration);
}
function ClearSubtitleText()
{
	managerCutScene.ClearText();
}

function RegisterChallengeManager(ChallengeScoreManager Challenge)
{
	managerChallenge = Challenge;
}

function RegisterQuidScoreManager(QuidScoreManager QuidScore)
{
	managerQuidScore = QuidScore;
}

function RegisterMuggleMeter(MuggleMeterManager MuggleMeter)
{
	managerMuggleMeter = MuggleMeter;
}

function RegisterSpellLesson(SpellLessonTrigger SpellLesson)
{
	managerSpellLesson = SpellLesson;
}

function RegisterMagicStrength(MagicStrengthManager MagicStrength)
{
	managerMagicStrength = MagicStrength;
}

function RegisterEnemyHealth(EnemyHealthManager EnemyHealth)
{
	managerEnemyHealth = EnemyHealth;
}

function RegisterQuidditchBar(QuidditchBar QBar)
{
    managerQuidditchBar = QBar;
}

function RegisterCountdownTimerManager(CountdownTimerManager CountdownTimer)
{
	managerCountdownTimer = CountdownTimer;
}

function RegisterSpellSelector(SpellSelector RegSpellSelector)
{
    managerSpellSelector = RegSpellSelector;
}

function RegisterVendorManager(VendorManager VManager)
{
	CurrVendorManager = VManager;
}

// When props are flying to the hud, they need to be drawn in PostRender if 
// they hit the wall.  This is so they can be forced to display in front of the 
// wall.  When a prop should be drawn in PostRender, it gets registered with
// the hud so the hud can call a function in HProp.
//
// Props that need to be called in PostRender are stored in an array.  When
// a new prop is registered, it is stored in the first "empty" array slot.
// When a prop is unregistered, all props in subsequent slots will be moved up.
// So, if you loop through the prop array and encounter an empty slot, all 
// subsequent slots will be empty.  This allows us to speed things up in 
// PostRender... when incrementing through the array, it can stop looking at 
// array slots as soon as it encounters an empty slot.
function RegisterPickupProp(HProp prop)
{
	local int  i;
	local bool bFoundSlot;

	// Loop until the first empty slot is found.  When it is, save off the
	// new prop in that slot.
	bFoundSlot = false;
	for(i=0; i<ArrayCount(propArray); i++)
	{
		if (propArray[i] == None)	
		{
			bFoundSlot = true;
			propArray[i] = prop;
			break;
		}
	}

	// If there are more props flying at one time than we've anticipated, throw
	// out an error message.
	if (bFoundSlot == false)
	{
		Harry(owner).ClientMessage("WARNING: Not enough prop slots in HPHud");
		log("WARNING: Not enough prop slots in HPHud");
	}

	// Testing stuff.
	//for(i=0; i<ArrayCount(propArray); i++)
	//	Harry(owner).ClientMessage("register list " $" " $i $" " $propArray[i].Name);

}

// See comments for RegisterPickupProp above.
function UnregisterPickupProp(HProp prop)
{
	local int i;
	local int j;

	// Find the slot containing the prop we want to unregister
	for(i=0; i<ArrayCount(propArray); i++)
	{
		// Found the prop in our array.
		if (propArray[i] == prop)
		{
			// Set slot to empty
			propArray[i] = None;

			// Move subsequent props up a slot
			for (j=i+1; j<ArrayCount(propArray); j++)
			{
				// Move up a slot
				propArray[j-1] = propArray[j];
				propArray[j] = None;
			}

			break;  // exit i loop
		}
	}

	// Testing
	//for(i=0; i<ArrayCount(propArray); i++)
	//	Harry(owner).ClientMessage("unregister list" $" " $i $" " $propArray[i].Name);
}

function bool IsCutSceneOrPopupInProgress()
{
	return (bCutSceneMode ||
			bCutPopupMode || 
			managerCutScene.bPopupBorderActive ||
			managerCutScene.bBothBordersActive);
}

event Tick(float deltaTime)
{
	Super.Tick(deltaTime);

	if(bScoreCountup)
	{
		fScoreCountTime -= deltaTime;
		if(fScoreCountTime <= 0)
		{
			fScoreCountTime = 0;
			bScoreCountup = false;
		}
	}
}

function DrawSpellIcon(Canvas canvas)
{
local Texture icon;

	icon=baseWand(PlayerPawn(Owner).weapon).GetSpellIcon();

	if(Icon!=None)
		{
		Canvas.SetPos(5,(Canvas.SizeY-64)-5);
		Canvas.DrawIcon(icon,1);
		}
}


function DrawHoops(Canvas canvas, int iNumber, int iMaxNumber)
{
	local int	Ox, Oy;

	Ox = 8;
	Oy = Canvas.SizeY - 156;			// magic numbers I'm afraid, the bitmaps have to be of a certain

	if (iNumber < 10)
	{
		Canvas.SetPos(Ox + 94, Oy + 100);
	}
	else
	{
		Canvas.SetPos(Ox + 85, Oy + 100);
	}
	Canvas.DrawText(iNumber $"/" $iMaxNumber, False);
/*	Canvas.SetPos(Ox + 135,Oy + 104);
	Canvas.DrawText(iMaxNumber , False);*/
//	Ox = Canvas.SizeX / 2 - 128;
//	Oy = Canvas.SizeY - 176;			// magic numbers I'm afraid, the bitmaps have to be of a certain
									// size, and the actual graphic is inside it.
/*	Canvas.SetPos(Ox - 8,Oy + 128 - 10);
	Canvas.DrawText(iNumber, False);
	Canvas.SetPos(Ox + 256,Oy + 128 - 10);
	Canvas.DrawText(iMaxNumber , False);*/
}


//****************************************************************************************************************************************
simulated function PreBeginPlay()
{
	local int i;

	Super.PreBeginPlay();

	if (managerCutScene == None)
		managerCutScene = spawn(class'CutSceneManager');

	// @@@ For now a pixel from the ememy strength bar will be used for drawing a
	// fade over the screen when the in game menu is up
	textureMenuBkgrd = texture(DynamicLoadObject("HP_Menu.Hud.MagicStrengthEmpty" , class'Texture'));

	// Initialize prop array.
	for(i=0; i<ArrayCount(propArray); i++)
		propArray[i] = None;

}

simulated function PostBeginPlay()
{
	Super.PostBeginPlay();
}

simulated function bool DisplayMessages(canvas Canvas)
{
	if(HPConsole(playerpawn(owner).player.console).bDebugMode)
		return(false);	//allow base class to draw messages

	return true;	//tell base class not to draw messages
}

function DrawInGameMenuBkgrd(Canvas canvas)
{
	Canvas.SetPos(0, 0);
	Canvas.Style = 3;
	canvas.DrawTile(textureMenuBkgrd, canvas.SizeX, Canvas.SizeY, 0, 0, 1, 1);
	Canvas.Style = 1;
}

simulated function PostRender( canvas Canvas )
{
	local FEBook menuBook;
	local int    i;
	local bool   bInGameMenuUp;
    local bool   bFullCutMode;
    local bool   bHalfCutMode;

	HUDSetup(canvas);

    bFullCutMode = (bCutSceneMode == true) || managerCutScene.bBothBordersActive;
    bHalfCutMode = (bCutPopupMode == true)  || managerCutScene.bPopupBorderActive;

	if ( PlayerPawn(Owner) != None )
		{
		if ( PlayerPawn(Owner).PlayerReplicationInfo == None )
			return;
		}

	menuBook = HPConsole(playerpawn(owner).player.console).MenuBook;
	if (menuBook != None)
	{
		if (menuBook.bIsOpen)
		{
			bInGameMenuUp = menuBook.IsInGameMenuShowing();
			if (!bInGameMenuUp)
				return;
		}
	}

	if (bInGameMenuUp)
	{
		DrawInGameMenuBkgrd(Canvas);
		if (!menuBook.IsInGameSubMenuShowing())
			Harry(owner).managerStatus.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);
	}
	else
	{
		// If just bottom cut border, render cutscene
		if (bHalfCutMode)
			managerCutScene.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

		// If full cutscene is in progress, render the cutscene
		if(bFullCutMode)
			managerCutScene.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

		// Only draw these hud items if full cutscene is not in progress
		else
		{	
			// @PAB debug info
	//		DrawDebug(Canvas);

			if (managerEnemyHealth != None)
				managerEnemyHealth.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

			if (managerQuidditchBar != None)
				managerQuidditchBar.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

			if (managerMagicStrength != None)
				managerMagicStrength.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

			if (managerCountdownTimer != None)
				managerCountdownTimer.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

            if (managerSpellSelector != None)
                managerSpellSelector.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

		}

		if (managerSpellLesson == None && managerMagicStrength == None && !bHideStatus)
			Harry(owner).managerStatus.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

		// Update props that need to be postrendered (See comments in
		// RegisterPickupProp() above).
		for(i=0; i<ArrayCount(propArray); i++)
		{
			if (propArray[i] == None)	
				break;
			else
				propArray[i].RenderHud(Canvas);
		}

		// Let vendor rendor it's hud
		if (CurrVendorManager != None)
			CurrVendorManager.RenderHud(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

		// Call ChallengeManager when in or out of cutscene.  It will determine
		// when it needs to draw.
		if (managerChallenge != None)
			managerChallenge.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

		// Call QuidScoreManager when in or out of cutscene.  It will determine
		// when it needs to draw.
		if (managerQuidScore != None)
			managerQuidScore.RenderHudItemManager(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

		// Always pass along render to spell lesson- it will decide whether or
		// not to draw in cutscene mode.
		if (managerSpellLesson != None)
			managerSpellLesson.RenderHudItems(Canvas, bInGameMenuUp, bFullCutMode, bHalfCutMode);

		DrawPopup(Canvas);		//moved here by gk 7/26 to allow popup during cutscene
								// PAB 10/18 Moved here so that they appear on top of health etc
	}
}

// Draw cutscene style text (currently used by cutscenes and wizard card descriptions on folio menu).
function DrawCutStyleText(Canvas canvas, string strText, int nXPos, int nYPos, int nHeight, color colorText, optional Font fontText)
{
	local font   fontSave;
	local color  colorSave;
	local int    nStyleSave;
	local float  fTextW, fTextH;
	local int    nLines, nAvailLines;
	local string strTextLine, strSearch;
	local int    nOrgPos, nNewPos;

	// If no text to draw, just return.
	if (strText == "")
		return;

	// Save off canvas props
	fontSave   = Canvas.Font;
	colorSave  = Canvas.DrawColor;
	nStyleSave = Canvas.Style;

	// Set canvas font, style and color
	//Canvas.Font=baseConsole(level.PlayerHarryActor.player.console).LocalIconMessageFont;
    if (fontText == None)
    	Canvas.Font=baseConsole(playerpawn(owner).player.console).LocalMedFont;
    else
        Canvas.Font = fontText;
	Canvas.Style = 2;
	Canvas.DrawColor = colorText;

	// Calculate size of text at current font
	Canvas.TextSize(strText, fTextW, fTextH);

	//Massive KLUDGE. cmp 10-18 The +90 below is a fudge factor to overcome the spaces added to a string when it gets word wrapped. 
	//specificly to fix German storybook_new_20
	nLines = ((fTextW+90)/Canvas.SizeX)+1;
	nAvailLines = nHeight/ fTextH;

	// If text won't fit at current font, try other fonts
	if(nLines > nAvailLines)
	{
		Canvas.Font=baseConsole(playerpawn(owner).player.console).LocalMedFont;
		Canvas.TextSize(strText, fTextW, fTextH);
		nLines = ((fTextW+90)/Canvas.SizeX)+1;
		nAvailLines= nHeight / fTextH;
		if(nLines > nAvailLines)
			{
			Canvas.Font=baseConsole(playerpawn(owner).player.console).LocalSmallFont;
			canvas.TextSize(strText, fTextW, fTextH);
			nLines=((fTextW+90)/Canvas.SizeX)+1;
			nAvailLines = nHeight / fTextH;
			if(nLines > nAvailLines)
			{
				Canvas.Font=baseConsole(playerpawn(owner).player.console).LocalTinyFont;
				Canvas.TextSize(strText, fTextW, fTextH);
				nLines=((fTextW+90)/Canvas.SizeX)+1;
				nAvailLines = nHeight / fTextH;
				}
			}
	}

	if (caps(GetLanguage()) == "THA")
	{
		strTextLine = "";

		nOrgPos = 0;

		Canvas.SetPos(nXPos, nYPos);

		strSearch = strText;

		while (nOrgPos <= Len(strText))
		{
			nNewPos = InStr(strSearch, "_");

			if (nNewPos != -1)
			{
				strTextLine = strTextLine $Left(strSearch, nNewPos);
			}
			else
			{
				strTextLine = strTextLine $strSearch;
			}

			Canvas.TextSize(strTextLine, fTextW, fTextH);

			if ( fTextW > canvas.SizeX - 16 - nXPos)
			{
				// We've gone past the line, go back and print out the string
				strTextLine = Left(strTextLine, nOrgPos - 1);
				Canvas.DrawText(strTextLine, False);	
				nYPos += fTextH;
				Canvas.SetPos(nXPos, nYPos);
				strTextLine = "";
			}
			else
			{
				if (nNewPos != -1)
				{
					nOrgPos += nNewPos;
					strSearch = Right(strSearch, Len(strSearch) - nNewPos - 1);
				}
				else
				{
					break;
				}
			}
		}

		Canvas.TextSize(strTextLine, fTextW, fTextH);

		if ( fTextW < canvas.SizeX - 16 - nXPos)
		{
			nXPos = (canvas.SizeX - fTextW - nXPos) / 2;
		}

		Canvas.SetPos(nXPos, nYPos);
		Canvas.DrawText(strTextLine, False);	
	}
	else
	{
		if ( fTextW < canvas.SizeX - 16 - nXPos)
		{
			nXPos = (canvas.SizeX - fTextW - nXPos) / 2;
		}

		Canvas.SetPos(nXPos, nYPos);
		Canvas.DrawText(strText, False);

	}

	Canvas.Font      = fontSave;
	Canvas.DrawColor = colorSave;
	Canvas.Style     = nStyleSave;
}


auto state Loading
{
    event BeginState()
    {
        local Cutscene aCut;

        // If there's a cutscene set to start on level load, we're going to flag
        // status items (especially health) not to draw for a bit.  This is so
        // when a level loads, status items don't flash on the screen momentarily
        // before the cutscene actually starts.
    	foreach AllActors(class'CutScene', aCut)
        {
            if (aCut.bLevelLoadStarts)
            {
	    	    bHideStatus = true;
                break;
            }
        }
    }

begin:
    if (bHideStatus)
    {
        sleep (1.0);
        bHideStatus = false;
        GoToState('Idle');
    }
}

state Idle
{
    event BeginState()
    {
        bHideStatus = false;
    }
}


defaultproperties
{
}

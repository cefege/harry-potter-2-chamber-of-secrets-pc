class FEInGamePage expands baseFEPage;

const fTOP_BAR_X =   0;
const fTOP_BAR_Y =   0;
const fTOP_BAR_H =  88;

const fBOTTOM_BAR_X =   0;
const fBOTTOM_BAR_Y = 412;
const fBOTTOM_BAR_H =  68;

const fOBJECTIVE_LABEL_Y        = 420;
const fOBJECTIVE_LABEL_H        = 16;
const fOBJECTIVE_TEXT_X         = 6;
const fOBJECTIVE_TEXT_Y         = 436;
const fOBJECTIVE_TEXT_W         = 640;
const fOBJECTIVE_TEXT_H         = 38;

var HPMessageBox ConfirmQuit;

var bool bSetupAfterPageSwitch;

// Top bar
var UWindowButton BeansButton;
var UWindowButton HousepointsButton;
var UWindowButton SecretsButton;
var UWindowButton PotionsButton;
var UWindowButton FMucusButton;
var UWindowButton WBarkButton;

// Middle of screen
var UWindowButton ChallengesButton;
var UWindowButton DuelButton;
var UWindowButton FolioButton;
var UWindowButton MapButton;
var UWindowButton QuidditchButton;

// Bottom Row of buttons
var UWindowButton QuitButton;
var UWindowButton InputButton;
var UWindowButton CreditsButton;
var UWindowButton SoundVideoButton;

var texture textureLowerBarLeft;
var texture textureLowerBarMid;
var texture textureLowerBarRight;
var texture textureTopBarLeft;
var texture textureTopBarMid;
var texture textureTopBarRight;

var string strCurrToolTip;

function Paint(Canvas canvas,float x,float y)
{
	local float   fScaleFactor;
	local bool    bHaveObjectiveText;

	fScaleFactor = Canvas.SizeX/WinWidth; 

    DrawBarBackgrounds(Canvas, fScaleFactor);

    if (strCurrToolTip != "")
        PaintToolTipText(Canvas, fScaleFactor);
    else
	    PaintObjectiveText(Canvas, fScaleFactor);    

	Super.Paint(canvas, x, y);
}

function AfterPaint(Canvas canvas,float x,float y)
{
	local float   fScaleFactor;
	local bool    bHaveObjectiveText;

	fScaleFactor = Canvas.SizeX/WinWidth; 

    PaintCountText(Canvas, fScaleFactor);

    Super.AfterPaint(canvas, x, y);
}

function PaintToolTipText(Canvas canvas, float fScaleFactor)
{
    local font   fontText;
    local color  colorText;
    local Harry  playerHarry;

    // Get easy access to Harry
	playerHarry = Harry(Root.Console.viewport.actor);

    //DrawObjectiveAndTipBackground(Canvas, fScaleFactor);

    // Select font to draw text in.
    if (Canvas.SizeX <= 512)
        fontText = baseConsole(playerHarry.player.console).LocalSmallFont;
    else
        fontText = baseConsole(playerHarry.player.console).LocalMedFont;

    // Select color for text
    colorText.R = 255;
    colorText.G = 255;
    colorText.B = 255;

    HPHud(playerHarry.myHud).DrawCutStyleText(Canvas,
	                                          strCurrToolTip,
                                              fOBJECTIVE_TEXT_X * fScaleFactor,
	                                          fOBJECTIVE_TEXT_Y * fScaleFactor,
	                                          fOBJECTIVE_TEXT_H * fScaleFactor,
				     					      colorText,
                                              fontText);
}

function PaintObjectiveText(Canvas canvas, float fScaleFactor)
{
    local string strObjectiveId; 
    local string strObjective;
    local font   fontText;
    local color  colorText;
    local Harry  playerHarry;

    // Get easy access to Harry
	playerHarry = Harry(Root.Console.viewport.actor);

    // Get objective id
	strObjectiveId = playerHarry.strObjectiveId;

    // If have objective text, display it.
    if (strObjectiveId != "")
    {
        // Get actual text from the id.
		strObjective = Localize("All", strObjectiveId, "HPDialog");
        
	    strObjective = playerHarry.HandleFacialExpression(strObjective, 0, true);

        // Select font to draw text in.
        if (Canvas.SizeX <= 512)
            fontText = baseConsole(playerHarry.player.console).LocalSmallFont;
        else
            fontText = baseConsole(playerHarry.player.console).LocalMedFont;

        // Select color for text
        colorText.R = 255;
        colorText.G = 255;
        colorText.B = 255;

        // Draw Objective "label": "Objective"
	    //Canvas.SetPos(fOBJECTIVE_TEXT_X, fOBJECTIVE_LABEL_Y * fScaleFactor);
	    HPHud(playerHarry.myHud).DrawCutStyleText(Canvas,
		                                          GetLocalFEString("InGameMenu_0027"),
                                                  fOBJECTIVE_TEXT_X * fScaleFactor,
		                                          fOBJECTIVE_LABEL_Y * fScaleFactor,
		                                          fOBJECTIVE_LABEL_H * fScaleFactor,
					     					      colorText,
                                                  fontText);


        // Draw objective text.
	    //Canvas.SetPos(fOBJECTIVE_TEXT_X, fOBJECTIVE_TEXT_Y * fScaleFactor);
	    HPHud(playerHarry.myHud).DrawCutStyleText(Canvas,
		                                          strObjective,
                                                  fOBJECTIVE_TEXT_X * fScaleFactor,
		                                          fOBJECTIVE_TEXT_Y * fScaleFactor,
		                                          fOBJECTIVE_TEXT_H * fScaleFactor,
					     					      colorText,
                                                  fontText);
    }			
}

function DrawBarBackgrounds(Canvas canvas, float fScaleFactor)
{
    // Draw bottom bar (left, middle and right pieces)
	Canvas.SetPos(fBOTTOM_BAR_X * fScaleFactor, fBOTTOM_BAR_Y * fScaleFactor);
	Canvas.DrawTile(textureLowerBarLeft, textureLowerBarLeft.USize * fScaleFactor, fBOTTOM_BAR_H * fScaleFactor, 0, 0, textureLowerBarLeft.USize, fBOTTOM_BAR_H);
    Canvas.SetPos((fBOTTOM_BAR_X + textureLowerBarLeft.USize)* fScaleFactor, fBOTTOM_BAR_Y * fScaleFactor);
    Canvas.DrawTile(textureLowerBarMid, textureLowerBarMid.USize * fScaleFactor, fBOTTOM_BAR_H * fScaleFactor, 0, 0, textureLowerBarMid.USize, fBOTTOM_BAR_H);
    Canvas.SetPos(Canvas.SizeX - (textureLowerBarRight.USize * fScaleFactor), fBOTTOM_BAR_Y * fScaleFactor);
    Canvas.DrawTile(textureLowerBarRight, textureLowerBarRight.USize * fScaleFactor, fBOTTOM_BAR_H * fScaleFactor, 0, 0, textureLowerBarRight.USize, fBOTTOM_BAR_H);

    // Draw top bar (left, middle and right pieces
	Canvas.SetPos(fTOP_BAR_X * fScaleFactor, fTOP_BAR_Y * fScaleFactor);
	Canvas.DrawTile(textureTopBarLeft, textureTopBarLeft.USize * fScaleFactor, fTOP_BAR_H * fScaleFactor, 0, 0, textureTopBarLeft.USize, fTOP_BAR_H);
    Canvas.SetPos((fTOP_BAR_X + textureTopBarLeft.USize)* fScaleFactor, fTOP_BAR_Y * fScaleFactor);
    Canvas.DrawTile(textureTopBarMid, textureTopBarMid.USize * fScaleFactor, fTOP_BAR_H * fScaleFactor, 0, 0, textureTopBarMid.USize, fTOP_BAR_H);
    Canvas.SetPos(Canvas.SizeX - (textureTopBarRight.USize * fScaleFactor), fTOP_BAR_Y * fScaleFactor);
    Canvas.DrawTile(textureTopBarRight, textureTopBarRight.USize * fScaleFactor, fTOP_BAR_H * fScaleFactor, 0, 0, textureTopBarRight.USize, fTOP_BAR_H);
}

function PaintCountText(Canvas canvas, float fScaleFactor)
{
    local StatusManager managerStatus;
    local StatusItem    si;

    managerStatus = Harry(Root.Console.viewport.actor).managerStatus;

    si = managerStatus.GetStatusItem(class'StatusGroupJellybeans', class'StatusItemJellybeans');
    si.DrawCount(Canvas, BeansButton.WinLeft * fScaleFactor, BeansButton.WinTop * fScaleFactor, fScaleFactor);

    si = managerStatus.GetStatusItem(class'StatusGroupHousepoints', class'StatusItemGryffindorPts');
    si.DrawCount(Canvas, HousepointsButton.WinLeft * fScaleFactor, HousepointsButton.WinTop * fScaleFactor, fScaleFactor);

    si = managerStatus.GetStatusItem(class'StatusGroupPotions', class'StatusItemWiggenwell');
    si.DrawCount(Canvas, PotionsButton.WinLeft * fScaleFactor, PotionsButton.WinTop * fScaleFactor, fScaleFactor);

    si = managerStatus.GetStatusItem(class'StatusGroupPotionIngr', class'StatusItemFlobberMucus');
    si.DrawCount(Canvas, FMucusButton.WinLeft * fScaleFactor, FMucusButton.WinTop * fScaleFactor, fScaleFactor);

    si = managerStatus.GetStatusItem(class'StatusGroupPotionIngr', class'StatusItemWiggenBark');
    si.DrawCount(Canvas, WBarkButton.WinLeft * fScaleFactor, WBarkButton.WinTop * fScaleFactor, fScaleFactor);
}

function int GetObjectiveAreaTop(int nCanvasSizeX, int nCanvasSizeY)
{
	local float fScaleFactor;

	fScaleFactor = nCanvasSizeX/WinWidth; 

    return (nCanvasSizeY - fBOTTOM_BAR_H * fScaleFactor);
}

function Created()
{
    textureLowerBarLeft   = texture(DynamicLoadObject("HP_Menu.Hud.MenuBottomBarLeft", class'Texture'));
    textureLowerBarMid    = texture(DynamicLoadObject("HP_Menu.Hud.MenuBottomBarMid", class'Texture'));
    textureLowerBarRight  = texture(DynamicLoadObject("HP_Menu.Hud.MenuBottomBarRight", class'Texture'));
    textureTopBarLeft     = texture(DynamicLoadObject("HP_Menu.Hud.MenuTopBarLeft", class'Texture'));
    textureTopBarMid      = texture(DynamicLoadObject("HP_Menu.Hud.MenuTopBarMid", class'Texture'));
    textureTopBarRight    = texture(DynamicLoadObject("HP_Menu.Hud.MenuTopBarRight", class'Texture'));

    // Top row of buttons
	PotionsButton=UWindowButton(CreateControl(class'UWindowButton', 30,10,52,64));
	PotionsButton.ToolTipString=GetLocalFEString("InGameMenu_0020");
	PotionsButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.WiggenwellPotion", class'Texture'));;     
	PotionsButton.OverTexture=PotionsButton.UpTexture; 
    PotionsButton.DownTexture=PotionsButton.OverTexture;
    PotionsButton.DownSound=None;

	FMucusButton=UWindowButton(CreateControl(class'UWindowButton', 110,10,54,64));
	FMucusButton.ToolTipString=GetLocalFEString("InGameMenu_0008");
	FMucusButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.FlobberwormMucus", class'Texture'));;     
	FMucusButton.OverTexture=FMucusButton.UpTexture; 
    FMucusButton.DownTexture=FMucusButton.OverTexture;
    FMucusButton.DownSound=None;

	WBarkButton=UWindowButton(CreateControl(class'UWindowButton', 192,10,60,64));
	WBarkButton.ToolTipString=GetLocalFEString("InGameMenu_0019");
	WBarkButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.WiggentreeBark", class'Texture'));;     
	WBarkButton.OverTexture=WBarkButton.UpTexture; 
    WBarkButton.DownTexture=WBarkButton.OverTexture;
    WbarkButton.DownSound=None;

	BeansButton=UWindowButton(CreateControl(class'UWindowButton', 430,10,52,64));
	BeansButton.ToolTipString=GetLocalFEString("InGameMenu_0013");
	BeansButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.BeanCounter", class'Texture'));    
	BeansButton.OverTexture=BeansButton.UpTexture; 
    BeansButton.DownTexture=BeansButton.OverTexture;
    BeansButton.DownSound=None;

	SecretsButton=UWindowButton(CreateControl(class'UWindowButton', 562,14,64,64));
	SecretsButton.ToolTipString=GetSecretsText();
	SecretsButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuSecrets", class'Texture'));     
	SecretsButton.OverTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuSecretsRO", class'Texture'));     
    SecretsButton.DownTexture=SecretsButton.OverTexture;
    SecretsButton.DownSound=None;

	HousepointsButton=UWindowButton(CreateControl(class'UWindowButton', 280,5,74,95));
	HousepointsButton.ToolTipString=GetLocalFEString("InGameMenu_0046");
	HousepointsButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.HousepointsGryff", class'Texture'));     
	HousepointsButton.OverTexture=HousepointsButton.UpTexture; 
    HousepointsButton.DownTexture=HousepointsButton.OverTexture;

    // Middle of screen buttons
  	ChallengesButton=UWindowButton(CreateControl(class'UWindowButton', 146,114,64,64));
	ChallengesButton.ToolTipString=GetLocalFEString("InGameMenu_0044");
	ChallengesButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuChallenges", class'Texture'));
	ChallengesButton.OverTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuChallengesRO", class'Texture')); 
    ChallengesButton.DownTexture=ChallengesButton.OverTexture;

  	DuelButton=UWindowButton(CreateControl(class'UWindowButton', 146,248,64,64));
	DuelButton.ToolTipString=GetLocalFEString("InGameMenu_0043");
	DuelButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuWizardDuel", class'Texture'));;    
    DuelButton.OverTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuWizardDuelRO", class'Texture'));    
	DuelButton.DownTexture=DuelButton.OverTexture; 

	FolioButton=UWindowButton(CreateControl(class'UWindowButton', 252,134,136,106));
	FolioButton.ToolTipString=GetLocalFEString("InGameMenu_0004");
	FolioButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuFolioButtonIdle", class'Texture'));
	FolioButton.OverTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuFolioButtonRO", class'Texture'));
    FolioButton.DownTexture=FolioButton.OverTexture;

  	MapButton=UWindowButton(CreateControl(class'UWindowButton', 438,114,64,64));
	MapButton.ToolTipString=GetLocalFEString("InGameMenu_0045");
	MapButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuMap", class'Texture'));
	MapButton.OverTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuMapRO", class'Texture')); 
    MapButton.DownTexture=MapButton.OverTexture;

  	QuidditchButton=UWindowButton(CreateControl(class'UWindowButton', 438,248,64,64));
	QuidditchButton.ToolTipString=GetLocalFEString("InGameMenu_0042");
	QuidditchButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuQuidditch", class'Texture'));
	QuidditchButton.OverTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuQuidditchRO", class'Texture')); 
    QuidditchButton.DownTexture=QuidditchButton.OverTexture;

    // Bottom row of buttons
	QuitButton=UWindowButton(CreateControl(class'UWindowButton', 12,354,48,48));
	QuitButton.ToolTipString=GetLocalFEString("InGameMenu_0002");
	QuitButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuQuit", class'Texture')); 
	QuitButton.OverTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuQuitRO", class'Texture'));
    QuitButton.DownTexture=QuitButton.OverTexture;

	InputButton=UWindowButton(CreateControl(class'UWindowButton',72,354,48,48));
	InputButton.ToolTipString=GetLocalFEString("InGameMenu_0047");
	InputButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuInputOptions", class'Texture')); 
	InputButton.OverTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuInputOptionsRO", class'Texture'));
    InputButton.DownTexture=InputButton.OverTexture;
	
	SoundVideoButton=UWindowButton(CreateControl(class'UWindowButton',520,354,48,48));
	SoundVideoButton.ToolTipString=GetLocalFEString("InGameMenu_0048");
	SoundVideoButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuSoundOptions", class'Texture')); 
	SoundVideoButton.OverTexture=texture(DynamicLoadObject("HP_Menu.Hud.MenuSoundOptionsRO", class'Texture')); 
    SoundVideoButton.DownTexture=SoundVideoButton.OverTexture;
	
    CreateBackPageButton(578, 354);

    if(HPConsole(root.console).bDebugMode)
	{
		CreditsButton=UWindowButton(CreateControl(class'UWindowButton',135,354,48,48));
        CreditsButton.ToolTipString="Credits";
		CreditsButton.UpTexture=InputButton.UpTexture;
		CreditsButton.OverTexture=InputButton.OverTexture;
        CreditsButton.DownTexture=InputButton.DownTexture;
	}

	Super.Created(); 
}	

function WindowDone(UWindowWindow W)
{
	if(W == ConfirmQuit)
	{
		if(ConfirmQuit.Result == ConfirmQuit.button1.text)
		{
			Root.DoQuitGame();
		}
		ConfirmQuit = None;
	}
}

function bool KeyEvent( byte/*EInputKey*/ Key, byte/*EInputAction*/ Action, FLOAT Delta )
{
	return (false);
}

function Notify(UWindowDialogControl C, byte E)
{
	local int i;

	if(e==DE_Click)
	{		
		switch(c)								
		{
		case InputButton:
			FEBook(book).ChangePageNamed( "INPUT" );
			break;
		case SoundVideoButton:
			FEBook(book).ChangePageNamed( "SOUNDVIDEO" );
			break;
		case QuitButton:
			ConfirmQuit = doHPMessageBox(GetLocalFEString("InGameMenu_0026"), //"Are you sure you want to quit this game?"
									     GetLocalFEString("Shared_Menu_0003"),// "Yes"
										 GetLocalFEString("Shared_Menu_0004") //"No"
										);
			break;
        case BackPageButton:
            FEBook(book).CloseBook();
            break;
		case FolioButton:
			FEBook(book).ChangePageNamed("FOLIO");
			break;
		case QuidditchButton:
			FEBook(book).ChangePageNamed("QUIDDITCH");
			break;
		case DuelButton:
			FEBook(book).ChangePageNamed("DUEL");
			break;
		case ChallengesButton:
			FEBook(book).ChangePageNamed("CHALLENGES");
			break;
        case MapButton:
			FEBook(book).ChangePageNamed("MAP");
			break;
		case CreditsButton:
			FEBook(book).ChangePageNamed( "CREDITSPAGE" );
			break;
        case HousepointsButton:
			FEBook(book).ChangePageNamed( "HPOINTS" );
			break;
		default :
			break;
		}
	}
}

function PreSwitchPage()
{    
	Super.PreSwitchPage();
    SecretsButton.ToolTipString=GetSecretsText();
}

function ToolTip(string strSetTip)
{
    strCurrToolTip = strSetTip;
}

function string GetSecretsText()
{
    local string           strSecrets;
    local int              nNumSecrets;
    local int              nNumSecretsFound;
    local SecretAreaMarker Marker;

	foreach (Root.Console.viewport.actor).AllActors(class'SecretAreaMarker', Marker)
    {
        Harry(Root.Console.viewport.actor).ClientMessage("found secret area marker");
        nNumSecrets++;
        if (Marker.bFound)
            nNumSecretsFound++;
    }

    strSecrets = GetLocalFEString("Report_Card_0006");
    strSecrets = nNumSecretsFound $"/" $nNumSecrets $" " $strSecrets;

    return (strSecrets);
}

defaultProperties
{
}
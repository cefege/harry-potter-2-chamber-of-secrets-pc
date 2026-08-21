//===============================================================================
//  [FEFolioPage] 
//
//  The folio magi page is accessed from the In Game menu and displays data
//  about the wizard cards that the player has picked up.
//
//===============================================================================

class FEFolioPage expands baseFEPage;

// Imports for missing and hilite wizard cards.  Non-missing textures will be
// retrieved from the corresponding WizardCardIcon derived class.
#EXEC TEXTURE IMPORT NAME=WizCardMissingBigTexture		FILE=TEXTURES\menu\Folio\Cards\missingcardbig.bmp		GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardMissingSmallTexture	FILE=TEXTURES\menu\Folio\Cards\missingcardsmall.bmp		GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardHilite	                FILE=TEXTURES\menu\Folio\Cards\cardhighlight.pcx		GROUP="Icons" FLAGS=2 MIPS=OFF 

// Number of cards in each wizard card set.  Each set will display on a page of the folio.
const nCARDS_PER_SET = 10;

// Number of pages for each card group.
const nNUM_PAGES_BRONZE = 5;
const nNUM_PAGES_SILVER = 4;
const nNUM_PAGES_GOLD   = 1;

// Large card coordinates for a 640x480 screen.  These values will be
// scaled in other resolutions.
const nLARGE_CARD_X  = 192;
const nLARGE_CARD_Y  = 4;

// Status (tooltip) text vertical coord
const nSTATUS_BAR_Y  = 374;

// Location and height of wizard description (640x480 coords- scaled in other resoltions).
const nWIZARD_TEXT_X = 0;
const nWIZARD_TEXT_Y = 400;
const nWIZARD_TEXT_H = 80;

// Tip text for next and previous folio pages.
const strPAGE0 = "1 - 10";
const strPAGE1 = "11 - 20";
const strPAGE2 = "21 - 30";
const strPAGE3 = "31 - 40";
const strPAGE4 = "41 - 50";


// Easy access to Harry
var Harry playerHarry;

// Wizard description text color changes based on card group currently selected.
var color colorBronze, colorSilver, colorGold;

// Buttons for Harry's card, the hilite card and the small cards for the current set.
var UWindowButton  HarryCardBmp;
var UWindowButton  HiliteCardBmp;
var UWindowButton  SmallCardBmp [10];

// Forward and back page buttons
var UWindowButton ForwardButton;
var UWindowButton BackButton;

// Next and previous page contents shown next to forward and back buttons.
var UWindowLabelControl PreviousPageLabel;
var UWindowLabelControl NextPageLabel;

// Wizard card group buttons.
var UWindowButton BronzeButton;
var UWindowButton SilverButton;
var UWindowButton GoldButton;

// Folio textures.
var texture textureDescBkgrd;	    // Wizard description background (black bar)
var texture textureRightUp;         // Next page normal state
var texture textureRightOver;       // Next page hilite/press state
var texture textureLeftUp;          // Previous page normal state
var texture textureLeftOver;        // Previous page hilite/press state
var texture textureBronzeNormal;    // Bronze group button normal state
var texture textureBronzeHilite;    // Bronze group button selected state
var texture textureSilverNormal;    // Silver group button normal state
var texture textureSilverHilite;    // Silver group button selected state
var texture textureGoldNormal;      // Gold group button normal state
var texture textureGoldHilite;      // Gold group button selected state
var texture textureSilverKey;       // Key texture- one displayed for each silver set
var texture textureBronzeHealth;    // Health bar- 1 displayed for each bronze set
var texture textureCurrLargeCard;   // Large card texture set based on current small card selected

//**************************************************************************************************
	//CMP: save cur class for layered 3d card support
var class<WizardCardIcon> classCurWC;
//**************************************************************************************************

var string  strCardCountBronze;     // "X / Y" count text
var string  strCardCountSilver;
var string  strCardCountGold;

var int     nBronzeHealthBars;      // Display this many health bars next to bronze selector
var int     nSilverKeys;            // Display this many keys next to silver selector

// Possible card groups
enum ECardGroup
{
	CardGroup_Bronze,
	CardGroup_Silver,
	CardGroup_Gold
};

// Current state of things
var ECardGroup CurrCardGroup;       // Current card group- gold, silver, bronze
var int        nCurrPage;           // Current page within card group
var int        nCurrItemOnPage;     // Current small card selected on page
var int        nCurrNumPages;       // Number of pages in current group
var string     strCurrDesc;         // Wizard description for current card


// FEFolioPage Created.  Load in textures and create buttons.
function Created()
{
	local int i;                    // Looping var

	// Load all textures
	textureDescBkgrd    = texture(DynamicLoadObject("HGame.Icons.leftPanel" , class'Texture'));
	textureBronzeNormal = texture(DynamicLoadObject("HP_Menu.Hud.FolioBronzeNormal", class'Texture'));
	textureBronzeHilite = texture(DynamicLoadObject("HP_Menu.Hud.FolioBronzeHilite", class'Texture'));
	textureSilverNormal = texture(DynamicLoadObject("HP_Menu.Hud.FolioSilverNormal", class'Texture'));
	textureSilverHilite = texture(DynamicLoadObject("HP_Menu.Hud.FolioSilverHilite", class'Texture'));
	textureGoldNormal   = texture(DynamicLoadObject("HP_Menu.Hud.FolioGoldNormal", class'Texture'));
	textureGoldHilite   = texture(DynamicLoadObject("HP_Menu.Hud.FolioGoldHilite", class'Texture'));
	textureRightUp      = texture(DynamicLoadObject("HP_Menu.Hud.FolioRightUp", class'Texture'));
	textureRightOver    = texture(DynamicLoadObject("HP_Menu.Hud.FolioRightOver", class'Texture'));
	textureLeftUp       = texture(DynamicLoadObject("HP_Menu.Hud.FolioLeftUp", class'Texture'));
	textureLeftOver     = texture(DynamicLoadObject("HP_Menu.Hud.FolioLeftOver", class'Texture'));
	textureSilverKey    = texture(DynamicLoadObject("HP_Menu.Hud.SilverCardKey", class'Texture'));
	textureBronzeHealth = texture(DynamicLoadObject("HP_Menu.Hud.FolioBronzeHealth", class'Texture'));

    // Create Harry card that is to the side of currently selected big card.
	HarryCardBmp = UWindowButton(CreateWindow(class'UWindowButton', 480, 50, 128, 128));
	HarryCardBmp.Register(self);
	HarryCardBmp.bDisabled = true;
	HarryCardBmp.bStretched = true;

	// Small cards on the current page
	SmallCardBmp[0] = UWindowButton(CreateWindow(class'UWindowButton',  5,  254, 60, 60));
	SmallCardBmp[1] = UWindowButton(CreateWindow(class'UWindowButton',  68, 254, 60, 60));
	SmallCardBmp[2] = UWindowButton(CreateWindow(class'UWindowButton', 131, 254, 60, 60));
	SmallCardBmp[3] = UWindowButton(CreateWindow(class'UWindowButton', 194, 254, 60, 60));
	SmallCardBmp[4] = UWindowButton(CreateWindow(class'UWindowButton', 257, 254, 60, 60));
	SmallCardBmp[5] = UWindowButton(CreateWindow(class'UWindowButton', 320, 254, 60, 60));
	SmallCardBmp[6] = UWindowButton(CreateWindow(class'UWindowButton', 383, 254, 60, 60));
	SmallCardBmp[7] = UWindowButton(CreateWindow(class'UWindowButton', 446, 254, 60, 60));
	SmallCardBmp[8] = UWindowButton(CreateWindow(class'UWindowButton', 509, 254, 60, 60));
	SmallCardBmp[9] = UWindowButton(CreateWindow(class'UWindowButton', 572, 254, 60, 60));

    // Setup props for small cards.
	for (i=0; i !=ArrayCount(SmallCardBmp); ++i)
	{
		SmallCardBmp[i].Register(self);
		SmallCardBmp[i].bStretched = true;
	}

	// Hilite card (hilites small or Harry card)
	HiliteCardBmp = UWindowButton(CreateWindow(class'UWindowButton',  5,  254, 60, 60));
	HiliteCardBmp.UpTexture   = Texture'WizCardHilite';
	HiliteCardBmp.DownTexture = HiliteCardBmp.UpTexture;
	HiliteCardBmp.OverTexture = HiliteCardBmp.UpTexture;
	HiliteCardBmp.Register(self);
	HiliteCardBmp.bStretched = true;

	// Group selector buttons
	GoldButton = UWindowButton(CreateWindow(class'UWindowButton',10, 100, 82, 35));
	GoldButton.Register(self);
	GoldButton.UpTexture=textureGoldNormal;
	GoldButton.DownTexture=textureGoldNormal;
	GoldButton.OverTexture=textureGoldHilite;
	GoldButton.ToolTipString=GetLocalFEString("Folio_Menu_0003");  

	SilverButton = UWindowButton(CreateWindow(class'UWindowButton',10, 145, 82, 35));
	SilverButton.Register(self);
	SilverButton.UpTexture=textureSilverNormal;
	SilverButton.DownTexture=textureSilverNormal;
	SilverButton.OverTexture=textureSilverHilite;
	SilverButton.ToolTipString=GetLocalFEString("Folio_Menu_0004");  

	BronzeButton = UWindowButton(CreateWindow(class'UWindowButton',10, 190, 82, 35));
	BronzeButton.Register(self);
	BronzeButton.UpTexture=textureBronzeNormal;
	BronzeButton.DownTexture=textureBronzeNormal;
	BronzeButton.OverTexture=textureBronzeHilite;
	BronzeButton.ToolTipString=GetLocalFEString("Folio_Menu_0005");  

    // Forward and back page buttons
	ForwardButton = UWindowButton(CreateWindow(class'UWindowButton',605, 316, 30, 28));
	ForwardButton.Register(self);
	ForwardButton.UpTexture=textureRightUp;
	ForwardButton.DownTexture=textureRightOver;
	ForwardButton.OverTexture=textureRightOver;
	ForwardButton.ToolTipString=GetLocalFEString("Folio_Menu_0001");  // next card set

	BackButton = UWindowButton(CreateWindow(class'UWindowButton',5, 316, 30, 28));
	BackButton.Register(self);
	BackButton.UpTexture=textureLeftUp;
	BackButton.DownTexture=textureLeftOver;
	BackButton.OverTexture=textureLeftOver;
	BackButton.ToolTipString=GetLocalFEString("Folio_Menu_0002");  // previous card set

	// Forward and back page numbers
	NextPageLabel=UWindowLabelControl(CreateControl(class'UWindowLabelControl', 605-100-5,316+5,100,28));
	NextPageLabel.setFont(F_HPMenuLarge);
	NextPageLabel.TextColor.r=255;
	NextPageLabel.TextColor.g=255;
	NextPageLabel.TextColor.b=255;
	NextPageLabel.Align=TA_Right;
	NextPageLabel.bShadowText=true;

	PreviousPageLabel=UWindowLabelControl(CreateControl(class'UWindowLabelControl', 5+30+5,316+5,100,28));
	PreviousPageLabel.setFont(F_HPMenuLarge);
	PreviousPageLabel.TextColor.r=255;
	PreviousPageLabel.TextColor.g=255;
	PreviousPageLabel.TextColor.b=255;
	PreviousPageLabel.Align=TA_Left;
	PreviousPageLabel.bShadowText=true;

	// Back to menu button
	CreateBackPageButton(594, 350);
}

// Set "X/Y" text for each card type
function SetCardCountData()
{
	local StatusGroupWizardCards sgCards;
	local StatusItemWizardCards  siCards;
	local int                    nCount;
	local int                    nMaxCount;
	local UWindowLabelControl    LabelControl;

    // Get wizard card status group
	sgCards = StatusGroupWizardCards(playerHarry.managerStatus.GetStatusGroup(class'StatusGroupWizardCards'));

    // Set bronze card text
	siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemBronzeCards'));
	nCount    = siCards.nCount;
	nMaxCount = siCards.nMaxCount;
    strCardCountBronze = nCount $"/" $nMaxCount;
    nBronzeHealthBars = nCount / nCARDS_PER_SET; // This many health bars display next to counts

    siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemSilverCards'));
	nCount    = siCards.nCount;
	nMaxCount = siCards.nMaxCount;
    strCardCountSilver = nCount $"/" $nMaxCount;
    nSilverKeys = nCount / nCARDS_PER_SET;     // This many locks display next to counts

	siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemGoldCards'));
	nCount    = siCards.nCount;
	nMaxCount = siCards.nMaxCount;
    strCardCountGold = nCount $"/" $nMaxCount;
}

function ShowWindow()
{
	UpdateDisplayDetails ();
	Super.ShowWindow ();
}

function PreOpenBook()
{
	ShowWindow();
}


// Set large card texture and wizard description text that gets displayed
// for the currently selected card.
function SetLargeCardProps(class<WizardCardIcon> classWC)
{
	local string strDescId;

		//CMP: Save cur class for layered card drawing.
	classCurWC=classWC;

	textureCurrLargeCard = classWC.default.textureBig;

	strDescId = classWC.default.strDescriptionId;
	if (strDescId == "")
		strCurrDesc = "";
	else
		strCurrDesc = GetLocalFEString(strDescId); 
}

// Update 
function UpdateDisplayDetails()
{
    UpdatePreviousNextButons();     // Previous and next buttons
    UpdatePageCards();              // Small cards, harry card, big card
	UpdateGroupButtonTextures();    // Gold, silver, bronze group buttons.
}


function UpdatePreviousNextButons()
{
	// Enable/disable previous/next page buttons
	BackButton.bDisabled = (nCurrPage == 0);
	if (CurrCardGroup == CardGroup_Gold)
		ForwardButton.bDisabled = true;
	else
		ForwardButton.bDisabled = (nCurrPage == (nCurrNumPages - 1));


	switch (nCurrPage)
	{
	case (0):
		PreviousPageLabel.SetText("");
		if (CurrCardGroup == CardGroup_Gold)
			NextPageLabel.SetText("");
		else
			NextPageLabel.SetText(strPAGE1);
		break;
	case (1):
		PreviousPageLabel.SetText(strPAGE0);
		if (CurrCardGroup == CardGroup_Gold)
			NextPageLabel.SetText("");
		else
			NextPageLabel.SetText(strPAGE2);
		break;
	case (2):
		PreviousPageLabel.SetText(strPAGE1);
		if (CurrCardGroup == CardGroup_Gold)
			NextPageLabel.SetText("");
		else
			NextPageLabel.SetText(strPAGE3);
		break;
	case (3):
		PreviousPageLabel.SetText(strPAGE2);
		if (CurrCardGroup == CardGroup_Bronze)
			NextPageLabel.SetText(strPAGE4);
		else
			NextPageLabel.SetText("");
		break;
	case (4):
		PreviousPageLabel.SetText(strPAGE3);
		NextPageLabel.SetText("");
		break;
	default:
		PreviousPageLabel.SetText("");
		NextPageLabel.SetText("");
		break;
	}
}

// Update card buttons for current page
function UpdatePageCards()
{
	local int i;                           // Looping var
	local StatusGroupWizardCards sgCards;  // Status group for wizard cards
	local StatusItemWizardCards  siCards;  // Gold, silver or bronze status item object
	local class<actor> classWC;            // WizardCardIcon derived class
	local int nCardId;                     // Wizard card id (1 - 101)

    // Get wizard card StatusGroup and appropriate StatusItem objects.
	sgCards = StatusGroupWizardCards(playerHarry.managerStatus.GetStatusGroup(class'StatusGroupWizardCards'));
	switch (CurrCardGroup)
	{
	case (CardGroup_Bronze):
		siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemBronzeCards'));
		break;
	case (CardGroup_Silver):
		siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemSilverCards'));
		break;
	case (CardGroup_Gold):
		siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemGoldCards'));
		break;
	default:
		log("ERROR: Invalid card group in FEFoliPage " $CurrCardGroup);
		break;
	}

    // Default to using missing wizard card texture for large card.
	textureCurrLargeCard = Texture'WizCardMissingBigTexture';
	classCurWC=None;//CMP:default wizard card

    // Clear out wizard description.
	strCurrDesc = "";

    // Get the texture to use for each small card.
	for (i=0; i<ArrayCount(SmallCardBmp); i++)
	{
		nCardId = siCards.GetCardId(i + (nCurrPage * ArrayCount(SmallCardBmp)));
		if (nCardId != 0 && siCards.IsOwnedByHarry(nCardId))
		{
			classWC = siCards.GetCardClassFromId(nCardId);

			SmallCardBmp[i].UpTexture = class<WizardCardIcon>(classWC).default.textureBig;
			if (i == nCurrItemOnPage)
				SetLargeCardProps(class<WizardCardIcon>(classWC));
		}
		else
			SmallCardBmp[i].UpTexture = Texture'WizCardMissingSmallTexture';

		SmallCardBmp[i].DownTexture = SmallCardBmp[i].UpTexture;
		SmallCardBmp[i].OverTexture = SmallCardBmp[i].UpTexture;
	}

    // Enable and display Harry Potter card button if have the Harry Potter card.
    // Also set the large card to Harry Potter if it is the current card.  If
    // don't have the Harry Potter card, disable the Harry Potter card button.
	if (CurrCardGroup == CardGroup_Gold)
	{
		classWC = class'WCPotter';
		if (siCards.IsOwnedByHarry(class<WizardCardIcon>(classWC).default.ID))
		{
			HarryCardBmp.bDisabled = false;
			HarryCardBmp.UpTexture   = class<WizardCardIcon>(classWC).default.textureBig;
			HarryCardBmp.DownTexture = HarryCardBmp.UpTexture;
			HarryCardBmp.OverTexture = HarryCardBmp.UpTexture;

			if (nCurrItemOnPage == ArrayCount(SmallCardBmp))
				SetLargeCardProps(class<WizardCardIcon>(classWC));
		}
	}
	else
		HarryCardBmp.bDisabled = true;

    // Put a hilite over the selected card.
	HiliteCurrCard();
}

// Update group buttons to use the appropriate textrues.
function UpdateGroupButtonTextures()
{
	switch (CurrCardGroup)
	{
	case (CardGroup_Bronze):
		GoldButton.UpTexture=textureGoldNormal;
		GoldButton.DownTexture=textureGoldNormal;
		GoldButton.OverTexture=textureGoldNormal;

		SilverButton.UpTexture=textureSilverNormal;
		SilverButton.DownTexture=textureSilverNormal;
		SilverButton.OverTexture=textureSilverNormal;

		BronzeButton.UpTexture=textureBronzeHilite;
		BronzeButton.DownTexture=textureBronzeHilite;
		BronzeButton.OverTexture=textureBronzeHilite;

		break;
	case (CardGroup_Silver):
		GoldButton.UpTexture=textureGoldNormal;
		GoldButton.DownTexture=textureGoldNormal;
		GoldButton.OverTexture=textureGoldNormal;

		SilverButton.UpTexture=textureSilverHilite;
		SilverButton.DownTexture=textureSilverHilite;
		SilverButton.OverTexture=textureSilverHilite;

		BronzeButton.UpTexture=textureBronzeNormal;
		BronzeButton.DownTexture=textureBronzeNormal;
		BronzeButton.OverTexture=textureBronzeNormal;

		break;
	case (CardGroup_Gold):
		GoldButton.UpTexture=textureGoldHilite;
		GoldButton.DownTexture=textureGoldHilite;
		GoldButton.OverTexture=textureGoldHilite;

		SilverButton.UpTexture=textureSilverNormal;
		SilverButton.DownTexture=textureSilverNormal;
		SilverButton.OverTexture=textureSilverNormal;

		BronzeButton.UpTexture=textureBronzeNormal;
		BronzeButton.DownTexture=textureBronzeNormal;
		BronzeButton.OverTexture=textureBronzeNormal;

		break;
	default:
		break;
	}
}

// Button messages
function Notify(UWindowDialogControl C, byte E)
{
	local int i;

	if(e==DE_Click)
	{
		switch(c)								
		{
        // Go to next folio page
		case ForwardButton:
			if (nCurrPage < (nCurrNumPages - 1))
			{
				++nCurrPage;
				UpdateDisplayDetails();
			}
			return;

        // Go to previous folio page
		case BackButton:
			if (nCurrPage > 0)
			{
				--nCurrPage;
				UpdateDisplayDetails();
			}
			return;

        // Select bronze card group
		case BronzeButton:
			if (CurrCardGroup != CardGroup_Bronze)
			{
				CurrCardGroup   = CardGroup_Bronze;
				nCurrPage       = 0;
				nCurrItemOnPage = 0;
				nCurrNumPages   = nNUM_PAGES_BRONZE;

				UpdateDisplayDetails();
			}
			return;

        // Select silver card group
		case SilverButton:
			if (CurrCardGroup != CardGroup_Silver)
			{
				CurrCardGroup   = CardGroup_Silver;
				nCurrPage       = 0;
				nCurrItemOnPage = 0;
				nCurrNumPages   = nNUM_PAGES_SILVER;

				UpdateDisplayDetails();
			}
			return;

        // Select gold card group
		case GoldButton:
			if (CurrCardGroup != CardGroup_Gold)
			{
				CurrCardGroup   = CardGroup_Gold;
				nCurrPage       = 0;
				nCurrItemOnPage = 0;
				nCurrNumPages   = nNUM_PAGES_GOLD;

				UpdateDisplayDetails();
			}
			return;

        // Select Harry Potter card
		case HarryCardBmp:					
			nCurrItemOnPage = ArrayCount(SmallCardBmp);
			UpdateDisplayDetails();
			return;

        // Go back to In Game menu
		case BackPageButton:
			FEBook(book).DoEscapeFromPage();
			return;
		}

        // Select the small card clicked
		for (i=0; i !=ArrayCount(SmallCardBmp); ++i)
		{
			if (SmallCardBmp[i] == C)
			{
				nCurrItemOnPage = i;
				UpdateDisplayDetails();
				return;
			}
		}

	}
}

// Hilite the current small card or Harry card
function HiliteCurrCard()
{
	if (nCurrItemOnPage < ArrayCount(SmallCardBmp))
	{
		HiliteCardBmp.WinLeft   = SmallCardBmp[nCurrItemOnPage].WinLeft;
		HiliteCardBmp.WinTop    = SmallCardBmp[nCurrItemOnPage].WinTop;
		HiliteCardBmp.WinWidth  = SmallCardBmp[nCurrItemOnPage].WinWidth;
		HiliteCardBmp.WinHeight = SmallCardBmp[nCurrItemOnPage].WinHeight;
	}
	else
	{
		HiliteCardBmp.WinLeft   = HarryCardBmp.WinLeft;
		HiliteCardBmp.WinTop    = HarryCardBmp.WinTop;
		HiliteCardBmp.WinWidth  = HarryCardBmp.WinWidth;
		HiliteCardBmp.WinHeight = HarryCardBmp.WinHeight;
	}		
}

// Draw things that are drawn straight to the canvas instead of using Unreal
// Windows.  Notice, we're drawing these things in AfterPaint() so they
// will go on top of everything else that is drawn as Unreal Windows.  If
// we wanted a different Z order, we could be calling these things in 
// BeforePaint() or Paint().
function AfterPaint(Canvas canvas,float x,float y)
{
	local float fScaleFactor;

	super.AfterPaint(canvas, x, y);

	fScaleFactor = Canvas.SizeX/WinWidth; 

	PaintLargeCard(Canvas, fScaleFactor);
	PaintWizardText(Canvas, fScaleFactor);
    PaintCardStatData(Canvas, fScaleFactor);
}

// We paint the large card here instead of assigning the texture to an Unreal window.
// We want to display the large card at a scale of 1 whenever possible so it looks
// as clean as possible.  Unreal windows get automatically rescaled.
function PaintLargeCard(Canvas canvas, float fScaleFactor)
{
	local int   nLargeCardX;
	local int   nLargeCardY;
	local float mouseX,mouseY;
	local float offX,offY;

	// If the canvas resolution is bigger than the resolution of the folio page, 
	// we won't scale the card itself.  We do reposition the card based
	// on the scale factor though.
	if (Canvas.SizeX > WinWidth)
	{
		// Center card along horizontal axis
		nLargeCardX = Canvas.SizeX/2 - textureCurrLargeCard.USize/2;

		// Adjust card vertically so that it is centered in the area it would
		// be displayed in if the card were to be scaled.
		nLargeCardY = nLARGE_CARD_Y + 
			          (((textureCurrLargeCard.VSize * fScaleFactor) - textureCurrLargeCard.VSize)/2);

			//Check if card is layered.
		if(classCurWC!=None && classCurWC.default.bIsLayered)
		{
			GetMouseXY(mouseX,mouseY);
			offX=(mouseX-(Canvas.SizeX/2)) / (Canvas.SizeX/2);
			offY=(mouseY-(Canvas.SizeY/2)) / (Canvas.SizeY/2);
			offX*=6;	//maximum travel dist per layer.
			offY*=6;


			// Draw the layers with scale of 1
			if(classCurWC.default.bLastLayerIsFire)
				Canvas.SetPos(nLargeCardX, nLargeCardY);	//fire textures dont paralax.
			else
				Canvas.SetPos(nLargeCardX+(offX*2), nLargeCardY+(offY*2));
			Canvas.DrawIcon(classCurWC.default.textureLayers[2], 1);

			Canvas.SetPos(nLargeCardX+(offX*1), nLargeCardY+(offY*1));
			Canvas.DrawIcon(classCurWC.default.textureLayers[1], 1);

			Canvas.SetPos(nLargeCardX, nLargeCardY);
			Canvas.DrawIcon(classCurWC.default.textureLayers[0], 1);
		}
		else
		{
			// Draw the card with scale of 1
			Canvas.SetPos(nLargeCardX, nLargeCardY);
			Canvas.DrawIcon(textureCurrLargeCard, 1);
		}
	}

	// If the canvas resolution is smaller than the screen resolution, then we will
	// scale down the large card.  It doesn't look so bad scaled down and we don't have
	// enough room for the card to display it at it's full texture size in lower
	// resolutions.
	else
	{
		Canvas.SetPos(nLARGE_CARD_X * fScaleFactor, nLARGE_CARD_Y * fScaleFactor);
		Canvas.DrawIcon(textureCurrLargeCard, fScaleFactor);
	}
}

// Draw text for description of the currently selected wizard card.
function PaintWizardText(Canvas canvas, float fScaleFactor)
{
	local color colorText;
    local font  fontText;

    // Set text color to bronze, silver or gold depending on current card group.
	switch (CurrCardGroup)
	{
	case (CardGroup_Bronze) :
		colorText = colorBronze;
		break;
	case (CardGroup_Silver) :
		colorText = colorSilver;
		break;
	case (CardGroup_Gold) :
		colorText = colorGold;
		break;
	default:
		break;
	}

    // Draw black background over text area.
	Canvas.SetPos(nWIZARD_TEXT_X, nWIZARD_TEXT_Y * fScaleFactor);
	Canvas.DrawTile(textureDescBkgrd, Canvas.SizeX, 80 * fScaleFactor, 0, 0, 1, 1);

    if (Canvas.SizeX <= 512)
        fontText = baseConsole(playerHarry.player.console).LocalSmallFont;
    else
        fontText = baseConsole(playerHarry.player.console).LocalMedFont;

    // Draw wizard description text
	HPHud(playerHarry.myHud).DrawCutStyleText(Canvas,
		                                      strCurrDesc,
                                              0,
		                                      nWIZARD_TEXT_Y * fScaleFactor,
		                                      nWIZARD_TEXT_H * fScaleFactor,
					 					      colorText,
                                              fontText);

}

function PaintCardStatData(Canvas canvas, float fCanvasScaleFactor)
{
	local color colorSave;            // Original canvas color
    local font  fontSave;             // Original canvas font
    local int   nXPos, nYPos;         // Draw at this pos
    local float fXTextLen, fYTextLen; // Width and Height of text on canvas
    local int   i;                    // generic loop var
    local float fWindowScaleFactor;   

    colorSave = Canvas.DrawColor;
    fontSave  = Canvas.Font;

	fWindowScaleFactor = Canvas.SizeX/WinWidth; 

    // Draw "X / Y" text for the card selector buttons
    if (Canvas.SizeX <= 640)
        Canvas.Font = baseConsole(playerHarry.player.console).LocalSmallFont;
    else if (Canvas.SizeX <= 800)
        Canvas.Font = baseConsole(playerHarry.player.console).LocalMedFont;
    else
        Canvas.Font = baseConsole(playerHarry.player.console).LocalBigFont;


    Canvas.DrawColor = colorGold;
	Canvas.TextSize(strCardCountGold, fXTextLen, fYTextLen);
	Canvas.SetPos((GoldButton.WinLeft * fWindowScaleFactor) + (54 * fWindowScaleFactor) - fXTextLen/2,
		          (GoldButton.WinTop * fWindowScaleFactor) + (18 * fWindowScaleFactor) - fYTextLen/2);    
    Canvas.DrawText(strCardCountGold, false);

    Canvas.DrawColor = colorSilver;
	Canvas.TextSize(strCardCountSilver, fXTextLen, fYTextLen);
	Canvas.SetPos((SilverButton.WinLeft * fWindowScaleFactor) + (54 * fWindowScaleFactor) - fXTextLen/2,
		          (SilverButton.WinTop * fWindowScaleFactor) + (18 * fWindowScaleFactor) - fYTextLen/2);    
    Canvas.DrawText(strCardCountSilver, false);
    
    Canvas.DrawColor = colorBronze;
	Canvas.TextSize(strCardCountBronze, fXTextLen, fYTextLen);
	Canvas.SetPos((BronzeButton.WinLeft * fWindowScaleFactor) + (54 * fWindowScaleFactor) - fXTextLen/2,
		          (BronzeButton.WinTop * fWindowScaleFactor) + (18 * fWindowScaleFactor) - fYTextLen/2);    
    Canvas.DrawText(strCardCountBronze, false);

    Canvas.DrawColor.R = 255;
    Canvas.DrawColor.G = 255;
    Canvas.DrawColor.B = 255;

    // Display appropriate # of health bars next to the bronze selector button
    nXPos = (BronzeButton.WinLeft + BronzeButton.WinWidth + 10) * fWindowScaleFactor;   // Left edge of selector button + widths selector + count stat width
    nYPos = BronzeButton.WinTop * fWindowScaleFactor;
    for (i=0; i<nBronzeHealthBars; i++)
    {
        Canvas.SetPos(nXPos, nYPos);
        Canvas.DrawIcon(textureBronzeHealth, fWindowScaleFactor);
        nXPos += (14 * fWindowScaleFactor);
    }

    // Display appropriate # of keys next to the silver selector button
    nXPos = (SilverButton.WinLeft + SilverButton.WinWidth + 10) * fWindowScaleFactor;   // Left edge of selector button + widths selector + count stat width
    nYPos = SilverButton.WinTop * fWindowScaleFactor;
    for (i=0; i<nSilverKeys; i++)
    {
        Canvas.SetPos(nXPos, nYPos);
        Canvas.DrawIcon(textureSilverKey, (fWindowScaleFactor - 0.3));
        nXPos += (28 * (fWindowScaleFactor - 0.3));
    }

    Canvas.DrawColor = colorSave;
    Canvas.Font      = fontSave;
}

// Get Vertical position of status text (overridden from base class).
function int GetStatusY()
{
	return (nSTATUS_BAR_Y);
}

// Preswitch page is called by FEBook before control gets to this folio menu
function PreSwitchPage()
{
    // Save off Harry for ease of use
	playerHarry = Harry(Root.Console.viewport.actor);

    // Set the "X/Y" text indicating card counts
	SetCardCountData();

    // Set initial group, page and item on page
	SetInitialSelection();

    // Setup the lock and health buttons based on how many bronze and silver card
    // sets the player has
	//UpdateLockAndHealthButtons();

}

// Setup current selection when folio menu is brought up.  The currently
// selected card will be the last card that the player picked up.
function SetInitialSelection()
{
	local StatusGroupWizardCards sgCards;
	local StatusItemWizardCards  siCards;
	local int                    nNumSmallPerPage;

    // Set the current card group and StatusItem object to use.
	sgCards = StatusGroupWizardCards(playerHarry.managerStatus.GetStatusGroup(class'StatusGroupWizardCards'));
	switch (sgCards.GetLastObtainedCardType())
	{
		case (sgCards.ECardType.CardType_Gold):
			CurrCardGroup = CardGroup_Gold;
			siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemGoldCards'));
			break;
		case (sgCards.ECardType.CardType_Silver):
			CurrCardGroup = CardGroup_Silver;
			siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemSilverCards'));
			break;
		case (sgCards.ECardType.CardType_Bronze):
			CurrCardGroup = CardGroup_Bronze;
			siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemBronzeCards'));
			break;
		case (sgCards.ECardType.CardType_None):
			// Use bronze group if last obtained card type is not set.
			CurrCardGroup = CardGroup_Bronze;
			siCards = StatusItemWizardCards(sgCards.GetStatusItem(class'StatusItemBronzeCards'));
			break;
	}

	// Get number of small cards displayed on page at once
	nNumSmallPerPage = ArrayCount(SmallCardBmp);

	// Figure out which page to display
	if (CurrCardGroup == CardGroup_Gold)
		nCurrPage = 0;
	else
	{
		if (siCards.nCount <= (1 * nCARDS_PER_SET))
			nCurrPage = 0;
		else if (siCards.nCount <= (2 * nCARDS_PER_SET))
			nCurrPage = 1;
		else if (siCards.nCount <= (3 * nCARDS_PER_SET))
			nCurrPage = 2;
		else if (siCards.nCount <= (4 * nCARDS_PER_SET))
			nCurrPage = 3;
		else if (siCards.nCount <= (5 * nCARDS_PER_SET))
			nCurrPage = 4;
	}

	// Calcuate which button should be highlighted

	// If no cards in group, current selection is the first slot
	if (siCards.nCount == 0)
		nCurrItemOnPage = 0;

	// One or more cards in group.
	else
	{
		// If gold group, then item # is 0-10 (with 10 being the Harry Potter card)
		if (CurrCardGroup == CardGroup_Gold)
			nCurrItemOnPage = siCards.nCount - 1;

		// For silver and bronze, item # is 0-9
		else
		{
			if (siCards.nCount % nNumSmallPerPage == 0)
				nCurrItemOnPage = nNumSmallPerPage - 1;
			else 
				nCurrItemOnPage = siCards.nCount % nNumSmallPerPage - 1;
		}
	}
}


defaultproperties
{
	colorBronze=(R=132,G=64,B=44)
	colorSilver=(R=115,G=123,B=140)
	colorGold=(R=181,G=115,B=0)
	CurrCardGroup=CardGroup_Bronze
	nCurrPage=0
	nCurrItemOnPage=0
	nCurrNumPages=4;

 
 
}
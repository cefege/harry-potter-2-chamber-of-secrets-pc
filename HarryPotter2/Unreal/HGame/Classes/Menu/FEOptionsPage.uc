class FEOptionsPage expands baseFEPage;

const fOBJECTIVE_X              = 0;
const fOBJECTIVE_W              = 640;
const fOBJECTIVE_H              = 20;
const fOBJECTIVE_Y_BOTTOM       = 460.0;
const fOBJECTIVE_Y_MIDDLE       = 440.0;
const fOBJECTIVE_Y_TOP          = 420.0;

var HPMessageBox		ConfirmQuit;

var bool				bSetupAfterPageSwitch;

var UWindowButton		HudButtonList[30];

var UWindowButton		QuitButton;
var UWindowButton		InputButton;
var UWindowButton		SoundVideoButton;

var UWindowLabelControl QuitLabel;
var UWindowLabelControl InputLabel;
var UWindowLabelControl SoundVideoLabel;


var UWindowLabelControl ObjectiveLabel;
/*
var UWindowLabelControl MainObjectiveLabel;
var UWindowLabelControl Polyjuice1ObjectiveLabel;
var UWindowLabelControl Polyjuice2ObjectiveLabel;

var texture				textureObjectiveBkgrd;	
*/
var texture				textureLionClick;
var texture				textureLionIdle;
var texture				textureLionRO;
/*
var texture				textureFolioClick;
var texture				textureFolioIdle;
var texture				textureFolioRO;
*/
function BeforePaint(Canvas C, float X, float Y)
{
/*
// Add back if need tooltip text for hud buttons.
	if (bSetupAfterPageSwitch)
	{
		SetupHudButtons(C);		

		bSetupAfterPageSwitch = false;
	}
*/

	Super.BeforePaint(C,X,Y);
}

function Paint( Canvas canvas, float x, float y)
{
	local float   fScaleFactor;
	local bool    bHaveObjectiveText;

	fScaleFactor = Canvas.SizeX/WinWidth; 

/*	if (MainObjectiveLabel.WindowIsVisible())
	{
		Canvas.SetPos(0, MainObjectiveLabel.WinTop * fScaleFactor);
		Canvas.DrawTile(textureObjectiveBkgrd, Canvas.SizeX, fOBJECTIVE_H * fScaleFactor, 0, 0, 1, 1);
	}
	if (Polyjuice1ObjectiveLabel.WindowIsVisible())
	{
		Canvas.SetPos(0, Polyjuice1ObjectiveLabel.WinTop * fScaleFactor);
		Canvas.DrawTile(textureObjectiveBkgrd, Canvas.SizeX, fOBJECTIVE_H * fScaleFactor, 0, 0, 1, 1);
	}
	if (Polyjuice2ObjectiveLabel.WindowIsVisible())
	{
		Canvas.SetPos(0, Polyjuice2ObjectiveLabel.WinTop * fScaleFactor);
		Canvas.DrawTile(textureObjectiveBkgrd, Canvas.SizeX, fOBJECTIVE_H * fScaleFactor, 0, 0, 1, 1);
	}
	if (ObjectiveLabel.WindowIsVisible())
	{
		Canvas.SetPos(0, ObjectiveLabel.WinTop * fScaleFactor);
		Canvas.DrawTile(textureObjectiveBkgrd, Canvas.SizeX, fOBJECTIVE_H * fScaleFactor, 0, 0, 1, 1);
	}
*/	Super.Paint(canvas, x, y);
}

function int GetObjectiveAreaTop(int nCanvasSizeX, int nCanvasSizeY)
{
	local float fScaleFactor;

	fScaleFactor = nCanvasSizeX/WinWidth; 

	if (ObjectiveLabel.WindowIsVisible())
		return (ObjectiveLabel.WinTop * fScaleFactor);
	else
		return (nCanvasSizeY);
}

function Created()
{
//	textureObjectiveBkgrd = texture(DynamicLoadObject("HGame.Icons.leftPanel" , class'Texture'));
	textureLionClick      = texture(DynamicLoadObject("HP_Menu.Hud.MenuLionButtonClick", class'Texture'));
	textureLionIdle       = texture(DynamicLoadObject("HP_Menu.Hud.MenuLionButtonIdle", class'Texture'));
	textureLionRO         = texture(DynamicLoadObject("HP_Menu.Hud.MenuLionButtonRO", class'Texture'));
//	textureFolioClick     = texture(DynamicLoadObject("HP_Menu.Hud.MenuFolioButtonClick", class'Texture'));
//	textureFolioIdle      = texture(DynamicLoadObject("HP_Menu.Hud.MenuFolioButtonIdle", class'Texture'));
//	textureFolioRO        = texture(DynamicLoadObject("HP_Menu.Hud.MenuFolioButtonRO", class'Texture'));
	
//	log("OptionsPage " $textureObjectiveBkgrd $" " $textureLionClick);

	InputButton=UWindowButton(CreateControl(class'UWindowButton',182,310,60,60));
//	InputButton.ToolTipString=GetLocalFEString("InGameMenu_0003");
	InputButton.UpTexture=textureLionIdle; //Texture'GreenUpTexture';
	InputButton.DownTexture=textureLionClick; //Texture'GreenDownTexture';
	InputButton.OverTexture=textureLionRO; //Texture'GreenOverTexture';
	
	InputLabel=UWindowLabelControl(CreateControl(class'UWindowLabelControl', 182-50,310+60,200,64));
	InputLabel.SetFont(F_HPMenuLarge);
	InputLabel.TextColor.r=215;
	InputLabel.TextColor.g=0;
	InputLabel.TextColor.b=215;
	InputLabel.Align=TA_Center;
	InputLabel.bShadowText=true;
	InputLabel.SetText( GetLocalFEString("Options_0040") ); //InGameMenu_0034=Go to Input Options
	
	SoundVideoButton=UWindowButton(CreateControl(class'UWindowButton', 252,120,136,106));
//	SoundVideoButton.ToolTipString=GetLocalFEString("InGameMenu_0004");
	SoundVideoButton.UpTexture=textureLionIdle; //Texture'GreenUpTexture';
	SoundVideoButton.DownTexture=textureLionClick; //Texture'GreenDownTexture';
	SoundVideoButton.OverTexture=textureLionRO; //Texture'GreenOverTexture';

	
	SoundVideoLabel=UWindowLabelControl(CreateControl(class'UWindowLabelControl', 252-50,120+108,200,64));
	SoundVideoLabel.setFont(F_HPMenuLarge);
	SoundVideoLabel.TextColor.r=215;
	SoundVideoLabel.TextColor.g=0;
	SoundVideoLabel.TextColor.b=215;
	SoundVideoLabel.Align=TA_Center;
	SoundVideoLabel.bShadowText=true;
	SoundVideoLabel.SetText(GetLocalFEString("Options_0041")); //InGameMenu_0035=Go to Sound And Video Options



	QuitButton=UWindowButton(CreateControl(class'UWindowButton', 394,310,60,60));
//	QuitButton.ToolTipString=GetLocalFEString("InGameMenu_0002");
	QuitButton.UpTexture=textureLionIdle; 
	QuitButton.DownTexture=textureLionClick;
	QuitButton.OverTexture=textureLionRO; 

	QuitLabel=UWindowLabelControl(CreateControl(class'UWindowLabelControl', 394+30-50,310+62,200,64));
	QuitLabel.setFont(F_HPMenuLarge);
	QuitLabel.TextColor.r=215;
	QuitLabel.TextColor.g=0;
	QuitLabel.TextColor.b=215;
	QuitLabel.Align=TA_Center;
	QuitLabel.bShadowText=true;
	QuitLabel.setText(GetLocalFEString("InGameMenu_0025"));

/*	// Main objective label will usually be displayed alone on the bottom center of the
	// screen.  It will be moved above the polyjuice objective labels if there is
	// any polyjuice objective text to display at runtime.
	MainObjectiveLabel=UWindowLabelControl(CreateControl(class'UWindowLabelControl', 
		                                                 fOBJECTIVE_X, 
														 fOBJECTIVE_Y_BOTTOM, 
														 fOBJECTIVE_W,
														 fOBJECTIVE_H));
	MainObjectiveLabel.setFont(F_HPMenuMedium);
	MainObjectiveLabel.TextColor.r=255;
	MainObjectiveLabel.TextColor.g=255;
	MainObjectiveLabel.TextColor.b=255;
	MainObjectiveLabel.Align=TA_Center;
	MainObjectiveLabel.bShadowText=true;

	// The first polyjuice objective text will always appear centered at bottom of screen
	// if it exists.
	Polyjuice1ObjectiveLabel=UWindowLabelControl(CreateControl(class'UWindowLabelControl',
				                                               fOBJECTIVE_X, 
														       fOBJECTIVE_Y_BOTTOM, 
														       fOBJECTIVE_W,
														       fOBJECTIVE_H));
	Polyjuice1ObjectiveLabel.setFont(F_HPMenuMedium);
	Polyjuice1ObjectiveLabel.TextColor.r=255;
	Polyjuice1ObjectiveLabel.TextColor.g=255;
	Polyjuice1ObjectiveLabel.TextColor.b=255;
	Polyjuice1ObjectiveLabel.Align=TA_Center;
	Polyjuice1ObjectiveLabel.bShadowText=true;

	// The second polyjuice objective text will always appear above the first polyjuice objective
	// text if it exists.
	Polyjuice2ObjectiveLabel=UWindowLabelControl(CreateControl(class'UWindowLabelControl',
				                                               fOBJECTIVE_X, 
														       fOBJECTIVE_Y_MIDDLE, 
														       fOBJECTIVE_W,
														       fOBJECTIVE_H));
	Polyjuice2ObjectiveLabel.setFont(F_HPMenuMedium);
	Polyjuice2ObjectiveLabel.TextColor.r=255;
	Polyjuice2ObjectiveLabel.TextColor.g=255;
	Polyjuice2ObjectiveLabel.TextColor.b=255;
	Polyjuice2ObjectiveLabel.Align=TA_Center;
	Polyjuice2ObjectiveLabel.bShadowText=true;

	ObjectiveLabel=UWindowLabelControl(CreateControl(class'UWindowLabelControl', 
	                                                 fOBJECTIVE_X, 
													 fOBJECTIVE_Y_BOTTOM - (fOBJECTIVE_H/2), 
													 fOBJECTIVE_W,
													 fOBJECTIVE_H));
	ObjectiveLabel.setFont(F_HPMenuLarge);
	ObjectiveLabel.TextColor.r=255;
	ObjectiveLabel.TextColor.g=255;
	ObjectiveLabel.TextColor.b=255;
	ObjectiveLabel.Align=TA_Center;
	ObjectiveLabel.bShadowText=true;
*/
	// Create our BackPage button
//	CreateBackPageButton( 500, 400 );

	Super.Created(); 
}	


function WindowDone(UWindowWindow W)
{
	if(W == ConfirmQuit)
	{
		if(ConfirmQuit.Result == ConfirmQuit.button1.text)
		{
			Root.DoQuitGame();
/*
			// This code goes back to the main menu.
			FEBook(book).CloseBook();
			baseConsole(root.console).ChangeLevel("startup.unr",false);

			FEBook(book).bGamePlaying=false;
			FEBook(book).OpenBook();
			FEBook(book).ChangePage(FEBook(book).MainPage);
*/
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
		case QuitButton:
			ConfirmQuit = doHPMessageBox(GetLocalFEString("InGameMenu_0026"), //"Are you sure you want to quit this game?"
									     GetLocalFEString("Shared_Menu_0003"),// "Yes"
										 GetLocalFEString("Shared_Menu_0004") //"No"
										);
			break;
		case InputButton:
			FEBook(book).ChangePageNamed( "INPUT" );
			break;
		case SoundVideoButton:
			FEBook(book).ChangePageNamed( "SOUNDVIDEO" );
			break;
		default :
			break;
		}
	}
}

function PreSwitchPage()
{
	// Setup hud buttons
//	UpdateHudButtonMenuStatus();

//	SetupObjectiveLabels();

	// Add back if need tooltip text for hud buttons.
	//bSetupAfterPageSwitch = true;	

	Super.PreSwitchPage();
}
/*
function SetupObjectiveLabels()
{
	local string strMainObjectiveId;
	local string strPJ1ObjectiveId;
	local string strPJ2ObjectiveId;

	local string strMainObjective;
	local string strPJ1Objective;
	local string strPJ2Objective;

	// Get object text ids
	strMainObjectiveId = Harry(Root.Console.viewport.actor).strObjectiveId[0];
	strPJ1ObjectiveId  = Harry(Root.Console.viewport.actor).strObjectiveId[1];
	strPJ2ObjectiveId  = Harry(Root.Console.viewport.actor).strObjectiveId[2];

	// Get objective text if the id isn't empty
	if (strMainObjectiveId != "")
		strMainObjective = Localize("All", strMainObjectiveId, "HPDialog");
	if (strPJ1ObjectiveId != "")
		strPJ1Objective  = Localize("All", strPJ1ObjectiveId, "HPDialog");
	if (strPJ2ObjectiveId != "")
		strPJ2Objective  = Localize("All", strPJ2ObjectiveId, "HPDialog");

	// @@@ for testing
	//strMainObjective = "1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890";
	//strPJ1Objective = "1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890";
	//strPJ2Objective = "1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890";

	//log("objective labels " $strMainObjectiveId $" " $strPJ1ObjectiveId $" " $strPJ2ObjectiveId);

	// Setup polyjuice objective labels based on current polyjuice objective
	// text.  Also, setup position of main objective label.  The main objective
	// goes at bottom of screen if there are no polyjuice objectives.  If there
	// are polyjuice objectives, the main objective is displayed above them.

	// If no polyjuice objectives, hide both polyjuice objective labels
	// and position main objective at bottom of screen.
	if (strPJ1Objective == "" && strPJ2Objective == "")
	{
		Polyjuice1ObjectiveLabel.HideWindow();
		Polyjuice2ObjectiveLabel.HideWindow();

		if (strMainObjective == "")
		{
			MainObjectiveLabel.HideWindow();
			ObjectiveLabel.HideWindow();
		}
		else
		{
			MainObjectiveLabel.ShowWindow();
			MainObjectiveLabel.WinTop  = fOBJECTIVE_Y_BOTTOM;			
			MainObjectiveLabel.SetText(strMainObjective);

			ObjectiveLabel.ShowWindow();
			ObjectiveLabel.WinTop = fOBJECTIVE_Y_BOTTOM - (fOBJECTIVE_H);			
			ObjectiveLabel.SetText(GetLocalFEString("InGameMenu_0027"));
		}
	}

	// If have one polyjuice objective, use the first polyjuice
	// lavel and position the main objective above the polyjuice objective.
	else if (strPJ1Objective == "" && strPJ2Objective != "")
	{
		Polyjuice1ObjectiveLabel.SetText(strPJ2Objective);

		Polyjuice1ObjectiveLabel.ShowWindow();
		Polyjuice2ObjectiveLabel.HideWindow();

		if (strMainObjective == "")
		{
			MainObjectiveLabel.HideWindow();

			ObjectiveLabel.ShowWindow();
			ObjectiveLabel.WinTop = fOBJECTIVE_Y_BOTTOM - (fOBJECTIVE_H);			
			ObjectiveLabel.SetText(GetLocalFEString("InGameMenu_0027"));
		}
		else
		{
			MainObjectiveLabel.ShowWindow();
			MainObjectiveLabel.WinTop = fOBJECTIVE_Y_MIDDLE;
			MainObjectiveLabel.SetText(strMainObjective);

			ObjectiveLabel.ShowWindow();
			ObjectiveLabel.WinTop = fOBJECTIVE_Y_MIDDLE - (fOBJECTIVE_H);			
			ObjectiveLabel.SetText(GetLocalFEString("InGameMenu_0028"));
		}
	}

	// If have the other polyjuice objective, still use the first polyjuice
	// lavel and position the main objective above the polyjuice objective.
	else if (strPJ1Objective != "" && strPJ2Objective == "")
	{
		Polyjuice1ObjectiveLabel.SetText(strPJ1Objective);

		Polyjuice1ObjectiveLabel.ShowWindow();
		Polyjuice2ObjectiveLabel.HideWindow();

		if (strMainObjective == "")
		{
			MainObjectiveLabel.HideWindow();

			ObjectiveLabel.ShowWindow();
			ObjectiveLabel.WinTop = fOBJECTIVE_Y_BOTTOM - (fOBJECTIVE_H);			
			ObjectiveLabel.SetText(GetLocalFEString("InGameMenu_0027"));
		}
		else
		{
			MainObjectiveLabel.ShowWindow();
			MainObjectiveLabel.WinTop = fOBJECTIVE_Y_MIDDLE;			
			MainObjectiveLabel.SetText(strMainObjective);

			ObjectiveLabel.ShowWindow();
			ObjectiveLabel.WinTop = fOBJECTIVE_Y_MIDDLE - (fOBJECTIVE_H);			
			ObjectiveLabel.SetText(GetLocalFEString("InGameMenu_0028"));
		}
	}

	// If have both polyjuice objectives, set the objective text,
	// show the polyjuice labels and position the main objective on
	// top of both polyjuice objectives.
	else
	{
		Polyjuice1ObjectiveLabel.SetText(strPJ1Objective);
		Polyjuice2ObjectiveLabel.SetText(strPJ2Objective);

		Polyjuice1ObjectiveLabel.ShowWindow();
		Polyjuice2ObjectiveLabel.ShowWindow();

		if (strMainObjective == "")
		{
			MainObjectiveLabel.HideWindow();

			ObjectiveLabel.ShowWindow();
			ObjectiveLabel.WinTop = fOBJECTIVE_Y_MIDDLE - (fOBJECTIVE_H);			
			ObjectiveLabel.SetText(GetLocalFEString("InGameMenu_0028"));
		}
		else
		{
			MainObjectiveLabel.ShowWindow();
			MainObjectiveLabel.WinTop = fOBJECTIVE_Y_TOP;	
			MainObjectiveLabel.SetText(strMainObjective);

			ObjectiveLabel.ShowWindow();
			ObjectiveLabel.WinTop = fOBJECTIVE_Y_TOP - (fOBJECTIVE_H);			
			ObjectiveLabel.SetText(GetLocalFEString("InGameMenu_0028"));
		}
	}
}

function UpdateHudButtonMenuStatus()
{
	local StatusManager managerStatus;
	local StatusGroup   sgLoop;

	managerStatus = Harry(Root.Console.viewport.actor).managerStatus;
	for (sgLoop=managerStatus.sgList; sgLoop!=None; sgLoop=sgLoop.sgNext)
		sgLoop.UpdateDisplayOnMenuStatus();
}
*/
/* 
// ADD BACK IF NEED TOOLTIP TEXT FOR HUD ITEMS 
// We're leaving off tooltips for the in game screen because we lack the 
// space because of the objective text.  If the code below were uncommented
// it would attempt to put tooltip text in the lower left corner.  If we
// want to add back tooltip text for hud items, we probably want the text
// to go right next to the icon and not at the bottom of the screen.
function SetupHudButtons(Canvas C)
{
	local StatusManager managerStatus;
	local StatusGroup   sgLoop;
	local StatusItem    siLoop;
	local int           x, y, w, h;
	local int           i;
	local float         fScaleFactor;

	fScaleFactor = WinWidth/C.GetBaseResolutionX(); 

	i = 0;
	managerStatus = Harry(Root.Console.viewport.actor).managerStatus;
	for (sgLoop=managerStatus.sgList; sgLoop!=None; sgLoop=sgLoop.sgNext)
	{
		if (sgLoop.bDisplayOnMenu)
		{
			for (siLoop=sgLoop.siList; siLoop!=None; siLoop=siLoop.siNext)
			{
				// Get coordinates for hud icon
				sgLoop.GetItemPosition(siLoop.class, x, y, WinWidth, WinHeight);
				w = siLoop.GetHudIconUSize() * fScaleFactor;
				h = siLoop.GetHudIconVSize() * fScaleFactor;

				if (i >= ArrayCount(HudButtonList))
					log("ERROR: HudButtonList array not big enough in FEInGamePage!");
				if (HudButtonList[i] == None)
					HudButtonList[i] = UWindowButton(CreateControl(class'UWindowButton',x,y,w,h));
				else
				{
					HudButtonList[i].WinTop    = y;
					HudButtonList[i].WinLeft   = x;
					HudButtonList[i].WinWidth  = w;
					HudButtonList[i].WinHeight = h;
				}
				HudButtonList[i].bDisabled = false;
				HudButtonList[i].ToolTipString = siLoop.GetToolTip();
				
				
				// For debugging- show text on the button to help see button placement.				
				//HudButtonList[i].TextColor.r=250;
				//HudButtonList[i].TextColor.g=5;
				//HudButtonList[i].TextColor.b=5;
				//HudButtonList[i].Align=TA_Left; 
				//HudButtonList[i].setText(siLoop.GetToolTip());		

				i++;

				// If only ever want to display first status item, break first time thru.
				if (sgLoop.bDisplayJustFirstItem)
					break;
			}

		}
	}

	// Destroy any buttons that were being used previously, but not anymore.
	for (i=i; i<ArrayCount(HudButtonList); i++)
	{
		if (HudButtonList[i] != None)
		{
			HudButtonList[i].bDisabled = true;
			HudButtonList[i].ToolTipString = "";
		}
	}
}
*/

defaultProperties
{
}
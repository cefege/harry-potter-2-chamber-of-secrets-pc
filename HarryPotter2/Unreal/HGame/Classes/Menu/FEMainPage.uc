class FEMainPage expands baseFEPage;

#EXEC TEXTURE IMPORT NAME=HPLogoTexture	 FILE=TEXTURES\Menu\FE\HPLogo.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
//#EXEC TEXTURE IMPORT NAME=HPLogoTexture	 FILE=TEXTURES\FE\BookPiece6.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


var UWindowButton NewGameButton;
var UWindowSmallButton LoadGameButton;
var UWindowButton OptionsButton;
var UWindowButton CreditsButton;
var UWindowButton ExitButton;
var UWindowButton LangButton;
var UWindowButton LogoWindow;


var string LegalText;

var HPMessageBox ConfirmExit;

var UWindowSmallButton VersionButton;

var bool bE3DemoMode;
var UWindowButton E3DemoStartButton;

struct LevelListItem
{
	var string LevelName;
	var string LevelUrl;
	var UWindowButton button;
};

var LevelListItem LevelList[30];
var int FirstPreviewIndex;

function BeforePaint(Canvas C, float X, float Y)
{
	Super.BeforePaint(C,X,Y);
	if(HPConsole(root.console).bDebugMode!=CreditsButton.bWindowVisible)
		{ 
		if(HPConsole(root.console).bDebugMode)
			{
//			LangButton.ShowWindow();
//			CreditsButton.ShowWindow();
			VersionButton.ShowWindow();
			}
		else
			{
			LangButton.HideWindow();
			CreditsButton.HideWindow();
			VersionButton.HideWindow();
			}
		}

//log("here");		
}

function Paint(Canvas canvas,float x,float y)
{
local float w,h;
local font saveFont;

	saveFont=canvas.font;
	canvas.font=root.fonts[0];
	TextSize(canvas,LegalText, w, h);
	Root.SetPosScaled(canvas,WinWidth-w-8,WinHeight-h-8);
	Canvas.DrawText(LegalText);
	canvas.font=saveFont;
}

function Created()
{
local int i;
local Texture tempTexture;
local float x,y,w,h;
local int LevelButtonHeight;
 

	Super.Created(); 
	

	LegalText=Localize("all","legal_title_01","Pickup");

	VersionButton = UWindowSmallButton(CreateControl(class'UWindowSmallButton',WinWidth-84,WinHeight-30,84, 25));
	VersionButton.setFont(F_Normal);
	VersionButton.TextColor.r=250;
	VersionButton.TextColor.g=250;
	VersionButton.TextColor.b=250;
	VersionButton.Align=TA_Center; 
	VersionButton.setText(class'Version'.default.version); 
//	VersionButton.HideWindow();	//debug mode only.


	// Level jump buttons for current build
	x = 470;
	y = 220;
	w = 150; 
	h = 14;

	if(!bE3DemoMode)
		{
		// Level jump buttons for official build levels
		for(i=0;i<FirstPreviewIndex;i++)
			{
			if(LevelList[i].levelName!="")
				{
				LevelList[i].Button = UWindowButton(CreateControl(class'UWindowButton',x,y,w,h));
				LevelList[i].Button.setFont(F_HPMenuLarge);
				LevelList[i].Button.TextColor.r=250;
				LevelList[i].Button.TextColor.g=250;
				LevelList[i].Button.TextColor.b=250;
				LevelList[i].Button.bColorOver=true;
				LevelList[i].Button.OverColor.r=250;
				LevelList[i].Button.OverColor.g=5;
				LevelList[i].Button.OverColor.b=5;
				LevelList[i].Button.Align=TA_Center; 
				LevelList[i].Button.setText(LevelList[i].levelName);		
				LevelList[i].Button.ToolTipString=LevelList[i].levelName;
				y += h;
				}
			}

		// Exit button
		y += h;			// Gap
		ExitButton = UWindowButton(CreateControl(class'UWindowButton',x,y,w,h));
		ExitButton.setFont(F_HPMenuLarge);
		ExitButton.TextColor.r=250;
		ExitButton.TextColor.g=250;
		ExitButton.TextColor.b=250;
		ExitButton.bColorOver=true;
		ExitButton.OverColor.r=250;
		ExitButton.OverColor.g=5;
		ExitButton.OverColor.b=5;
		ExitButton.Align=TA_Center; 
		ExitButton.setText("Exit");		
		ExitButton.ToolTipString="Exit To Windows";

		// Level jump buttons for un-official preview levels
		x = 32;
		y = 210;
		for(i=FirstPreviewIndex; i<30; i++)
			{
			if(LevelList[i].levelName!="")
				{
				LevelList[i].Button = UWindowButton(CreateControl(class'UWindowButton',x,y,w,h));
				LevelList[i].Button.setFont(F_HPMenuLarge);
				LevelList[i].Button.TextColor.r=96;
				LevelList[i].Button.TextColor.g=96;
				LevelList[i].Button.TextColor.b=112;
				LevelList[i].Button.TextColor.a=192;
				LevelList[i].Button.bColorOver=true;
				LevelList[i].Button.OverColor.r=128;
				LevelList[i].Button.OverColor.g=96;
				LevelList[i].Button.OverColor.b=5;
				LevelList[i].Button.Align=TA_Center; 
				LevelList[i].Button.setText(LevelList[i].levelName);		
				LevelList[i].Button.ToolTipString= LevelList[i].levelName;
				y += h;
				}
			}

		}
	else
		{
		E3DemoStartButton= UWindowButton(CreateControl(class'UWindowButton',(WinWidth/2)-80,WinHeight-100,160,60));
		E3DemoStartButton.ToolTipString="Start Demo";

		ExitButton = UWindowButton(CreateControl(class'UWindowButton',WinWidth-50,WinHeight-60,50,30));
		ExitButton.setFont(F_HPMenuLarge);
		ExitButton.TextColor.r=250;
		ExitButton.TextColor.g=250;
		ExitButton.TextColor.b=250;
		ExitButton.bColorOver=true;
		ExitButton.OverColor.r=250;
		ExitButton.OverColor.g=5;
		ExitButton.OverColor.b=5;
		ExitButton.Align=TA_Center; 
		ExitButton.setText("Exit");		
		ExitButton.ToolTipString="Exit To Windows";

		}

	
//	bE3DemoMode=true;
//	saveConfig();
}

function WindowDone(UWindowWindow W)
{
	if(W == ConfirmExit)
		{
		if(ConfirmExit.Result == ConfirmExit.button1.text)
			{
			Root.DoQuitGame();
			}
		ConfirmExit = None;
		}
}

function bool KeyEvent( byte/*EInputKey*/ Key, byte/*EInputAction*/ Action, FLOAT Delta )
{
	if(Action==1 && key==0x1b )	// Escape to exit program
	{
		ConfirmExit = doHPMessageBox(
			"Are you sure you want to exit?","Yes","No");
	}
	else

	if(Action==1 && key==0x7B/*F12*/)
	{
	//Notify( NewGameButton, DE_Click );
		FEBook(book).CloseBook();
	}

}
function Notify(UWindowDialogControl C, byte E)
{
local int i;

	if(e==DE_Click)
		{
			//see if clicked a level button.
		for(i=0;i<30;i++)
			if(LevelList[i].button==C)
				FEBook(book).RunURL( levelList[i].levelurl, false );

		if(E3DemoStartButton==C)
			FEBook(book).RunURL( "e3demo.unr", false );
		
		switch(c)								
			{
			case NewGameButton:
				break;
			case LoadGameButton:
				FEBook(book).ChangePageNamed("load");
				break;
			case OptionsButton:
				FEBook(book).ChangePageNamed("options");
				break;
			case CreditsButton:
				if(HPConsole(root.console).bDebugMode)
					FEBook(book).ShowCredits();
				break;
			case ExitButton:
				Root.DoQuitGame();
				if(HPConsole(root.console).bDebugMode)
					Root.DoQuitGame();
				else			
				{
					ConfirmExit = doHPMessageBox(
						GetLocalFEString("Main_Menu_0006"), //"Are you sure you want to exit?"
						GetLocalFEString("Shared_Menu_0003"),// "Yes"
						GetLocalFEString("Shared_Menu_0004") //"No"
						);
				}
			// MessageBox("Exit", "Are you sure you want to exit to windows?", MB_YesNo, MR_No, MR_None, 10);
				break;
			case LangButton:
				FEBook(book).ChangePageNamed("LANG");
				break;
			}
		}



}

function PreSwitchPage()
{
}

/*
// ShowButtons no longer needed.  The real problem was that MainPage was sometimes 
// hanging around as the current page when None really should have been the 
// current page.

// Show/Hide menu buttons.  If the user presses Esc in the middle of the game, menu
// buttons will show up during the "Really Exit" dialog unless we force them to
// be hidden.  So call this function before that dialog comes up to hide the buttons.
// Then, if the user is quitting back to the main menu (and not continuing on
// with the game, call this function to show the buttons again.
function ShowButtons(bool bShow)
{
local int i;

	if (bShow)
	{
		for(i=0;i<30;i++)
			if(LevelList[i].levelName!="")
				LevelList[i].button.ShowWindow();
		ExitButton.ShowWindow();
	}

	else
	{
		for(i=0;i<30;i++)
			if(LevelList[i].levelName!="")
				LevelList[i].button.HideWindow();

		ExitButton.HideWindow();
	}
}
*/

defaultProperties
{
	levelList(0)=(LevelName="Privet Drive",LevelURL="PrivetDr.unr");
	levelList(1)=(LevelName="Whomping Willow",LevelURL="adv1Willow.unr");
	levelList(2)=(LevelName="Ch1 Rictusempra",LevelURL="Ch1Rictusempra.unr");
	levelList(3)=(LevelName="Ch2 Skurge",LevelURL="Ch2Skurge.unr");
	levelList(4)=(LevelName="Dungeon Ingredient",LevelURL="Adv3DungeonQuest.unr");
	levelList(5)=(LevelName="Ch3 Diffindo",LevelURL="Ch3Diffindo.unr");
	levelList(6)=(LevelName="Greenhouse Ingredient",LevelURL="Adv4Greenhouse.unr");
	levelList(7)=(LevelName="A Bit of Goyle",LevelURL="Adv6Goyle.unr");
	levelList(8)=(LevelName="Slytherin Common Room",LevelURL="Adv7SlythComRoom.unr");
	levelList(9)=(LevelName="Ch4 Spongify",LevelURL="Ch4Spongify.unr");
	levelList(10)=(LevelName="Forbidden Forest",LevelURL="Adv8Forest.unr");
	levelList(11)=(LevelName="Aragog",LevelURL="Adv9Aragog.unr");	
	levelList(12)=(LevelName="Corridor of Secrets A",LevelURL="Adv11aCorridor.unr");
	levelList(13)=(LevelName="Corridor of Secrets B",LevelURL="Adv11bSecrets.unr");
	levelList(14)=(LevelName="Chamber of Secrets",LevelURL="Adv12Chamber.unr");


	FirstPreviewIndex=15;

	levelList(15)=(LevelName="Test Level",LevelURL="Studies\\TestLevel.unr");
	levelList(16)=(LevelName="Quidditch Lesson",LevelURL="Quidditch_Intro.unr");
	levelList(17)=(LevelName="Quidditch",LevelURL="Quidditch.unr");
	levelList(18)=(LevelName="Wizard Dueling",LevelURL="Arena.unr");
	levelList(19)=(LevelName="Bean Bonus",LevelURL="BeanRewardRoom.unr");
	levelList(20)=(LevelName="Gold Wizard Card",LevelURL="Ch6WizardCard.unr");
	levelList(21)=(LevelName="Entry Hall",LevelURL="Entryhall_hub.unr");
	levelList(22)=(LevelName="Grandstaircase",LevelURL="grandstaircase_hub.unr");
	levelList(23)=(LevelName="Grounds Hub",LevelURL="Grounds_Hub.unr");
	levelList(24)=(LevelName="Grounds (Night)",LevelURL="grounds_night.unr");
	levelList(25)=(LevelName="Gryffindor Challenge Chamber",LevelURL="Ch7Gryffindor.unr");


	bE3DemoMode=false;
}
class FEBook extends baseFEBook;

#EXEC TEXTURE IMPORT NAME=FELegalTexture1	 file=textures\menu\FE\Legal1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FELegalTexture2	 file=textures\menu\FE\Legal2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FELegalTexture3	 file=textures\menu\FE\Legal3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FELegalTexture4	 file=textures\menu\FE\Legal4.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FELegalTexture5	 file=textures\menu\FE\Legal5.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FELegalTexture6	 file=textures\menu\FE\Legal6.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=FEEALogoTexture1	 file=textures\menu\FE\EALogo001.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEEALogoTexture2	 file=textures\menu\FE\EALogo002.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEEALogoTexture3	 file=textures\menu\FE\EALogo003.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEEALogoTexture4	 file=textures\menu\FE\EALogo004.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEEALogoTexture5	 file=textures\menu\FE\EALogo005.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEEALogoTexture6	 file=textures\menu\FE\EALogo006.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=FEOptionsBackTexture1	 file=textures\menu\Options\001OptionBack.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOptionsBackTexture2	 file=textures\menu\Options\002OptionBack.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOptionsBackTexture3	 file=textures\menu\Options\003OptionBack.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOptionsBackTexture4	 file=textures\menu\Options\004OptionBack.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOptionsBackTexture5	 file=textures\menu\Options\005OptionBack.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOptionsBackTexture6	 file=textures\menu\Options\006OptionBack.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

var UWindowSmallButton DismissButton;


var hpMessageBox ConfirmQuitGame;

var baseFEPage	MainPage;
var baseFEPage	ReportPage;
var baseFEPage	FolioPage;
var baseFEPage	ChapterPage;
var baseFEPage	SavePage;
var FEInGamePage  InGamePage;

var baseFEPage	SlotPage;

// AMM
var baseFEPage	CreditsPage;

var baseFEPage	LoadPage;

//var baseFEPage	OptionsPage;
var baseFEPage		InputPage;
var baseFEPage		SoundVideoPage;

var baseFEPage	SplashPage;

var baseFEPage	LangPage;

var baseFEPage	MapPage;

var baseFEPage	QuidPage;

var baseFEPage	DuelPage;

var baseFEPage  HousepointsPage;

var baseFEPage	curPage;
var baseFEPage	prevPage;

var string      _URLToLoad;
var bool        bTravelItemsOnLoad;
//var float       _URLToLoadTimer;

var Texture LogoTexture1,LogoTexture2;

var bool bIsOpen;
var bool bShowBackground;

var bool bDrawLogo;

// AMM
var bool bInEndGame;

struct Background
{
	var Texture p1,p2,p3,p4,p5,p6;
	var float durration;
};

var Background SplashScreens[5];
var int numSplashScreens;
var int curSplashScreen;

var bool bShowSplash;
var float fShowSplashTime;

var bool bNewGame;			// Level was loaded because New Game button was clicked
var bool bGamePlaying;

//var UWindowSmallButton VersionButton;

var UWindowWrappedTextArea StatusBarTextWindow;

var bool bResolutionChanged;

var Background OptionsBackground;

var Background curBackground;

var bool bPlayingQuidditch;


function SaveSelectedSlot()
{
/*
	if(SlotPage!=None)
		{
		FESlotPage(SlotPage).SaveSelectedSlot();
		}
*/
}

function LoadSelectedSlot()
{
/*
	if(SlotPage!=None)
		{
		FESlotPage(SlotPage).LoadSelectedSlot();
		}
*/
}

function ToolTip(string strTip)
{
	StatusBarTextWindow.clear();
	StatusBarTextWindow.addText(strTip);
}

function ResolutionChanged(float W, float H)
{
	Super.ResolutionChanged(W,H);
	bResolutionChanged=true;
}

function ChangePage( baseFEPage page )
{
	log("ChangePage"@ page);

	if(curPage!=page)
	{
		prevPage = curPage;

		if (curPage != None)
			curPage.HideWindow();

		curPage=page;
		if( curPage != None )
		{
			curPage.PreSwitchPage();
			// Main page gets shown after startup.unr is loaded (otherwise, we see
			// the menu text way before the background text shows up).
			if (curPage != MainPage)
				curPage.ShowWindow();
		}
	}

	bShowBackground=false;
		//decide what tabs to show with this page.
	switch(curPage)
	{
		case MainPage:
			bGamePlaying = False;
			// AMM
			bInEndGame = False;
			break;
		
//		case OptionsPage:
//			bShowBackground=false;
//			break;

		case InputPage:
			bShowBackground=true;
			curBackground=OptionsBackground;
				//cmp 10-16 bug fix. eur2993
			break;
		
		case SoundVideoPage:
			bShowBackground=true;
			curBackground=OptionsBackground;
				//cmp 10-16 bug fix. eur2993
			break;
		
		case LangPage:
			break;

		case InGamePage:
			break;
		case QuidPage:
			break;
		case DuelPage:
			break;
        case HousepointsPage:
            break;
		case FolioPage:
			//curBackground=FolioBackground;			
			break;
		// JDW
		case CreditsPage:
//			ShowBackButton(false);
//			ShowTabs(false);
			break;
        case MapPage:
            break;
        case None:
            prevPage = None;
            break;
		default :
			log("UnknownPage in FEBook: " $page);
			break;
	}

	if (curPage != None)
		StatusBarTextWindow.WinTop = curPage.GetStatusY();
}


function ChangePagePrevious()
{
	if( prevPage != None && prevPage != curPage )
	{
		ChangePage( prevPage );
		prevPage = None;
	}
}

function ChangePageNamed(string name)
{
	
	switch(caps(name))
		{
		case "MAIN":
			changePage(MainPage);
			break;

//		case "OPTIONS":
//			log("changepagenamed options");
//			changePage(OptionsPage);
//			break;
		
		case "INPUT":
			log("changepagenamed input");
			changePage(InputPage);
			break;
		
		case "SOUNDVIDEO":
			log("changepagenamed soundvideo");
			changePage(SoundVideoPage);
			break;

		case "LANG":
		case "LANGUAGE":
			changePage(LangPage);
			break;

		case "INGAME" :
			changePage(InGamePage);
			break;

		case "FOLIO" :
			log("changepagenamed folio");
			changepage(FolioPage);
			break;

		case "MAP" :
			changePage(MapPage);
			break;
		case "QUID" :
		case "QUIDDITCH" :
			changePage(QuidPage);
			break;
		case "DUEL" :
			changePage(DuelPage);
			break;
        case "HPOINTS" :
            changePage(HousepointsPage);
            break;
		// JDW
		case "CREDITSPAGE":
			changePage(CreditsPage);
			break;
		default :
			log("UnknownPage in FEBook: " $name);
			break;
		}
}

//****************************************************************************************
// Set _URLToLoad to the map you want to load, and tick'll pick it up.
//  This is all that RunURL( string levURL ) does.
event Tick(float delta)
{
	local bool	bTravelItems;

	if( _URLToLoad != "" )
	{

		// If we're passing items, assume we're in the normal hub-to-hub game flow
		HPConsole(root.console).bInHubFlow = bTravelItemsOnLoad;

		// Suppress passing items if this is the first level of a new game
		if ( bNewGame )
		{
			bTravelItems = false;
			bNewGame = false;
		}
		else
			bTravelItems = bTravelItemsOnLoad;

		// Switch levels
		baseConsole(root.console).ChangeLevel(_URLToLoad,bTravelItems);
		_URLToLoad = "";
	
		if(_URLToLoad~="startup.unr")
		{
			OpenBook();
			ChangePageNamed("Main");
		}
		else
			CloseBook();
	}


}


function Created()
{
local int i;
local Texture tempTexture;
local LevelInfo lev;

	Super.Created();
 
	bNewGame = false;

		//debug mode button to allow you to go into a level specified on the command line
	DismissButton = UWindowSmallButton(CreateControl(class'UWindowSmallButton',WinWidth-10,0,10, 10));
	DismissButton.setFont(F_HPMenuLarge);

	StatusBarTextWindow = UWindowWrappedTextArea( CreateControl(class'UWindowWrappedTextArea', 0, WinHeight-26, 500, 26) );
	StatusBarTextWindow.Clear();
	StatusBarTextWindow.AddText("");
	StatusBarTextWindow.Font = F_HPMenuLarge;

	bShowBackground=true;

	InGamePage = FEInGamePage(CreateWindow(class'FEInGamePage',0,0,640,480)); 
	InGamePage.book=self;
	InGamePage.hideWindow();

	FolioPage = baseFEPage(CreateWindow(class'FEFolioPage',0,0,640,480)); 
	FolioPage.book=self;
	FolioPage.hideWindow();
	
//	OptionsPage = baseFEPage(CreateWindow(class'FEOptionsPage',0,0,640,480)); 
//	OptionsPage.book=self;
//	OptionsPage.hideWindow();

	InputPage = baseFEPage(CreateWindow(class'FEInputPage',0,0,640,480)); 
	InputPage.book=self;
	InputPage.hideWindow();

	SoundVideoPage = baseFEPage(CreateWindow(class'FESoundVideoPage',0,0,640,480)); 
	SoundVideoPage.book=self;
	SoundVideoPage.hideWindow();

	QuidPage = baseFEPage(CreateWindow(class'FEQuidPage',0,0,640,480)); 
	QuidPage.book=self;
	QuidPage.hideWindow();

	DuelPage = baseFEPage(CreateWindow(class'FEDuelPage',0,0,640,480)); 
	DuelPage.book=self;
	DuelPage.hideWindow();

	HousepointsPage = baseFEPage(CreateWindow(class'FEHousepointsPage',0,0,640,480)); 
	HousepointsPage.book=self;
	HousepointsPage.hideWindow();

	MainPage = baseFEPage(CreateWindow(class'FEMainPage',0,0,640,480)); 
	MainPage.book=self; 
	OpenBook("MAIN");
//	MainPage.hideWindow();

	MapPage = baseFEPage(CreateWindow(class'FEMapPage',0,0,640,480)); 
	MapPage.book=self;
	MapPage.hideWindow();

	CreditsPage = baseFEPage(CreateWindow(class'FECreditsPage',0,0,640,480));
	CreditsPage.book=self; 
	CreditsPage.hideWindow();

	LangPage = baseFEPage(CreateWindow(class'FESoundBrowser',0,0,640,480)); 
	LangPage.book=self;
	LangPage.hideWindow();



			//find out if this was an autoplay level
	lev=GetLevel();
	if( InStr(caps(lev.GetLocalUrl()),"AUTOPLAY")>=0)
	{	//yup so bypass the menus
		log("autoplay");
		bGamePlaying=true;
		bShowSplash=false;
		CloseBook();
	}


}

function ScaleAndDraw(Canvas canvas,float x,float y,Texture tex)
{
local float fx,fy;

	if(tex==None)
		return;

	fx=(canvas.SizeX/640.0);
	fy=(canvas.SizeY/480.0);

	fx=(canvas.SizeX/640.0);
	fy=(canvas.SizeY/480.0);
fx=1;fy=1;

	DrawStretchedTexture( canvas, x*fx, y*fy, tex.USize*fx,tex.VSize*fy,tex);

}


function Paint(Canvas canvas,float x,float y)
{
local int width;
local int i;
local int ox,oy;
local color saveColor;

	if(bResolutionChanged)
	{
		root.SetScale(root.realwidth/640);
		bResolutionChanged=false;
	}


	if(bShowBackground)
	{
		ScaleAndDraw(canvas,0,0,curBackground.p1);
		ScaleAndDraw(canvas,256,0,curBackground.p2);
		ScaleAndDraw(canvas,512,0,curBackground.p3);

		ScaleAndDraw(canvas,0,256,curBackground.p4);
		ScaleAndDraw(canvas,256,256,curBackground.p5);
		ScaleAndDraw(canvas,512,256,curBackground.p6);
	}
/*	if(HPConsole(root.console).bDebugMode)
		{
		canvas.SetPos(0,0);
		canvas.DrawText("Debug");
		}
*/
}


//****************************************************************************************

function OpenBook(optional string pageName)
{
		//revisit: find out what bLocked and quickkeyenable are.
	if (HPConsole(root.console).bLocked)
		return;
	HPConsole(root.console).bQuickKeyEnable = False;

	HPConsole(root.console).LaunchUWindow();

	bIsOpen=true;
	if (pageName != "")
		ChangePageNamed(pageName);

	log("OpenBook"@ pageName $","@ CurPage);

	if(CurPage!=None)
		CurPage.PreOpenBook();

}

function CloseBook()
{
	root.console.CloseUWindow();
	bIsOpen=false;
	ChangePage(None);
}

function bool IsInGameMenuShowing()
{
	return ( bIsOpen && ( (curPage == InGamePage) || (curPage == FolioPage) )     );
}

function bool IsInGameSubMenuShowing()
{
	return ( bIsOpen && ( (curPage != InGamePage) )     );
}

function HPMessageBox doHPMessageBox(string msg, string textButton1, optional string textButton2, optional float timeOut)
{
	local HPMessageBox w;
	
	w = HPMessageBox(Root.CreateWindow(class'HPMessageBox', (640-246)/2, (480-102)/2, 246, 102, Self));
	w.Setup (msg, textButton1, textButton2, timeOut);

	root.ShowModal(w);
	return w;
}

function ExitFromGame()
{
	if (HPConsole(root.console).bLocked)
		return;
	HPConsole(root.console).bQuickKeyEnable = False;

	HPConsole(root.console).LaunchUWindow();

	bGamePlaying=false;
	ChangePage(MainPage);
	
}

function WindowDone(UWindowWindow W)
{

	bShowBackGround=true;

	if(W == ConfirmQuitGame)
	{
		if(ConfirmQuitGame.Result == ConfirmQuitGame.button1.text)
		{
			CloseBook();
			baseConsole(root.console).ChangeLevel("startup.unr",false);

			bGamePlaying=false;
			OpenBook();
			ChangePage(MainPage);
		}
		else
		{
			CloseBook();
		}
		ConfirmQuitGame = None;
	}
}

function OnLevelLoadDone()
{
	if (bIsOpen && (CurPage == MainPage))
		MainPage.ShowWindow();
}

//***********************************************************************************************
function EscFromConsole()
{
	//revisit: find out what bLocked and quickkeyenable are.
	if (HPConsole(root.console).bLocked)
		return;
	HPConsole(root.console).bQuickKeyEnable = False;
	HPConsole(root.console).LaunchUWindow(true);

	OpenBook("INGAME");
	ChangePage(InGamePage);
}



function DoMapFromConsole()
{
	//revisit: find out what bLocked and quickkeyenable are.
	if (HPConsole(root.console).bLocked)
		return;
	HPConsole(root.console).bQuickKeyEnable = False;
	HPConsole(root.console).LaunchUWindow(true);

	OpenBook("MAP");
	ChangePage(MapPage);
}

function ExitFromConsole()
{
	//revisit: find out what bLocked and quickkeyenable are.
	if (HPConsole(root.console).bLocked)
		return;
	HPConsole(root.console).bQuickKeyEnable = False;
	HPConsole(root.console).LaunchUWindow();

	bShowBackGround=false;

	ConfirmQuitGame = doHPMessageBox("Are you sure you want to quit","Yes","No");
	return;
}

function Notify(UWindowDialogControl C, byte E)
{

	if(e==DE_Click)
		{
		switch(c)
			{
			case DismissButton:
				CloseBook();
				break;
			default :
				log("FEBook::Notify " $c);
				break;
			}
		}
}


function WindowEvent(WinMessage Msg, Canvas C, float X, float Y, int Key) 
{
	if(Msg == WM_Paint || !root.WaitModal())
		Super.WindowEvent(Msg, C, X, Y, Key);
}


//***********************************************************************************************
function bool KeyEvent( byte/*EInputKey*/ Key, byte/*EInputAction*/ Action, FLOAT Delta )
{

	// Dont process key if root window is displaying a modal dialog
	if( root.ModalWindow != None )
		return false;

	//check early dismiss of splash screen.
	if(bShowSplash &&HPConsole(root.console).bDebugMode)
		if(Action==1 /*IST_Press*/)
			{
			fShowSplashTime=-1;	//force change of splash screen.
			return true;
			}

	// AMM 
	// If page handles key then early out
	if(curPage != None )
	{
		if ( curPage.KeyEvent( Key, Action, Delta ) )
		{
			return true;
		}
	}


	//handle escape key
	if(Action==1 /*IST_Press*/ && key==0x1b /*escape*/)
	{
        log("febook escape");

        return (DoEscapeFromPage());
	}

	if(Action==1 /*IST_Press*/ && key==0xbb /*equals*/)
	{
        log("febook backspace");
		if( curPage == MapPage )
		{
		    CloseBook();
			return (true);
		}
	}

	return false;
}

function bool DoEscapeFromPage()
{
	if( curPage == InGamePage)
	{
	    CloseBook();
		return (true);
	}
	else if(curPage != MainPage)
	{
        if (prevPage == None)
            CloseBook();

        else
    		ChangePagePrevious();
		
		log( "FEBook: curPage == " $curPage $" prevPage == " $prevPage );
		return (true);
	}
	else if (  prevPage == CreditsPage )
	{
		CloseBook();
	//	Root.DoQuitGame();
	}

     else 
         return (true);
}

//***********************************************************************************************
//Called from FELevSelectPage, and FEStoryBookPage
function RunURL( string levURL, bool bTravelItems )
{
	log("runurl");
	bTravelItemsOnLoad = bTravelItems;
	_URLToLoad = levURL;
	bGamePlaying=true;
}

//***********************************************************************************************
// AMM
function EndGame()
{
	// This changes behaviour of space and escape for folio page
	bInEndGame = true;

}


function RunTheCredits()
{
	if (HPConsole(root.console).bLocked)
		return;
	HPConsole(root.console).bQuickKeyEnable = False;
	HPConsole(root.console).LaunchUWindow(true);

	OpenBook();
	ShowCredits();
//	ChangePage(CreditsPage);
}


function ShowCredits()
{
	// This changes behaviour of space and escape for folio page
	bInEndGame = true;
//	curPage = CreditsPage;

	if( CreditsPage == None )
	{
		CreditsPage = baseFEPage(CreateWindow(class'FECreditsPage',0,0,640,480));
		CreditsPage.book=self; 
		CreditsPage.hideWindow();
	}

//	ShowBackButton(false);
	ChangePageNamed("CREDITSPAGE");
}


//***********************************************************************************************
//Called from FELevSelectPage, and FEStoryBookPage
function DoStoryBookInterlude( int StoryBookIdx, name EventWhenDone )
{
}

//***********************************************************************************************


defaultproperties
{
	numSplashScreens=0;
	SplashScreens(0)=(durration=0,p1=Texture'FEEALogoTexture1',p2=Texture'FEEALogoTexture2',p3=Texture'FEEALogoTexture3',p4=Texture'FEEALogoTexture4',p5=Texture'FEEALogoTexture5',p6=Texture'FEEALogoTexture6');
	SplashScreens(1)=(durration=0,p1=Texture'FELegalTexture1',p2=Texture'FELegalTexture2',p3=Texture'FELegalTexture3',p4=Texture'FELegalTexture4',p5=Texture'FELegalTexture5',p6=Texture'FELegalTexture6');
	OptionsBackground=(durration=999999.0,p1=Texture'FEOptionsBackTexture1',p2=Texture'FEOptionsBackTexture2',p3=Texture'FEOptionsBackTexture3',p4=Texture'FEOptionsBackTexture4',p5=Texture'FEOptionsBackTexture5',p6=Texture'FEOptionsBackTexture6');
}

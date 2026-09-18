//=============================================================================
// baseHUD
//=============================================================================
class baseHud extends HUD;

#EXEC TEXTURE IMPORT NAME=leftPanel  FILE=TEXTURES\Menu\HUD\leftPanel.pcx GROUP=Icons MIPS=off
#EXEC TEXTURE IMPORT NAME=middlePanel  FILE=TEXTURES\Menu\HUD\middlePanel.pcx GROUP=Icons MIPS=off
#EXEC TEXTURE IMPORT NAME=rightPanel  FILE=TEXTURES\Menu\HUD\rightPanel.pcx GROUP=Icons MIPS=off

//#exec new TrueTypeFontFactory Name=InkFont FontName="InkPotFitCaps" Height=16 AntiAlias=1 CharactersPerPage=32 
//#exec new TrueTypeFontFactory Name=InkFont FontName="Georgia" Height=16 AntiAlias=1 CharactersPerPage=32 
//#exec new TrueTypeFontFactory Name=InkFont FontName="Georgia" Height=18 AntiAlias=0 CharactersPerPage=32 
//#exec new TrueTypeFontFactory Name=ChiFont FontName="MHei Medium" Height=20 AntiAlias=0 RenderNative=1 Count=65535
#exec new TrueTypeFontFactory Name=HugeInkFont FontName="Times New Roman" Xpad=2 Height=24 AntiAlias=0 CharactersPerPage=32 
#exec new TrueTypeFontFactory Name=BigInkFont FontName="Times New Roman" Height=18 AntiAlias=0 CharactersPerPage=32 
#exec new TrueTypeFontFactory Name=MedInkFont FontName="Times New Roman" Height=14 AntiAlias=0 CharactersPerPage=32 
#exec new TrueTypeFontFactory Name=SmallInkFont FontName="Times New Roman" Height=12 AntiAlias=0 CharactersPerPage=32 
#exec new TrueTypeFontFactory Name=TinyInkFont FontName="Times New Roman" Height=10 AntiAlias=0 CharactersPerPage=32 

#exec new TrueTypeFontFactory Name=AsianFontHuge FontName="Gulim" Height=24 AntiAlias=0 RenderNative=1 
#exec new TrueTypeFontFactory Name=AsianFontBig FontName="Gulim" Height=18 AntiAlias=0 RenderNative=1  
#exec new TrueTypeFontFactory Name=AsianFontMed FontName="Gulim" Height=14 AntiAlias=0 RenderNative=1  
#exec new TrueTypeFontFactory Name=AsianFontSmall FontName="Gulim" Height=12 AntiAlias=0 RenderNative=1 

#exec new TrueTypeFontFactory Name=JapFontHuge FontName="PMingLiU" Height=24 AntiAlias=0 RenderNative=1 
#exec new TrueTypeFontFactory Name=JapFontBig FontName="PMingLiU" Height=18 AntiAlias=0 RenderNative=1  
#exec new TrueTypeFontFactory Name=JapFontMed FontName="PMingLiU" Height=14 AntiAlias=0 RenderNative=1  
#exec new TrueTypeFontFactory Name=JapFontSmall FontName="PMingLiU" Height=12 AntiAlias=0 RenderNative=1 


#exec new TrueTypeFontFactory Name=SystemFontHuge FontName="system" Height=24 AntiAlias=0 RenderNative=1 CharactersPerPage=64 
#exec new TrueTypeFontFactory Name=SystemFontBig FontName="system" Height=18 AntiAlias=0 RenderNative=1 CharactersPerPage=64 
#exec new TrueTypeFontFactory Name=SystemFontMed FontName="system" Height=14 AntiAlias=0 RenderNative=1 CharactersPerPage=64 
#exec new TrueTypeFontFactory Name=SystemFontSmall FontName="system" Height=12 AntiAlias=0 RenderNative=1 CharactersPerPage=64 


//#exec new TrueTypeFontFactory Name=ThaiFontHuge FontName="CordiaUPC" Height=24 AntiAlias=0 RenderNative=1 CharactersPerPage=64 
//#exec new TrueTypeFontFactory Name=ThaiFontBig FontName="CordiaUPC" Height=18 AntiAlias=0 RenderNative=1 CharactersPerPage=64 
//#exec new TrueTypeFontFactory Name=ThaiFontMed FontName="CordiaUPC" Height=14 AntiAlias=0 RenderNative=1 CharactersPerPage=64 
//#exec new TrueTypeFontFactory Name=ThaiFontSmall FontName="CordiaUPC" Height=12 AntiAlias=0 RenderNative=1 CharactersPerPage=64 

#exec new TrueTypeFontFactory Name=ThaiFontHuge FontName="Tahoma" Height=24 AntiAlias=0 RenderNative=1 CharactersPerPage=64 
#exec new TrueTypeFontFactory Name=ThaiFontBig FontName="Tahoma" Height=20 AntiAlias=0 RenderNative=1 CharactersPerPage=64 
#exec new TrueTypeFontFactory Name=ThaiFontMed FontName="Tahoma" Height=18 AntiAlias=0 RenderNative=1 CharactersPerPage=64 
#exec new TrueTypeFontFactory Name=ThaiFontSmall FontName="Tahoma" Height=14 AntiAlias=0 RenderNative=1 CharactersPerPage=64 


var bool bCutSceneMode;
var bool bCutPopupMode;
var bool bDrawDialogText;


enum _HUDGameType
{
	HUDG_QUIDDITCH,
	HUDG_FLYINGKEYS,
};

struct IconMessage
{
	var bool valid;
	var Texture icon;
	var string message;
	var float duration;	//time that message will disappear
};

var _HUDGameType HUDGameType;

var IconMessage curIconMessage;

var basePopup curPopup;

// @PAB debug info

var string	DebugString;
var int		DebugValA;
var int		DebugValX;
var int		DebugValY;
var int		DebugValZ;

var string	DebugString2;
var int		DebugValA2;
var int		DebugValX2;
var int		DebugValY2;
var int		DebugValZ2;

var bool	bScoreCountup;
var float	fScoreCountTime;
var float	fMaxScoreCountTime;

var bool bPlayQHUDGame;
//var baseQHUDGame	QHUDGame;


event Tick(float fDeltaTime)
{
	super.tick(fDeltaTime);

	if(curIconMessage.valid)
		{
		curIconMessage.duration-=fDeltaTime;
		if(curIconMessage.duration<0)
			curIconMessage.valid=false;
		}
}
function SetScoreCountTime(float t)
{
	fScoreCountTime = t;
	fMaxScoreCountTime = t;
}

function PlayHUDGame(bool bEnable )
{
	bPlayQHUDGame = bEnable;
}

function SetHUDGameType(_HUDGameType GameType )
{
	HUDGameType = GameType;
}

function DrawDebug(Canvas canvas)
{
	Canvas.SetPos(8, Canvas.SizeY-240);
	Canvas.DrawText("Text " $DebugString, False);
	Canvas.SetPos(8, Canvas.SizeY-224);
	Canvas.DrawText("ValA " $DebugValA, False);
	Canvas.SetPos(8, Canvas.SizeY-208);
	Canvas.DrawText("ValX " $DebugValX, False);
	Canvas.SetPos(8, Canvas.SizeY-192);
	Canvas.DrawText("ValY " $DebugValY, False);
	Canvas.SetPos(8, Canvas.SizeY-176);
	Canvas.DrawText("ValZ " $DebugValZ, False);

	Canvas.SetPos(8, Canvas.SizeY-144);
	Canvas.DrawText("Text " $DebugString2, False);
	Canvas.SetPos(8, Canvas.SizeY-128);
	Canvas.DrawText("ValA " $DebugValA2, False);
	Canvas.SetPos(8, Canvas.SizeY-112);
	Canvas.DrawText("ValX " $DebugValX2, False);
	Canvas.SetPos(8, Canvas.SizeY-96);
	Canvas.DrawText("ValY " $DebugValY2, False);
	Canvas.SetPos(8, Canvas.SizeY-80);
	Canvas.DrawText("ValZ " $DebugValZ2, False);

}

function ShowPopup(class<basePopup> popup)
{
	curPopup=Spawn(popup);
}

function DestroyPopup()
{
	if (curPopup != None)
	{
		curPopup.destroy();
		curPopup = None;
	}
}

function DrawPopup(Canvas canvas)
{
	if(curPopup==None)
		return;
	curPopup.Draw(canvas);

	if(curPopup.bDeleteMe)
		curPopup=None;

}


function ReceiveIconMessage(Texture icon,string message,float duration)
{
	curIconMessage.icon=icon;
	curIconMessage.message=message;
	curIconMessage.duration=duration;
	curIconMessage.valid=true;
}


simulated function HUDSetup(canvas canvas)
{
	// Setup the way we want to draw all HUD elements
	Canvas.Reset();
	Canvas.SpaceX=0;
	Canvas.bNoSmooth = True;
	Canvas.DrawColor.r = 255;
	Canvas.DrawColor.g = 255;
	Canvas.DrawColor.b = 255;	

	Canvas.Font=baseConsole(playerpawn(owner).player.console).LocalMedFont;
/*
	if(baseConsole(playerpawn(owner).player.console).bUseAsianFont)
		Canvas.Font=Font'AsianFontMed';
	if(baseConsole(playerpawn(owner).player.console).bUseThaiFont)
		Canvas.Font=Font'ThaiFontMed';
	else
		Canvas.Font=Font'MedInkFont';
*/
}
exec function ToggleDialog()
{
	bDrawDialogText=!bDrawDialogText;
}


defaultproperties
{
	bDrawDialogText=true;
}

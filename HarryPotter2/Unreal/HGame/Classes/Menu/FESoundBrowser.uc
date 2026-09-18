class FESoundBrowser expands baseFEPage;

var bool bAlreadyLoaded;

var FELangGrid langGrid;

var string MasterList[3000];
var string status[3000];
var string MasterText[3000];
var int MasterCount;

function Created()
{
local int i;
local Texture tempTexture;
local float x,y;
	Super.Created(); 

	langGrid=FELangGrid(CreateWindow(class'FELangGrid', 10,20,620,398));
	langGrid.browser=self;


}

function Paint(Canvas canvas,float x,float y)
{
local float w,h;

	super.Paint(canvas, x, y);

	TextSize(canvas,"Language Browser", w, h);
	Root.SetPosScaled(canvas,320-(w/2),0);
	Canvas.DrawText("Language Browser");

	HPHud(root.console.viewport.Actor.MyHud).managerCutScene.RenderHudItemManager(Canvas, false, true, false);


//	baseHud(root.console.viewport.Actor.MyHud).DrawIconMessages(canvas);
//	baseHud(root.console.viewport.Actor.MyHud).fCutSceneBoarderOffset=canvas.SizeY/8;

}

function LoadDialogKeys()
{
local string id,key;
//local int count;

	masterCount=0;
	while(true)
	{
		id="key_"$masterCount;
		key=Localize( "all", id,"HPKeys" );
		if(instr(key,"<")>-1)
			return;	//past the end of the list.

		masterList[masterCount]=key;
		masterText[masterCount]=Localize( "all", masterList[masterCount],"HPdialog" );

			//if not found look in bumpdialog 
		if(instr(masterText[masterCount],"<")>-1)
			masterText[masterCount]=Localize( "all", masterList[masterCount],"BumpDialog" );

			//if not found clear 
		if(instr(masterText[masterCount],"<")>-1)
			masterText[masterCount]="";


		masterCount++;
	}

}

function PreSwitchPage()
{
local string text;
local sound sound;
local int i;

	if(bAlreadyLoaded)
		return;
	bAlreadyLoaded=true;

//	harry(root.console.viewport.Actor).bIsCaptured=true;
//	harry(root.console.viewport.Actor).myHud.StartCutScene();

	LoadDialogKeys();
	
	for(i=0;i<MasterCount;i++)
		{
		sound = Sound( DynamicLoadObject("ALLDialog."$masterList[i], class'Sound') );

//		baseHarry(root.console.viewport.Actor).theNarrator.FindDialog(MasterList[i],sound,MasterText[i]);
		if(MasterText[i]=="")
			{
			status[i]="NOTXT";
			}		
		else
			{
			if(sound==None)
				status[i]="NOSND";
			else
				status[i]="OK";
			}
		}
}

function Notify(UWindowDialogControl C, byte E)
{
local int i;


	if(e==DE_Click)
		{
/*		if(c==ResumeButton)
			{
			return;
			}
*/		}
}

defaultProperties
{

}
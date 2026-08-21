class FEDuelPage expands baseFEPage;

//#EXEC TEXTURE IMPORT NAME=QuidMatchLockTexture	 file=textures\menu\QuidMatchLock.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


var HPMessageBox ConfirmReplay;
var harry PlayerHarry;

var UWindowButton rankingButtons[10];
var UWindowButton lockedButtons[10];
var texture lockedTexture;

var UWindowLabelControl pageTitle;		//Wizard Rankings.


var string duelistNames[10];


function Created()
{
//	textureObjectiveBkgrd = texture(DynamicLoadObject("HGame.Icons.leftPanel" , class'Texture'));

	local int startX,startY,gameBoxWidth,gameBoxHeight,gameSpaceX,gameSpaceY;
	local int i,row,col;

	local texture crestIcons[4];

	startY=40;

	playerHarry=Harry(HPConsole(root.console).Viewport.Actor);

	pageTitle=UWindowLabelControl(CreateControl(class'UWindowLabelControl', (WinWidth/2)-200,startY,400,30));
	pageTitle.setFont(F_HPMenuLarge);
	pageTitle.TextColor.r=215;
	pageTitle.TextColor.g=0;
	pageTitle.TextColor.b=215;
	pageTitle.Align=TA_Center;
	pageTitle.bShadowText=true;
	pageTitle.setText("Wizard Dueling Rankings");

	startY+=60;

//	lockedTexture=texture'QuidMatchLockTexture';
//	lockedTexture=texture(DynamicLoadObject("HP_Menu.Hud.FolioSilverLock", class'Texture'));

    CreateBackPageButton();

	for(i=0;i<10;i++)
	{
		rankingButtons[i]=UWindowButton(CreateControl(class'UWindowButton',(WinWidth/2)-200,startY+(i*22),400,20));
		rankingButtons[i].setFont(F_HPMenuLarge);
		rankingButtons[i].TextColor.r=255;
		rankingButtons[i].TextColor.g=255;
		rankingButtons[i].TextColor.b=255;
		rankingButtons[i].Align=TA_Center;
		rankingButtons[i].bShadowText=true;
		rankingButtons[i].setText("Duelist "$i);

/*		lockedButtons[i]=UWindowButton(CreateControl(class'UWindowButton',(WinWidth/2)-232,startY+(i*22),32,32));
		lockedButtons[i].UpTexture=lockedTexture; 
		lockedButtons[i].DownTexture=lockedTexture; 
		lockedButtons[i].OverTexture=lockedTexture; 
*/	}

	Super.Created(); 
}	



function PreSwitchPage()
{
local int i,count;
local int nGameState;
local string sortedDuelistNames[10];

	playerHarry=Harry(HPConsole(root.console).Viewport.Actor);

	nGameState=playerHarry.ConvertGameStateToNumber();

/*	lastUnlockedDuelist--;
	if(lastUnlockedDuelist<0)
		lastUnlockedDuelist=0;
*/

	if(nGameState>=80)
		playerHarry.lastUnlockedDuelist=8;
	if(nGameState>=90)
		playerHarry.lastUnlockedDuelist=7;
	if(nGameState>=100)
		playerHarry.lastUnlockedDuelist=6;
	if(nGameState>=110)
		playerHarry.lastUnlockedDuelist=5;
	if(nGameState>=115)
		playerHarry.lastUnlockedDuelist=4;
	if(nGameState>=130)
		playerHarry.lastUnlockedDuelist=3;
	if(nGameState>=140)
		playerHarry.lastUnlockedDuelist=1;
	if(nGameState>=145)
		playerHarry.lastUnlockedDuelist=0;

	count=0;
	for(i=0;i<10;i++)
	{
		if(i==playerHarry.curWizardDuelRank)
		{
			sortedDuelistNames[i]=""$i+1$". Harry";

		}
		else
		{
			sortedDuelistNames[i]="" $i+1 $". " $duelistNames[count];
			count++;
		}
	}

	for(i=0;i<10;i++)
	{

		if(i<playerHarry.lastUnlockedDuelist)
		{
			rankingButtons[i].TextColor.r=128;
			rankingButtons[i].TextColor.g=128;
			rankingButtons[i].TextColor.b=128;
		}
		else
		{
			rankingButtons[i].TextColor.r=255;
			rankingButtons[i].TextColor.g=255;
			rankingButtons[i].TextColor.b=255;
		}
		if(i==playerHarry.curWizardDuelRank)
		{
			rankingButtons[i].TextColor.r=255;
			rankingButtons[i].TextColor.g=255;
			rankingButtons[i].TextColor.b=0;
		}

		rankingButtons[i].setText(sortedDuelistNames[i]);

	}

	Super.PreSwitchPage();
}








function BeforePaint(Canvas C, float X, float Y)
{

	Super.BeforePaint(C,X,Y);
}

function Paint(Canvas canvas,float x,float y)
{
	local float   fScaleFactor;
	local bool    bHaveObjectiveText;
	local float wid,hei;

	fScaleFactor = Canvas.SizeX/WinWidth; 
	
	Super.Paint(canvas, x, y);
}



function LaunchWizardDuel()
{
local string levName;
	FEBook(book).CloseBook();

	levName="Duel0" $playerHarry.curWizardDuel+1 $".unr";
	HPConsole(root.console).Viewport.Actor.ClientMessage("Launching Wizard Dueling:"$levName);
	HPConsole(root.console).viewport.Actor.Level.ServerTravel( levName, true );
}

// Note: Escape handled by FEBook
//function bool KeyEvent( byte/*EInputKey*/ Key, byte/*EInputAction*/ Action, FLOAT Delta )
//{
//	if(Action==1 && key==0x1b )	// Escape to exit program
//	{
//		FEBook(book).CloseBook();
//	}
//	return (false);
//}

function WindowDone(UWindowWindow W)
{
	if(W == ConfirmReplay)
		{
		if(ConfirmReplay.Result == ConfirmReplay.button1.text)
			{
			HPConsole(root.console).Viewport.Actor.ClientMessage("Launching Quidditch match "$playerHarry.curQuidMatchNum);
			LaunchWizardDuel();
			}
		ConfirmReplay = None;
		}
}

function Notify(UWindowDialogControl C, byte E)
{
	local int i;

	if(e==DE_Click)
	{	
		if(c==BackPageButton)
		{
			FEBook(book).DoEscapeFromPage();
		}
		for(i=0;i<10;i++)
		{
			if(rankingButtons[i]==C)
			{
				if(i<playerHarry.lastUnlockedDuelist ||i == playerHarry.curWizardDuelRank)
					return;

                // If got to DuelPage from InGame menu, can't actually play- can only
                // look at this screen.
                if (FEBook(book).prevPage == FEBook(book).InGamePage)
                    return;

				playerHarry.curWizardDuel=i;

//WinDuel();				
				LaunchWizardDuel();
			}
		}

	}
}


defaultProperties
{


	duelistNames(0)="Edward";
	duelistNames(1)="Rebecca";
	duelistNames(2)="Heather";
	duelistNames(3)="Roy";
	duelistNames(4)="Stewart";
	duelistNames(5)="Bridget";
	duelistNames(6)="Andrew";
	duelistNames(7)="Rachel";
	duelistNames(8)="Emily";
	duelistNames(9)="Peter";


//	duelistNames(10)={"Harry"};


}
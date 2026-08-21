class FEQuidPage expands baseFEPage;

#EXEC TEXTURE IMPORT NAME=QuidMatchBoxTexture	 file=textures\menu\QuidMatchBox.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=QuidMatchLockTexture	 file=textures\menu\QuidMatchLock.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=GryfCrestTexture		 file=textures\menu\FE\griffendorsmall.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=HuffCrestTexture		 file=textures\menu\FE\Hufflepuffsmall.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=RaveCrestTexture		 file=textures\menu\FE\ravenclawsmall.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=SlytCrestTexture		 file=textures\menu\FE\slytherinsmall.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


var HPMessageBox ConfirmReplay;
var harry PlayerHarry;

var UWindowButton startGameButtons[6];
var UWindowButton opponentCrests[6];
var UWindowButton myCrests[6];

var UWindowLabelControl opponentScores[6];
var UWindowLabelControl myScores[6];
var UWindowLabelControl myPoints[6];
var texture lockedTexture;

var UWindowLabelControl pageTitle;		//Quidditch
var UWindowLabelControl matchLabel[6];




function PreSwitchPage()
{
local int i;
local int nGameState;

	playerHarry=Harry(HPConsole(root.console).Viewport.Actor);
	nGameState=playerHarry.ConvertGameStateToNumber();

	HPConsole(root.console).Viewport.Actor.ClientMessage("Launching Quidditch menu curGameState:"$nGameState);

	if(nGameState>=40)
		playerHarry.quidGameResults[0].bLocked=false;
	if(nGameState>=50)
		playerHarry.quidGameResults[1].bLocked=false;
	if(nGameState>=80)
		playerHarry.quidGameResults[2].bLocked=false;
	if(nGameState>=100)
		playerHarry.quidGameResults[3].bLocked=false;
	if(nGameState>=130)
		playerHarry.quidGameResults[4].bLocked=false;
	if(nGameState>=145)
		playerHarry.quidGameResults[5].bLocked=false;



	for(i=0;i<ArrayCount(playerHarry.quidGameResults);i++)
	{
		if(playerHarry.quidGameResults[i].bLocked)
		{
			startGameButtons[i].ToolTipString="Locked";  
			startGameButtons[i].UpTexture=lockedTexture; //Texture'GreenUpTexture';
			startGameButtons[i].DownTexture=lockedTexture; //Texture'GreenUpTexture';
			startGameButtons[i].OverTexture=lockedTexture; //Texture'GreenUpTexture';
			myScores[i].setText("");
			opponentScores[i].setText("");
			myPoints[i].setText("");
			myCrests[i].HideWindow(); 
			opponentCrests[i].HideWindow(); 

		}
		else
		{
			startGameButtons[i].ToolTipString="Play match "$i;  
			startGameButtons[i].UpTexture=texture 'QuidMatchBoxTexture'; //Texture'GreenUpTexture';
			startGameButtons[i].DownTexture=texture 'QuidMatchBoxTexture'; //Texture'GreenUpTexture';
			startGameButtons[i].OverTexture=texture 'QuidMatchBoxTexture'; //Texture'GreenUpTexture';

				//update scores.
			myScores[i].setText(""$playerHarry.quidGameResults[i].myScore);
			opponentScores[i].setText(""$playerHarry.quidGameResults[i].opponentScore);
			myPoints[i].setText("Points " $playerHarry.quidGameResults[i].housePoints);
			myCrests[i].ShowWindow(); 
			opponentCrests[i].ShowWindow(); 
		}
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

function Created()
{
//	textureObjectiveBkgrd = texture(DynamicLoadObject("HGame.Icons.leftPanel" , class'Texture'));

	local int startX,startY,gameBoxWidth,gameBoxHeight,gameSpaceX,gameSpaceY;
	local int i,row,col;

	local texture crestIcons[4];

	gameBoxWidth=128;
	gameBoxHeight=128;

	gameSpaceX=64+gameBoxWidth;
	gameSpaceY=32+gameBoxHeight;

	startX=(WinWidth/2) - ( (3*gameBoxWidth +(2*64))/2);
	startY=40;

	playerHarry=Harry(HPConsole(root.console).Viewport.Actor);

	pageTitle=UWindowLabelControl(CreateControl(class'UWindowLabelControl', (WinWidth/2)-200,startY,400,30));
	pageTitle.setFont(F_HPMenuLarge);
	pageTitle.TextColor.r=215;
	pageTitle.TextColor.g=0;
	pageTitle.TextColor.b=215;
	pageTitle.Align=TA_Center;
	pageTitle.bShadowText=true;
	pageTitle.setText("Quidditch");

	startY+=60;


	crestIcons[0]=texture'GryfCrestTexture';
	crestIcons[1]=texture'HuffCrestTexture';
	crestIcons[2]=texture'RaveCrestTexture';
	crestIcons[3]=texture'SlytCrestTexture';

	lockedTexture=texture'QuidMatchLockTexture';

    CreateBackPageButton();

	for(i=0;i<ArrayCount(playerHarry.quidGameResults);i++)
	{
		if(i>2)
		{
			row=1;
			col=i-3;
		}
		else
		{
			row=0;
			col=i;
		}

		matchLabel[i]=UWindowLabelControl(CreateControl(class'UWindowLabelControl', 
				startX+(col*gameSpaceX),startY+(row*gameSpaceY)-20,gameBoxWidth,30));
		matchLabel[i].setFont(F_HPMenuLarge);
		matchLabel[i].TextColor.r=215;
		matchLabel[i].TextColor.g=100;
		matchLabel[i].TextColor.b=215;
		matchLabel[i].Align=TA_Center;
		matchLabel[i].bShadowText=true;
		matchLabel[i].setText("Match "$i);

		myCrests[i]=UWindowButton(CreateControl(class'UWindowButton',startX+(col*gameSpaceX),startY+(row*gameSpaceY)+5,64,64));
		myCrests[i].Align=TA_Center;
		myCrests[i].UpTexture=crestIcons[0]; 
		myCrests[i].DownTexture=crestIcons[0]; 
		myCrests[i].OverTexture=crestIcons[0]; 

		opponentCrests[i]=UWindowButton(CreateControl(class'UWindowButton',startX+(col*gameSpaceX)+64,startY+(row*gameSpaceY)+5,64,64));
		opponentCrests[i].Align=TA_Center;
		opponentCrests[i].UpTexture=crestIcons[1+(i%3)]; 
		opponentCrests[i].DownTexture= crestIcons[1+(i%3)]; 
		opponentCrests[i].OverTexture= crestIcons[1+(i%3)]; 


		startGameButtons[i]=UWindowButton(CreateControl(class'UWindowButton',startX+(col*gameSpaceX),startY+(row*gameSpaceY),
											gameBoxWidth,gameBoxHeight));
		startGameButtons[i].setFont(F_HPMenuLarge);
		startGameButtons[i].Align=TA_Center;
		startGameButtons[i].bShadowText=true;
		startGameButtons[i].ToolTipString="Play match "$i;  
		startGameButtons[i].UpTexture=texture 'QuidMatchBoxTexture'; //Texture'GreenUpTexture';
		startGameButtons[i].DownTexture=texture 'QuidMatchBoxTexture'; //Texture'GreenUpTexture';
		startGameButtons[i].OverTexture=texture 'QuidMatchBoxTexture'; //Texture'GreenUpTexture';


		myScores[i]=UWindowLabelControl(CreateControl(class'UWindowLabelControl',startX+(col*gameSpaceX),startY+(row*gameSpaceY)+5+64,64,20));
		myScores[i].setFont(F_HPMenuLarge);
		myScores[i].TextColor.r=255;
		myScores[i].TextColor.g=255;
		myScores[i].TextColor.b=255;
		myScores[i].Align=TA_Center;
		myScores[i].bShadowText=true;
		myScores[i].setText("");

		opponentScores[i]=UWindowLabelControl(CreateControl(class'UWindowLabelControl',startX+(col*gameSpaceX)+64,startY+(row*gameSpaceY)+5+64,64,20));
		opponentScores[i].setFont(F_HPMenuLarge);
		opponentScores[i].TextColor.r=255;
		opponentScores[i].TextColor.g=255;
		opponentScores[i].TextColor.b=255;
		opponentScores[i].Align=TA_Center;
		opponentScores[i].bShadowText=true;
		opponentScores[i].setText("");

		myPoints[i]=UWindowLabelControl(CreateControl(class'UWindowLabelControl',startX+(col*gameSpaceX)+32,startY+(row*gameSpaceY)+100,64,20));
		myPoints[i].setFont(F_HPMenuLarge);
		myPoints[i].TextColor.r=255;
		myPoints[i].TextColor.g=255;
		myPoints[i].TextColor.b=255;
		myPoints[i].Align=TA_Center;
		myPoints[i].bShadowText=true;
		myPoints[i].setText("");


	
	}



	Super.Created(); 
}	


function LaunchQuidditch()
{
	FEBook(book).CloseBook();
	HPConsole(root.console).viewport.Actor.Level.ServerTravel( "Quidditch.unr", true );
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
			LaunchQuidditch();
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
		for(i=0;i<ArrayCount(startGameButtons);i++)
		{
			if(startGameButtons[i]==C)
			{
				playerHarry.curQuidMatchNum=i;

				if(playerHarry.quidGameResults[i].bLocked)
					return;

                // If got to QuidPage from InGame menu, can't actually play- can only
                // look at this screen.
                if (FEBook(book).prevPage == FEBook(book).InGamePage)
                    return;

				if(playerHarry.quidGameResults[i].bWon)
				{
					ConfirmReplay = doHPMessageBox(("Do you want to replay this game?"), //
											 GetLocalFEString("Shared_Menu_0003"),// "Yes"
											 GetLocalFEString("Shared_Menu_0004")); //"No"
				}										
				else
				{
					HPConsole(root.console).Viewport.Actor.ClientMessage("Launching Quidditch match "$playerHarry.curQuidMatchNum);
					LaunchQuidditch();
				}
			}
		}


	}
}


defaultProperties
{



}
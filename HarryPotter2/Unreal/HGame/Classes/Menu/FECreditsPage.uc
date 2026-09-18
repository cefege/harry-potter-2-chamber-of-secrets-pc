
class FECreditsPage expands baseFEPage;

// AMM - Credits
var HPCreditsControl CreditsTextWindow;

//***************************************************************************************************************
function Created()
{
	if( CreditsTextWindow == none )
	{
		CreditsTextWindow = HPCreditsControl( CreateControl(class'HPCreditsControl', 50, 10, WinWidth-100, WinHeight - 20) );
	}
}
//***************************************************************************************************************
function Paint( Canvas C, float MouseX, float MouseY )
{
//	DrawStretchedTexture( C, 0, 0, 640, 480, texture'UWindow.BlackTexture' );

	Super.Paint( C, MouseX, MouseY );
}
//***********************************************************************************************
function bool KeyEvent( byte/*EInputKey*/ Key, byte/*EInputAction*/ Action, FLOAT Delta )
{
	//if( _bEndStory )
	//	return true;

//	if(Action==1/*IST_Press*/ && ( Key==0x1b/*IK_Escape*/ || Key==0x20 ) )
//	{
//		// FEBook(book).bGamePlaying = false;
//		FEBook(book).ChangePageNamed("MAIN");
//
//		return true;
//	}

	return false;
}

//***************************************************************************************************************
function ShowWindow()
{
	CreditsTextWindow.Reset();
	Super.ShowWindow ();
}

//***************************************************************************************************************

defaultproperties
{
}

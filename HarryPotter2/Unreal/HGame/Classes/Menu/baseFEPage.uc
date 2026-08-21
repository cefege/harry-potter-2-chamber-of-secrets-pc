class baseFEPage extends UWindowDialogClientWindow;

#EXEC TEXTURE IMPORT NAME=FELeftReturnUpIcon	 file=textures\menu\Arrows\leftreturnarrowup.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FELeftReturnOverIcon	 file=textures\menu\Arrows\leftreturnover.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

var baseFEBook    book;
var UWindowButton BackPageButton;
var texture       textureReturnUp;
var texture       textureReturnOver;

function string GetLocalFEString(string strId)
{
	return (Localize("All", strId, "HPMenu"));
}

function Paint(Canvas canvas,float x,float y)
{

}

	//called by the FEBook.changePage right before the page is displayed.
function PreSwitchPage()
{
    local bool bBackPage;

    // Assume we want "return to previous page"
    bBackPage = true;

    // If there is no previous page or at the In Game page, want
    // "return to game"
    if ((FEBook(book).prevPage == None) ||
        (FEBook(book).curPage == FEBook(book).InGamePage))
    {
        bBackPage = false;
    }

    Harry(Root.Console.viewport.actor).ClientMessage("Preswitch Page " $FEBook(book).prevPage $" " $FEBook(book).curPage);
    SetBackPageToolTip(bBackPage);
}

function PreOpenBook()
{

}

function CreateBackPageButton(optional int nX, optional int nY)
{
    // If now postion passed in, put in lower right corner
    if (nX == 0)
        nX = 582;
    if (nY == 0)
        nY = 422;

	if (textureReturnUp == None)
	{
		textureReturnUp   = texture(DynamicLoadObject("HP_Menu.Hud.MenuBackToGame", class'Texture'));
		textureReturnOver = texture(DynamicLoadObject("HP_Menu.Hud.MenuBackToGameRO", class'Texture'));
	}

	BackPageButton = UWindowButton(CreateControl(class'UWindowButton',
		                           nX,
								   nY,
								   48,
								   48));
	BackPageButton.UpTexture=textureReturnUp;
	BackPageButton.DownTexture=textureReturnOver;
	BackPageButton.OverTexture=textureReturnOver;
	BackPageButton.ToolTipString=GetLocalFEString("Shared_Menu_0002");  // return to previous page
	BackPageButton.Register(self);
}

function SetBackPageToolTip(bool bBackPage)
{
    if (BackPageButton != None)
    {
        if (bBackPage)
            BackPageButton.ToolTipString=GetLocalFEString("Shared_Menu_0002");  // return to previous page
        else
            BackPageButton.ToolTipString=GetLocalFEString("Main_Menu_0010");    // return to game
    }
}

function int GetStatusY()
{
	return (WinHeight - 26);
}

//-----------------------------------------------------------------------------------------------
// Message box code ....
//-----------------------------------------------------------------------------------------------

function HPMessageBox doHPMessageBox(string msg, string textButton1, optional string textButton2, optional float timeOut)
{
	local HPMessageBox w;
	
	w = HPMessageBox(Root.CreateWindow(class'HPMessageBox', (640-246)/2, (480-102)/2, 246, 102, Self));
	w.Setup (msg, textButton1, textButton2, timeOut);

	root.ShowModal(w);

	return w;
}

function WindowEvent(WinMessage Msg, Canvas C, float X, float Y, int Key) 
{
	if(Msg == WM_Paint || !root.WaitModal())
		Super.WindowEvent(Msg, C, X, Y, Key);
}




//***********************************************************************************************
function bool KeyEvent( byte/*EInputKey*/ Key, byte/*EInputAction*/ Action, FLOAT Delta )
{
	return false;
}

defaultproperties
{
}
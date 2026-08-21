//===============================================================================
//  [FEHousepointsPage] 
//
//  The housepoints page is accessed from the In Game menu.  It shows the number
//  of housepoints each house has accumulated.
//
//===============================================================================

class FEHousepointsPage expands baseFEPage;

// A button for each house.  These "buttons" don't respond to clicks, but they're
// buttons anyway so we can tie into the tooltip text.
var UWindowButton GryffButton;
var UWindowButton HuffButton;
var UWindowButton RaveButton;
var UWindowButton SlythButton;

// FEHousepointsPage Created.  This only gets called once per game.
// Load in textures and create buttons.
function Created()
{
    // Base class handles creating "back page" button (multiple pages need it).
    CreateBackPageButton();

    // Create buttons for each house.
	GryffButton=UWindowButton(CreateControl(class'UWindowButton', 154,110,74,95));
	GryffButton.ToolTipString=GetLocalFEString("InGameMenu_0009");
	GryffButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.HousepointsGryff", class'Texture'));;     
	GryffButton.OverTexture=GryffButton.UpTexture; 
    GryffButton.DownTexture=GryffButton.OverTexture;
    GryffButton.DownSound=None;

	HuffButton=UWindowButton(CreateControl(class'UWindowButton', 154,252,74,95));
	HuffButton.ToolTipString=GetLocalFEString("InGameMenu_0010");
	HuffButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.HousepointsHuff", class'Texture'));;     
	HuffButton.OverTexture=HuffButton.UpTexture; 
    HuffButton.DownTexture=HuffButton.OverTexture;
    HuffButton.DownSound=None;

	RaveButton=UWindowButton(CreateControl(class'UWindowButton', 430,110,74,95));
	RaveButton.ToolTipString=GetLocalFEString("InGameMenu_0011");
	RaveButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.HousepointsRav", class'Texture'));;     
	RaveButton.OverTexture=RaveButton.UpTexture; 
    RaveButton.DownTexture=RaveButton.OverTexture;
    RaveButton.DownSound=None;

	SlythButton=UWindowButton(CreateControl(class'UWindowButton', 430,252,74,95));
	SlythButton.ToolTipString=GetLocalFEString("InGameMenu_0012");
	SlythButton.UpTexture=texture(DynamicLoadObject("HP_Menu.Hud.HousepointsSlyth", class'Texture'));;     
	SlythButton.OverTexture=SlythButton.UpTexture; 
    SlythButton.DownTexture=SlythButton.OverTexture;
    SlythButton.DownSound=None;

	super.Created();
}

// Button messages
function Notify(UWindowDialogControl C, byte E)
{
	local int i;

	if(e==DE_Click)
	{
        // If back page button clicked, let FEBook handle it.
        if (C == BackPageButton)
            FEBook(book).DoEscapeFromPage();
	}

}

// AfterPaint handles painting of housepoint counts- doing it in AfterPaint
// makes the text go on top of the button.
function AfterPaint(Canvas canvas,float x,float y)
{
	local float   fScaleFactor;

	fScaleFactor = Canvas.SizeX/WinWidth; 

    PaintCountText(Canvas, fScaleFactor);

    Super.AfterPaint(canvas, x, y);
}

// Paint count on top of each housepoint button.
function PaintCountText(Canvas canvas, float fScaleFactor)
{
    local StatusManager managerStatus;
    local StatusGroup   sg;
    local StatusItem    si;

    // Get status objects and let them draw the count because they already know how.
    managerStatus = Harry(Root.Console.viewport.actor).managerStatus;
    sg = managerStatus.GetStatusGroup(class'StatusGroupHousepoints');

    si = sg.GetStatusItem(class'StatusItemGryffindorPts');
    si.DrawCount(Canvas, GryffButton.WinLeft * fScaleFactor, GryffButton.WinTop * fScaleFactor, fScaleFactor);

    si = sg.GetStatusItem(class'StatusItemHufflepuffPts');
    si.DrawCount(Canvas, HuffButton.WinLeft * fScaleFactor, HuffButton.WinTop * fScaleFactor, fScaleFactor);

    si = sg.GetStatusItem(class'StatusItemRavenclawPts');
    si.DrawCount(Canvas, RaveButton.WinLeft * fScaleFactor, RaveButton.WinTop * fScaleFactor, fScaleFactor);

    si = sg.GetStatusItem(class'StatusItemSlytherinPts');
    si.DrawCount(Canvas, SlythButton.WinLeft * fScaleFactor, SlythButton.WinTop * fScaleFactor, fScaleFactor);
}


defaultproperties
{
}
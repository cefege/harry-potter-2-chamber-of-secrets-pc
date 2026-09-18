//=============================================================================
// ShortCutClientWindow - The Client Window that is attached to ShortCutWindow
//=============================================================================
class ShortCutClientWindow expands UWindowDialogClientWindow;

var ShortCutBrowser	browser;

// called after object has been created...add content
function Created()
{
	local string curToken;
	local int	 i;

	Super.Created();

	// Create and Add our browser
	browser = ShortCutBrowser( CreateWindow( class'ShortCutBrowser',0,0,WinWidth,WinHeight ) );
	
	
	// *** Create our Custom lists and add them to the browser

	// NOTE: If you make a local var of a shortcutList then it will not work correctly
	//		 You need to have the shortcutList local to the browser and not outside of the
	//		 browser. If its local outside of the browser then Unreal will try to update
	//		 the class itself, when the browser should take care of that.
	
	
	// Add bookmark list
	browser.AddList( "Bookmarks", ShortCutListBookmarks( 
		CreateWindow( class'ShortCutListBookmarks', 0,16,WinWidth,WinHeight-12)) );
	
	// Add level list
	browser.AddList( "Levels",	ShortCutListLevels( 
		CreateWindow( class'ShortCutListLevels', 0,16,WinWidth,WinHeight-12)) );

	// Add sounds list
	browser.AddList( "Sounds",	ShortCutListSounds( 
		CreateWindow( class'ShortCutListSounds', 0,16,WinWidth,WinHeight-12)) );
	
	
	// Add all MasterGameState tokens
	i = 0;
	curToken = GetPlayerOwner().GetGameStateMasterListToken( 0 );
	while( curToken != "" )
	{
		browser.AddGameState( curToken );
		curToken = GetPlayerOwner().GetGameStateMasterListToken( ++i );
	}
	
	browser.UpdateCurrentGameStateSelection();
}

function Activated()
{
	if( browser != None )
		browser.Activated();
}

function Resized()
{
	Super.Resized();
	
	// add resize support with our grid	
	browser.WinWidth  = WinWidth;
	browser.WinHeight = WinHeight;
	browser.Resized();
}

function Paint(Canvas canvas,float x,float y)
{
	Super.Paint( canvas, x, y);
	
	browser.Paint( canvas, x, y);
}

/*
function AddList( string strName, ShortCutList newGrid )
{
	browser.AddList( strName, newGrid );
}
*/

/*
function Paint(Canvas canvas,float x,float y)
{
	Super.Paint(canvas, x, y );
	
	canvas.DrawColor.r = 255;
	canvas.DrawColor.g = 255;
	canvas.DrawColor.b = 255;

	LookAndFeel.DrawClientArea(Self, canvas);

//	buttonLaunch.LookAndFeel();
}
*/

//=============================================================================
// ShortCutWindow - The basic window for our short cuts
//=============================================================================
class ShortCutWindow extends UWindowFramedWindow;

function Created()
{
	Super.Created();
}

function BeginPlay()
{
	Super.BeginPlay();
	
	// Set the title of our Framed Window
	WindowTitle = "Shortcut Window";
	
	// The overloaded = operator will create then 
	// attach a ClientWindow to the window frame
	ClientClass = class'ShortCutClientWindow';
	
	
	ToolTip( "short cut tool tip" );
	SetAcceptsFocus();
	
	bTransient		= false;
//	bAlwaysOnTop	= true;
	bUWindowActive  = true;
	bLeaveOnScreen	= true;
	bAcceptsHotKeys	= true;
	
	// Make the Framed Window resizable
	bSizable = true;
/*	
	bBLSizing	= false;
	bBRSizing	= false;
	bBSizing	= false;
	bLSizing	= false;
	bMoving		= false;
	bRSizing	= false;
	bSizable	= false;
	bStatusBar	= false;
	bTLSizing	= false;
	bTRSizing	= false;
	bTSizing	= false;
*/
}

function Activated()
{
	local UWindowWindow Prev, Child;

	for(Child = LastChildWindow;Child != None;Child = Prev)
	{
		Prev = Child.PrevSiblingWindow;
		Child.Activated();
	}

	bUWindowActive = true;
}

function Close( optional bool bByParent )
{
	Super.Close( bByParent );
	bUWindowActive = false;
}
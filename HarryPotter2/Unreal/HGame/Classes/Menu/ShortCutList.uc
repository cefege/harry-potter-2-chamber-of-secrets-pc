//=============================================================================
// ShortCutList - Base class for handling the list of shortcuts
//=============================================================================
class ShortCutList extends UWindowGrid;

var int		SelectedRow;
var int		NumRows;



function Reset()
{
	// this function should reset the list of shortcuts
	// over ride
}

function LaunchShortcut( int row )
{
	// over ride
}

function OnLaunchButton()
{
	LaunchShortcut( SelectedRow );
}

// *****************************************
// ******** UWindowGrid functions **********
// *****************************************

function Created()
{
	Super.Created();
	
	RowHeight	 = 12;
	bShowHorizSB = true;
	bAlwaysOnTop = true;
}

function Paint(Canvas canvas, float x, float y)
{
	// over ride
}

function PaintColumn(Canvas C, UWindowGridColumn Column, float MouseX, float MouseY) 
{
	// over ride
}

function DoubleClickRow( int row )
{
	// launch shortcut
	LaunchShortcut( SelectedRow );
}

function SelectRow( int row )
{
	local int curRow;
	curRow = VertSB.Pos + row;
	if( row != SelectedRow && curRow < NumRows )
	{
		SelectedRow = curRow;
	}
}

function RightClickRow(int Row, float X, float Y)
{

}

function SortColumn(UWindowGridColumn Column) 
{
	// should we allow sorting by column?
}

/*
function MouseMove(float X, float Y)
{

	// **** DEBUG ****
	// **** WE NEVER GET HERE?!?!? ****

	// detect when the mouse button is down
	if( bMouseDown )
	{
		HPConsole(root.console).Viewport.Actor.ClientMessage( "mmove "$X $" "$Y );

		SelectedRow = (VertSB.Pos);// + (Y / RowHeight)  );	
	}

	Super.MouseMove( X, Y );
}
*/

defaultproperties
{

}

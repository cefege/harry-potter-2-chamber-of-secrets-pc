//=============================================================================
// ShortCutListLevels - list of bookmarks
//=============================================================================
class ShortCutListLevels extends ShortCutList;


// custom
var	array<int>		lableNumbers;
var array<string>	lableMapNames;



// *****************************************
// ******** ShortCutList functions *********
// *****************************************

function Reset()
{
	local string FirstMap, NextMap, TestMap;
	local int i;

	// Add the correct mapName to our lavleMapName array

	FirstMap = GetPlayerOwner().GetMapName("","", 0);
	NextMap  = FirstMap;
	i=0;
	while( !(FirstMap ~= TestMap) )
	{
		lableNumbers[i]	= i;
		lableMapNames[i] = NextMap;
		NextMap = GetPlayerOwner().GetMapName("", NextMap, 1);
		TestMap = NextMap;
		++i;
	}

	NumRows = i;
}

function LaunchShortcut( int row )
{
	// Through some crazy indirection get the ChangeLevel function and load our level
	baseConsole(root.console).ChangeLevel( lableMapNames[row], true );
}


// *****************************************
// ******** UWindowGrid functions **********
// *****************************************

function Created()
{
	Super.Created();
	
	AddColumn(" Level", 28);
	AddColumn(" MapName", 512);
	
	NumRows = 0;
	
	Reset();
}

function PaintColumn(Canvas C, UWindowGridColumn Column, float MouseX, float MouseY) 
{
	local int TopMargin, BottomMargin;
	local int NumRowsVisible;
	local int CurRow;
	local int CurOffset;
	local int LastRow;


	// Get Bottom and Top Margin
	if(bShowHorizSB) 
		BottomMargin = LookAndFeel.Size_ScrollbarWidth;
	else 
		BottomMargin = 0;

	TopMargin = LookAndFeel.ColumnHeadingHeight;
	
	// Calculate the number of rows visible
	NumRowsVisible	= (WinHeight - (TopMargin + BottomMargin)) / RowHeight;

	// Set the range for the Vertical Scrollbar
	VertSB.SetRange(0, NumRows, NumRowsVisible);
	
	// Setup our Current Row and LastRow indicies
	CurRow		= VertSB.Pos;
	LastRow		= CurRow + NumRowsVisible;
	CurOffset	= 0;
	
	if(LastRow > NumRows)
		LastRow = NumRows;

	C.DrawColor.g = 255;	
	
	while( CurRow < LastRow )
	{
		// If this is our selected row then color it green.
		if( CurRow == SelectedRow )
		{
			C.DrawColor.r = 0;
			C.DrawColor.b = 0;
		}
		else
		{
			C.DrawColor.r = 255;	
			C.DrawColor.b = 255;
		}
		
		// Clip the text according to the column we are drawing
		switch( Column.ColumnNum )
		{
			case 0:	Column.ClipText( C, 2, TopMargin + CurOffset, lableNumbers[CurRow] );	break;
			case 1:	Column.ClipText( C, 2, TopMargin + CurOffset, lableMapNames[CurRow] );	break;
		}
		
		// Inc our current offset and row
		CurOffset += RowHeight;
		++CurRow;
	}
	
}



defaultproperties
{

}

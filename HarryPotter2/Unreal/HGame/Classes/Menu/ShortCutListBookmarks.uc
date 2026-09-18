//=============================================================================
// ShortCutListBookmarks - list of bookmarks
//=============================================================================
class ShortCutListBookmarks extends ShortCutList;

var array<Name>		lableName;
var array<string>	lableDesc;


function Created()
{
	Super.Created();
	
	RowHeight	 = 12;
	bShowHorizSB = true;
		
	AddColumn(" Index",28);
	AddColumn(" Name", 64);
	AddColumn(" Description", 512);
	
	Reset();
}

// *****************************************
// ******** ShortCutList functions *********
// *****************************************

function Reset()
{
	local navShortcut	sc;
	
	// Go through all the bookmarks that our player
	// is in and record the name, tag and description.
	NumRows = 0;
	
	foreach GetPlayerOwner().AllActors( class'navShortcut', sc )
	{
		lableName[NumRows]	= sc.Name;
		lableDesc[NumRows]	= sc.Description;
		NumRows++;
	}

	// If we didn't find any shortcuts let the user know
	if( NumRows == 0)
	{
		NumRows = 1;
		lableName[0]	= '0';
		lableDesc[0]	= "Could not find any bookmarks!";
	}
}

function LaunchShortcut( int row )
{
	local navShortcut	sc;
	local int			i;
	
	i = 0;
	foreach GetPlayerOwner().AllActors(class'navShortcut', sc)
	{
		// Because the order that we put the navShortcuts in our list is always the same
		if( i == row )
		{
			Harry(GetPlayerOwner()).GotoLocation( sc.location );
		}
		i++;
	}
}

// *****************************************
// ******** UWindowGrid functions **********
// *****************************************


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

	if(LastRow > NumRows)
		LastRow = NumRows;

	CurOffset	= 0;

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
			case 0:	Column.ClipText( C, 2, TopMargin + CurOffset, CurRow );				break;
			case 1:	Column.ClipText( C, 2, TopMargin + CurOffset, lableName[CurRow] );	break;
			case 2:	Column.ClipText( C, 2, TopMargin + CurOffset, lableDesc[CurRow] );	break;
		}
		
		// Inc our current offset and row
		CurOffset += RowHeight;
		++CurRow;
	}
	
}


function SortColumn(UWindowGridColumn Column) 
{
	// should we allow sorting by column?
	HPConsole(root.console).Viewport.Actor.ClientMessage( "sort column "$Column.ColumnNum );
//	UBrowserInfoClientWindow(GetParent(class'UBrowserInfoClientWindow')).Server.PlayerList.SortByColumn(Column.ColumnNum);
}


defaultproperties
{

}

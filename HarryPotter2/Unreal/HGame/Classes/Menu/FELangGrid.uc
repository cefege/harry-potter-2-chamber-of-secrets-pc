class FELangGrid extends UWindowGrid;

var UWindowGridColumn ngSecretColumn;
var FESoundBrowser browser;
var int SelectedRow;

function Created() 
{
	Super.Created();

	RowHeight = 12;

	AddColumn("Status", 50);
	AddColumn("Name", 200);
	AddColumn("Text", 380);
}

function PaintColumn(Canvas C, UWindowGridColumn Column, float MouseX, float MouseY) 
{
	local int Visible;
	local int Count;
	local int Skipped;
	local int Y;
	local int TopMargin;
	local int BottomMargin;

	local int index;
	local color colorCutTextBlue;

	if(bShowHorizSB)
		BottomMargin = LookAndFeel.Size_ScrollbarWidth;
	else
		BottomMargin = 0;

	TopMargin = LookAndFeel.ColumnHeadingHeight;

	Count = browser.MasterCount;

	C.Font = Root.Fonts[F_Normal];
	Visible = int((WinHeight - (TopMargin + BottomMargin))/RowHeight);
	
	VertSB.SetRange(0, Count+1, Visible);
	TopRow = VertSB.Pos;

	Skipped = 0;

	Y = 1;
	index=0;

//	C.DrawTile(Texture'HGame.Icons.leftPanel',c.SizeX, c.SizeY, 0, 0, 1, 1);

/*	colorCutTextBlue.r=127;
	colorCutTextBlue.G=127;
	colorCutTextBlue.B=255;

	HPHud(harry(root.console.viewport.Actor).myHud).DrawCutStyleText(c,
		                                                 browser.MasterText[SelectedRow],
                                                         0,
		                                                 c.SizeY - (c.SizeY/8) + 1,
		                                                 (c.SizeY/8),
													     colorCutTextBlue);
*/
	while((Y < RowHeight + WinHeight - RowHeight - (TopMargin + BottomMargin)) && (index<browser.MasterCount))
	{
		if(Skipped >= VertSB.Pos)
		{
			if(index==SelectedRow)
				{
				C.DrawColor.r=255;
				C.DrawColor.g=5;
				C.DrawColor.b=5;
				}
			else
				{
				C.DrawColor.r=255;
				C.DrawColor.g=255;
				C.DrawColor.b=255;
				}


			switch(Column.ColumnNum)
			{
			case 0:
				Column.ClipText( C, 2, Y + TopMargin, browser.status[index] );
				break;
			case 1:
				Column.ClipText( C, 2, Y + TopMargin, browser.MasterList[index] );
				break;
			case 2:
				Column.ClipText( C, 2, Y + TopMargin, browser.MasterText[index]);
				break;
			}

			Y = Y + RowHeight;			
		} 
		Skipped ++;
		index++;
	}
}

function RightClickRow(int Row, float X, float Y)
{
}

function SortColumn(UWindowGridColumn Column) 
{
}

function SelectRow(int Row) 
{
local sound snd;
	if(row!=SelectedRow)
		{
		SelectedRow=Row;
//HPHud(harry(root.console.viewport.Actor).myHud)
		HPHud(harry(root.console.viewport.Actor).myHud).SetSubtitleText(browser.MasterText[row], 360);
		snd=Sound( DynamicLoadObject("AllDialog."$browser.MasterList[row], class'Sound') );
		
		harry(root.console.viewport.Actor).playsound(snd);

//		root.console.viewport.Actor.DeliverLocalizedDialog(browser.MasterList[row],true,360,"HPDialog");
		}
}

defaultproperties
{
}
class baseSpellPage expands basePopup;

var Texture pagePieces[4];
var int yOffset;


function Draw(Canvas canvas)
{
local int width;

	width=pagePieces[0].USize+pagePieces[1].USize;

	Canvas.SetPos((canvas.sizeX/2)-(width/2),canvas.sizeY-yOffset);
	Canvas.DrawIcon(pagePieces[0],1);

	Canvas.SetPos(((canvas.sizeX/2)-(width/2))+256,canvas.sizeY-yOffset);
	Canvas.DrawIcon(pagePieces[1],1);

	if(pagePieces[2]!=None)
		{
		Canvas.SetPos((canvas.sizeX/2)-(width/2),(canvas.sizeY-yOffset)+256);
		Canvas.DrawIcon(pagePieces[2],1);
		}

	if(pagePieces[3]!=None)
		{
		Canvas.SetPos(((canvas.sizeX/2)-(width/2))+256,(canvas.sizeY-yOffset)+256);
		Canvas.DrawIcon(pagePieces[3],1);
		}

}
defaultproperties
{
	yOffset=440;
}
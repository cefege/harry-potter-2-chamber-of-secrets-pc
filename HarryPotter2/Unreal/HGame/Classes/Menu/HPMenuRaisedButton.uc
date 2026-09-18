class HPMenuRaisedButton extends UWindowButton;

var int textOffsetX;

function Created()
{
	Super.Created();
}

function BeforePaint(Canvas C, float X, float Y)
{
	local float W, H;

	Super.BeforePaint(C, X, Y);

	WinHeight = 24;

	TextSize(C, Text, W, H);

	TextY = (WinHeight - H) / 2;

	switch(Align)
	{
	case TA_Left:
		TextX = textOffsetX;
		break;
	case TA_Right:
		TextX = WinWidth - W - textOffsetX;
		break;
	case TA_Center:	
		TextX = (WinWidth - W) / 2;
		break;
	}
}

function Paint(Canvas C, float X, float Y)
{
	Super.Paint(C, X, Y);
}

defaultproperties
{
	textOffsetX=11
}

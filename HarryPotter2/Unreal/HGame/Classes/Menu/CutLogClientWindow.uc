class CutLogClientWindow extends UWindowDialogClientWindow;

var UWindowConsoleTextAreaControl TextArea;

function Created()
{
	TextArea = UWindowConsoleTextAreaControl(CreateWindow(class'UWindowConsoleTextAreaControl', 0, 0, WinWidth, WinHeight));
}

function Notify(UWindowDialogControl C, byte E)
{
	local string s;
	Super.Notify(C, E);

	switch(E)
	{
	case DE_EnterPressed:
		break;
	case DE_WheelUpPressed:
		TextArea.VertSB.Scroll(-1);
		break;
	case DE_WheelDownPressed:
		TextArea.VertSB.Scroll(1);
		break;
	}
}

function BeforePaint(Canvas C, float X, float Y)
{
	Super.BeforePaint(C, X, Y);

	TextArea.SetSize(WinWidth, WinHeight);
}

function Paint(Canvas C, float X, float Y)
{
	DrawStretchedTexture(C, 0, 0, WinWidth, WinHeight, Texture'BlackTexture');
}

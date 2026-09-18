class CutLogWindow extends UWindowFramedWindow;

function Created() 
{
//log(self $" Created***");

	Super.Created();
	bSizable = false;
	bStatusBar = false;
	bLeaveOnScreen = True;
}
defaultproperties
{
	WindowTitle="CutScene Log";
	ClientClass=class'CutLogClientWindow'
}
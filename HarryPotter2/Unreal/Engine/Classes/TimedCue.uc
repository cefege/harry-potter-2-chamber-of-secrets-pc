class TimedCue extends Actor;

var bool bAlreadyDestroyed;
function Timer()
{
	if(bAlreadyDestroyed)
		return;

	if(CutNotifyActor!=None && sCutNotifyCue !="")
		CutNotifyActor.CutCue(sCutNotifyCue);

	bAlreadyDestroyed=true;
	destroy();
}

function SetupTimer(float delay,string cue)
{
	SetTimer(delay,false);
	sCutNotifyCue=cue;
}

defaultproperties
{
	bHidden=true;
	bCollideActors=false;
	bCollideWorld=false;
	bBlockActors=false;
	bBlockPlayers=false;
}
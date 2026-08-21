class PopupTrigger expands Trigger;

var (Popup) string dialogId;  
var (Popup) bool bPlayDialogSound;
var (Popup) float fPopupDuration;	//not applicable if bPlayDialogSound.

var (Popup) bool bDoNothingIfHarryCaptured;	//set if you want popup to not play while harry is captured.

function Trigger( actor Other, pawn EventInstigator )
{
	Activate(other,EventInstigator);
}


function Activate( actor Other, pawn Instigator )
{
	if(bDoNothingIfHarryCaptured && harry(level.playerHarryActor).bIsCaptured)
		return;

	if(dialogId!="")
		DeliverLocalizedDialog(dialogId,bPlayDialogSound,fPopupDuration);
}

defaultproperties
{
	bHidden=true
	bTriggerOnceOnly=true
	TriggerType=TT_PlayerProximity

	bPlayDialogSound=true;
	fPopupDuration=0.0;

	bDoNothingIfHarryCaptured=true;

}

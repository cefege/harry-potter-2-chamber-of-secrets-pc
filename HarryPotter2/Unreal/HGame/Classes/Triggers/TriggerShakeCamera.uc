
class TriggerShakeCamera extends Triggers;

var() float fShakeTime;
var() float fRollMagnitude;
var() float fVertMagnitude;

//*******************************************************************************


//*******************************************************************************
event Trigger( Actor Other, Pawn EventInstigator )
{
	ProcessTrigger();
}

//*******************************************************************************
function ProcessTrigger()
{
	Harry(level.playerharryactor).ShakeView( fShakeTime, fRollMagnitude, fVertMagnitude );
}

defaultproperties
{
	fShakeTime=2.0f
	fRollMagnitude=100
	fVertMagnitude=100
}

class DuelTrigger extends trigger;

var   Harry		playerHarry;
var   Duellist  duelOpponent;

//*******************************************************************************
function PostBeginPlay()
{
	ForEach AllActors(class'Harry', playerHarry)
		break;

	ForEach AllActors(class'Duellist', duelOpponent)
		break;
}

//*******************************************************************************
function TriggerEvent( Name EventName, Actor Other, Pawn EventInstigator )
{
	if( playerHarry == none )
		return;

	if( duelOpponent == none )
		return;

	playerHarry.TurnOnDuelingMode(duelOpponent);
	duelOpponent.gotostate('stateStartDuel');
//	gotostate('stateAuto');
}

state stateAuto
{
	begin:
		gotostate('NormalTrigger');
}

//*****************************************************************************
defaultproperties
{
	bSendEventOnEvent=true
}
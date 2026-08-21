
class DisableTrigger extends trigger;

//**************************************************************************
//
//	Will send a TriggerDisable to all actors with a matching event
//	This is a work in progress. It will not disable everything since a disable
//	needs to be done on a case by case basis. The TriggerDisable() function needs
//	to be added for the individual class with the specific disable functionality.
//
//**************************************************************************

//*******************************************************************************
event Trigger( Actor Other, Pawn EventInstigator )
{
	ProcessTrigger(other);
}

function touch(actor other)
{
	if ( other == Level.PlayerHarryActor )
	{
		ProcessTrigger(other);
	}

}

//*******************************************************************************
function ProcessTrigger(actor other)
{
	local Actor   sp;

	//Find Disable
	if( Event != 'None' )
	{
		foreach AllActors( class'Actor', sp, Event )
			break;
	}



	if( sp == none )
	{
		Log("DisableTrigger: Couldn't find Disable Tag");
		return;
	}
	else
	{
//		Level.PlayerHarryActor.ClientMessage("sending triggerdisable to " $sp);
 		sp.TriggerDisable();
	}


}


//*****************************************************************************
defaultproperties
{

	bTriggerOnceOnly=True

}
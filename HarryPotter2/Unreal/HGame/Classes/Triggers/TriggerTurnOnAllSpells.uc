
class TriggerTurnOnAllSpells extends Triggers;


//*******************************************************************************
auto state Waiting
{
	//*******************************************************************************
	event Trigger( Actor Other, Pawn EventInstigator )
	{
		ProcessTrigger();
	}

	function touch(actor other)
	{
		super.touch(other);
		if( other == level.playerharryactor )
			ProcessTrigger();
	}
}

//*******************************************************************************
function ProcessTrigger()
{
	Harry(level.playerHarryActor).ClientMessage( "<*> Turning on ALL spells for this level!! <*>" );

	// this will reset when Harry calls PreBeginPlay() (when starting a new level)
	Harry(level.playerHarryActor).bNoSpellBookCheck = true;
}

defaultproperties
{
}
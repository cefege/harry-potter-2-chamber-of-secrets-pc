
class TriggerChangeLevel extends trigger;

var() string     NewMapName;


auto state Waiting
{
	//*******************************************************************************
	event Trigger( Actor Other, Pawn EventInstigator )
	{
		ProcessTrigger();

		level.playerharryactor.clientmessage(self $" Here1");
	}

	function touch(actor other)
	{
		super.touch(other);
		if(other==level.playerharryactor)
			ProcessTrigger();
	}


}
//*******************************************************************************
function ProcessTrigger()
{
	local Characters	A;
	local Harry			playerHarry;

	playerHarry = Harry(Level.playerHarryActor);
	if( playerHarry == none )
	{
		Log("TriggerChangeLevel: Couldn't find Harry, and that ain't right!");
		return;
	}
	

	// Go through all HPawns and save their current state and animation so we can 
	// go back to this state and animation sequence later.
	//
	// Ideally the engine would save the "FrameState" of the actor. Which would include
	// the current state, animation and line of script code the actor is in.
	// BUT saving the current frameState is not easy with the unreal engine, and we don't
	// have the enough time to rewrite the save/load package behavior.
	
	// I've acomplished saving and loading the properties of a class so we will use
	// that to get back to our desired state and animation.
	foreach AllActors( class'Characters', A )
	{
		if( A.bPersistent )
		{
			A.PersistentState		 = A.GetStateName();
			A.PersistentLeadingActor = A.LeadingActor.Name;
			
			log("*!* " $A $" P_SAVING: PersistentState: " $A.PersistentState $" for " $A );
			log("*!* " $A $" P_SAVING: LeadingActor: " $A.PersistentLeadingActor
					   $" AnimSequence: "	$A.AnimSequence );
		}
	}
	
	// --- Save our persistent actors to a persistent Actor file
	baseConsole(playerHarry.player.console).ConsoleCommand("SavePActors");
	
	// --- Change our level
	baseConsole(playerHarry.player.console).ChangeLevel( NewMapName, true );

	if( InStr(caps(NewMapName),"STARTUP")>-1 )
	{	//yup so bypass the menus
		hpConsole(playerHarry.player.console).MenuBook.bGamePlaying=false;
		hpConsole(playerHarry.player.console).MenuBook.OpenBook("Main");
		hpConsole(playerHarry.player.console).LaunchUWindow();
	}


//	a.Level.ServerTravel( NewMapName, true );
}


//*****************************************************************************
defaultproperties
{
	InitialState=None;

}
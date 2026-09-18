class hideTrigger extends trigger;

var (trigger) bool bUnHide;

event Trigger( Actor Other, Pawn EventInstigator )
{
local actor A;

	if ( ReTriggerDelay > 0 )
		{
		if ( Level.TimeSeconds - TriggerTime < ReTriggerDelay )
			return;
		TriggerTime = Level.TimeSeconds;
		}
	// Broadcast the Trigger message to all matching actors.
	if( Event != '' )
		foreach AllActors( class 'Actor', A, Event )
		{
			A.bHidden=!bUnHide;
			//A.bBlockPlayers=!A.bHidden;
			A.SetCollision( , , !A.bHidden );
		}

	if ( Other.IsA('Pawn') && (Pawn(Other).SpecialGoal == self) )
		Pawn(Other).SpecialGoal = None;
				
	if( Message != "" )
		// Send a string message to the toucher.
		Other.Instigator.ClientMessage( Message );

	if( bTriggerOnceOnly )
		// Ignore future touches.
		SetCollision(False);
	else if ( RepeatTriggerTime > 0 )
		SetTimer(RepeatTriggerTime, false);
	
}
function Touch( actor Other )
{
	local actor A;
	if( IsRelevant( Other ) )
	{
		if ( ReTriggerDelay > 0 )
		{
			if ( Level.TimeSeconds - TriggerTime < ReTriggerDelay )
				return;
			TriggerTime = Level.TimeSeconds;
		}
		// Broadcast the Trigger message to all matching actors.
		if( Event != '' )
			foreach AllActors( class 'Actor', A, Event )
				{
				A.bHidden=!bUnHide;
				//A.bBlockPlayers=!A.bHidden;
				//Only change bBlockPlayers
				A.SetCollision( , , !A.bHidden );
				}
		if ( Other.IsA('Pawn') && (Pawn(Other).SpecialGoal == self) )
			Pawn(Other).SpecialGoal = None;
				
		if( Message != "" )
			// Send a string message to the toucher.
			Other.Instigator.ClientMessage( Message );

		if( bTriggerOnceOnly )
			// Ignore future touches.
			SetCollision(False);
		else if ( RepeatTriggerTime > 0 )
			SetTimer(RepeatTriggerTime, false);
	}
}
defaultproperties
{
	bUnHide=false;
//	bCollideActors=false;
//	bProjTarget=false;
}

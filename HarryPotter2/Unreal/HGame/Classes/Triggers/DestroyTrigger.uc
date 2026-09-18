//=============================================================================
// DestroyTrigger.
//=============================================================================

class DestroyTrigger expands Trigger;

var() name Events[8];
var() bool bActivatedByTrigger;

event Trigger( Actor Other, Pawn EventInstigator )
{
	local actor A;
	local int   i;

	if( bActivatedByTrigger )
		for (i=0; i<8; i++)
			if (Events[i] != 'None')
				foreach AllActors( class 'Actor', A, Events[i] )
					A.Destroy();

}

function PassThru(Actor Other)
{
	local actor A;
	local int i;

	// Broadcast the Trigger message to all matching actors.

	for (i=0; i<8; i++)
	{
		if (Events[i] != 'None')
			foreach AllActors( class 'Actor', A, Events[i] )
				A.Destroy();
	}

	if ( Message != "" )
		Other.Instigator.ClientMessage( Message );

	Destroy();
}

function Touch( actor Other )
{
	local actor A;
	local int i;

	if( IsRelevant( Other ) )
	{
		// Broadcast the Trigger message to all matching actors.

		for (i=0; i<8; i++)
		{
			if (Events[i] != 'None')
				foreach AllActors( class 'Actor', A, Events[i] )
					A.Destroy();
		}

		Destroy();
	}
}

defaultproperties
{
}

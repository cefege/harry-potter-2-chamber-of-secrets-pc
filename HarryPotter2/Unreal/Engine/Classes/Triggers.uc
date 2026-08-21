//=============================================================================
// Event.
//=============================================================================
class Triggers extends Actor
	native;

//************************************************************************************************************
// This function will be called by the engine when it is time to resolve what this actor does
// if it is inside the current GameState or not.
event OnResolveGameState()
{
	// If we are not in the current gamestate then become hidden and have no collision
	if( !bInCurrentGameState )
	{
		// We are NOT in the current state
		bHidden = true;
		SetCollision(false,false,false);
		Disable('Trigger');
	}
}

defaultproperties
{
     bHidden=True
     CollisionRadius=+00040.000000
     CollisionHeight=+00040.000000
     bCollideActors=True
}

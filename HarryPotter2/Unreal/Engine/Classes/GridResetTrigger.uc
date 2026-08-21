//=============================================================================
// GridResetTrigger: Moves all GridMovers back to start.
//=============================================================================
class GridResetTrigger extends Trigger;

function Activate( actor Other, pawn Instigator )
{
	local GridMover M;

	foreach AllActors( class'GridMover', M )
		M.InstantReset();
}


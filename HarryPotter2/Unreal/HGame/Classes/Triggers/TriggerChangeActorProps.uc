
class TriggerChangeActorProps extends trigger;

//Sorry about this:  For bool vars, if int is zero, do nothing, if -1 false, if 1 true;
var() int  iNewColActors;
var() int  iNewBlockActors;
var() int  iNewBlockPlayers;


//*******************************************************************************
//function PostBeginPlay()
//{
//	ForEach AllActors(class'baseHarry', playerHarry)
//		break;
//}

//*******************************************************************************
event Trigger( Actor Other, Pawn EventInstigator )
{
	ProcessTrigger();
}

function touch(actor other)
{
	super.touch(other);

	ProcessTrigger();
}

//*******************************************************************************
function ProcessTrigger()
{
	local actor a;
	local bool bNewColActors, bNewBlockActors, bNewBlockPlayers;

	ForEach AllActors(class'actor', a, event)
	{
		if( iNewColActors!=0 || iNewBlockActors!=0 || iNewBlockPlayers!=0 )
		{
			if     ( iNewColActors ==  1 )      bNewColActors = true;
			else if( iNewColActors == -1 )      bNewColActors = false;
			else                                bNewColActors = a.bCollideActors;

			if     ( iNewBlockActors ==  1 )    bNewBlockActors = true;
			else if( iNewBlockActors == -1 )    bNewBlockActors = false;
			else                                bNewBlockActors = a.bBlockActors;

			if     ( iNewBlockPlayers ==  1 )   bNewBlockPlayers = true;
			else if( iNewBlockPlayers == -1 )   bNewBlockPlayers = false;
			else                                bNewBlockPlayers = a.bBlockPlayers;

			a.SetCollision( bNewColActors, bNewBlockActors, bNewBlockPlayers );
		}
	}

}


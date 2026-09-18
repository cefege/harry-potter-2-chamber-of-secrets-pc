


//=============================================================================
// starsTrigger.
//=============================================================================
class StarsTrigger expands Trigger;

var(StarsTrigger) name loserTrigger;
var(StarsTrigger) name avgTrigger;
var(StarsTrigger) name winnerTrigger;
var(StarsTrigger) int avgStarCount;
var(StarsTrigger) int winnerStarCount;
var bool alreadytriggered;

/*
function Touch( actor Other )
{
	super.touch(other);
	if (alreadytriggered==true)
	{
		return;
	}
	alreadytriggered=true;
	if( Harry(Other) != none )
	{
		if(Harry(other).numStars>=winnerStarCount)
		{
			Harry(other).numStars=0;
			TriggerEvent(winnerTrigger, self,Pawn(other));
			Harry(other).addhousepoints(20);
			return;
		}
		else
		{
			if(Harry(other).numStars>=avgStarCount)
			{
				Harry(other).numStars=0;
				Harry(other).clientmessage("called avgTrigger "$avgTrigger);
				TriggerEvent(avgTrigger, self, pawn(other));
				Harry(other).addhousepoints(10);
				return;
			}
			else
			{
				Harry(other).numStars=0;
				TriggerEvent(loserTrigger, self, pawn(other));
				Harry(other).addhousepoints(5);
				return;
			}
		}
	Harry(other).numStars=0;
	}
}
*/


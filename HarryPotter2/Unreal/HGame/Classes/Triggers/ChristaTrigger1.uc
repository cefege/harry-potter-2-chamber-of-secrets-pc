class ChristaTrigger1 expands trigger;

var()  name  MatchingMoversObjectName;

function bool IsRelevant( actor Other )
{
	if( Harry(Other) != None || Other.Name == MatchingMoversObjectName )
		return true;
	else
		return false;
}

function Activate( actor Other, pawn Instigator )
{
	local CutScene   a;

cm("***** Other.name="$Other.name);

	if( Other.name == MatchingMoversObjectName )
	{
		ForEach AllActors( class'CutScene', a, event )
		{
cm("***** Found cutscene="$a.name);
			a.bPlayOnce = true;
			a.nPlayedCount = 0;
		}
	}

	super.Activate( Other, Instigator );
}

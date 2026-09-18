//=============================================================================
// SpellChallengeTrigger 
//
// Used to start the spell challenge countdown.  This class may be temporary.  
// Eventually, a cutscene will probably directly setup the countdown.
//=============================================================================

class SpellChallengeTrigger extends Trigger;

// Vars passed to ChallengeScoreManager.BeginChallenge()
var(SpellChallenge) int	nChallengeId;
var(SpellChallenge) int	nMaxHousepoints;
var(SpellChallenge) int	nMaxScore;

// On activation of trigger, start the challenge.
function Activate( actor Other, pawn Instigator )
{
	local Harry playerHarry;
	local ChallengeScoreManager managerChallenge;


	foreach AllActors( class'Harry', playerHarry )
	{
		if( playerHarry.bIsPlayer && playerHarry != Self )
			break;
	}

	foreach AllActors(class'ChallengeScoreManager', managerChallenge )
		break;

	managerChallenge.BeginChallenge();
}

defaultproperties
{
	bHidden=false
	bTriggerOnceOnly=true
	TriggerType=TT_PlayerProximity

}


class JumpLineTrigger extends Trigger;

var() bool  bEnabled;

var() float MinCosTrigger;		// Min facing match needed to trigger jump.
// Examples:	-1: facing behind trigger
//				0: sideways
//				1: facing toward trigger

var() float ForceJumpForward;	// Fraction to force jump direction forward.

var() float fHorizSpeedMultiplier;

static function vector GetFacing( actor A )
{
	return vec(1,0,0) >> A.Rotation;
}

function MakeJump( actor Actor )
{
	local vector dir;
	dir = Normal(Actor.Velocity) * (1-ForceJumpForward)
		+ GetFacing(self) * ForceJumpForward;
	Actor.Velocity = Pawn(Actor).GroundSpeed * Normal(dir) * fHorizSpeedMultiplier;
	PlayerPawn(Actor).Jump();
}

function Tick( float T )
{
	local int i;
	local vector facing, other_facing;

	if( !bEnabled )
		return;

	for (i=0; i<ArrayCount(Touching); i++)
	{
		facing = GetFacing(self);
		if (PlayerPawn(Touching[i]) != none)
		{
			// Trigger when the object crosses the line.
			if ((Touching[i].Location - Location) dot facing >= 0.0
			&&  (Touching[i].OldLocation - Location) dot facing < 0.0)
			{
				// Force velocity to max in jump direction.
				other_facing = GetFacing(Touching[i]);
				if (other_facing dot facing >= MinCosTrigger)
					MakeJump(Touching[i]);
			}
		}
	}
}

defaultproperties
{
	bEnabled=false

	bDirectional=true
	TriggerType=TT_PlayerProximity
	MinCosTrigger=0
	ForceJumpForward=1
	fHorizSpeedMultiplier=1
}

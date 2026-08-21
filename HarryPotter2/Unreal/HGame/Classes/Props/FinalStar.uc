//===============================================================================
//  FinalStar
//
//  Final star is used in spell challenge levels.  Picking up the final star
//  triggers the start of the score tally process (tallying challenge score
//  into house points).
//
//===============================================================================

class FinalStar extends HProp;

state PickupProp
{
	function EndState()
	{
		local ChallengeScoreManager managerChallenge;

		foreach AllActors(class'ChallengeScoreManager', managerChallenge )
			break;

		// Let ChallengeManager know our challenge has ended.
		managerChallenge.PickedUpFinalStar();

		// Send out a trigger.  Can use this to trigger a cutscene when
		// the final star is picked up.
		TriggerEvent( Event, none, none );
	}
}

defaultproperties
{
     soundPickup=Sound'HPSounds.Magic_sfx.pickup_star'
     bPickupOnTouch=True
     attachedParticleClass(0)=Class'HPParticle.GoldstarFinal'
     attachedParticleOffset(0)=(Z=40)
     Physics=PHYS_Rotating
     Mesh=SkeletalMesh'HProps.skChallengeStarFinalMesh'
     AmbientGlow=125
     CollisionRadius=48
     CollisionWidth=20
     CollisionHeight=64
     bBlockActors=False
     bBlockPlayers=False
     bFixedRotationDir=True
     bRotateToDesired=False
     RotationRate=(Pitch=0,Yaw=20000,Roll=0)
}

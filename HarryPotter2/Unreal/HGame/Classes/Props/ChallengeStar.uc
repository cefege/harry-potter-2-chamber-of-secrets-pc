//===============================================================================
//  [ChallengeStar] 
//===============================================================================

class ChallengeStar extends HProp;

state PickupProp
{
	function EndState()
	{
		local ChallengeScoreManager managerChallenge;

		foreach AllActors(class'ChallengeScoreManager', managerChallenge )
			break;

		managerChallenge.PickedUpStar();
	}
}

defaultproperties
{
     soundPickup=Sound'HPSounds.Magic_sfx.pickup_star'
     soundPickup2=Sound'HPSounds.Magic_sfx.pickup_star1'
     bPickupOnTouch=True
     PickupFlyTo=FT_HudPosition
     classStatusGroup=Class'HGame.StatusGroupStars'
     classStatusItem=Class'HGame.StatusItemStars'
     attachedParticleClass(0)=Class'HPParticle.Goldstar01'
     Physics=PHYS_Rotating
     Mesh=SkeletalMesh'HProps.skChallengeStarMesh'
     DrawScale=1.25
     AmbientGlow=200
     CollisionRadius=10
     CollisionHeight=16
     bBlockActors=False
     bBlockPlayers=False
     bFixedRotationDir=True
     bRotateToDesired=False
     RotationRate=(Pitch=0,Yaw=20000,Roll=0)
	 
	 bBlockCamera=False
}

class FireflyActor extends Hchar;

defaultproperties
{
     attachedParticleClass(0)=Class'HPParticle.Firefly2'
     SplineSpeed=10
     bHidden=True
     Physics=PHYS_Flying
     InitialState=patrolFollowSpline
     DrawType=DT_Sprite
     Mesh=None
     AmbientGlow=65
     CollisionRadius=15
     CollisionHeight=15
     bCollideActors=False
     bBlockActors=False
     bBlockPlayers=False
}

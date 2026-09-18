class FireballOnHarry extends Fireball;


defaultproperties
{
	 drawType=DT_NONE
     attachedParticleClass(0)=Class'HPParticle.Crabfire2'
     attachedParticleClass(1)=Class'HPParticle.CrabSmoke'
     attachedParticleOffset(0)=(Z=-32)
     CollisionRadius=10
     CollisionHeight=22
     bCollideActors=True
     bCollideWorld=True
	 bTouch=True
	 fLifeTime=0.3;
}

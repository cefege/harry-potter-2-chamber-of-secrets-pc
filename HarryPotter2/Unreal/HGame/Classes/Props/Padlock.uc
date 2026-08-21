//===============================================================================
//  [Padlock] 
//===============================================================================

class Padlock extends HAlohomora;

defaultproperties
{
     attachedParticleClass(0)=Class'HPParticle.Lock'
     Mesh=SkeletalMesh'HProps.skPadlockMesh'
     DrawScale=1.2
     AmbientGlow=120
     CollisionRadius=10
     CollisionHeight=16
}

//===============================================================================
// Wiggentree bark spawns from Jar
//===============================================================================

class BarkSpawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.WiggentreeBark'
     Snds=(Spawning=Sound'HPSounds.hub1_sfx.vase_breaking')
     Limits=(Max=1,Min=1)
     BaseParticles=Class'HPParticle.GlassJarBreak'
     bDestroable=True
     Mesh=SkeletalMesh'HProps.skJarWiggentreeBarkMesh'
     DrawScale=1.2
     AmbientGlow=75
     CollisionRadius=10
     CollisionHeight=18
     CollideType=CT_OrientedCylinder
}

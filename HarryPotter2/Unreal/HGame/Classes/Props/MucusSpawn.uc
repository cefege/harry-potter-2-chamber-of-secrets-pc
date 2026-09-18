//===============================================================================
//  Flobberworm mucus spawns from jar
//===============================================================================

class MucusSpawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.FlobberwormMucus'
     Snds=(Spawning=Sound'HPSounds.hub1_sfx.vase_breaking')
     Limits=(Max=1,Min=1)
     BaseParticles=Class'HPParticle.GlassJarBreak'
     bDestroable=True
     Mesh=SkeletalMesh'HProps.skJarFlobberwormMucusMesh'
     DrawScale=1.2
     AmbientGlow=75
     MultiSkins(0)=WetTexture'HPParticle.hp_fx.Particles.FlobberM'
     CollisionRadius=10
     CollisionHeight=18
     CollideType=CT_OrientedCylinder
}

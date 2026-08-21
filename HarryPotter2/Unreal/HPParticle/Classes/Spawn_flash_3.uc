//=============================================================================
// Spawn_flash_3.
//=============================================================================
class Spawn_flash_3 expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=5)
     speed=(Base=20,Rand=30)
     Lifetime=(Base=0.5)
     ColorStart=(Base=(R=52,G=231,B=83))
     ColorEnd=(Base=(R=89,G=60,B=210))
     SizeWidth=(Base=120,Rand=50)
     SizeLength=(Base=120,Rand=50)
     SpinRate=(Base=-1,Rand=2)
     SizeDelay=2
     Chaos=3
     ChaosDelay=0.5
     ParticlesAlive=5
     ParticlesMax=5
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     CollisionRadius=40
     CollisionHeight=40
}

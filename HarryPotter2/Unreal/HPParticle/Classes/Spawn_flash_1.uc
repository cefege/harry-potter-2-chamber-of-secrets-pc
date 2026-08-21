//=============================================================================
// Spawn_flash_1.
//=============================================================================
class Spawn_flash_1 expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=5)
     speed=(Base=20,Rand=30)
     Lifetime=(Base=0.5)
     ColorStart=(Base=(R=172,G=40,B=242))
     ColorEnd=(Base=(R=23,G=52,B=249))
     SizeWidth=(Base=120,Rand=50)
     SizeLength=(Base=120,Rand=50)
     SpinRate=(Base=-5,Rand=10)
     SizeDelay=2
     Chaos=3
     ChaosDelay=0.5
     ParticlesAlive=5
     ParticlesMax=5
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     CollisionRadius=40
     CollisionHeight=40
}

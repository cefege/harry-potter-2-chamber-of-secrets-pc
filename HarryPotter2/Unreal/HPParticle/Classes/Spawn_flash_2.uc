//=============================================================================
// Spawn_flash_2.
//=============================================================================
class Spawn_flash_2 expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=5)
     speed=(Base=20,Rand=30)
     Lifetime=(Base=0.5)
     ColorStart=(Base=(R=252,G=142,B=22))
     ColorEnd=(Base=(R=247,G=255,B=142))
     SizeWidth=(Base=120,Rand=50)
     SizeLength=(Base=120,Rand=50)
     SpinRate=(Base=-4,Rand=8)
     SizeDelay=2
     Chaos=3
     ChaosDelay=0.5
     ParticlesAlive=5
     ParticlesMax=5
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     CollisionRadius=40
     CollisionHeight=40
}

//=============================================================================
// avifors_hit.
//=============================================================================
class avifors_hit expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=5.000000)
     speed=(Base=20.000000,Rand=30.000000)
     Lifetime=(Base=0.500000)
     ColorStart=(Base=(R=230,G=234,B=253))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=120.000000,Rand=50.000000)
     SizeLength=(Base=120.000000,Rand=50.000000)
     SpinRate=(Base=0.500000)
     SizeDelay=2.000000
     Chaos=3.000000
     ChaosDelay=0.500000
     ParticlesAlive=5
     ParticlesMax=5
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_1'
}

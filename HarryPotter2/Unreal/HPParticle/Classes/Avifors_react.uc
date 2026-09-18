//=============================================================================
// Avifors_react.
//=============================================================================
class Avifors_react expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=1.000000,Rand=2.000000)
     SourceWidth=(Base=0.500000)
     SourceHeight=(Base=0.500000)
     SourceDepth=(Base=5.000000)
     speed=(Base=10.000000)
     Lifetime=(Base=6.000000)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(G=255,B=255))
     SpinRate=(Base=1.500000,Rand=0.500000)
     Chaos=8.000000
     ChaosDelay=0.750000
     Elasticity=0.500000
     Damping=0.750000
     GravityModifier=0.002000
     ParticlesAlive=8
     ParticlesMax=8
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Feather'
     Style=STY_Masked
}

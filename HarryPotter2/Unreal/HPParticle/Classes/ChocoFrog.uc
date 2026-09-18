//=============================================================================
// ChocoFrog.
//=============================================================================
class ChocoFrog expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=10.000000,Rand=15.000000)
     SourceWidth=(Base=20.000000,Rand=10.000000)
     SourceHeight=(Base=20.000000,Rand=10.000000)
     SourceDepth=(Base=10.000000,Rand=20.000000)
     AngularSpreadWidth=(Base=10.000000,Rand=20.000000)
     AngularSpreadHeight=(Base=10.000000,Rand=20.000000)
     speed=(Base=5.000000,Rand=20.000000)
     Lifetime=(Base=7.000000)
     ColorStart=(Base=(R=0,G=0,B=0))
     ColorEnd=(Base=(R=172,G=130),Rand=(R=253,G=88))
     SizeWidth=(Base=1.000000,Rand=10.000000)
     SizeLength=(Base=1.000000,Rand=10.000000)
     SizeEndScale=(Base=2.000000)
     SpinRate=(Base=0.200000,Rand=2.000000)
     SizeDelay=2.000000
     Chaos=1.000000
     ChaosDelay=0.500000
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_1'
     Rotation=(Pitch=-16352)
     DesiredRotation=(Pitch=-16352)
}

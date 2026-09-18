//=============================================================================
// ScrollFX.
//=============================================================================
class ScrollFX expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=5.000000,Rand=20.000000)
     SourceWidth=(Base=8.000000,Rand=15.000000)
     SourceHeight=(Base=2.000000,Rand=10.000000)
     SourceDepth=(Base=8.000000,Rand=15.000000)
     AngularSpreadWidth=(Base=10.000000)
     AngularSpreadHeight=(Base=1.000000)
     speed=(Base=10.000000,Rand=15.000000)
     Lifetime=(Rand=5.000000)
     ColorStart=(Base=(R=116,G=55,B=176),Rand=(R=168,G=84,B=237))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=2.000000,Rand=8.000000)
     SizeLength=(Base=2.000000,Rand=8.000000)
     SizeEndScale=(Base=0.100000,Rand=20.000000)
     SpinRate=(Base=0.500000,Rand=20.000000)
     SizeDelay=3.000000
     Chaos=3.000000
     ChaosDelay=1.000000
     Attraction=(X=10.000000,Y=10.000000)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_4'
}

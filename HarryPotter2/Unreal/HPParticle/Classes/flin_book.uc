//=============================================================================
// Flin_book.
//=============================================================================
class Flin_book expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=10.000000,Rand=15.000000)
     SourceWidth=(Base=20.000000,Rand=10.000000)
     SourceHeight=(Base=20.000000,Rand=10.000000)
     SourceDepth=(Base=20.000000)
     AngularSpreadWidth=(Base=0.000000)
     AngularSpreadHeight=(Base=0.000000)
     bSteadyState=True
     speed=(Base=10.000000,Rand=50.000000)
     Lifetime=(Rand=3.000000)
     ColorStart=(Base=(R=254,G=134,B=69))
     ColorEnd=(Base=(R=243,G=37,B=227))
     AlphaStart=(Base=0.000000,Rand=1.000000)
     SizeWidth=(Base=5.000000,Rand=30.000000)
     SizeLength=(Base=5.000000,Rand=30.000000)
     SizeEndScale=(Base=-1.000000,Rand=3.000000)
     SpinRate=(Base=-4.000000,Rand=8.000000)
     Chaos=5.000000
     ChaosDelay=0.500000
     Damping=1.000000
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=16640)
}

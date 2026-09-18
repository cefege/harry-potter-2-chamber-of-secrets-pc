//=============================================================================
// Aragog Attack fx for AragogAttack that flys out Aragog.
//=============================================================================
class AragogAttackFx expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=20,Rand=5)
     SourceWidth=(Base=15,Rand=5)
     SourceHeight=(Base=15,Rand=5)
     SourceDepth=(Base=15,Rand=5)
     AngularSpreadWidth=(Base=90,Rand=90)
     AngularSpreadHeight=(Base=90,Rand=90)
     bSteadyState=True
     Speed=(Base=5,Rand=5)
     Lifetime=(Base=0.5,Rand=0.1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=0,G=128))
     SizeWidth=(Base=15,Rand=10)
     SizeLength=(Base=15,Rand=10)
     SizeEndScale=(Base=3)
     SpinRate=(Base=-4,Rand=4)
     bSystemRelative=True
     Attraction=(X=20,Y=20,Z=20)
     Textures(0)=Texture'HPParticle.particle_fx.noisy5_pfx'
     Rotation=(Pitch=16384)
}

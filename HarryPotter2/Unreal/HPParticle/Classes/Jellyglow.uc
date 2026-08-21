//=============================================================================
// Jellyglow.
//=============================================================================
class Jellyglow expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=10.000000)
     SourceWidth=(Base=15.000000)
     SourceHeight=(Base=15.000000)
     SourceDepth=(Base=15.000000)
     AngularSpreadWidth=(Base=2.000000)
     AngularSpreadHeight=(Base=2.000000)
     speed=(Base=2.000000)
     Lifetime=(Base=2.000000)
     ColorStart=(Base=(R=157,G=101,B=203))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=2.000000,Rand=10.000000)
     SizeLength=(Base=2.000000,Rand=10.000000)
     SizeEndScale=(Base=-0.500000)
     SpinRate=(Base=0.500000,Rand=10.000000)
     Attraction=(X=10.000000,Y=10.000000)
     ParticlesAlive=10
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_1'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}

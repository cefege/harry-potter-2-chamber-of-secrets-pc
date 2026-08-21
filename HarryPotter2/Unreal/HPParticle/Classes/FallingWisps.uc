//=============================================================================
// FallingWisps.
//=============================================================================
class FallingWisps expands ParticleFX;

defaultproperties
{
     SourceDepth=(Base=15.000000)
     AngularSpreadWidth=(Base=90.000000,Rand=20.000000)
     AngularSpreadHeight=(Base=90.000000,Rand=20.000000)
     speed=(Base=5.000000,Rand=15.000000)
     Lifetime=(Base=2.000000,Rand=5.000000)
     ColorStart=(Base=(R=0,G=0,B=0))
     ColorEnd=(Base=(G=255,B=255))
     SizeWidth=(Base=1.000000,Rand=15.000000)
     SizeLength=(Base=1.000000,Rand=15.000000)
     SizeEndScale=(Base=-1.000000)
     Attraction=(X=5.000000,Y=5.000000)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_2'
     Rotation=(Pitch=-16352)
     DesiredRotation=(Pitch=-16352)
}

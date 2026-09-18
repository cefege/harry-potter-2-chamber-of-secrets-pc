//=============================================================================
// Verd_hit.
//=============================================================================
class Verd_hit expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=30.000000)
     SourceWidth=(Base=20.000000,Rand=30.000000)
     SourceHeight=(Base=20.000000,Rand=30.000000)
     SourceDepth=(Base=60.000000,Rand=30.000000)
     AngularSpreadWidth=(Base=40.000000)
     AngularSpreadHeight=(Base=40.000000)
     bSteadyState=True
     speed=(Base=10.000000,Rand=40.000000)
     Lifetime=(Base=3.000000)
     ColorStart=(Base=(R=15,G=217,B=4),Rand=(R=64,G=114,B=56))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=15.000000)
     SizeLength=(Base=15.000000)
     SizeEndScale=(Base=-5.000000,Rand=2.000000)
     SpinRate=(Base=-2.000000,Rand=4.000000)
     Chaos=5.000000
     Attraction=(X=20.000000,Y=20.000000)
     ParticlesMax=200
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_8'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}

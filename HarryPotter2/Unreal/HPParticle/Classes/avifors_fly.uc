//=============================================================================
// avifors_fly.
//=============================================================================
class avifors_fly expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=80.000000,Rand=10.000000)
     SourceWidth=(Base=2.000000)
     SourceHeight=(Base=2.000000)
     AngularSpreadWidth=(Base=10.000000)
     AngularSpreadHeight=(Base=10.000000)
     speed=(Base=30.000000,Rand=15.000000)
     Lifetime=(Base=3.000000)
     ColorStart=(Base=(G=171,B=15),Rand=(R=255,G=43,B=197))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=25.000000)
     SizeLength=(Base=25.000000)
     SizeEndScale=(Base=-1.000000)
     SpinRate=(Base=5.000000,Rand=10.000000)
     Chaos=3.000000
     GravityModifier=0.050000
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_3'
     Rotation=(Pitch=16640)
}

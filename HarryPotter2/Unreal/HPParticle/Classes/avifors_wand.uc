//=============================================================================
// avifors_wand.
//=============================================================================
class avifors_wand expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=10.000000,Rand=10.000000)
     SourceWidth=(Base=2.000000)
     SourceHeight=(Base=2.000000)
     AngularSpreadWidth=(Base=10.000000)
     AngularSpreadHeight=(Base=1.000000)
     speed=(Base=30.000000,Rand=15.000000)
     Lifetime=(Base=1.500000)
     ColorStart=(Base=(G=171,B=15),Rand=(R=255,G=43,B=197))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=2.000000,Rand=8.000000)
     SizeLength=(Base=2.000000,Rand=8.000000)
     SizeEndScale=(Base=0.100000)
     SpinRate=(Base=-1.000000,Rand=10.000000)
     SizeDelay=3.000000
     Chaos=3.000000
     ChaosDelay=1.000000
     GravityModifier=0.050000
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_3'
     Rotation=(Pitch=16640)
}

//=============================================================================
// avif_book.
//=============================================================================
class avif_book expands AllSpellCast_FX;

defaultproperties
{
    ParticlesPerSec=(Base=5.000000,Rand=10.000000)
    SourceWidth=(Base=20.000000,Rand=10.000000)
    SourceHeight=(Base=20.000000,Rand=10.000000)
    SourceDepth=(Base=20.000000)
    AngularSpreadWidth=(Base=0.000000)
    AngularSpreadHeight=(Base=0.000000)
    bSteadyState=True
    speed=(Base=5.000000,Rand=15.000000)
    Lifetime=(Rand=3.000000)
    ColorStart=(Base=(R=249,G=203,B=66))
    ColorEnd=(Base=(R=228,G=41,B=102))
    SizeWidth=(Base=5.000000,Rand=10.000000)
    SizeLength=(Base=5.000000,Rand=10.000000)
    SizeEndScale=(Base=-2.000000,Rand=4.000000)
    SpinRate=(Base=-2.000000,Rand=4.000000)
    Chaos=5.000000
    ChaosDelay=0.500000
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_1'
    Rotation=(Pitch=16640)
}

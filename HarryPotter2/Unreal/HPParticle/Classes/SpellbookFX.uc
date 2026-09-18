//=============================================================================
// SpellbookFX.
//=============================================================================
class SpellbookFX expands ParticleFX;

defaultproperties
{
    ParticlesPerSec=(Base=5.000000,Rand=30.000000)
    SourceWidth=(Base=15.000000,Rand=15.000000)
    SourceHeight=(Base=15.000000,Rand=15.000000)
    SourceDepth=(Base=8.000000,Rand=15.000000)
    AngularSpreadWidth=(Rand=10.000000)
    AngularSpreadHeight=(Rand=10.000000)
    bSteadyState=True
    speed=(Base=5.000000,Rand=30.000000)
    Lifetime=(Rand=3.000000)
    ColorStart=(Base=(G=255,B=255),Rand=(R=253,G=45))
    ColorEnd=(Base=(R=0))
    SizeWidth=(Base=2.000000,Rand=8.000000)
    SizeLength=(Base=2.000000,Rand=8.000000)
    SizeEndScale=(Base=-1.000000,Rand=10.000000)
    SpinRate=(Base=-2.000000,Rand=4.000000)
    SizeDelay=1.000000
    Chaos=10.000000
    ChaosDelay=2.000000
    Attraction=(Z=2.000000)
    GravityModifier=0.005000
    Textures(0)=Texture'HPParticle.hp_fx.General.CandleF'
}

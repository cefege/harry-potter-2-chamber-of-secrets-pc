//=============================================================================
// Verd_Book.
//=============================================================================
class Verd_Book expands AllSpellCast_FX;

defaultproperties
{
    ParticlesPerSec=(Base=5.000000,Rand=20.000000)
    SourceWidth=(Base=15.000000,Rand=15.000000)
    SourceHeight=(Base=15.000000,Rand=15.000000)
    SourceDepth=(Base=8.000000,Rand=15.000000)
    AngularSpreadWidth=(Rand=10.000000)
    AngularSpreadHeight=(Rand=10.000000)
    bSteadyState=True
    speed=(Base=10.000000,Rand=30.000000)
    Lifetime=(Rand=3.000000)
    ColorStart=(Base=(R=31,G=220,B=31))
    ColorEnd=(Base=(R=60,G=124,B=29))
    SizeWidth=(Base=2.000000)
    SizeLength=(Base=1.000000,Rand=10.000000)
    SizeEndScale=(Base=-1.000000,Rand=10.000000)
    SizeDelay=1.000000
    Chaos=5.000000
    ChaosDelay=0.500000
    Attraction=(X=6.000000,Y=6.000000,Z=2.000000)
    Damping=0.250000
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_2'
    Rotation=(Pitch=16640)
    bRotateToDesired=True
}

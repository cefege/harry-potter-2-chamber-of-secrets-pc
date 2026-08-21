//=============================================================================
// Crabsmoke fx for fire crabs landing fire.
//=============================================================================
class Crabsmoke expands ParticleFX;

defaultproperties
{
    ParticlesPerSec=(Base=5)
    SourceWidth=(Base=2,Rand=2)
    SourceHeight=(Base=2,Rand=2)
    AngularSpreadWidth=(Base=20,Rand=10)
    bSteadyState=True
    Speed=(Base=15,Rand=10)
    Lifetime=(Base=3,Rand=3)
    ColorStart=(Base=(R=171,G=172,B=173))
    ColorEnd=(Base=(R=0))
    AlphaStart=(Base=0.25)
    AlphaEnd=(Base=1)
    SizeWidth=(Base=16,Rand=6)
    SizeLength=(Base=16,Rand=6)
    SizeEndScale=(Base=0,Rand=0.001)
    SpinRate=(Base=-2,Rand=4)
    Chaos=5
    ChaosDelay=1
    Attraction=(X=1,Y=1)
    Damping=0.2
    ParticlesMax=25
    Textures(0)=Texture'CVresearch.Base.cvsmoketest'
    Rotation=(Pitch=16384)
    Style=STY_Modulated
}


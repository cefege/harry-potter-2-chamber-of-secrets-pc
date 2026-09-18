//=============================================================================
// Skurge_hit2 for the large ectoplasm
//=============================================================================
class Skurge_hit2 expands Skurge_hit;

defaultproperties
{
    ParticlesPerSec=(Base=50)
    SourceWidth=(Base=96)
    SourceHeight=(Base=96)
    AngularSpreadWidth=(Base=20,Rand=20)
    AngularSpreadHeight=(Base=20,Rand=20)
    bSteadyState=False
    Speed=(Rand=300)
    Lifetime=(Base=3,Rand=3)
    Damping=0.75
    ParticlesMax=0
}

//=============================================================================
// Fx for Dobby to appear in Harry's bedroom
//=============================================================================
class DobbyBurst expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=50)
    SourceWidth=(Base=32,Rand=5)
    SourceHeight=(Base=32,Rand=5)
    SourceDepth=(Base=40)
    bSteadyState=True
    Speed=(Base=-2,Rand=4)
    Lifetime=(Base=2,Rand=1)
    ColorStart=(Base=(G=226,B=111))
    ColorEnd=(Base=(R=241,G=164,B=3))
    SizeWidth=(Base=3,Rand=2)
    SizeLength=(Base=3,Rand=2)
    SizeEndScale=(Base=4,Rand=6)
    SpinRate=(Base=-2,Rand=4)
    Chaos=2
    Attraction=(X=40,Y=40)
    Damping=0.5
    ParticlesMax=100
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
    Rotation=(Pitch=16280)
}

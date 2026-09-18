//=============================================================================
// Firefly fx CV
//=============================================================================
class Firefly2 expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=0.09)
    SourceWidth=(Base=100,Rand=64)
    SourceHeight=(Base=100,Rand=64)
    SourceDepth=(Base=100,Rand=20)
    AngularSpreadWidth=(Base=90,Rand=90)
    AngularSpreadHeight=(Base=90,Rand=90)
    bSteadyState=True
    Speed=(Base=3,Rand=3)
    Lifetime=(Base=4,Rand=6)
    ColorStart=(Base=(R=253,G=253,B=66))
    ColorEnd=(Base=(R=34))
    SizeEndScale=(Base=1.5)
    SizeWidth=(Base=4)
    SizeLength=(Base=4)
    Chaos=1
    Textures(0)=Texture'HPParticle.hp_fx.General.CandleF'
}

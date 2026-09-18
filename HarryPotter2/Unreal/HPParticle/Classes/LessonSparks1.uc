//=============================================================================
// Spell lesson sparks that fly off lesson ball
//=============================================================================
class LessonSparks1 expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=20,Rand=10)
    SourceWidth=(Base=5)
    SourceHeight=(Base=5)
    AngularSpreadWidth=(Base=180,Rand=30)
    AngularSpreadHeight=(Base=180)
    bSteadyState=True
    Speed=(Base=20)
    Lifetime=(Rand=1.5)
    ColorStart=(Base=(R=252,G=242,B=65),Rand=(R=255,G=191))
    ColorEnd=(Base=(R=203,G=39,B=55),Rand=(R=255,G=77,B=77))
    SizeWidth=(Base=8,Rand=6)
    SizeLength=(Base=8,Rand=6)
    SizeEndScale=(Base=0)
    SpinRate=(Base=-2,Rand=4)
    Damping=1
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
    Rotation=(Pitch=48995)
    
}

//=============================================================================
// Spongify hit spell fx
//=============================================================================
class Spongify_Hit expands AllSpellCast_FX;

defaultproperties
{
    ParticlesPerSec=(Base=5000)
    AngularSpreadWidth=(Base=180)
    AngularSpreadHeight=(Base=180)
    bSteadyState=True
    Speed=(Base=30,Rand=30)
    Lifetime=(Rand=1)
    ColorStart=(Base=(G=0,B=0))
    SizeWidth=(Base=2)
    SizeLength=(Base=2)
    bSystemRelative=True
    Damping=1.5
    ParticlesMax=50
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_2'
    Rotation=(Pitch=48995)
}

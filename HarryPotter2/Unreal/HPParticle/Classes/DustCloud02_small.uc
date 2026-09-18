//=============================================================================
// small dust cloud
//=============================================================================
class DustCloud02_small expands Dustclouds;

defaultproperties
{
    ParticlesPerSec=(Base=500)
    SourceWidth=(Base=32)
    SourceHeight=(Base=32)
    AngularSpreadWidth=(Base=45)
    AngularSpreadHeight=(Base=45)
    bSteadyState=True
    Speed=(Base=20)
    Lifetime=(Base=2,Rand=1)
    ColorStart=(Base=(R=156,G=139,B=124),Rand=(R=176,G=138,B=87))
    ColorEnd=(Base=(R=141,G=121,B=114),Rand=(R=168,G=139,B=81))
    SizeWidth=(Rand=6)
    SizeLength=(Rand=6)
    SizeEndScale=(Base=2,Rand=1)
    SpinRate=(Base=-1,Rand=2)
    Chaos=1
    Damping=0.5
    ParticlesMax=50
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
    Rotation=(Pitch=48995)
}

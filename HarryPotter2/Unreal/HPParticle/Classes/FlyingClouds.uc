//=============================================================================
// FlyingClouds particles that fly towards Ford Anglia 
//=============================================================================
class FlyingClouds expands FlyingFord;

defaultproperties
{
ParticlesPerSec=(Base=0.05)
    Texture=Texture'HPParticle.hp_fx.Particles.SepiaFlamefx'
    SourceWidth=(Base=500,Rand=200)
    SourceHeight=(Base=500,Rand=200)
    SourceDepth=(Base=0,Rand=0)
    Period=(Base=300)
    AngularSpreadWidth=(Base=0,Rand=0)
    AngularSpreadHeight=(Base=0,Rand=0)
    bSteadyState=False
    Speed=(Base=1200,Rand=400)
    Lifetime=(Base=3000,Rand=0)
    ColorStart=(Base=(R=255,G=255,B=255),Rand=(R=0))
    ColorEnd=(Base=0,Rand=0)
    AlphaEnd=(Base=0.5)
    SizeWidth=(Base=400,Rand=200)
    SizeLength=(Base=400,Rand=200)
    SizeEndScale=(Base=750)
    SpinRate=(Base=-0.25,Rand=0.5)
    DripTime=(Base=5)
    GravityModifier=0
    ParticlesAlive=10
    Textures(0)=Texture'HPParticle.hp_fx.Particles.FF_Cloud'
}

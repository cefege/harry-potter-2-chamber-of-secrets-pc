//=============================================================================
// Ecto_fly.
//=============================================================================
class Ecto_fly expands ParticleFX;

defaultproperties
{
    ParticlesPerSec=(Base=25)
    SourceWidth=(Base=1,Rand=2)
    SourceHeight=(Base=1,Rand=2)
    SourceDepth=(Rand=2)
    bSteadyState=True
    Speed=(Base=15,Rand=15)
    Lifetime=(Base=2,Rand=2)
    ColorStart=(Base=(R=104,G=167,B=78))
    ColorEnd=(Base=(R=66,G=167,B=37))
    SizeWidth=(Base=6,Rand=15)
    SizeLength=(Base=6,Rand=15)
    SizeEndScale=(Base=-1,Rand=2)
    SpinRate=(Base=-2,Rand=4)
    DripTime=(Base=0.5)
    Chaos=2
    Elasticity=0.1
    Damping=0.5
    GravityModifier=0.1
    Textures(0)=Texture'HPParticle.hp_fx.Particles.blob32'
}

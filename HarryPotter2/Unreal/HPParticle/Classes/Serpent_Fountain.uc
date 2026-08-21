//=============================================================================
// Serpent_Fountain.
//=============================================================================
class Serpent_Fountain expands ParticleFX;

defaultproperties
{
    ParticlesPerSec=(Rand=10.000000)
    SourceWidth=(Base=6.000000)
    SourceHeight=(Base=6.000000)
    bSteadyState=True
    Lifetime=(Base=2.000000)
    ColorStart=(Base=(R=58,G=193,B=13))
    ColorEnd=(Base=(R=61,G=171,B=71))
    SizeWidth=(Base=12.000000)
    SizeLength=(Base=3.000000)
    SizeEndScale=(Base=4.000000,Rand=3.000000)
    GravityModifier=0.100000
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_1'
    RenderPrimitive=PPRIM_Liquid
    Rotation=(Pitch=-6208,Yaw=128)
    bRotateToDesired=True
}

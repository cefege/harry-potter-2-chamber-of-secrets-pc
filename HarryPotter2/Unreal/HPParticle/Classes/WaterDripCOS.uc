//=============================================================================
// Water drip for the chamber of Secrets 
//=============================================================================
class WaterDripCOS expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=0.1,Rand=1)
    SourceWidth=(Base=14,Rand=14)
    SourceHeight=(Base=0)
    bSteadyState=True
    Speed=(Base=1)
    Lifetime=(Base=3,Rand=2)
    ColorStart=(Base=(R=66,G=87,B=130))
    ColorEnd=(Base=(R=70,G=80,B=132))
    SizeWidth=(Base=4,Rand=6)
    SizeLength=(Base=4,Rand=6)
    DripTime=(Base=0.5,Rand=0.5)
    GravityModifier=0.35
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_Neutral'
    RenderPrimitive=PPRIM_Liquid
}

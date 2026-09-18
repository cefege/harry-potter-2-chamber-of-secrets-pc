//=============================================================================
// Basic rain particle fx,  should be modified for surface area coverage
//=============================================================================
class RainBasic expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=75,Rand=50)
    SourceWidth=(Base=384)
    SourceHeight=(Base=384)
    bSteadyState=True
    Speed=(Base=300)
    Lifetime=(Base=3)
    ColorStart=(Base=(R=214,G=222,B=241))
    ColorEnd=(Base=(R=63,G=85,B=146))
    SizeWidth=(Base=2)
    SizeLength=(Base=2)
    Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_Neutral'
    RenderPrimitive=PPRIM_Liquid
    Rotation=(Pitch=-16608)
}

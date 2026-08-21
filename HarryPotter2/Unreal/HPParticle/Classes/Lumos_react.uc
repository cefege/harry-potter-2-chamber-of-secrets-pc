//=============================================================================
// Lumos_react.
//=============================================================================
class Lumos_react expands AllSpellCast_FX;

defaultproperties
{
    SourceWidth=(Base=128)
    SourceHeight=(Base=128)
    SourceDepth=(Base=128)
    bSteadyState=True
    Speed=(Base=-10,Rand=20)
    Lifetime=(Base=2,Rand=2)
    ColorStart=(Base=(R=236,G=220,B=17),Rand=(R=211,G=202,B=12))
    ColorEnd=(Base=(R=254,G=254,B=1),Rand=(R=243,G=189,B=1))
    SizeWidth=(Base=3,Rand=6)
    SizeLength=(Base=3,Rand=6)
    Chaos=0.5
    Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
    Rotation=(Pitch=16464)
}

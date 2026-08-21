//=============================================================================
// Spongify fly spell fx
//=============================================================================
class Spongify_Fly expands AllSpellCast_FX;

defaultproperties
{
     SourceWidth=(Base=5)
     SourceHeight=(Base=5)
     SourceDepth=(Base=5)
     bSteadyState=True
     Speed=(Base=5,Rand=10)
     Lifetime=(Base=2)
     ColorStart=(Base=(R=143,G=63,B=192))
     ColorEnd=(Base=(R=43,G=62,B=138))
     SizeWidth=(Base=4,Rand=8)
     SizeLength=(Base=4,Rand=8)
     bSystemRelative=True
     Chaos=10
     Damping=10
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=48995)
}

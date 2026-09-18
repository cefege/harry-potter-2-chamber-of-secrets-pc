//=============================================================================
// duelExpelliarmus_fly.
//=============================================================================
class duelExpelliarmus_fly expands AllSpellCast_FX;

defaultproperties
{
     SourceWidth=(Base=1)
     SourceHeight=(Base=1)
     bSteadyState=True
     Speed=(Base=0)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=237,B=15))
     ColorEnd=(Base=(G=191,B=60))
     SizeWidth=(Base=7)
     SizeLength=(Base=7)
     SizeEndScale=(Base=0)
     bSystemRelative=True
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=0)
}

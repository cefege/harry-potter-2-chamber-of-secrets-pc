//=============================================================================
// SkurgeSpell_hit.
//=============================================================================
class SkurgeSpell_hit expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=500)
     SourceWidth=(Base=1)
     SourceHeight=(Base=1)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=200)
     Lifetime=(Base=1.20)
     ColorStart=(Base=(R=34,G=67,B=255))
     ColorEnd=(Base=(R=113,G=6,B=164))
     SizeWidth=(Base=20,Rand=10)
     SizeLength=(Base=20,Rand=10)
     SizeEndScale=(Base=0.001)
     SpinRate=(Base=-2,Rand=3)
     bSystemRelative=True
     Damping=7
     ParticlesMax=50
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
}

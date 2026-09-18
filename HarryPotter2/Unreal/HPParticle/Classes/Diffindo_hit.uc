//=============================================================================
// Diffindo hit spell fx
//=============================================================================
class Diffindo_hit expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=1000)
     SourceWidth=(Rand=5)
     SourceHeight=(Rand=5)
     SourceDepth=(Base=5)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=100,Rand=30)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=121,G=255,B=11))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=5,Rand=8)
     SizeLength=(Base=5,Rand=8)
     DripTime=(Base=0.1)
     bSystemRelative=True
     Damping=1
     ParticlesMax=100
     Textures(0)=Texture'HPParticle.particle_fx.noisy2_pfx'
     Rotation=(Pitch=48995)
}

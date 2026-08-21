//=============================================================================
// Flip_hit.
//=============================================================================
class Flip_hit expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=500)
     SourceWidth=(Base=1)
     SourceHeight=(Base=1)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=600)
     Lifetime=(Rand=0.5)
     ColorStart=(Base=(R=250,G=140,B=20),Rand=(R=133,G=133,B=133))
     ColorEnd=(Base=(R=200,G=28,B=28))
     SizeWidth=(Base=30,Rand=10)
     SizeLength=(Base=30,Rand=10)
     SizeEndScale=(Base=0.001)
     SpinRate=(Base=-3,Rand=6)
     bSystemRelative=True
     Damping=7
     ParticlesMax=50
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
}

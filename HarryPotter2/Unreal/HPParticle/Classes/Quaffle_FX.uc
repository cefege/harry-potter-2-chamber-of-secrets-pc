//=============================================================================
// Quaffle_FX.
//=============================================================================
class Quaffle_FX expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=50)
     SourceWidth=(Base=0,Rand=5)
     SourceHeight=(Base=0,Rand=5)
     SourceDepth=(Rand=5)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     bSteadyState=True
     Speed=(Base=0,Rand=50)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=0,B=0))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=1,Rand=8)
     SizeLength=(Base=1,Rand=8)
     SizeEndScale=(Base=0,Rand=13)
     SpinRate=(Base=-6,Rand=6)
     Chaos=1
     Damping=2
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_fire_01'
     CollisionRadius=10000
}

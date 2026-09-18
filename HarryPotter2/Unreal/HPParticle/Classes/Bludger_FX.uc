//=============================================================================
// Bludger_FX.
//=============================================================================
class Bludger_FX expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=80)
     SourceWidth=(Base=0,Rand=5)
     SourceHeight=(Base=0,Rand=5)
     SourceDepth=(Rand=5)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     bSteadyState=True
     Speed=(Base=0,Rand=30)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=83,G=0,B=255))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=1,Rand=8)
     SizeLength=(Base=1,Rand=8)
     SizeEndScale=(Base=0,Rand=15)
     SpinRate=(Base=-6,Rand=6)
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_BlueSmoke'
     CollisionRadius=10000
}

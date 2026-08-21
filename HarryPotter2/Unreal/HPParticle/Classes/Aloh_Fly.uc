//=============================================================================
// Aloh_fly.
//=============================================================================
class Aloh_fly expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=25,Rand=10)
     SourceWidth=(Base=5,Rand=2)
     SourceHeight=(Base=5,Rand=2)
     bSteadyState=True
     Speed=(Base=20)
     ColorStart=(Base=(R=253,G=152,B=0))
     ColorEnd=(Base=(G=202,B=40))
     SpinRate=(Base=-1,Rand=2)
     Chaos=5
     ChaosDelay=0.25
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Key3'
     Physics=PHYS_Rotating
     bFixedRotationDir=True
     RotationRate=(Yaw=500000)
}

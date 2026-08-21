//=============================================================================
// SleepFX.
//=============================================================================
class SleepFX expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=0.3,Rand=0.1)
     SourceWidth=(Base=4)
     SourceHeight=(Base=4)
     AngularSpreadWidth=(Base=30)
     AngularSpreadHeight=(Base=30)
     bSteadyState=True
     Speed=(Base=5)
     Lifetime=(Base=3.5,Rand=1.5)
     ColorStart=(Base=(R=0,G=0,B=0))
     ColorEnd=(Base=(G=255,B=255))
     SizeWidth=(Base=2)
     SizeLength=(Base=2)
     SizeEndScale=(Base=8)
     AlphaDelay=1.25
     ColorDelay=0.7
     Chaos=6
     ChaosDelay=1
     Damping=2
     Gravity=(Z=35)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Z'
     Tag=SleepFX
}

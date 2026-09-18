//=============================================================================
// WizCard_Explo.
//=============================================================================
class WizCard_Explo expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=1000)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     bSteadyState=True
     Speed=(Base=200,Rand=50)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=130,B=4),Rand=(R=255,G=255,B=255))
     SizeEndScale=(Base=3)
     Damping=3.5
     GravityModifier=0.2
     ParticlesMax=80
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     LastUpdateLocation=(Z=32)
     LastEmitLocation=(Z=32)
     LastUpdateRotation=(Pitch=16400)
     Age=643.2258
     ParticlesEmitted=50
     bDynamicLight=True
     Tag=Dummyparticle
     Location=(Z=32)
     Rotation=(Pitch=16400)
     OldLocation=(Z=32)
}

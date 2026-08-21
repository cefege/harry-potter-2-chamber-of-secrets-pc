//=============================================================================
// Potion_flash.
//=============================================================================
class Potion_flash expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=80)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=300)
     Lifetime=(Rand=2)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(G=9,B=15),Rand=(R=255,G=255,B=255))
     SizeWidth=(Rand=4)
     SizeLength=(Rand=4)
     SizeEndScale=(Base=4)
     SpinRate=(Base=-6,Rand=12)
     Attraction=(X=1.5,Y=1.5,Z=1.5)
     Damping=11
     GravityModifier=-0.1
     ParticlesMax=0
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_5_BW'
     LastUpdateLocation=(X=5.960464e-008,Z=-50.73127)
     LastEmitLocation=(X=5.960464e-008,Z=-50.73127)
     LastUpdateRotation=(Pitch=16480)
     Age=1260.923
     ParticlesEmitted=80
     bDynamicLight=True
     Tag=Dummyparticle
     Location=(X=5.960464e-008,Z=-50.73127)
     Rotation=(Pitch=16480)
     OldLocation=(Z=32)
     bSelected=True
}

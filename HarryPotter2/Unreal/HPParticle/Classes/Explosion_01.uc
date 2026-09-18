//=============================================================================
// Explosion_01.
//=============================================================================
class Explosion_01 expands ParticleFX;

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
     Speed=(Base=200,Rand=100)
     Lifetime=(Rand=1)
     ColorStart=(Base=(B=0),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(R=0),Rand=(R=255,G=255,B=255))
     SizeWidth=(Base=1,Rand=6)
     SizeLength=(Base=1,Rand=6)
     SizeEndScale=(Base=0,Rand=10)
     Chaos=1
     Attraction=(X=-0.01,Y=-0.01,Z=-0.01)
     Damping=18
     GravityModifier=-0.05
     ParticlesMax=80
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_fire_01'
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

//=============================================================================
// Glass Jar Breaking effect for potion ingrediants
//=============================================================================
class GlassJarBreak expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=300)
     SourceWidth=(Base=15)
     SourceHeight=(Base=15)
     SourceDepth=(Base=30)
     AngularSpreadWidth=(Base=65)
     AngularSpreadHeight=(Base=65)
     bSteadyState=True
     Speed=(Base=60,Rand=10)
     Lifetime=(Rand=0.5)
     ColorStart=(Base=(G=255,B=255),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(R=192,G=192,B=192),Rand=(R=255,G=255,B=255))
     AlphaStart=(Base=0.5)
     AlphaEnd=(Base=0.5)
     SizeWidth=(Base=1,Rand=1)
     SizeLength=(Base=1,Rand=1)
     SizeEndScale=(Base=0,Rand=10)
     SpinRate=(Base=20,Rand=-5)
     Attraction=(X=-0.01,Y=-0.01,Z=-0.01)
     GravityModifier=0.1
     Gravity=(Z=-50)
     ParticlesMax=20
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Glass'
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

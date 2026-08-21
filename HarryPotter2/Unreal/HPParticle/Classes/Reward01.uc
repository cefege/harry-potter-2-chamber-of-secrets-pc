//=============================================================================
// Reward01.
//=============================================================================
class Reward01 expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=300)
     SourceWidth=(Base=1)
     SourceHeight=(Base=1)
     SourceDepth=(Base=1)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=150)
     Lifetime=(Base=2,Rand=-2)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=44,G=34,B=221))
     SizeWidth=(Base=4,Rand=-34)
     SizeLength=(Base=4,Rand=-34)
     SpinRate=(Base=-3,Rand=6)
     Damping=20
     GravityModifier=0.1
     ParticlesMax=30
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_3'
     EmitDelay=0.79248
     LastUpdateLocation=(X=224,Z=32)
     LastEmitLocation=(X=224,Z=32)
     LastUpdateRotation=(Pitch=16336)
     Age=1451.504
     bDynamicLight=True
     Tag=Dummyparticle
     Location=(X=224,Z=32)
     Rotation=(Pitch=16336)
     OldLocation=(X=224,Z=32)
}

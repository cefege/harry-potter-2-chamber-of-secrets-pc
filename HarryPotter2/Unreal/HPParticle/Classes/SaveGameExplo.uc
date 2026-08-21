//=============================================================================
// SaveGameExplo.
//=============================================================================
class SaveGameExplo expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=400)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=100)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=0,G=255,B=0),Rand=(R=255))
     ColorEnd=(Base=(G=128))
     SizeWidth=(Base=2,Rand=6)
     SizeLength=(Base=2,Rand=6)
     SizeEndScale=(Base=0,Rand=3)
     SpinRate=(Base=0.5)
     Chaos=1
     Damping=10
     ParticlesMax=40
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
}

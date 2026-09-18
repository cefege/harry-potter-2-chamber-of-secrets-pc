//=============================================================================
// SorcererStoneFX.
//=============================================================================
class SorcererStoneFX expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=30,Rand=30)
     SourceWidth=(Base=1,Rand=10)
     SourceHeight=(Base=1,Rand=10)
     SourceDepth=(Base=1,Rand=10)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=1)
     Lifetime=(Rand=1)
     ColorStart=(Base=(B=0),Rand=(R=128,G=128,B=128))
     SizeWidth=(Base=1,Rand=2)
     SizeLength=(Base=1,Rand=2)
     SizeEndScale=(Base=3)
     Chaos=1
     GravityModifier=-0.02
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_3'
}

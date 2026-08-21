//=============================================================================
// SpellTarget.
//=============================================================================
class SpellTarget expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=80)
     SourceWidth=(Base=5,Rand=5)
     SourceHeight=(Base=5,Rand=5)
     SourceDepth=(Base=5,Rand=5)
     AngularSpreadWidth=(Base=180,Rand=181)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=1,Rand=40)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=0,B=0),Rand=(R=255,G=255))
     ColorEnd=(Base=(R=0,B=255))
     SizeWidth=(Base=2,Rand=5)
     SizeLength=(Base=2,Rand=5)
     SizeEndScale=(Base=0,Rand=3)
     Chaos=1
     Damping=4
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
}

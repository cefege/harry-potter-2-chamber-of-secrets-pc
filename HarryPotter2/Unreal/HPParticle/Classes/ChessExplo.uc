//=============================================================================
// ChessExplo.
//=============================================================================
class ChessExplo expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     ParticlesPerSec=(Base=0,Rand=1000)
     SourceWidth=(Base=0,Rand=50)
     SourceHeight=(Base=0,Rand=150)
     SourceDepth=(Rand=50)
     AngularSpreadWidth=(Base=180,Rand=180)
     AngularSpreadHeight=(Base=180,Rand=180)
     Speed=(Base=1,Rand=400)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=128,B=128),Rand=(R=128,G=128,B=128))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=3,Rand=7)
     SizeLength=(Base=3,Rand=7)
     SizeEndScale=(Base=0,Rand=25)
     SpinRate=(Base=-6,Rand=6)
     Chaos=1
     Damping=4
     GravityModifier=-0.05
     ParticlesMax=100
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_fire_01'
}

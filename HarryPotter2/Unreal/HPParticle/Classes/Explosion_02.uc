//=============================================================================
// Explosion_02.
//=============================================================================
class Explosion_02 expands Explosion_01;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     SourceWidth=(Rand=10)
     SourceHeight=(Rand=10)
     SourceDepth=(Rand=10)
     Speed=(Base=300)
     ColorStart=(Base=(B=64))
     ColorEnd=(Base=(R=64),Rand=(R=0,G=0,B=0))
     SizeWidth=(Base=4,Rand=8)
     SizeLength=(Base=4,Rand=8)
     SizeEndScale=(Rand=20)
     Chaos=10
     Attraction=(X=-0.08,Y=-0.08,Z=-0.08)
     Damping=10
     GravityModifier=-0.02
}

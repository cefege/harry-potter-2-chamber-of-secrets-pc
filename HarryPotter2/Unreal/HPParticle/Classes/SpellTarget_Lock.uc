//=============================================================================
// SpellTarget_Lock.
//=============================================================================
class SpellTarget_Lock expands SpellTarget;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     SourceWidth=(Base=0,Rand=50)
     SourceHeight=(Base=0,Rand=50)
     SourceDepth=(Base=0,Rand=50)
     AngularSpreadWidth=(Rand=180)
     Speed=(Base=0,Rand=0)
     ColorStart=(Base=(R=0,G=255),Rand=(G=0))
     ColorEnd=(Rand=(R=255,G=255))
     SizeWidth=(Base=3,Rand=6)
     SizeLength=(Base=3,Rand=6)
     SizeEndScale=(Rand=2)
     Chaos=0
     Attraction=(X=10,Y=10,Z=10)
     Damping=0
}

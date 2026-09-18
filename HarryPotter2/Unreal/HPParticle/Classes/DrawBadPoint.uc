//=============================================================================
// DrawBadPoint.
//=============================================================================
class DrawBadPoint expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

defaultproperties
{
     Distribution=DIST_Uniform
     ParticlesPerSec=(Base=3.000000)
     SourceWidth=(Base=0.000000)
     SourceHeight=(Base=0.000000)
     AngularSpreadWidth=(Base=0.000000)
     AngularSpreadHeight=(Base=0.000000)
     speed=(Base=0.000000)
     Lifetime=(Base=60.000000)
     ColorStart=(Base=(R=0,G=0,B=0))
     ColorEnd=(Base=(R=0,G=0,B=0))
     SizeEndScale=(Base=1)
}

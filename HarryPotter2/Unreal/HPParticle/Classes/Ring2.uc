//=============================================================================
// Ring2.
//=============================================================================
class Ring2 expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=175)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=20)
     Lifetime=(Rand=0.5)
     ColorStart=(Base=(R=6,G=143,B=255))
     ColorEnd=(Base=(R=191,B=255))
     SizeWidth=(Base=35,Rand=15)
     SizeLength=(Base=35,Rand=15)
     SizeEndScale=(Base=-1)
     Distribution=DIST_OwnerMesh
     GravityModifier=0.1
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=16640)
}

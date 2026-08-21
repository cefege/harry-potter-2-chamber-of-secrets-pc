//=============================================================================
// Ring5.
//=============================================================================
class Ring5 expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=175)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=20)
     Lifetime=(Rand=0.5)
     ColorStart=(Base=(G=28,B=33))
     ColorEnd=(Base=(R=199,G=98,B=210))
     SizeWidth=(Base=35,Rand=15)
     SizeLength=(Base=35,Rand=15)
     SizeEndScale=(Base=-1)
     Distribution=DIST_OwnerMesh
     GravityModifier=0.1
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=16640)
}

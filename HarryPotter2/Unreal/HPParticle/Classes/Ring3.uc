//=============================================================================
// Ring3.
//=============================================================================
class Ring3 expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=175)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=20)
     Lifetime=(Rand=0.5)
     ColorStart=(Base=(R=99,G=243,B=44))
     ColorEnd=(Base=(R=247,G=254,B=107))
     SizeWidth=(Base=35,Rand=15)
     SizeLength=(Base=35,Rand=15)
     SizeEndScale=(Base=-1)
     Distribution=DIST_OwnerMesh
     GravityModifier=0.1
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=16640)
}

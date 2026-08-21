//=============================================================================
// Ring4.
//=============================================================================
class Ring4 expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=175)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     bSteadyState=True
     Speed=(Base=20)
     Lifetime=(Rand=0.5)
     ColorStart=(Base=(G=207,B=15))
     ColorEnd=(Base=(G=102,B=28))
     SizeWidth=(Base=35,Rand=15)
     SizeLength=(Base=35,Rand=15)
     SizeEndScale=(Base=-1)
     Distribution=DIST_OwnerMesh
     GravityModifier=0.1
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
     Rotation=(Pitch=16640)
}

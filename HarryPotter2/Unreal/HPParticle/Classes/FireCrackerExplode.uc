//=============================================================================
// Aloh_hit.
//=============================================================================
class FireCrackerExplode expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=4.000000)
     SourceWidth=(Base=4.000000)
     SourceHeight=(Base=4.000000)
     SourceDepth=(Base=4.000000)
     AngularSpreadWidth=(Base=180.000000)
     AngularSpreadHeight=(Base=180.000000)
     bSteadyState=True
     speed=(Base=10.000000,Rand=5.000000)
     Lifetime=(Base=2.000000)
     ColorStart=(Base=(G=255,B=255),Rand=(R=253,G=45))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=6.000000,Rand=8.000000)
     SizeLength=(Base=6.000000,Rand=8.000000)
     SizeEndScale=(Base=2.000000,Rand=4.000000)
     SpinRate=(Base=-2.000000,Rand=4.000000)
     GravityModifier=0.005000
     ParticlesMax=20
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Key1'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}

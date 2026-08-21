//=============================================================================
// Snail trial particle fx
//=============================================================================
class SnailTrailFX expands ParticleFX;

defaultproperties
{
	 CollisionRadius=400
     ParticlesPerSec=(Base=40.000000)
     SourceWidth=(Base=15.000000)
     SourceHeight=(Base=0.000000)
     SourceDepth=(Base=10.000000)
     AngularSpreadWidth=(Base=30.000000)
     AngularSpreadHeight=(Base=0.000000)
     speed=(Base=0.000000)
     Lifetime=(Base=3.000000)
     ColorStart=(Base=(G=100,B=6))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=6.000000)
     SizeLength=(Base=6.000000)
     SizeEndScale=(Base=2.000000)
     SpinRate=(Base=0.500000,Rand=10.000000)
     AlphaDelay=6.000000
     GravityModifier=0.000100
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_1'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}

//=============================================================================
// WaterShowerFXBase - For growing plant scene.
//=============================================================================
class WaterShowerFXBase expands ParticleFadeFX;

// Needs to ramp ParticlePerSec Base and Rand from 0 to the new defaults
// on trigger, then back to 0 after what ever time it takes for the plant to
// grow.

defaultproperties
{
     ParticlesPerSec=(Base=30.000000,Rand=10.000000)
     SourceWidth=(Base=0.000000)
     SourceHeight=(Base=0.000000)
     AngularSpreadWidth=(Base=50.000000,Rand=5.000000)
     AngularSpreadHeight=(Base=50.000000,Rand=5.000000)
     bSteadyState=True
     speed=(Rand=30.000000)
     Lifetime=(Base=1.650000)
     ColorStart=(Base=(R=1,G=205,B=143))
     ColorEnd=(Base=(R=23,G=255,B=35),Rand=(R=159,B=4))
     SizeWidth=(Rand=4.000000)
     SizeLength=(Base=2.000000,Rand=3.000000)
     SizeEndScale=(Base=0.400000,Rand=3.000000)
     AlphaDelay=1.000000
     Attraction=(X=1.000000,Y=1.000000)
     Damping=0.300000
     GravityModifier=0.250000
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Dot_1'
     RenderPrimitive=PPRIM_Liquid
     Rotation=(Pitch=16384)
     CollisionRadius=60.000000
     CollisionHeight=250.000000
     bRotateToDesired=True
     DesiredRotation=(Pitch=0)
}

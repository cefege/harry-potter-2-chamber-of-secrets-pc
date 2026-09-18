//=============================================================================
// WaterShowerFX - For growing plant scene.
//=============================================================================
class WaterShowerFX expands WaterShowerFXBase;

// Needs to ramp ParticlePerSec Base and Rand from 0 to the new defaults
// on trigger, then back to 0 after what ever time it takes for the plant to
// grow.

defaultproperties
{
     ParticlesPerSec=(Base=50)
     speed=(Base=60)
     SizeLength=(Base=3)
     SizeWidth=(Base=12)
}

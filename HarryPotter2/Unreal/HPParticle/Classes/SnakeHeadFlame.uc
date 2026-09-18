//=============================================================================
// Snake head flame for final challenge hp2
//=============================================================================
class SnakeHeadFlame expands particlefx;

defaultproperties
{
    ParticlesPerSec=(Base=75)
    bSteadyState=True
    Speed=(Base=300,Rand=100)
    Lifetime=(Base=2.5,Rand=2.5)
    ColorStart=(Base=(R=250,G=185,B=14),Rand=(R=255,G=208,B=98))
    ColorEnd=(Base=(R=151),Rand=(R=206,G=81,B=81))
    SizeWidth=(Base=12,Rand=16)
    SizeLength=(Base=12,Rand=16)
    SizeEndScale=(Base=2)
    SpinRate=(Base=-3,Rand=6)
    Damping=2
    GravityModifier=-0.1
    ParticlesMax=200
    Textures(0)=Texture'HPParticle.particle_fx.PotFire08'
    Rotation=(Pitch=-2336,Yaw=-16272,Roll=-240)
}

//=============================================================================
// Levitate_fly.
//=============================================================================
class Levitate_fly expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=80.000000)
     SourceWidth=(Base=2.000000)
     SourceHeight=(Base=2.000000)
     AngularSpreadWidth=(Base=10.000000)
     AngularSpreadHeight=(Base=10.000000)
     speed=(Base=20.000000,Rand=30.000000)
     Lifetime=(Base=2.000000)
     ColorStart=(Base=(G=255,B=255),Rand=(R=54,G=44,B=245))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=20.000000,Rand=20.000000)
     SizeLength=(Base=20.000000,Rand=20.000000)
     SizeEndScale=(Base=-0.500000)
     SpinRate=(Base=2.000000)
     bVelocityRelative=True
     Chaos=3.000000
     GravityModifier=-0.010000
     Textures(0)=FireTexture'HPParticle.hp_fx.Particles.F_spark'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}

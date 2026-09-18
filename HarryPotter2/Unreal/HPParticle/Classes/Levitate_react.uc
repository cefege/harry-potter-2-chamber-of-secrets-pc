//=============================================================================
// Levitate_react.
//=============================================================================
class Levitate_react expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=50.000000)
     SourceWidth=(Base=48.000000)
     SourceHeight=(Base=48.000000)
     Decay=(Rand=1.000000)
     AngularSpreadWidth=(Base=0.000000,Rand=8.000000)
     AngularSpreadHeight=(Base=0.000000,Rand=8.000000)
     bSteadyState=True
     speed=(Base=15.000000,Rand=25.000000)
     Lifetime=(Base=3.000000,Rand=2.000000)
     ColorStart=(Base=(R=191,G=191,B=255))
     ColorEnd=(Base=(R=138,G=141,B=255))
     SizeWidth=(Base=30.000000,Rand=10.000000)
     SizeLength=(Base=30.000000,Rand=10.000000)
     SizeEndScale=(Base=2.000000)
     SpinRate=(Base=1.000000,Rand=5.000000)
     DripTime=(Base=1.500000,Rand=1.500000)
     Textures(0)=FireTexture'HPParticle.hp_fx.Particles.F_spark'
     Rotation=(Pitch=16640)
}

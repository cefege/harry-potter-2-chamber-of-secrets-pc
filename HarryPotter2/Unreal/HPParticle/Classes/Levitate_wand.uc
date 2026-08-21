//=============================================================================
// Levitate_wand.
//=============================================================================
class Levitate_wand expands AllSpellCast_FX;

defaultproperties
{
     ParticlesPerSec=(Base=60.000000)
     SourceWidth=(Base=2.000000)
     SourceHeight=(Base=2.000000)
     AngularSpreadHeight=(Base=1.000000)
     speed=(Base=20.000000)
     Lifetime=(Base=0.250000)
     ColorStart=(Base=(R=169,G=184,B=241),Rand=(R=60,G=39,B=175))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=1.000000)
     SizeLength=(Base=1.000000)
     SizeEndScale=(Base=-1.000000,Rand=10.000000)
     SpinRate=(Base=1.000000,Rand=20.000000)
     Chaos=1.000000
     GravityModifier=0.003000
     Textures(0)=FireTexture'HPParticle.hp_fx.Particles.F_spark'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}

//=============================================================================
// QuestionFX.
//=============================================================================
class QuestionFX expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=200,Rand=50)
     SourceWidth=(Base=64)
     SourceHeight=(Base=64)
     SourceDepth=(Base=25)
     bSteadyState=True
     Speed=(Base=40,Rand=20)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=20,G=252,B=20))
     ColorEnd=(Base=(R=44,G=58,B=188))
     SizeWidth=(Base=16,Rand=4)
     SizeLength=(Base=16,Rand=4)
     SizeEndScale=(Base=-1,Rand=1)
     SpinRate=(Base=-8,Rand=8)
     DripTime=(Base=0.25)
     SizeGrowPeriod=0.75
     Damping=0.25
     ParticlesMax=300
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
     Rotation=(Pitch=-16840,Yaw=7776)
     bRotateToDesired=True
}

//=============================================================================
// Effect for correct key press during spell lessons
//=============================================================================
class SpellLessonHit expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=500)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=90,Rand=90)
     AngularSpreadHeight=(Base=90,Rand=90)
     bSteadyState=True
     Speed=(Base=300,Rand=100)
     Lifetime=(Base=0.6,Rand=0.1)
     ColorStart=(Base=(R=0))
     ColorEnd=(Base=(G=255))
     SizeWidth=(Base=10,Rand=4)
     SizeLength=(Base=10,Rand=4)
     SizeEndScale=(Base=-1,Rand=2)
     SpinRate=(Base=-12,Rand=12)
     AlphaDelay=1
     ColorDelay=0.4
     Chaos=10
     Damping=30
     GravityModifier=0.3
     ParticlesMax=50
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_04'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}

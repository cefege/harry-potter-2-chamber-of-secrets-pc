//=============================================================================
// Effect for incorrect key press during spell lessons
//=============================================================================
class SpellLessonMiss expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=300)
     SourceWidth=(Base=0)
     SourceHeight=(Base=0)
     AngularSpreadWidth=(Base=90,Rand=90)
     AngularSpreadHeight=(Base=90,Rand=90)
     Speed=(Base=200,Rand=100)
     Lifetime=(Base=0.5,Rand=0.1)
     ColorStart=(Base=(R=128,B=128))
     ColorEnd=(Base=(R=0))
     SizeWidth=(Base=12,Rand=4)
     SizeLength=(Base=12,Rand=4)
     SizeEndScale=(Rand=1)
     SpinRate=(Base=-2,Rand=2)
     AlphaDelay=1
     Chaos=10
     Damping=25
     GravityModifier=0.3
     ParticlesMax=25
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Smoke4'
     Rotation=(Pitch=16640)
     CollisionRadius=10
     CollisionHeight=10
     bRotateToDesired=True
}

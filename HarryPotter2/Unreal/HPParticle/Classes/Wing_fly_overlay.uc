//=============================================================================
// Wing_fly_overlay.
//=============================================================================
class Wing_fly_overlay expands Wing_fly;

defaultproperties
{
     ParticlesPerSec=(Base=40,Rand=40)
     SourceWidth=(Rand=30)
     SourceHeight=(Rand=30)
     SourceDepth=(Rand=30)
     AngularSpreadWidth=(Rand=180)
     AngularSpreadHeight=(Rand=180)
     Speed=(Base=10)
     SizeWidth=(Base=2,Rand=4)
     SizeLength=(Base=2,Rand=4)
     SpinRate=(Base=0,Rand=0)
     Damping=0
     GravityModifier=0
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_01'
}

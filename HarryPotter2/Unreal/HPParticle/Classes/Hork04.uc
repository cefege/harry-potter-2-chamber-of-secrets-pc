//=============================================================================
// Hork04, this particle fx is the Horkalump top pieces exploding into bits.
//=============================================================================
class Hork04 expands horklumpsfx;

defaultproperties
{
     ParticlesPerSec=(Base=500)
     SourceWidth=(Rand=4)
     SourceHeight=(Rand=4)
     SourceDepth=(Base=10,Rand=4)
     AngularSpreadWidth=(Base=20,Rand=40)
     AngularSpreadHeight=(Base=20,Rand=40)
     Speed=(Base=100,Rand=50)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=255,B=255),Rand=(R=255,G=255,B=255))
     ColorEnd=(Base=(G=255,B=255),Rand=(R=255,G=255,B=255))
     SizeWidth=(Base=2,Rand=10)
     SizeLength=(Base=2,Rand=10)
     SizeEndScale=(Base=0)
     SpinRate=(Base=-4,Rand=4)
     SizeDelay=0.6
     Chaos=3
     Elasticity=0.1
     Damping=1
     GravityModifier=0.5
     ParticlesMax=20
     Textures(0)=Texture'HPParticle.hp_fx.Particles.HorkaChunk'
     Rotation=(Pitch=16323)
     Style=STY_Masked
     AmbientGlow=255
}

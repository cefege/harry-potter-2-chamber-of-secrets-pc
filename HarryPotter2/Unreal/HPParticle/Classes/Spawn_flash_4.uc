//=============================================================================
// Spawn_flash_4.
//=============================================================================
class Spawn_flash_4 expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=500)
     SourceWidth=(Base=2)
     SourceHeight=(Base=2)
     AngularSpreadWidth=(Base=0)
     AngularSpreadHeight=(Base=0)
     Speed=(Base=20,Rand=30)
     Lifetime=(Base=0.5)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=89,G=60,B=210))
     SizeWidth=(Base=75)
     SizeLength=(Base=75)
     SpinRate=(Base=-4,Rand=8)
     ParticlesAlive=3
     ParticlesMax=3
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_01'
     CollisionRadius=40
     CollisionHeight=40
}

//=============================================================================
// DeathTorch used for torches in Skurge challenge.
//=============================================================================
class DeathTorch expands ParticleFX;

defaultproperties
{
     SourceWidth=(Base=2)
     SourceHeight=(Base=2)
     SourceDepth=(Base=2)
     bSteadyState=True
     Speed=(Base=10,Rand=10)
     Lifetime=(Base=3,Rand=3)
     ColorStart=(Base=(R=33,G=28,B=255))
     ColorEnd=(Base=(R=121,G=205,B=255))
     SizeWidth=(Rand=5)
     SizeLength=(Rand=5)
     SpinRate=(Base=-5,Rand=10)
     ColorDelay=1
     Chaos=0.5
     Textures(0)=Texture'HPParticle.hp_fx.Spells.LesBlueFire_01'
     AmbientSound=Sound'HPSounds.Ch2Skurge.blue_death_torch'
     SoundRadius=12
     SoundVolume=220	
     SoundPitch=70	
}

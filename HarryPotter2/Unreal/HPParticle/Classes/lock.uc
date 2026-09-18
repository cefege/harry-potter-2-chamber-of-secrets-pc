//=============================================================================
// Lock.
//=============================================================================
class Lock expands ParticleFX;

defaultproperties
{
     ParticlesPerSec=(Base=40)
     SourceWidth=(Base=20)
     SourceHeight=(Base=20)
     SourceDepth=(Base=20)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     bSteadyState=True
     Speed=(Base=5,Rand=10)
     Lifetime=(Rand=1)
     ColorStart=(Base=(R=89,G=131,B=255))
     ColorEnd=(Base=(R=72,G=63,B=194))
     SizeWidth=(Base=6,Rand=2)
     SizeLength=(Base=6,Rand=2)
     SizeEndScale=(Base=0)
     SpinRate=(Base=-4,Rand=8)
     Chaos=2
     ChaosDelay=0.5
     Attraction=(X=5,Y=5)
     Distribution=DIST_OwnerMesh
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Les_Sparkle_01'
     Rotation=(Pitch=-16352)
     DesiredRotation=(Pitch=-16352)
}

//=============================================================================
// Crabfire fx for fire crabs landing fire.
//=============================================================================
class Crabfire3 expands ParticleFX;

defaultproperties
{
     SourceWidth=(Rand=4)
     SourceHeight=(Rand=4)
     bSteadyState=True
     Speed=(Base=10,Rand=15)
     Lifetime=(Rand=2)
     SizeWidth=(Base=24,Rand=6)
     SizeLength=(Base=24,Rand=6)
     SizeEndScale=(Base=.1,Rand=2)
     SpinRate=(Base=-2,Rand=4)
     Chaos=1
     ChaosDelay=1
     ParticlesMax=50
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_fire_01'
     Rotation=(Pitch=16384)
}

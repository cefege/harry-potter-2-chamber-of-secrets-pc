//=============================================================================
// Crabfireball fx for fireball that flys out from large crabs.
//=============================================================================
class Crabfireball expands ParticleFX;

defaultproperties
{
    ParticlesPerSec=(Rand=5)
    SourceWidth=(Base=15,Rand=5)
    SourceHeight=(Base=15,Rand=5)
    SourceDepth=(Base=15,Rand=5)
    AngularSpreadWidth=(Base=90,Rand=90)
    AngularSpreadHeight=(Base=90,Rand=90)
    bSteadyState=True
    Speed=(Base=10,Rand=25)
    Lifetime=(Base=2,Rand=1)
    SizeWidth=(Rand=24)
    SizeLength=(Rand=24)
    SizeEndScale=(Base=0,Rand=2)
    SpinRate=(Base=-4,Rand=8)
    bSystemRelative=True
    Attraction=(X=10,Y=10,Z=10)
    Textures(0)=Texture'HPParticle.particle_fx.PotFire07'
     Rotation=(Pitch=16384)
}

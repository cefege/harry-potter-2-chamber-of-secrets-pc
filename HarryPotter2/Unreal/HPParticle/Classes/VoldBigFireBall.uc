//=============================================================================
//=============================================================================
class VoldBigFireBall expands ParticleFX;

var() float fFireBallSpeed;

//Someone else has to set the velocity  (voldemort)

function Tick(float dtime)
{
	SetLocation( Location + velocity*dtime );
}

defaultproperties
{
     ParticlesPerSec=(Base=50)
     SourceHeight=(Base=5)
     bSteadyState=True
     Speed=(Base=0)
     Lifetime=(Base=0.4)
     ColorStart=(Base=(G=255,B=255))
     SizeWidth=(Base=60)
     SizeLength=(Base=60)
     SizeEndScale=(Base=0.01)
     SpinRate=(Base=10,Rand=20)
     Chaos=4
     Textures(0)=Texture'HPParticle.particle_fx.PotFire08'

	LifeSpan=2

	fFireBallSpeed=400
}

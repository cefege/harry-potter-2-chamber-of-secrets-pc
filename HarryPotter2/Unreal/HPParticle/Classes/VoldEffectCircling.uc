//=============================================================================
//=============================================================================

class VoldEffectCircling expands ParticleFX;

var()   float  fRadius;
var()   float  fRadiusIncSpeed;
var     float  YawFromOwner;
var()   float  YawSpeed;
var     vector vStartLoc;
var()   float  zIncSpeed;
var     float  z;

event PostBeginPlay()
{
	vStartLoc = Location;
	z = Location.z + FRand()*40 - 20;
	YawFromOwner = FRand() * 65536;
	YawSpeed *= 0.8 + FRand()*0.4;
	if( Rand(2) == 0 )
		YawSpeed = -YawSpeed;
}

function Tick(float dtime)
{
	local Rotator r;

	YawFromOwner += YawSpeed * dtime;
	fRadius += fRadiusIncSpeed * dtime;	
	zIncSpeed += 20 * dtime;
	z += zIncSpeed * dtime;

	r.yaw = YawFromOwner;

	SetLocation( vStartLoc + vector( r ) * fRadius + vec(0,0,z) );
}

defaultproperties
{
     ParticlesPerSec=(Base=10,Rand=4)
     speed=(Base=30)
     Lifetime=(Base=0.5,Rand=0.1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=128,B=128))
     SizeEndScale=(Base=0,Rand=18)
     Chaos=1
     Distribution=DIST_Uniform
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare5'
     bDynamicLight=false

	 LifeSpan=10

	 fRadius=30
	 fRadiusIncSpeed=20
	 YawSpeed=90000
	 zIncSpeed=30

}

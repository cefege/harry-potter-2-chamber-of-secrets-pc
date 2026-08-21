//=============================================================================
//=============================================================================

class VoldEffectLob expands ParticleFX;

#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPparticle.hp_fx
#exec OBJ LOAD FILE=..\textures\Particles.utx PACKAGE=HPparticle.particle_fx

//    Textures(0)=Texture'HPParticle.hp_fx.Particles.rep_p'

/*    ColorStart=(Base=(R=255,G=255,B=255),Max=(R=0,G=0,B=0),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=0,G=0,B=255))
*/

/*    ColorStart=(Base=(R=128,G=128,B=128),Max=(R=0,G=0,B=0),Rand=(R=0,G=0,B=0))
    ColorEnd=(Base=(R=128,G=128,B=128))
    Textures(0)=Texture'HPParticle.hp_fx.Particles.SilverSparkle01'
*/

var()   float  fFlySpeed;
var()   float  fGravityEffect;

event PostBeginPlay()
{
	local vector  v;
	local rotator r;

	r.Yaw = FRand() * 65536;
	r.Pitch = 9000.0  +  FRand()*6000 - 3000.0;

	v = vector( r ) * fFlySpeed;

	Velocity = v;
	//Acceleration = vect(0,0,-20);

	//SetPhysics( PHYS_Flying );
}

function Tick(float dtime)
{
	velocity.z += -fGravityEffect * dtime;
	SetLocation( Location + velocity*dtime );
}


defaultproperties
{
     ParticlesPerSec=(Base=80)
     SourceWidth=(Base=2)
     SourceHeight=(Base=2)
     AngularSpreadWidth=(Base=180)
     AngularSpreadHeight=(Base=180)
     Speed=(Base=25)
     Lifetime=(Rand=1)
     ColorStart=(Base=(G=255,B=255))
     ColorEnd=(Base=(R=30,B=255))
     SizeWidth=(Base=3,Rand=6)
     SizeLength=(Base=3,Rand=6)
     SizeEndScale=(Base=5,Rand=8)
     SpinRate=(Base=-6,Rand=12)
     Attraction=(X=5,Y=5,Z=5)
     Textures(0)=Texture'HPParticle.hp_fx.Spells.Les_BlueSmoke'
     LastUpdateLocation=(Y=20,Z=32)
     LastEmitLocation=(Y=20,Z=32)
     EmissionResidue=0.06977463
     Age=434.0083
     ParticlesEmitted=88822
     Tag=Flip_fly
     Location=(Y=20,Z=32)
     Rotation=(Pitch=0)

	bCollideActors=false;
	bCollideWorld=false;
	bBlockActors=false;
	bBlockPlayers=false;

	LifeSpan=2

	fFlySpeed=450
	fGravityEffect=500
}

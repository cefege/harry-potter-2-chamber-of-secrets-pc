//===============================================================================
//  [skJellybean] 
//===============================================================================

class skJellybean extends HPMeshActor;

#exec MESH  MODELIMPORT MESH=skJellybeanMesh MODELFILE=models\skJellybean.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skJellybeanMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skJellybeanAnims ANIMFILE=models\skJellybean.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skJellybeanMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skJellybeanMesh ANIM=skJellybeanAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skJellybeanAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skJellybeanTex0  FILE=TEXTURES\jelybean_64.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeanBlackTex0  FILE=TEXTURES\BlackBean_64.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeanPurpleTex0  FILE=TEXTURES\Prupbean_64.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeanRedTex0  FILE=TEXTURES\jelybean_64.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeanDarkGreenTex0  FILE=TEXTURES\Drkgrenbean_64.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeanBogieTex0  FILE=TEXTURES\SnotBean_64.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBeanBlueSpotTex0  FILE=TEXTURES\BlueSpotBean_64.bmp  GROUP=Skins

//#EXEC MESHMAP SETTEXTURE MESHMAP=skJellybeanMesh NUM=0 TEXTURE=skJellybeanTex0




//#exec OBJ LOAD FILE=..\textures\HP_FX.utx PACKAGE=HPBase.FXPackage


//#EXEC MESHMAP SETTEXTURE MESHMAP=skJellybeanMesh NUM=0 TEXTURE=HPBase.FXPackage.jelly1

/*
var()	Sound good, bad;
//var actor jellyparticle;
var float fPickupFlyTime;

var bool	bTiming;
var float	fTimeout;

function PreBeginPlay()
{

//	jellyparticle=spawn(class'jellyglow');

	Super.PreBeginPlay();
}

function Spawned()
{
	SetPhysics(PHYS_Falling);
	bTiming = false;
}
//
//function PostBeginPlay()
//{
//	SetPhysics(PHYS_None);
//	Super.PostBeginPlay();
//}
//
function touch (actor other)
{

	if (other.IsA('Tut1Gnome'))
	{
		// AE:
		PlaySound(sound'HPSounds.magic_sfx.pickup11');
		Destroy();
	}

	if(other==playerharry && GetStateName()!='killbean')
	{
		gotostate('killbean');

	}


}

state deadbean
{

	begin:
	bhidden=true;
	setcollision(false,false,false);
	beanloop:
	sleep (1);
	goto 'beanloop';



}

auto state beano
{
	function BeginState()
	{
		bTiming = false;
		fTimeout = 5.0;
	}

	function tick(float deltatime)
	{
		local Rotator	NewRotation;

		NewRotation = Rotation;
		NewRotation.Yaw += (30000 * deltatime);
		NewRotation.Yaw = NewRotation.Yaw & 0xffff;

		SetRotation(NewRotation);
		if(vsize(location-playerharry.location)<60)
		{
			gotostate('killbean');
		}

		if (bTiming)
		{
			fTimeout -= deltatime;
		}
	}

	function HitWall( vector HitNormal, actor Wall )
	{
		Velocity *= 0.5;
		Velocity = MirrorVectorByNormal( Velocity, HitNormal );

		bTiming = true;
		if (bTiming && fTimeout >= 0)
		{
			if (abs(Velocity.z) > 5)
			{
				playsound(sound'HPSounds.Magic_sfx.bean_bounce');
			}
		}
	}

	begin:
	loop:
		sleep(1);
		goto 'loop';


}


state killbean
{
	ignores touch;

	event tick(float delta)
		{
		local vector dest;
		fPickupFlyTime-=delta;

		Move((playerharry.CameraToWorld(vect(0.75,0.75,150))-location)/(fPickupFlyTime/delta));
//		Move((playerharry.CameraToWorld(vect(0.75,0.75,150))-location)/5);
		}

	begin:
//		disable('touch');

		// AE:
		PlaySound(sound'HPSounds.magic_sfx.pickup11');
		bCollideWorld=false;
		fPickupFlyTime=0.25;
		playerHarry.AddBeans(1);
		while(fPickupFlyTime>0)
			{
			sleep(0.1);
			}
		destroy();
}

d e f a u l t p r o p e r t i e s
{
    Mesh=skJellybeanMesh
    DrawType=DT_Mesh
    CollisionRadius=10
    CollisionHeight=10
    bStatic=false
	bcollideactors=true
	bcollideworld=true
	physics=phys_walking
	bdobob=true
	ambientglow=200
    bBounce=True
}

*/
defaultproperties
{
    Mesh=skJellybeanMesh
    DrawType=DT_Mesh
    bStatic=false
}

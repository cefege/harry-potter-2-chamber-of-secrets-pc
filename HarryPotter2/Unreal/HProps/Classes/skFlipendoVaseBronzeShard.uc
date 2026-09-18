//===============================================================================
//  [skFlipendoVaseBronzeShard] 
//===============================================================================

class skFlipendoVaseBronzeShard extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFlipendoVaseBronzeShardMesh MODELFILE=models\skFlipendoVaseBronzeShard.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlipendoVaseBronzeShardMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFlipendoVaseBronzeShardAnims ANIMFILE=models\skFlipendoVaseBronzeShard.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFlipendoVaseBronzeShardMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlipendoVaseBronzeShardMesh ANIM=skFlipendoVaseBronzeShardAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFlipendoVaseBronzeShardAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFlipendoVaseBronzeShardTex0  FILE=TEXTURES\fvbrzbrk_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlipendoVaseBronzeShardMesh NUM=0 TEXTURE=skFlipendoVaseBronzeShardTex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: fvbrzbrk_64.bmp  Path: D:\Harry Potter\Art\Objects\Flipendo Vases 
/*
var rotator randrot;
var bool tickOn;


auto state fall
{



function tick(float deltaTime)
{
	local vector loc;
	loc=location;
	if(tickOn)
	{
	//	if(fasttrace(velocity*deltaTime))
			move(velocity*deltaTime);
		if(vsize(loc-location)>0.1)
		{
			velocity.z=velocity.z-(100*deltatime);
			setrotation(rotation+randrot);	
		}
	}



}


function touch(actor other)
{

	if(other==playerHarry)
		destroy();

}

function initfall()
{
	local rotator randx;
		randrot=rotrand();
		randx=rotation;
		randx.yaw=randx.yaw+rand(20000);
		randx.yaw=randx.yaw-10000;
		velocity=normal(vector(randx))*10;
		velocity.z=30+rand(100);	
	

}

	begin:
	
		initfall();
		tickOn=true;

	loop:
		sleep(5.5);
		tickon=false;
		goto 'loop';
		destroy();




}
d e f a u l t p r o p e r t i e s
{
    Mesh=skFlipendoVaseBronzeShardMesh
    DrawType=DT_Mesh
	bcollideworld=true
    bStatic=False
	Physics=PHYS_Flying
	drawscale=1
	collisionheight=1
	collisionradius=1
	tickOn=false

}
*/
defaultproperties
{
    Mesh=skFlipendoVaseBronzeShardMesh
    DrawType=DT_Mesh
    bStatic=False
}

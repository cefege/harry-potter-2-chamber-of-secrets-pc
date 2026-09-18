//===============================================================================
//  [skFlipendoVaseBronze] 
//===============================================================================

class skFlipendoVaseBronze extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFlipendoVaseBronzeMesh MODELFILE=models\skFlipendoVaseBronze.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFlipendoVaseBronzeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFlipendoVaseBronzeAnims ANIMFILE=models\skFlipendoVaseBronze.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFlipendoVaseBronzeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFlipendoVaseBronzeMesh ANIM=skFlipendoVaseBronzeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFlipendoVaseBronzeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFlipendoVaseBronzeTex0  FILE=TEXTURES\fvasebrz_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFlipendoVaseBronzeMesh NUM=0 TEXTURE=skFlipendoVaseBronzeTex0

// Original material [0] is [Material #9] SkinIndex: 0 Bitmap: fvasebrz_64.bmp  Path: D:\Harry Potter\Art\Objects\Flipendo Vases 
/*
var class<Actor> brokentype;

auto state waitforspell
{

function Trigger( actor Other, pawn EventInstigator )
{
	gotostate('break');
}

function bool TakeSpellEffect(baseSpell spell)
{
local vector spawnLoc;
local actor newSpawn;
	if(spell.class==class'spellflip')
		{
			gotostate('break');
			return true;
		}
}


	begin:
	//	SetPhysics(PHYS_walking);
	loop:
		sleep(1);
		goto 'loop';


}


state break
{

function generateobject()
{
	local vector dir;
	local vector vel;
	local actor newspawn;
	local rotator newrot;
		
	dir.x=20;
	dir.y=0;
	dir.z=0;
	dir=dir>>rotation;
	vel = dir * 4;
	dir=dir+location;

	playsound(sound'HPSounds.Hub1_sfx.vase_breaking');
	newspawn=spawn(class'avifors_hit',,,location,rot(0,0,0));
	newSpawn=Spawn(transformInto,,,dir);

	if (newspawn.isa('jellybean'))
	{
		// Special case with beans, let them spill out		
		newSpawn.Velocity = vel;
	}

	newrot=rotation;
	newrot.yaw=newrot.yaw+32000;
	newspawn=spawn(brokentype,,,location,newrot);
	dir.x=100;
	dir.y=0;
	dir.z=0;
	dir=dir>>rotation;
	dir=dir+location;

	newspawn=spawn(class'skFlipendoVaseBronzeShard',,,dir,rotation);
	newspawn=spawn(class'skFlipendoVaseBronzeShard',,,dir,rotation);
	newspawn=spawn(class'skFlipendoVaseBronzeShard',,,dir,rotation);
	newspawn=spawn(class'skFlipendoVaseBronzeShard',,,dir,rotation);
	newspawn=spawn(class'skFlipendoVaseBronzeShard',,,dir,rotation);
	newspawn=spawn(class'skFlipendoVaseBronzeShard',,,dir,rotation);



	
	
}




	begin:
	generateobject();
	destroy();
	loop:
	sleep(1.5);
	
	goto 'loop';


}

d e f a u l t p r o p e r t i e s
{
     brokentype=Class'HProps.skFlipendoVaseBronzeBroken'
     bStatic=False
     eVulnerableToSpell=SPELL_Flipendo
     SizeModifier=1.5
     CentreOffset=(Z=25)
     bDirectional=True
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skFlipendoVaseBronzeMesh'
     bCollideWorld=True
     bBlockPlayers=True
     bProjTarget=True
}
*/

defaultproperties
{
     Mesh=skFlipendoVaseBronzeMesh
}

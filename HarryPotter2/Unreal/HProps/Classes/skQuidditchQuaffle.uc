//===============================================================================
//  [skQuidditchQuaffle] 
//===============================================================================

class skQuidditchQuaffle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skQuidditchQuaffleMesh MODELFILE=models\skQuidditchQuaffle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skQuidditchQuaffleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skQuidditchQuaffleAnims ANIMFILE=models\skQuidditchQuaffle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skQuidditchQuaffleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skQuidditchQuaffleMesh ANIM=skQuidditchQuaffleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skQuidditchQuaffleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skQuidditchQuaffleTex0  FILE=TEXTURES\qquaffle_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skQuidditchQuaffleMesh NUM=0 TEXTURE=skQuidditchQuaffleTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: qquaffle_128.bmp  Path: D:\Harry Potter\Art\Objects\Qudditch 


defaultproperties
{
    Mesh=skQuidditchQuaffleMesh
    DrawType=DT_Mesh
    bStatic=False
}


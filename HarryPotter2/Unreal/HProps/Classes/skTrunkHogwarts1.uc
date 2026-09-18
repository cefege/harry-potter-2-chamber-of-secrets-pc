//===============================================================================
//  [skTrunkHogwarts1] 
//===============================================================================

class skTrunkHogwarts1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTrunkHogwarts1Mesh MODELFILE=models\skTrunkHogwarts1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTrunkHogwarts1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTrunkHogwarts1Anims ANIMFILE=models\skTrunkHogwarts1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTrunkHogwarts1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTrunkHogwarts1Mesh ANIM=skTrunkHogwarts1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTrunkHogwarts1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTrunkHogwarts1Tex0  FILE=TEXTURES\hogtrunk_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTrunkHogwarts1Mesh NUM=0 TEXTURE=skTrunkHogwarts1Tex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: hogtrunk_128.bmp  Path: C:\Harry Potter\ART\Objects\Chests_Boxes_Trunks\Hogwarts Trunk 


defaultproperties
{
    Mesh=skTrunkHogwarts1Mesh
    DrawType=DT_Mesh
    bStatic=False
}


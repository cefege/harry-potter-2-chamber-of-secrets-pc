//===============================================================================
//  [skBottlePotionGreen2] 
//===============================================================================

class skBottlePotionGreen2 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBottlePotionGreen2Mesh MODELFILE=models\skBottlePotionGreen2.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBottlePotionGreen2Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBottlePotionGreen2Anims ANIMFILE=models\skBottlePotionGreen2.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBottlePotionGreen2Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBottlePotionGreen2Mesh ANIM=skBottlePotionGreen2Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBottlePotionGreen2Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBottlePotionGreen2Tex0  FILE=TEXTURES\greenbtl_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBottlePotionGreen2Mesh NUM=0 TEXTURE=skBottlePotionGreen2Tex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: greenbtl_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Potions Puzzle Bottles\Green 


defaultproperties
{
    Mesh=skBottlePotionGreen2Mesh
    DrawType=DT_Mesh
    bStatic=False
}


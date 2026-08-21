//===============================================================================
//  [skBottlePotionBlue] 
//===============================================================================

class skBottlePotionBlue extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBottlePotionBlueMesh MODELFILE=models\skBottlePotionBlue.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBottlePotionBlueMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBottlePotionBlueAnims ANIMFILE=models\skBottlePotionBlue.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBottlePotionBlueMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBottlePotionBlueMesh ANIM=skBottlePotionBlueAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBottlePotionBlueAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBottlePotionBlueTex0  FILE=TEXTURES\bluebotl_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBottlePotionBlueMesh NUM=0 TEXTURE=skBottlePotionBlueTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: bluebotl_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Potions Puzzle Bottles\Blue 


defaultproperties
{
    Mesh=skBottlePotionBlueMesh
    DrawType=DT_Mesh
    bStatic=False
}


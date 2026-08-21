//===============================================================================
//  [skBottlePotionEmpty] 
//===============================================================================

class skBottlePotionEmpty extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBottlePotionEmptyMesh MODELFILE=models\skBottlePotionEmpty.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBottlePotionEmptyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBottlePotionEmptyAnims ANIMFILE=models\skBottlePotionEmpty.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBottlePotionEmptyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBottlePotionEmptyMesh ANIM=skBottlePotionEmptyAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBottlePotionEmptyAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBottlePotionEmptyTex0  FILE=TEXTURES\emptybtl_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBottlePotionEmptyMesh NUM=0 TEXTURE=skBottlePotionEmptyTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: emptybtl_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Potions Puzzle Bottles\Gray_Empty 


defaultproperties
{
    Mesh=skBottlePotionEmptyMesh
    DrawType=DT_Mesh
    bStatic=False
}


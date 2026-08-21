//===============================================================================
//  [skBottlePotionOrange] 
//===============================================================================

class skBottlePotionOrange extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBottlePotionOrangeMesh MODELFILE=models\skBottlePotionOrange.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBottlePotionOrangeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBottlePotionOrangeAnims ANIMFILE=models\skBottlePotionOrange.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBottlePotionOrangeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBottlePotionOrangeMesh ANIM=skBottlePotionOrangeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBottlePotionOrangeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBottlePotionOrangeTex0  FILE=TEXTURES\orangebt_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBottlePotionOrangeMesh NUM=0 TEXTURE=skBottlePotionOrangeTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: orangebt_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Potions Puzzle Bottles\Orange 


defaultproperties
{
    Mesh=skBottlePotionOrangeMesh
    DrawType=DT_Mesh
    bStatic=False
}


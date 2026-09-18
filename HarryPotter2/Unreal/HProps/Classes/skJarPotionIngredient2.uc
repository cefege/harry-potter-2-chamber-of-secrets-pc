//===============================================================================
//  [skJarPotionIngredient2] 
//===============================================================================

class skJarPotionIngredient2 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skJarPotionIngredient2Mesh MODELFILE=models\skJarPotionIngredient2.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skJarPotionIngredient2Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skJarPotionIngredient2Anims ANIMFILE=models\skJarPotionIngredient2.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skJarPotionIngredient2Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skJarPotionIngredient2Mesh ANIM=skJarPotionIngredient2Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skJarPotionIngredient2Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skJarPotionIngredient2Tex0  FILE=TEXTURES\ptionjr1_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skJarPotionIngredient2Mesh NUM=0 TEXTURE=skJarPotionIngredient2Tex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: ptionjr1_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Tall Ingredient Jar 


defaultproperties
{
    Mesh=skJarPotionIngredient2Mesh
    DrawType=DT_Mesh
    bStatic=False
}


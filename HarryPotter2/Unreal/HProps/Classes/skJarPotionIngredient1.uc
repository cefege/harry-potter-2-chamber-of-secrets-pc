//===============================================================================
//  [skJarPotionIngredient1] 
//===============================================================================

class skJarPotionIngredient1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skJarPotionIngredient1Mesh MODELFILE=models\skJarPotionIngredient1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skJarPotionIngredient1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skJarPotionIngredient1Anims ANIMFILE=models\skJarPotionIngredient1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skJarPotionIngredient1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skJarPotionIngredient1Mesh ANIM=skJarPotionIngredient1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skJarPotionIngredient1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skJarPotionIngredient1Tex0  FILE=TEXTURES\ptionjr2_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skJarPotionIngredient1Mesh NUM=0 TEXTURE=skJarPotionIngredient1Tex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: ptionjr2_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Short Ingredient Jar 


defaultproperties
{
    Mesh=skJarPotionIngredient1Mesh
    DrawType=DT_Mesh
    bStatic=False
}


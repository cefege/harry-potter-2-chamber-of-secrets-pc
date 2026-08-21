//===============================================================================
//  [skJarPotionGreen] 
//===============================================================================

class skJarPotionGreen extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skJarPotionGreenMesh MODELFILE=models\skJarPotionGreen.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skJarPotionGreenMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skJarPotionGreenAnims ANIMFILE=models\skJarPotionGreen.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skJarPotionGreenMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skJarPotionGreenMesh ANIM=skJarPotionGreenAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skJarPotionGreenAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skJarPotionGreenTex0  FILE=TEXTURES\ptionjr3_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skJarPotionGreenMesh NUM=0 TEXTURE=skJarPotionGreenTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: ptionjr3_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Geen Ingredient Jar 


defaultproperties
{
    Mesh=skJarPotionGreenMesh
    DrawType=DT_Mesh
    bStatic=False
}


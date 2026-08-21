//===============================================================================
//  [skJarPotionBrown] 
//===============================================================================

class skJarPotionBrown extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skJarPotionBrownMesh MODELFILE=models\skJarPotionBrown.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skJarPotionBrownMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skJarPotionBrownAnims ANIMFILE=models\skJarPotionBrown.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skJarPotionBrownMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skJarPotionBrownMesh ANIM=skJarPotionBrownAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skJarPotionBrownAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skJarPotionBrownTex0  FILE=TEXTURES\ptionbot_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skJarPotionBrownMesh NUM=0 TEXTURE=skJarPotionBrownTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: ptionbot_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Brown Ingredient Bottle 


defaultproperties
{
    Mesh=skJarPotionBrownMesh
    DrawType=DT_Mesh
    bStatic=False
}


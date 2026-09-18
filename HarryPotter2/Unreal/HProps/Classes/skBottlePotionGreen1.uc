//===============================================================================
//  [skBottlePotionGreen1] 
//===============================================================================

class skBottlePotionGreen1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBottlePotionGreen1Mesh MODELFILE=models\skBottlePotionGreen1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBottlePotionGreen1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBottlePotionGreen1Anims ANIMFILE=models\skBottlePotionGreen1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBottlePotionGreen1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBottlePotionGreen1Mesh ANIM=skBottlePotionGreen1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBottlePotionGreen1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBottlePotionGreen1Tex0  FILE=TEXTURES\greenbot_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBottlePotionGreen1Mesh NUM=0 TEXTURE=skBottlePotionGreen1Tex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: greenbot_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Green Life Potion Bottle 


defaultproperties
{
    Mesh=skBottlePotionGreen1Mesh
    DrawType=DT_Mesh
    bStatic=False
}


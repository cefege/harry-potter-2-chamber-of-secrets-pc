//===============================================================================
//  [skBottlePotionPurple] 
//===============================================================================

class skBottlePotionPurple extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBottlePotionPurpleMesh MODELFILE=models\skBottlePotionPurple.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBottlePotionPurpleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBottlePotionPurpleAnims ANIMFILE=models\skBottlePotionPurple.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBottlePotionPurpleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBottlePotionPurpleMesh ANIM=skBottlePotionPurpleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBottlePotionPurpleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBottlePotionPurpleTex0  FILE=TEXTURES\purplebt_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBottlePotionPurpleMesh NUM=0 TEXTURE=skBottlePotionPurpleTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: purplebt_128.bmp  Path: C:\Harry Potter\ART\Objects\Bottles_Jars\Potions Puzzle Bottles\Purple 


defaultproperties
{
    Mesh=skBottlePotionPurpleMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skDishesHagridTeaPot] 
//===============================================================================

class skDishesHagridTeaPot extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDishesHagridTeaPotMesh MODELFILE=models\skDishesHagridTeaPot.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDishesHagridTeaPotMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDishesHagridTeaPotAnims ANIMFILE=models\skDishesHagridTeaPot.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDishesHagridTeaPotMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDishesHagridTeaPotMesh ANIM=skDishesHagridTeaPotAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDishesHagridTeaPotAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDishesHagridTeaPotTex0  FILE=TEXTURES\hgteapot_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skDishesHagridTeaPotTex1  FILE=TEXTURES\hgteapot_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDishesHagridTeaPotMesh NUM=0 TEXTURE=skDishesHagridTeaPotTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skDishesHagridTeaPotMesh NUM=1 TEXTURE=skDishesHagridTeaPotTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: hgteapot_128.bmp  Path: C:\Harry Potter\ART\Objects\Dishes_Cups_Pots_Utensiles\Hagrids Teapot 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: hgteapot_128.bmp  Path: C:\Harry Potter\ART\Objects\Dishes_Cups_Pots_Utensiles\Hagrids Teapot 


defaultproperties
{
    Mesh=skDishesHagridTeaPotMesh
    DrawType=DT_Mesh
    bStatic=False
}


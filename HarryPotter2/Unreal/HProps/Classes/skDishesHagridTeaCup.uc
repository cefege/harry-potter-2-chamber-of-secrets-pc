//===============================================================================
//  [skDishesHagridTeaCup] 
//===============================================================================

class skDishesHagridTeaCup extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDishesHagridTeaCupMesh MODELFILE=models\skDishesHagridTeaCup.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDishesHagridTeaCupMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDishesHagridTeaCupAnims ANIMFILE=models\skDishesHagridTeaCup.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDishesHagridTeaCupMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDishesHagridTeaCupMesh ANIM=skDishesHagridTeaCupAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDishesHagridTeaCupAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDishesHagridTeaCupTex0  FILE=TEXTURES\hgteacup_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skDishesHagridTeaCupTex1  FILE=TEXTURES\hgteacup_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDishesHagridTeaCupMesh NUM=0 TEXTURE=skDishesHagridTeaCupTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skDishesHagridTeaCupMesh NUM=1 TEXTURE=skDishesHagridTeaCupTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: hgteacup_128.bmp  Path: C:\Harry Potter\ART\Objects\Dishes_Cups_Pots_Utensiles\Hagrids Teacup 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: hgteacup_128.bmp  Path: C:\Harry Potter\ART\Objects\Dishes_Cups_Pots_Utensiles\Hagrids Teacup 


defaultproperties
{
    Mesh=skDishesHagridTeaCupMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skTreeSprite2] 
//===============================================================================

class skTreeSprite2 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTreeSprite2Mesh MODELFILE=models\skTreeSprite2.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTreeSprite2Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTreeSprite2Anims ANIMFILE=models\skTreeSprite2.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTreeSprite2Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTreeSprite2Mesh ANIM=skTreeSprite2Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTreeSprite2Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTreeSprite2Tex0  FILE=TEXTURES\TreeSprite02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTreeSprite2Mesh NUM=0 TEXTURE=skTreeSprite2Tex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: TreeSprite02.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Tree Sprites 


defaultproperties
{
    Mesh=skTreeSprite2Mesh
    DrawType=DT_Mesh
    bStatic=False
}


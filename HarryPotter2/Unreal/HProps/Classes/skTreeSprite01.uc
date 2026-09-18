//===============================================================================
//  [skTreeSprite01] 
//===============================================================================

class skTreeSprite01 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTreeSprite01Mesh MODELFILE=models\skTreeSprite01.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTreeSprite01Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTreeSprite01Anims ANIMFILE=models\skTreeSprite01.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTreeSprite01Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTreeSprite01Mesh ANIM=skTreeSprite01Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTreeSprite01Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTreeSprite01Tex0  FILE=TEXTURES\TreeSprite01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTreeSprite01Mesh NUM=0 TEXTURE=skTreeSprite01Tex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: TreeSprite01.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Tree Sprites 


defaultproperties
{
    Mesh=skTreeSprite01Mesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skTreeSprite05] 
//===============================================================================

class skTreeSprite05 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTreeSprite05Mesh MODELFILE=models\skTreeSprite05.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTreeSprite05Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTreeSprite05Anims ANIMFILE=models\skTreeSprite05.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTreeSprite05Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTreeSprite05Mesh ANIM=skTreeSprite05Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTreeSprite05Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTreeSprite05Tex0  FILE=TEXTURES\TreeSprite05.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTreeSprite05Mesh NUM=0 TEXTURE=skTreeSprite05Tex0

// Original material [0] is [skin00.TWOSIDED] SkinIndex: 0 Bitmap: TreeSprite05.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Tree Sprites 


defaultproperties
{
    Mesh=skTreeSprite05Mesh
    DrawType=DT_Mesh
    bStatic=False
}


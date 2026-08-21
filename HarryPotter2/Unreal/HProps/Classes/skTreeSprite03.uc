//===============================================================================
//  [skTreeSprite03] 
//===============================================================================

class skTreeSprite03 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTreeSprite03Mesh MODELFILE=models\skTreeSprite03.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTreeSprite03Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTreeSprite03Anims ANIMFILE=models\skTreeSprite03.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTreeSprite03Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTreeSprite03Mesh ANIM=skTreeSprite03Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTreeSprite03Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTreeSprite03Tex0  FILE=TEXTURES\TreeSprite03.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTreeSprite03Mesh NUM=0 TEXTURE=skTreeSprite03Tex0

// Original material [0] is [skin00.TWOSIDED] SkinIndex: 0 Bitmap: TreeSprite03.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Tree Sprites 


defaultproperties
{
    Mesh=skTreeSprite03Mesh
    DrawType=DT_Mesh
    bStatic=False
}


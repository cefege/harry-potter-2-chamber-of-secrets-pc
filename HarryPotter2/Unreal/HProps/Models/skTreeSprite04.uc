//===============================================================================
//  [skTreeSprite04] 
//===============================================================================

class skTreeSprite04 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTreeSprite04Mesh MODELFILE=models\skTreeSprite04.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTreeSprite04Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTreeSprite04Anims ANIMFILE=models\skTreeSprite04.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTreeSprite04Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTreeSprite04Mesh ANIM=skTreeSprite04Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTreeSprite04Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTreeSprite04Tex0  FILE=TEXTURES\TreeSprite04.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTreeSprite04Mesh NUM=0 TEXTURE=skTreeSprite04Tex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: TreeSprite04.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Tree Sprites 


defaultproperties
{
    Mesh=skTreeSprite04Mesh
    DrawType=DT_Mesh
    bStatic=False
}


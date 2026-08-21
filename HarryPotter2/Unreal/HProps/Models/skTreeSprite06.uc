//===============================================================================
//  [skTreeSprite06] 
//===============================================================================

class skTreeSprite06 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTreeSprite06Mesh MODELFILE=models\skTreeSprite06.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTreeSprite06Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTreeSprite06Anims ANIMFILE=models\skTreeSprite06.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTreeSprite06Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTreeSprite06Mesh ANIM=skTreeSprite06Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTreeSprite06Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTreeSprite06Tex0  FILE=TEXTURES\TreeSprite06.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTreeSprite06Mesh NUM=0 TEXTURE=skTreeSprite06Tex0

// Original material [0] is [SKIN00.TWOSIDED] SkinIndex: 0 Bitmap: TreeSprite06.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Tree Sprites 


defaultproperties
{
    Mesh=skTreeSprite06Mesh
    DrawType=DT_Mesh
    bStatic=False
}


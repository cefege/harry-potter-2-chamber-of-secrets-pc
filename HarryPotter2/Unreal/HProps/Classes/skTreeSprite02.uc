//===============================================================================
//  [skTreeSprite02] 
//===============================================================================

class skTreeSprite02 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTreeSprite02Mesh MODELFILE=models\skTreeSprite02.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTreeSprite02Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTreeSprite02Anims ANIMFILE=models\skTreeSprite02.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTreeSprite02Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTreeSprite02Mesh ANIM=skTreeSprite02Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTreeSprite02Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTreeSprite02Tex0  FILE=TEXTURES\TreeSprite02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTreeSprite02Mesh NUM=0 TEXTURE=skTreeSprite02Tex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: TreeSprite02.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Tree Sprites 


defaultproperties
{
    Mesh=skTreeSprite02Mesh
    DrawType=DT_Mesh
    bStatic=False
}


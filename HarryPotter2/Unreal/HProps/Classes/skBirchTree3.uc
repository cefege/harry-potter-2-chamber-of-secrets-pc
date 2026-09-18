//===============================================================================
//  [skBirchTree3] 
//===============================================================================

class skBirchTree3 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBirchTree3Mesh MODELFILE=models\skBirchTree3.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBirchTree3Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBirchTree3Anims ANIMFILE=models\skBirchTree3.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBirchTree3Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBirchTree3Mesh ANIM=skBirchTree3Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBirchTree3Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBirchTree3Tex0  FILE=TEXTURES\BirchBark2.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBirchTree3Tex1  FILE=TEXTURES\BirchTreeBranches.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBirchTree3Mesh NUM=0 TEXTURE=skBirchTree3Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skBirchTree3Mesh NUM=1 TEXTURE=skBirchTree3Tex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: BirchBark2.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Birch Tree 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: BirchTreeBranches.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Birch Tree 


defaultproperties
{
    Mesh=skBirchTree3Mesh
    DrawType=DT_Mesh
    bStatic=False
}


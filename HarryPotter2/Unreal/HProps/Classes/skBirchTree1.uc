//===============================================================================
//  [skBirchTree1] 
//===============================================================================

class skBirchTree1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBirchTree1Mesh MODELFILE=models\skBirchTree1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBirchTree1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBirchTree1Anims ANIMFILE=models\skBirchTree1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBirchTree1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBirchTree1Mesh ANIM=skBirchTree1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBirchTree1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBirchTree1Tex0  FILE=TEXTURES\BirchBark.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBirchTree1Tex1  FILE=TEXTURES\BirchtreeBows.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBirchTree1Mesh NUM=0 TEXTURE=skBirchTree1Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skBirchTree1Mesh NUM=1 TEXTURE=skBirchTree1Tex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: BirchBark.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Birch Tree 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: BirchtreeBows.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Birch Tree 


defaultproperties
{
    Mesh=skBirchTree1Mesh
    DrawType=DT_Mesh
    bStatic=False
}


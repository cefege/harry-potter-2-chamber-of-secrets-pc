//===============================================================================
//  [skMapleTreeFull] 
//===============================================================================

class skMapleTreeFull extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skMapleTreeFullMesh MODELFILE=models\skMapleTreeFull.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skMapleTreeFullMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skMapleTreeFullAnims ANIMFILE=models\skMapleTreeFull.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skMapleTreeFullMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skMapleTreeFullMesh ANIM=skMapleTreeFullAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skMapleTreeFullAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skMapleTreeFullTex0  FILE=TEXTURES\MapletreeBark.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skMapleTreeFullTex1  FILE=TEXTURES\MapleTreeCanopy.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skMapleTreeFullTex2  FILE=TEXTURES\MapleTreeBranches.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skMapleTreeFullMesh NUM=0 TEXTURE=skMapleTreeFullTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skMapleTreeFullMesh NUM=1 TEXTURE=skMapleTreeFullTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skMapleTreeFullMesh NUM=2 TEXTURE=skMapleTreeFullTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: MapletreeBark.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Maple Tree 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: MapleTreeCanopy.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Maple Tree 
// Original material [2] is [SKIN02.TWOSIDED] SkinIndex: 2 Bitmap: MapleTreeBranches.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Maple Tree 


defaultproperties
{
    Mesh=skMapleTreeFullMesh
    DrawType=DT_Mesh
    bStatic=False
}


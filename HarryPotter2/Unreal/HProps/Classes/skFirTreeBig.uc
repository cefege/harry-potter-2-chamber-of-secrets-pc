//===============================================================================
//  [skFirTreeBig] 
//===============================================================================

class skFirTreeBig extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFirTreeBigMesh MODELFILE=models\skFirTreeBig.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFirTreeBigMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFirTreeBigAnims ANIMFILE=models\skFirTreeBig.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFirTreeBigMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFirTreeBigMesh ANIM=skFirTreeBigAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFirTreeBigAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFirTreeBigTex0  FILE=TEXTURES\FirtreeBark.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFirTreeBigTex1  FILE=TEXTURES\FirtreeBows.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFirTreeBigMesh NUM=0 TEXTURE=skFirTreeBigTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFirTreeBigMesh NUM=1 TEXTURE=skFirTreeBigTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: FirtreeBark.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Fir Tree 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: FirtreeBows.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Fir Tree 


defaultproperties
{
    Mesh=skFirTreeBigMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skFirTreeThick] 
//===============================================================================

class skFirTreeThick extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFirTreeThickMesh MODELFILE=models\skFirTreeThick.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFirTreeThickMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFirTreeThickAnims ANIMFILE=models\skFirTreeThick.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFirTreeThickMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFirTreeThickMesh ANIM=skFirTreeThickAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFirTreeThickAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFirTreeThickTex0  FILE=TEXTURES\FirtreeBark.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFirTreeThickTex1  FILE=TEXTURES\FirtreeBows.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFirTreeThickMesh NUM=0 TEXTURE=skFirTreeThickTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFirTreeThickMesh NUM=1 TEXTURE=skFirTreeThickTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: FirtreeBark.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Fir Tree 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: FirtreeBows.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Fir Tree 


defaultproperties
{
    Mesh=skFirTreeThickMesh
    DrawType=DT_Mesh
    bStatic=False
}


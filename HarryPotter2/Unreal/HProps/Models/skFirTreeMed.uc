//===============================================================================
//  [skFirTreeMed] 
//===============================================================================

class skFirTreeMed extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFirTreeMedMesh MODELFILE=models\skFirTreeMed.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFirTreeMedMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFirTreeMedAnims ANIMFILE=models\skFirTreeMed.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFirTreeMedMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFirTreeMedMesh ANIM=skFirTreeMedAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFirTreeMedAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFirTreeMedTex0  FILE=TEXTURES\FirtreeBark.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFirTreeMedTex1  FILE=TEXTURES\FirtreeBows.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFirTreeMedMesh NUM=0 TEXTURE=skFirTreeMedTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFirTreeMedMesh NUM=1 TEXTURE=skFirTreeMedTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: FirtreeBark.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Fir Tree 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: FirtreeBows.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Plants_Trees_Plant Pots_Seeds\Trees\Fir Tree 


defaultproperties
{
    Mesh=skFirTreeMedMesh
    DrawType=DT_Mesh
    bStatic=False
}


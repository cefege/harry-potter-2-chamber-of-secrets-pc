//===============================================================================
//  [skPlantsEmptyPot] 
//===============================================================================

class skPlantsEmptyPot extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skPlantsEmptyPotMesh MODELFILE=models\skPlantsEmptyPot.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPlantsEmptyPotMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPlantsEmptyPotAnims ANIMFILE=models\skPlantsEmptyPot.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPlantsEmptyPotMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPlantsEmptyPotMesh ANIM=skPlantsEmptyPotAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPlantsEmptyPotAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPlantsEmptyPotTex0  FILE=TEXTURES\emptypot_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPlantsEmptyPotMesh NUM=0 TEXTURE=skPlantsEmptyPotTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: emptypot_128.bmp  Path: C:\Harry Potter\ART\Objects\Plants_Trees_Plant Pots_Seeds\Terra Cotta Pots 


defaultproperties
{
    Mesh=skPlantsEmptyPotMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skDresser] 
//===============================================================================

class skDresser extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDresserMesh MODELFILE=models\skDresser.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDresserMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDresserAnims ANIMFILE=models\skDresser.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDresserMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDresserMesh ANIM=skDresserAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDresserAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDresserTex0  FILE=TEXTURES\DursleyDresser_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDresserMesh NUM=0 TEXTURE=skDresserTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: DursleyDresser_128.bmp  Path: C:\HP2_master\PD_cutscene\psd 


defaultproperties
{
    Mesh=skDresserMesh
    DrawType=DT_Mesh
    bStatic=False
}


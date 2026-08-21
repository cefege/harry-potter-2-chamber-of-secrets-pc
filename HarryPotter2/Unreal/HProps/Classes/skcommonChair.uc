//===============================================================================
//  [skcommonChair] 
//===============================================================================

class skcommonChair extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skcommonChairMesh MODELFILE=models\skcommonChair.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skcommonChairMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skcommonChairAnims ANIMFILE=models\skcommonChair.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skcommonChairMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skcommonChairMesh ANIM=skcommonChairAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skcommonChairAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skcommonChairTex0  FILE=TEXTURES\commonRm_chairmap.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skcommonChairMesh NUM=0 TEXTURE=skcommonChairTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: commonRm_chairmap.bmp  Path: C:\HP2_master\objects 


defaultproperties
{
    Mesh=skcommonChairMesh
    DrawType=DT_Mesh
    bStatic=False
}


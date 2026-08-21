//===============================================================================
//  [skcommonCouch] 
//===============================================================================

class skcommonCouch extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skcommonCouchMesh MODELFILE=models\skcommonCouch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skcommonCouchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skcommonCouchAnims ANIMFILE=models\skcommonCouch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skcommonCouchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skcommonCouchMesh ANIM=skcommonCouchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skcommonCouchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skcommonCouchTex0  FILE=TEXTURES\commonRm_couchmap.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skcommonCouchMesh NUM=0 TEXTURE=skcommonCouchTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: commonRm_couchmap.bmp  Path: C:\HP2_master\objects 


defaultproperties
{
    Mesh=skcommonCouchMesh
    DrawType=DT_Mesh
    bStatic=False
}


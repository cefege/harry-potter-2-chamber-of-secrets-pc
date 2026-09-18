//===============================================================================
//  [skfacestatue] 
//===============================================================================

class skfacestatue extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skfacestatueMesh MODELFILE=models\skfacestatue.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skfacestatueMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skfacestatueAnims ANIMFILE=models\skfacestatue.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skfacestatueMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skfacestatueMesh ANIM=skfacestatueAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skfacestatueAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skfacestatueTex0  FILE=TEXTURES\statueface.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skfacestatueMesh NUM=0 TEXTURE=skfacestatueTex0

// Original material [0] is [Material #4] SkinIndex: 0 Bitmap: statueface.bmp  Path: C:\HP2_master\chamber of secrets\psd 


defaultproperties
{
    Mesh=skfacestatueMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skhanginglampGS] 
//===============================================================================

class skhanginglampGS extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skhanginglampGSMesh MODELFILE=models\skhanginglampGS.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skhanginglampGSMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skhanginglampGSAnims ANIMFILE=models\skhanginglampGS.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skhanginglampGSMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skhanginglampGSMesh ANIM=skhanginglampGSAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skhanginglampGSAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skhanginglampGSTex0  FILE=TEXTURES\hanginglamptexture.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skhanginglampGSMesh NUM=0 TEXTURE=skhanginglampGSTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: hanginglamptexture.bmp  Path: C:\HP2_master\EntranceHall\psd 


defaultproperties
{
    Mesh=skhanginglampGSMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skChLightRay] 
//===============================================================================

class skChLightRay extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChLightRayMesh MODELFILE=models\skChLightRay.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChLightRayMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChLightRayAnims ANIMFILE=models\skChLightRay.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChLightRayMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChLightRayMesh ANIM=skChLightRayAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChLightRayAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChLightRayTex0  FILE=TEXTURES\ChamberRayTexture.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChLightRayMesh NUM=0 TEXTURE=skChLightRayTex0

// Original material [0] is [SKIN00.TRANSLUCENT] SkinIndex: 0 Bitmap: ChamberRayTexture.bmp  Path: C:\HP2_master\chamber of secrets\psd 


defaultproperties
{
    Mesh=skChLightRayMesh
    DrawType=DT_Mesh
    bStatic=False
}


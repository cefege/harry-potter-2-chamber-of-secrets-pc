//===============================================================================
//  [skLightRay] 
//===============================================================================

class skLightRay extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skLightRayMesh MODELFILE=models\skLightRay.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLightRayMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLightRayAnims ANIMFILE=models\skLightRay.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLightRayMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLightRayMesh ANIM=skLightRayAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLightRayAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLightRayTex0  FILE=TEXTURES\ray.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLightRayMesh NUM=0 TEXTURE=skLightRayTex0

// Original material [0] is [SKIN00.TRANSLUCENT] SkinIndex: 0 Bitmap: ray.bmp  Path: C:\Harry Potter 2\ART\Objects\Ray of Light 


defaultproperties
{
    Mesh=skLightRayMesh
    DrawType=DT_Mesh
    bStatic=False
}


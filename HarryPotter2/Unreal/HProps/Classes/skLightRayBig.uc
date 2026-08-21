//===============================================================================
//  [skLightRayBig] 
//===============================================================================

class skLightRayBig extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skLightRayBigMesh MODELFILE=models\skLightRayBig.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLightRayBigMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLightRayBigAnims ANIMFILE=models\skLightRayBig.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLightRayBigMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLightRayBigMesh ANIM=skLightRayBigAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLightRayBigAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLightRayBigTex0  FILE=TEXTURES\ray.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skLightRayBigTex1  FILE=TEXTURES\RayLarge.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLightRayBigMesh NUM=0 TEXTURE=skLightRayBigTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skLightRayBigMesh NUM=1 TEXTURE=skLightRayBigTex1

// Original material [0] is [SKIN00.TRANSLUCENT] SkinIndex: 0 Bitmap: ray.bmp  Path: C:\Harry Potter 2\ART\Objects\Ray of Light 
// Original material [1] is [SKIN01.TRANSLUCENT] SkinIndex: 1 Bitmap: RayLarge.bmp  Path: C:\Harry Potter 2\ART\Objects\Ray of Light 


defaultproperties
{
    Mesh=skLightRayBigMesh
    DrawType=DT_Mesh
    bStatic=False
}


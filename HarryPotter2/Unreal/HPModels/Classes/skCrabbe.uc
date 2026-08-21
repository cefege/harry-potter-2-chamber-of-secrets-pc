//===============================================================================
//  [skCrabbe] 
//===============================================================================

class skCrabbe extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skCrabbeMesh MODELFILE=models\skCrabbe.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCrabbeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skCrabbeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCrabbeMesh ANIM=skGenMaleAnims

#EXEC TEXTURE IMPORT NAME=skCrabbeTex0  FILE=TEXTURES\HP2CRABBE_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skCrabbeTex1  FILE=TEXTURES\HP2CRABBE_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCrabbeMesh NUM=0 TEXTURE=skCrabbeTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skCrabbeMesh NUM=1 TEXTURE=skCrabbeTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2CRABBE_SKIN00.bmp  Path: C:\potter\Characters\HP2\Crabbe 
// Original material [1] is [SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2CRABBE_SKIN01.bmp  Path: C:\potter\Characters\HP2\Crabbe 

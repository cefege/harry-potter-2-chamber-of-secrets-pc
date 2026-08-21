//===============================================================================
//  [skSecretLightRays] 
//===============================================================================

class skSecretLightRays extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSecretLightRaysMesh MODELFILE=models\skSecretLightRays.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSecretLightRaysMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSecretLightRaysAnims ANIMFILE=models\skSecretLightRays.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSecretLightRaysMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSecretLightRaysMesh ANIM=skSecretLightRaysAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSecretLightRaysAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSecretLightRaysTex0  FILE=TEXTURES\WW_LightRays2.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSecretLightRaysMesh NUM=0 TEXTURE=skSecretLightRaysTex0

// Original material [0] is [SKIN00.TRANS] SkinIndex: 0 Bitmap: WW_LightRays2.bmp  Path: C:\HarryPotter\Harry Potter 2\Unreal\HPModels\TEXTURES 

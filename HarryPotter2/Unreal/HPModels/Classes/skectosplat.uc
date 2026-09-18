//===============================================================================
//  [skectosplat] 
//===============================================================================

class skectosplat extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skectosplatMesh MODELFILE=models\skectosplat.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skectosplatMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skectosplatAnims ANIMFILE=models\skectosplat.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skectosplatMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skectosplatMesh ANIM=skectosplatAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skectosplatAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skectosplatTex0  FILE=TEXTURES\ectoplasm.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skectosplatMesh NUM=0 TEXTURE=skectosplatTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: ectoplasm.bmp  Path: C:\hp2_characters\HP2_EctoPlasma 

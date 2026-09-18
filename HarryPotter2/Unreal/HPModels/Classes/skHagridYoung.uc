//===============================================================================
//  [skHagridYoung] 
//===============================================================================

class skHagridYoung extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skHagridYoungMesh MODELFILE=models\skHagridYoung.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHagridYoungMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHagridYoungAnims ANIMFILE=models\skHagridYoung.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHagridYoungMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHagridYoungMesh ANIM=skHagridYoungAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHagridYoungAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHagridYoungTex0  FILE=TEXTURES\HP2HAGRIDY_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skHagridYoungTex1  FILE=TEXTURES\HP2HAGRIDY_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHagridYoungMesh NUM=0 TEXTURE=skHagridYoungTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skHagridYoungMesh NUM=1 TEXTURE=skHagridYoungTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2HAGRIDY_SKIN00.bmp  Path: C:\hp2_characters\HP2_Hagrid_young 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2HAGRIDY_SKIN01.bmp  Path: C:\hp2_characters\HP2_Hagrid_young 

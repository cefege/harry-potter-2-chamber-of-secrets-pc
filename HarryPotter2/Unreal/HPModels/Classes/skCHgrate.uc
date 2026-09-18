//===============================================================================
//  [skCHgrate] 
//===============================================================================

class skCHgrate extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skCHgrateMesh MODELFILE=models\skCHgrate.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCHgrateMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCHgrateAnims ANIMFILE=models\skCHgrate.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCHgrateMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCHgrateMesh ANIM=skCHgrateAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCHgrateAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCHgrateTex0  FILE=TEXTURES\Grate_skin00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCHgrateMesh NUM=0 TEXTURE=skCHgrateTex0

// Original material [0] is [grate_skin00] SkinIndex: 0 Bitmap: Grate_skin00.bmp  Path: C:\HP2_Objects\Grate 

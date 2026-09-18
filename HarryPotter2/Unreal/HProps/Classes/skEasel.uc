//===============================================================================
//  [skEasel] 
//===============================================================================

class skEasel extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skEaselMesh MODELFILE=models\skEasel.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skEaselMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skEaselAnims ANIMFILE=models\skEasel.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skEaselMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skEaselMesh ANIM=skEaselAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skEaselAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skEaselTex0  FILE=TEXTURES\Eslemap_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skEaselMesh NUM=0 TEXTURE=skEaselTex0

// Original material [0] is [SKIN00.TWOSIDDED] SkinIndex: 0 Bitmap: Eslemap_128.bmp  Path: C:\HP2_master\Dada\Objects 


defaultproperties
{
    Mesh=skEaselMesh
    DrawType=DT_Mesh
    bStatic=False
}


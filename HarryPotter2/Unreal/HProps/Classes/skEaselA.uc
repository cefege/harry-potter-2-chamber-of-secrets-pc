//===============================================================================
//  [skEaselA] 
//===============================================================================

class skEaselA extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skEaselAMesh MODELFILE=models\skEaselA.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skEaselAMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skEaselAAnims ANIMFILE=models\skEaselA.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skEaselAMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skEaselAMesh ANIM=skEaselAAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skEaselAAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skEaselATex0  FILE=TEXTURES\EisleMap3_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skEaselAMesh NUM=0 TEXTURE=skEaselATex0

// Original material [0] is [SKIN00.TWOSIDDED] SkinIndex: 0 Bitmap: EisleMap3_128.bmp  Path: C:\HP2_master\Dada\Objects 


defaultproperties
{
    Mesh=skEaselAMesh
    DrawType=DT_Mesh
    bStatic=False
}


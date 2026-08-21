//===============================================================================
//  [skMrytleSink] 
//===============================================================================

class skMrytleSink extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skMrytleSinkMesh MODELFILE=models\skMrytleSink.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skMrytleSinkMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skMrytleSinkAnims ANIMFILE=models\skMrytleSink.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skMrytleSinkMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skMrytleSinkMesh ANIM=skMrytleSinkAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skMrytleSinkAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skMrytleSinkTex0  FILE=TEXTURES\MrytlesinkTexture.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skMrytleSinkMesh NUM=0 TEXTURE=skMrytleSinkTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: MrytlesinkTexture.bmp  Path: C:\HP2_master\Grandstaircase\psd's 


defaultproperties
{
    Mesh=skMrytleSinkMesh
    DrawType=DT_Mesh
    bStatic=False
}


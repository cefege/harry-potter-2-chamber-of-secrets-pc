//===============================================================================
//  [skRiddlesDiary] 
//===============================================================================

class skRiddlesDiary extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skRiddlesDiaryMesh MODELFILE=models\skRiddlesDiary.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skRiddlesDiaryMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skRiddlesDiaryAnims ANIMFILE=models\skRiddlesDiary.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skRiddlesDiaryMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skRiddlesDiaryMesh ANIM=skRiddlesDiaryAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skRiddlesDiaryAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skRiddlesDiaryTex0  FILE=TEXTURES\diarytexturemap.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skRiddlesDiaryMesh NUM=0 TEXTURE=skRiddlesDiaryTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: diarytexturemap.bmp  Path: C:\HP2_master\chamber of secrets\psd 


defaultproperties
{
    Mesh=skRiddlesDiaryMesh
    DrawType=DT_Mesh
    bStatic=False
}


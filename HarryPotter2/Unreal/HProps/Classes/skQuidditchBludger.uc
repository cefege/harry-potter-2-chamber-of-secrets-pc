//===============================================================================
//  [skQuidditchBludger] 
//===============================================================================

class skQuidditchBludger extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skQuidditchBludgerMesh MODELFILE=models\skQuidditchBludger.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skQuidditchBludgerMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skQuidditchBludgerAnims ANIMFILE=models\skQuidditchBludger.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skQuidditchBludgerMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skQuidditchBludgerMesh ANIM=skQuidditchBludgerAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skQuidditchBludgerAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skQuidditchBludgerTex0  FILE=TEXTURES\qbludger_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skQuidditchBludgerMesh NUM=0 TEXTURE=skQuidditchBludgerTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: qbludger_128.bmp  Path: D:\Harry Potter\Art\Objects\Qudditch 


defaultproperties
{
    Mesh=skQuidditchBludgerMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skCrystalBall] 
//===============================================================================

class skCrystalBall extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCrystalBallMesh MODELFILE=models\skCrystalBall.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCrystalBallMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCrystalBallAnims ANIMFILE=models\skCrystalBall.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCrystalBallMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCrystalBallMesh ANIM=skCrystalBallAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCrystalBallAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCrystalBallTex0  FILE=TEXTURES\crysball_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCrystalBallMesh NUM=0 TEXTURE=skCrystalBallTex0

// Original material [0] is [Material #27] SkinIndex: 0 Bitmap: crysball_128.bmp  Path: C:\Harry Potter\ART\Objects\Crystal Ball 


defaultproperties
{
    Mesh=skCrystalBallMesh
    DrawType=DT_Mesh
    bStatic=False
}


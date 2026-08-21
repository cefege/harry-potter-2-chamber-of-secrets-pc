//===============================================================================
//  [skChallengeStar] 
//===============================================================================

class skChallengeStar extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChallengeStarMesh MODELFILE=models\skChallengeStar.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChallengeStarMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChallengeStarAnims ANIMFILE=models\skChallengeStar.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChallengeStarMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChallengeStarMesh ANIM=skChallengeStarAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChallengeStarAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChallengeStarTex0  FILE=TEXTURES\chalstar_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChallengeStarMesh NUM=0 TEXTURE=skChallengeStarTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: chalstar_64.bmp  Path: C:\Harry Potter\ART\Objects\Challenge Star 


defaultproperties
{
    Mesh=skChallengeStarMesh
    DrawType=DT_Mesh
    bStatic=False
}


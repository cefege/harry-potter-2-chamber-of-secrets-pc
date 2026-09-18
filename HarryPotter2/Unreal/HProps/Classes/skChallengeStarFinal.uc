//===============================================================================
//  [skChallengeStarFinal] 
//===============================================================================

class skChallengeStarFinal extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChallengeStarFinalMesh MODELFILE=models\skChallengeStarFinal.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChallengeStarFinalMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChallengeStarFinalAnims ANIMFILE=models\skChallengeStarFinal.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChallengeStarFinalMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChallengeStarFinalMesh ANIM=skChallengeStarFinalAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChallengeStarFinalAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChallengeStarFinalTex0  FILE=TEXTURES\FinalChallengeStar.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChallengeStarFinalMesh NUM=0 TEXTURE=skChallengeStarFinalTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: FinalChallengeStar.bmp  Path: C:\Harry Potter 2\ART\Objects\Challenge Star 


defaultproperties
{
    Mesh=skChallengeStarFinalMesh
    DrawType=DT_Mesh
    bStatic=False
}


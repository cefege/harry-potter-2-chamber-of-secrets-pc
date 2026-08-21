//===============================================================================
//  [skLadder] 
//===============================================================================

class skLadder extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skLadderMesh MODELFILE=models\skLadder.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLadderMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLadderAnims ANIMFILE=models\skLadder.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLadderMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLadderMesh ANIM=skLadderAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLadderAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLadderTex0  FILE=TEXTURES\LadderWood.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLadderMesh NUM=0 TEXTURE=skLadderTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: LadderWood.bmp  Path: C:\HP2 Art\Textures

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skLadderMesh'
}

//===============================================================================
//  [skChairDumble] 
//===============================================================================

class skChairDumble extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChairDumbleMesh MODELFILE=models\skChairDumble.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChairDumbleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChairDumbleAnims ANIMFILE=models\skChairDumble.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChairDumbleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChairDumbleMesh ANIM=skChairDumbleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChairDumbleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChairDumbleTex0  FILE=TEXTURES\DumbleChair.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChairDumbleMesh NUM=0 TEXTURE=skChairDumbleTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: DumbleChair.bmp  Path: C:\HP2 Art\Textures

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skChairDumbleMesh'
}

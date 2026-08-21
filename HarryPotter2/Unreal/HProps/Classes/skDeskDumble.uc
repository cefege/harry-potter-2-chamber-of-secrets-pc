//===============================================================================
//  [skDeskDumble] 
//===============================================================================

class skDeskDumble extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDeskDumbleMesh MODELFILE=models\skDeskDumble.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDeskDumbleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDeskDumbleAnims ANIMFILE=models\skDeskDumble.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDeskDumbleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDeskDumbleMesh ANIM=skDeskDumbleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDeskDumbleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDeskDumbleTex0  FILE=TEXTURES\DumbleDesk.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDeskDumbleMesh NUM=0 TEXTURE=skDeskDumbleTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: DumbleDesk.bmp  Path: C:\HP2 Art\Textures

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skDeskDumbleMesh'
}

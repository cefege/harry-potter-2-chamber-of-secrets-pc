//===============================================================================
//  [skFootStool] 
//===============================================================================

class skFootStool extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFootStoolMesh MODELFILE=models\skFootStool.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFootStoolMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFootStoolAnims ANIMFILE=models\skFootStool.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFootStoolMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFootStoolMesh ANIM=skFootStoolAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFootStoolAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFootStoolTex0  FILE=TEXTURES\LadderWood.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFootStoolMesh NUM=0 TEXTURE=skFootStoolTex0

// Original material [0] is [Material #9] SkinIndex: 0 Bitmap: LadderWood.bmp  Path: C:\HP2 Art\Textures

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skFootStoolMesh'
}

//===============================================================================
//  [skTableRoundSlytherin] 
//===============================================================================

class skTableRoundSlytherin extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTableRoundSlytherinMesh MODELFILE=models\skTableRoundSlytherin.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTableRoundSlytherinMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTableRoundSlytherinAnims ANIMFILE=models\skTableRoundSlytherin.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTableRoundSlytherinMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTableRoundSlytherinMesh ANIM=skTableRoundSlytherinAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTableRoundSlytherinAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTableRoundSlytherinTex0  FILE=TEXTURES\TableSly.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTableRoundSlytherinMesh NUM=0 TEXTURE=skTableRoundSlytherinTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: TableSly.bmp  Path: C:\Harry Potter 2\ART\Objects\Tables_Desks\Slytherin Large Round Table 


defaultproperties
{
    Mesh=skTableRoundSlytherinMesh
    DrawType=DT_Mesh
    bStatic=False
}


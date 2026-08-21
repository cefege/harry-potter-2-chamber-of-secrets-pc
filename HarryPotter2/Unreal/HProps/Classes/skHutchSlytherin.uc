//===============================================================================
//  [skHutchSlytherin] 
//===============================================================================

class skHutchSlytherin extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skHutchSlytherinMesh MODELFILE=models\skHutchSlytherin.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skHutchSlytherinMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skHutchSlytherinAnims ANIMFILE=models\skHutchSlytherin.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skHutchSlytherinMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skHutchSlytherinMesh ANIM=skHutchSlytherinAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skHutchSlytherinAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skHutchSlytherinTex0  FILE=TEXTURES\HutchSlytherin.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skHutchSlytherinMesh NUM=0 TEXTURE=skHutchSlytherinTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: HutchSlytherin.bmp  Path: C:\Harry Potter 2\ART\Objects\Cabinets_Hutches_Bookcases\Slytherin Hutch 


defaultproperties
{
    Mesh=skHutchSlytherinMesh
    DrawType=DT_Mesh
    bStatic=False
}


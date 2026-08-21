//===============================================================================
//  [skChairLeatherSlytherin] 
//===============================================================================

class skChairLeatherSlytherin extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChairLeatherSlytherinMesh MODELFILE=models\skChairLeatherSlytherin.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChairLeatherSlytherinMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChairLeatherSlytherinAnims ANIMFILE=models\skChairLeatherSlytherin.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChairLeatherSlytherinMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChairLeatherSlytherinMesh ANIM=skChairLeatherSlytherinAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChairLeatherSlytherinAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChairLeatherSlytherinTex0  FILE=TEXTURES\Slychair_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChairLeatherSlytherinMesh NUM=0 TEXTURE=skChairLeatherSlytherinTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: Slychair_128.bmp  Path: C:\Harry Potter 2\ART\Objects\Chairs_Stools_Sofas\Slytherin Leather Chair 


defaultproperties
{
    Mesh=skChairLeatherSlytherinMesh
    DrawType=DT_Mesh
    bStatic=False
}


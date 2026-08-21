//===============================================================================
//  [skArmoire] 
//===============================================================================

class skArmoire extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skArmoireMesh MODELFILE=models\skArmoire.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skArmoireMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skArmoireAnims ANIMFILE=models\skArmoire.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skArmoireMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skArmoireMesh ANIM=skArmoireAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skArmoireAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skArmoireTex0  FILE=TEXTURES\garmoire_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skArmoireMesh NUM=0 TEXTURE=skArmoireTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: garmoire_128.bmp  Path: C:\Harry Potter\ART\Objects\Cabinets_Hutches_Bookcases\skArmoire 


defaultproperties
{
    Mesh=skArmoireMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skArmorHelmet] 
//===============================================================================

class skArmorHelmet extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skArmorHelmetMesh MODELFILE=models\skArmorHelmet.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skArmorHelmetMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skArmorHelmetAnims ANIMFILE=models\skArmorHelmet.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skArmorHelmetMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skArmorHelmetMesh ANIM=skArmorHelmetAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skArmorHelmetAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skArmorHelmetTex0  FILE=TEXTURES\hwhelmet_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skArmorHelmetMesh NUM=0 TEXTURE=skArmorHelmetTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: hwhelmet_128.bmp  Path: C:\Harry Potter\ART\Objects\Armor\Helmet 


defaultproperties
{
    Mesh=skArmorHelmetMesh
    DrawType=DT_Mesh
    bStatic=False
}


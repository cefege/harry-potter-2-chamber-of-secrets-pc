//===============================================================================
//  [skTableWoodRectangle2] 
//===============================================================================

class skTableWoodRectangle2 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTableWoodRectangle2Mesh MODELFILE=models\skTableWoodRectangle2.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTableWoodRectangle2Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTableWoodRectangle2Anims ANIMFILE=models\skTableWoodRectangle2.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTableWoodRectangle2Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTableWoodRectangle2Mesh ANIM=skTableWoodRectangle2Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTableWoodRectangle2Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTableWoodRectangle2Tex0  FILE=TEXTURES\grytable_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTableWoodRectangle2Mesh NUM=0 TEXTURE=skTableWoodRectangle2Tex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: grytable_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Common Room Table 


defaultproperties
{
    Mesh=skTableWoodRectangle2Mesh
    DrawType=DT_Mesh
    bStatic=False
}


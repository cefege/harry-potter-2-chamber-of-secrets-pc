//===============================================================================
//  [skTableWoodRectangle1] 
//===============================================================================

class skTableWoodRectangle1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTableWoodRectangle1Mesh MODELFILE=models\skTableWoodRectangle1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTableWoodRectangle1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTableWoodRectangle1Anims ANIMFILE=models\skTableWoodRectangle1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTableWoodRectangle1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTableWoodRectangle1Mesh ANIM=skTableWoodRectangle1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTableWoodRectangle1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTableWoodRectangle1Tex0  FILE=TEXTURES\hogrectb_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTableWoodRectangle1Mesh NUM=0 TEXTURE=skTableWoodRectangle1Tex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: hogrectb_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Rectangular Table 


defaultproperties
{
    Mesh=skTableWoodRectangle1Mesh
    DrawType=DT_Mesh
    bStatic=False
}


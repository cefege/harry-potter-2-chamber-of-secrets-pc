//===============================================================================
//  [skChairsWood1] 
//===============================================================================

class skChairsWood1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChairsWood1Mesh MODELFILE=models\skChairsWood1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChairsWood1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChairsWood1Anims ANIMFILE=models\skChairsWood1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChairsWood1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChairsWood1Mesh ANIM=skChairsWood1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChairsWood1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChairsWood1Tex0  FILE=TEXTURES\grychair_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skChairsWood1Tex1  FILE=TEXTURES\grychair_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChairsWood1Mesh NUM=0 TEXTURE=skChairsWood1Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skChairsWood1Mesh NUM=1 TEXTURE=skChairsWood1Tex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: grychair_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Wood Chair 
// Original material [1] is [SKIN01.MASKED.TWOSIDED] SkinIndex: 1 Bitmap: grychair_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Wood Chair 


defaultproperties
{
    Mesh=skChairsWood1Mesh
    DrawType=DT_Mesh
    bStatic=False
}


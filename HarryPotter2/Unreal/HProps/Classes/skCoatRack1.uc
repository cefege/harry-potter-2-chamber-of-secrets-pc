//===============================================================================
//  [skCoatRack1] 
//===============================================================================

class skCoatRack1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCoatRack1Mesh MODELFILE=models\skCoatRack1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCoatRack1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCoatRack1Anims ANIMFILE=models\skCoatRack1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCoatRack1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCoatRack1Mesh ANIM=skCoatRack1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCoatRack1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCoatRack1Tex0  FILE=TEXTURES\coatrack_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skCoatRack1Tex1  FILE=TEXTURES\coatrack_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCoatRack1Mesh NUM=0 TEXTURE=skCoatRack1Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skCoatRack1Mesh NUM=1 TEXTURE=skCoatRack1Tex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: coatrack_128.bmp  Path: C:\Harry Potter\ART\Objects\Coatracks\Wall Coatrack 
// Original material [1] is [SKIN01.MASKED.TWOSIDED] SkinIndex: 1 Bitmap: coatrack_128.bmp  Path: C:\Harry Potter\ART\Objects\Coatracks\Wall Coatrack 


defaultproperties
{
    Mesh=skCoatRack1Mesh
    DrawType=DT_Mesh
    bStatic=False
}


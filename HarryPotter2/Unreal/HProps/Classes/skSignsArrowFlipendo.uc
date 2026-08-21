//===============================================================================
//  [skSignsArrowFlipendo] 
//===============================================================================

class skSignsArrowFlipendo extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSignsArrowFlipendoMesh MODELFILE=models\skSignsArrowFlipendo.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSignsArrowFlipendoMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSignsArrowFlipendoAnims ANIMFILE=models\skSignsArrowFlipendo.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSignsArrowFlipendoMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSignsArrowFlipendoMesh ANIM=skSignsArrowFlipendoAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSignsArrowFlipendoAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSignsArrowFlipendoTex0  FILE=TEXTURES\fliparow_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSignsArrowFlipendoMesh NUM=0 TEXTURE=skSignsArrowFlipendoTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: fliparow_128.bmp  Path: C:\Harry Potter\ART\Objects\Signs_Arrows\Flipendo Arrow 


defaultproperties
{
    Mesh=skSignsArrowFlipendoMesh
    DrawType=DT_Mesh
    bStatic=False
}


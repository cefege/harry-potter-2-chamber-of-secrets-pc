//===============================================================================
//  [skImpHome] 
//===============================================================================

class skImpHome extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skImpHomeMesh MODELFILE=models\skImpHome.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skImpHomeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skImpHomeAnims ANIMFILE=models\skImpHome.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skImpHomeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skImpHomeMesh ANIM=skImpHomeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skImpHomeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skImpHomeTex0  FILE=TEXTURES\ImpHomeTex.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skImpHomeTex1  FILE=TEXTURES\ImpHomeTex.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skImpHomeTex2  FILE=TEXTURES\ImpHomeTwigs.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skImpHomeMesh NUM=0 TEXTURE=skImpHomeTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skImpHomeMesh NUM=1 TEXTURE=skImpHomeTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skImpHomeMesh NUM=2 TEXTURE=skImpHomeTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: ImpHomeTex.bmp  Path: C:\Harry Potter 2\ART\Objects\Gnome Home\Imp Home 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: ImpHomeTex.bmp  Path: C:\Harry Potter 2\ART\Objects\Gnome Home\Imp Home 
// Original material [2] is [SKIN02.MASKED] SkinIndex: 2 Bitmap: ImpHomeTwigs.bmp  Path: C:\Harry Potter 2\ART\Objects\Gnome Home\Imp Home 


defaultproperties
{
    Mesh=skImpHomeMesh
    DrawType=DT_Mesh
    bStatic=False
}


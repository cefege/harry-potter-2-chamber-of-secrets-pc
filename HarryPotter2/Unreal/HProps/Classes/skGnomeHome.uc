//===============================================================================
//  [skGnomeHome] 
//===============================================================================

class skGnomeHome extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skGnomeHomeMesh MODELFILE=models\skGnomeHome.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGnomeHomeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skGnomeHomeAnims ANIMFILE=models\skGnomeHome.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skGnomeHomeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGnomeHomeMesh ANIM=skGnomeHomeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skGnomeHomeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skGnomeHomeTex0  FILE=TEXTURES\GnomeHomeTex.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGnomeHomeTex1  FILE=TEXTURES\GnomeHomeTex.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGnomeHomeMesh NUM=0 TEXTURE=skGnomeHomeTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skGnomeHomeMesh NUM=1 TEXTURE=skGnomeHomeTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: GnomeHomeTex.bmp  Path: C:\Harry Potter 2\ART\Objects\Gnome Home 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: GnomeHomeTex.bmp  Path: C:\Harry Potter 2\ART\Objects\Gnome Home 


defaultproperties
{
    Mesh=skGnomeHomeMesh
    DrawType=DT_Mesh
    bStatic=False
}


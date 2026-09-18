//===============================================================================
//  [skTelescope] 
//===============================================================================

class skTelescope extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTelescopeMesh MODELFILE=models\skTelescope.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTelescopeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTelescopeAnims ANIMFILE=models\skTelescope.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTelescopeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTelescopeMesh ANIM=skTelescopeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTelescopeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTelescopeTex0  FILE=TEXTURES\Telescope.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTelescopeMesh NUM=0 TEXTURE=skTelescopeTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: skTelescope.bmp  Path: C:\Harry Potter\ART\Objects\skTelescope 


defaultproperties
{
    Mesh=skTelescopeMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skBeanMauve] 
//===============================================================================

class skBeanMauve extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBeanMauveMesh MODELFILE=models\skBeanMauve.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBeanMauveMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBeanMauveAnims ANIMFILE=models\skBeanMauve.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBeanMauveMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBeanMauveMesh ANIM=skBeanMauveAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBeanMauveAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBeanMauveTex0  FILE=TEXTURES\BeanMauve_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBeanMauveMesh NUM=0 TEXTURE=skBeanMauveTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: BeanMauve_64.bmp  Path: C:\Harry Potter 2\ART\Objects\Food_Candy\Jellybeans 


defaultproperties
{
    Mesh=skBeanMauveMesh
    DrawType=DT_Mesh
    bStatic=False
}


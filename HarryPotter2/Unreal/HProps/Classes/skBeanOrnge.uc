//===============================================================================
//  [skBeanOrnge] 
//===============================================================================

class skBeanOrnge extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBeanOrngeMesh MODELFILE=models\skBeanOrnge.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBeanOrngeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBeanOrngeAnims ANIMFILE=models\skBeanOrnge.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBeanOrngeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBeanOrngeMesh ANIM=skBeanOrngeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBeanOrngeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBeanOrngeTex0  FILE=TEXTURES\BeanOrange_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBeanOrngeMesh NUM=0 TEXTURE=skBeanOrngeTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: BeanOrange_64.bmp  Path: C:\Harry Potter 2\ART\Objects\Food_Candy\Jellybeans 


defaultproperties
{
    Mesh=skBeanOrngeMesh
    DrawType=DT_Mesh
    bStatic=False
}


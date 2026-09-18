//===============================================================================
//  [skJarBeans] 
//===============================================================================

class skJarBeans extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skJarBeansMesh MODELFILE=models\skJarBeans.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skJarBeansMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skJarBeansAnims ANIMFILE=models\skJarBeans.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skJarBeansMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skJarBeansMesh ANIM=skJarBeansAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skJarBeansAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skJarBeansTex0  FILE=TEXTURES\BeanJar.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skJarBeansMesh NUM=0 TEXTURE=skJarBeansTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: BeanJar.bmp  Path: C:\Harry Potter 2\ART\Objects\Bottles_Jars\Bean Jar 


defaultproperties
{
    Mesh=skJarBeansMesh
    DrawType=DT_Mesh
    bStatic=False
}


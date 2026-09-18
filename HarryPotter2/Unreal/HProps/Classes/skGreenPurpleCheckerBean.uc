//===============================================================================
//  [skGreenPurpleCheckerBean] 
//===============================================================================

class skGreenPurpleCheckerBean extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skGreenPurpleCheckerBeanMesh MODELFILE=models\skGreenPurpleCheckerBean.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGreenPurpleCheckerBeanMesh X=0 Y=0 Z=16 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skGreenPurpleCheckerBeanAnims ANIMFILE=models\skGreenPurpleCheckerBean.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skGreenPurpleCheckerBeanMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGreenPurpleCheckerBeanMesh ANIM=skGreenPurpleCheckerBeanAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skGreenPurpleCheckerBeanAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skGreenPurpleCheckerBeanTex0  FILE=TEXTURES\chckbean_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGreenPurpleCheckerBeanMesh NUM=0 TEXTURE=skGreenPurpleCheckerBeanTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: chckbean_64.bmp  Path: D:\Harry Potter\A Lorian's Stuff\Hogwarts\General Objects\Candy 


defaultproperties
{
    Mesh=skGreenPurpleCheckerBeanMesh
    DrawType=DT_Mesh
    bStatic=False
}


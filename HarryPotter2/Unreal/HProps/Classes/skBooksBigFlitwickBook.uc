//===============================================================================
//  [skBooksBigFlitwickBook] 
//===============================================================================

class skBooksBigFlitwickBook extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksBigFlitwickBookMesh MODELFILE=models\skBooksBigFlitwickBook.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksBigFlitwickBookMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksBigFlitwickBookAnims ANIMFILE=models\skBooksBigFlitwickBook.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksBigFlitwickBookMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksBigFlitwickBookMesh ANIM=skBooksBigFlitwickBookAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksBigFlitwickBookAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksBigFlitwickBookTex0  FILE=TEXTURES\bigflitb_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksBigFlitwickBookMesh NUM=0 TEXTURE=skBooksBigFlitwickBookTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: bigflitb_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Single Large book_Flitwick 


defaultproperties
{
    Mesh=skBooksBigFlitwickBookMesh
    DrawType=DT_Mesh
    bStatic=False
}


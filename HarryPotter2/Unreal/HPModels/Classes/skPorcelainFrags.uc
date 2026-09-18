//===============================================================================
//  [skPorcelainFrags] 
//===============================================================================

class skPorcelainFrags extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skPorcelainFragsMesh MODELFILE=models\skPorcelainFrags.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPorcelainFragsMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPorcelainFragsAnims ANIMFILE=models\skPorcelainFrags.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPorcelainFragsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPorcelainFragsMesh ANIM=skPorcelainFragsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPorcelainFragsAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPorcelainFragsTex0  FILE=TEXTURES\TrollThrowToilet.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPorcelainFragsMesh NUM=0 TEXTURE=skPorcelainFragsTex0

// Original material [0] is [Toilet_skin00] SkinIndex: 0 Bitmap: TrollThrowToilet.bmp  Path: C:\Project Files\Harry Potter PC\HP Object Textures 


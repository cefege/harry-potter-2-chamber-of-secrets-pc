//===============================================================================
//  [skCandlesPlainCandle] 
//===============================================================================

class skCandlesPlainCandle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCandlesPlainCandleMesh MODELFILE=models\skCandlesPlainCandle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCandlesPlainCandleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCandlesPlainCandleAnims ANIMFILE=models\skCandlesPlainCandle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCandlesPlainCandleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCandlesPlainCandleMesh ANIM=skCandlesPlainCandleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCandlesPlainCandleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCandlesPlainCandleTex0  FILE=TEXTURES\plncndle_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCandlesPlainCandleMesh NUM=0 TEXTURE=skCandlesPlainCandleTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: plncndle_128.bmp  Path: C:\Harry Potter\ART\Objects\Candles_n_Candle_Sticks\Large Drippy Candle 


defaultproperties
{
    Mesh=skCandlesPlainCandleMesh
    DrawType=DT_Mesh
    bStatic=False
}


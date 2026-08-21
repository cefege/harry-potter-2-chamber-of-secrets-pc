//===============================================================================
//  [skCandlesTableDoubleCandle] 
//===============================================================================

class skCandlesTableDoubleCandle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCandlesTableDoubleCandleMesh MODELFILE=models\skCandlesTableDoubleCandle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCandlesTableDoubleCandleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCandlesTableDoubleCandleAnims ANIMFILE=models\skCandlesTableDoubleCandle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCandlesTableDoubleCandleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCandlesTableDoubleCandleMesh ANIM=skCandlesTableDoubleCandleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCandlesTableDoubleCandleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCandlesTableDoubleCandleTex0  FILE=TEXTURES\dblecndl_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCandlesTableDoubleCandleMesh NUM=0 TEXTURE=skCandlesTableDoubleCandleTex0

// Original material [0] is [Material #9] SkinIndex: 0 Bitmap: dblecndl_128.bmp  Path: C:\Harry Potter\ART\Objects\Candles_n_Candle_Sticks\Table Double Candlestick 


defaultproperties
{
    Mesh=skCandlesTableDoubleCandleMesh
    DrawType=DT_Mesh
    bStatic=False
}


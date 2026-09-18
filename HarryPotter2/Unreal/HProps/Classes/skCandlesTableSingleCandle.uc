//===============================================================================
//  [skCandlesTableSingleCandle] 
//===============================================================================

class skCandlesTableSingleCandle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCandlesTableSingleCandleMesh MODELFILE=models\skCandlesTableSingleCandle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCandlesTableSingleCandleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCandlesTableSingleCandleAnims ANIMFILE=models\skCandlesTableSingleCandle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCandlesTableSingleCandleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCandlesTableSingleCandleMesh ANIM=skCandlesTableSingleCandleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCandlesTableSingleCandleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCandlesTableSingleCandleTex0  FILE=TEXTURES\HogSnglCanStick_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCandlesTableSingleCandleMesh NUM=0 TEXTURE=skCandlesTableSingleCandleTex0

// Original material [0] is [Material #8] SkinIndex: 0 Bitmap: HogSnglCanStick_128.bmp  Path: C:\Harry Potter\ART\Objects\Candles_n_Candle_Sticks\Table Single Candlestick 


defaultproperties
{
    Mesh=skCandlesTableSingleCandleMesh
    DrawType=DT_Mesh
    bStatic=False
}


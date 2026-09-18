//=============================================================================
//  A torch fire fx for the HP COS version,
//  this is the larger version for the Hogfronttorch actor
//=============================================================================
class FireHP2_bigtorch1 expands fireHP2;

defaultproperties
{
    SourceWidth=(Base=6)
    SourceHeight=(Base=6)
    SourceDepth=(Base=2)
    bSteadyState=True
    Speed=(Base=15,Rand=25)
    SizeWidth=(Base=24,Rand=6)
    SizeLength=(Base=24,Rand=6)
    Rotation=(Pitch=16384)
}

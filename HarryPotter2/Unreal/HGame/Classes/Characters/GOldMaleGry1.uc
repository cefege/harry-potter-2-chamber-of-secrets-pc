//===============================================================================
// 
//===============================================================================

class GOldMaleGry1 extends characters;


//function respondToStation()
//{
//
//	if(destP.aiData[stationNumber].behavior==BH_die)
//	{
//	
//		destroy();
//	}
//
//}


defaultproperties
{
	
	BumpLineSetPrefix="Omg";

    MultiSkins(0)=Texture'HPModels.Skins.skhp2_genmale1_0Tex0'
    MultiSkins(1)=Texture'HPModels.Skins.skhp2_genmale1_0Tex1'    
	Mesh=SkeletalMesh'HPModels.skhp2_genmale1Mesh'
    DrawType=DT_Mesh
    DrawScale=1.1
    bStatic=False
	 CollisionHeight=42
	CollisionRadius=15
	 walkAnimName="walk"
	 RunAnimName="run"
	 idleAnimName="idle"
	 groundspeed=150
	GroundRunSpeed=220
	AmbientGlow=75

}


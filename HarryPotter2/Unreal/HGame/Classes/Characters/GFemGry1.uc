//===============================================================================
// 
//===============================================================================

class GFemGry1 extends characters;


//function respondToStation()
//{

//	if(destP.aiData[stationNumber].behavior==BH_die)
//	{
//	
//		destroy();
//	}

//}


defaultproperties
{
		BumpLineSetPrefix="Gfg";

    MultiSkins(0)=Texture'HPModels.Skins.skhp2_genfemale1_0Tex0'
    MultiSkins(1)=Texture'HPModels.Skins.skhp2_genfemale1_0Tex1'    
	//Mesh=SkeletalMesh'HPModels.skgen_fem_1Mesh'
	Mesh=SkeletalMesh'HPModels.skhp2_genfemale1Mesh'
    DrawType=DT_Mesh
    bStatic=False
	 CollisionHeight=42
	CollisionRadius=15
	 walkAnimName="walk"
	 RunAnimName="run"
	 idleAnimName="idle"
	 groundspeed=200
	GroundRunSpeed=220
	AmbientGlow=75

}


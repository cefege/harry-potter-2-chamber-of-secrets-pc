//===============================================================================
//  [WhompingWillow] 
//===============================================================================

class WhompingWillow extends HChar;

struct cRootTiming
{
	var()  float  step1_UpAnimRate;
	var()  float  step2_UpTime;
	var()  float  step3_DownAnimRate;
	var()  float  step4_OnGroundTime;
};

//This data gets used by the root controller
var() cRootTiming   Root1Timing[10]; //If you change this 10, change the 10 in WhompWRController
var() cRootTiming   Root2Timing[10];
var() cRootTiming   Root3Timing[10];

var() int           Damage1;
var() int           Damage2;
var() int           Damage3;

//******************************************************************************************************************
function PostBeginPlay()
{
	local WhompWRController  root;

	root = WhompWRController( CreateAnimChannel(class'WhompWRController', AT_Replace, 'Root1') );
	root.SetOwner( self );
	root.WhichRoot = 1;
	root.RootAnimNameUp = 'Root1Up';
	root.RootAnimNameLoop = 'Root1Loop';
	root.RootAnimNameDown = 'Root1Down';
	root.ColObj[0] = FindColObj('BlockPlayerRoot1_1');
	root.ColObj[1] = FindColObj('BlockPlayerRoot1_2');
	root.ColObj[2] = FindColObj('BlockPlayerRoot1_3');
	root.Damage = Damage1;

	root = WhompWRController( CreateAnimChannel(class'WhompWRController', AT_Replace, 'Root2') );
	root.SetOwner( self );
	root.WhichRoot = 2;
	root.RootAnimNameUp = 'Root2Up';
	root.RootAnimNameLoop = 'Root2Loop';
	root.RootAnimNameDown = 'Root2Down';
	root.ColObj[0] = FindColObj('BlockPlayerRoot2_1');
	root.ColObj[1] = FindColObj('BlockPlayerRoot2_2');
	root.ColObj[2] = FindColObj('BlockPlayerRoot2_3');
	root.Damage = Damage2;

	root = WhompWRController( CreateAnimChannel(class'WhompWRController', AT_Replace, 'Root3') );
	root.SetOwner( self );
	root.WhichRoot = 3;
	root.RootAnimNameUp = 'Root3Up';
	root.RootAnimNameLoop = 'Root3Loop';
	root.RootAnimNameDown = 'Root3Down';
	root.ColObj[0] = FindColObj('BlockPlayerRoot3_1');
	root.ColObj[1] = FindColObj('BlockPlayerRoot3_2');
	root.ColObj[2] = FindColObj('BlockPlayerRoot3_3');
	root.Damage = Damage3;
}

//******************************************************************************************************************
//Disable the roots when you get a trigger
function Trigger( Actor Other, Pawn EventInstigator )
{
	local WhompWRController  a;

	ForEach AllActors(class'WhompWRController', a)
		if( a.WhichRoot != 3 )
			a.bGoDisabled = true;
}

//******************************************************************************************************************
function GenericColObj FindColObj(name tag)
{
	local GenericColObj a;

	ForEach AllActors(class'GenericColObj', a, tag)
		return a;
	return none;
}

//******************************************************************************************************************
function float GetUpAnimRate(int WhichRoot, int TimingStage)
{
	switch( WhichRoot )
	{
		case 1:
			return Root1Timing[TimingStage].step1_UpAnimRate;
		case 2:
			return Root2Timing[TimingStage].step1_UpAnimRate;
		case 3:
			return Root3Timing[TimingStage].step1_UpAnimRate;
	}
	return 0;
}

//******************************************************************************************************************
function float GetUpTime(int WhichRoot, int TimingStage)
{
	switch( WhichRoot )
	{
		case 1:
			return Root1Timing[TimingStage].step2_UpTime;
		case 2:
			return Root2Timing[TimingStage].step2_UpTime;
		case 3:
			return Root3Timing[TimingStage].step2_UpTime;
	}
	return 0;
}

//******************************************************************************************************************
function float GetDownAnimRate(int WhichRoot, int TimingStage)
{
	switch( WhichRoot )
	{
		case 1:
			return Root1Timing[TimingStage].step3_DownAnimRate;
		case 2:
			return Root2Timing[TimingStage].step3_DownAnimRate;
		case 3:
			return Root3Timing[TimingStage].step3_DownAnimRate;
	}
	return 0;
}

//******************************************************************************************************************
function float GetOnGroundTime(int WhichRoot, int TimingStage)
{
	switch( WhichRoot )
	{
		case 1:
			return Root1Timing[TimingStage].step4_OnGroundTime;
		case 2:
			return Root2Timing[TimingStage].step4_OnGroundTime;
		case 3:
			return Root3Timing[TimingStage].step4_OnGroundTime;
	}
	return 0;
}

//******************************************************************************************************************
defaultproperties
{
     Mesh=SkeletalMesh'HPModels.skWhompingWillowMesh'
     DrawScale=10
     AmbientGlow=0

	Root1Timing(0)=(step1_UpAnimRate=1,step2_UpTime=0,step3_DownAnimRate=1,step4_OnGroundTime=0.25)
	Root1Timing(1)=(step1_UpAnimRate=1,step2_UpTime=0,step3_DownAnimRate=2,step4_OnGroundTime=2)

	Root2Timing(0)=(step1_UpAnimRate=1.5,step2_UpTime=0,step3_DownAnimRate=2,step4_OnGroundTime=0.25)
	Root2Timing(1)=(step1_UpAnimRate=1,step2_UpTime=0,step3_DownAnimRate=1,step4_OnGroundTime=2)

	Root3Timing(0)=(step1_UpAnimRate=1,step2_UpTime=0.25,step3_DownAnimRate=1,step4_OnGroundTime=0.25)
	Root3Timing(1)=(step1_UpAnimRate=1,step2_UpTime=0,step3_DownAnimRate=1,step4_OnGroundTime=0)
	Root3Timing(2)=(step1_UpAnimRate=3,step2_UpTime=0,step3_DownAnimRate=2,step4_OnGroundTime=0)
	Root3Timing(3)=(step1_UpAnimRate=3,step2_UpTime=0,step3_DownAnimRate=2,step4_OnGroundTime=0)
	Root3Timing(4)=(step1_UpAnimRate=3,step2_UpTime=0,step3_DownAnimRate=2,step4_OnGroundTime=1)

	Damage1=35
	Damage2=35
	Damage3=35

}


class CameraTrigger extends trigger;
/*
// Enumeration of camera types, some of these are not currently used
enum ECameraState
{
	CAM_Standard,
	CAM_Quiditch,
	CAM_Far,
//	CAM_Combat,
	CAM_Boss,
	CAM_High,
	CAM_Reverse,
//	CAM_Fixed,
//	CAM_Rotate,
//	CAM_Free,
	CAM_Cut,
	CAM_TrollChase,
	CAM_Patrol,
	CAM_TopDown,
	CAM_Test
};

var(Camera) ECameraState	CameraState;
var(camera) vector			CameraOffset;
var(camera) actor			DirectionActor;
var(camera) actor			PositionActor;
var(camera) bool			bUseStrafing;
var(camera) rotator			BossCamBox;

var(PatrolPoints) name firstPatrolPointTag;

var BaseCam		Camera;
var BaseHarry	HarryActor;

function PostBeginPlay()
{
	Super.PostBeginPlay();

	foreach allActors(class'BaseCam', Camera)
	{
		break;
	}

	foreach allActors(class'BaseHarry', HarryActor)
	{
		break;
	}
}

function SwitchCameraState()
{
	Camera.CameraOffset = CameraOffset;
	Camera.DirectionActor = DirectionActor;
	Camera.PositionActor = PositionActor;
	Camera.bUseStrafing = bUseStrafing;
	Camera.BossCamBox = BossCamBox;
	Camera.firstPatrolPointTag = firstPatrolPointTag;
	
	switch(CameraState)
	{
		case CAM_Standard:
			Camera.GotoState('StandardState');
			break;

		case CAM_Quiditch:
			Camera.GotoState('QuidditchState');
			break;

		case CAM_Far:
			Camera.GotoState('FarState');
			break;

		case CAM_Boss:
			HarryActor.BossTarget = BaseChar(DirectionActor);
			Camera.GotoState('BossState');
			break;
	
		case CAM_High:
			Camera.GotoState('HighState');
			break;
	
		case CAM_Reverse:
			Camera.GotoState('ReverseState');
			break;

		case CAM_Cut:
			Camera.GotoState('CutState');
			break;

		case CAM_TrollChase:
			Camera.GotoState('TrollChaseState');
			break;

		case CAM_Patrol:
			Camera.GotoState('PatrolState');
			break;

		case CAM_TopDown:
			Camera.GotoState('TopDownState');
			break;

		default:
			log("Bad camera state passed down!");
			break;
	}
}

event Trigger( Actor Other, Pawn EventInstigator )
{
	Super.Trigger(Other, EventInstigator);
	SwitchCameraState();

	if (Event != '')
	{
		TriggerEvent( Event, none, none );
	}
}

function Touch( actor Other )
{
	Super.Touch(Other);
	SwitchCameraState();

	if (Event != '')
	{
		TriggerEvent( Event, none, none );
	}
}
*/
//defaultproperties
//{
//	TriggerType=TT_PlayerProximity
//	CameraState=CAM_Standard
//	CameraOffset=(x=0,y=0,z=0)
//	DirectionActor=none
//	PositionActor=none
//	bUseStrafing=true
//	BossCamBox=(pitch=0,yaw=0,roll=0)
//	firstPatrolPointTag=''
//}

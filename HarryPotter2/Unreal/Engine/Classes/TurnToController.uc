class TurnToController extends Info;

function Init(Pawn parent)
{
	SetOwner( parent );
}

function Tick(float dtime)
{
	pawn(owner).DesiredRotation.yaw = rotator(pawn(owner).TurnTo_TargetActor.Location - pawn(owner).Location).yaw;
}

//auto state stateTurningOwner
//{
//  Begin:
//
//	pawnTurnToward( pawn(owner).TurnTo_TargetActor );
//	//DesiredRotation.Yaw = Rotation.Yaw;
//	Goto 'Begin';
//}
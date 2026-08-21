//=============================================================================
// QuidditchCameraTarget	-- A hidden player that marks where the camera should look
//=============================================================================
class QuidditchCameraTarget extends QuidditchPlayer;

function TakeDamage( int Damage, Pawn InstigatedBy, Vector HitLocation, 
					 Vector Momentum, name DamageType )
{
	// Ignore all damage
}

//-------------------------------------------------------------------------------------------
// States
//
// Pursue	- Chasing target while free-flying
//-------------------------------------------------------------------------------------------

state Fly
{
	function BeginState()
	{
		PlayerHarry.ClientMessage( Name$' Begin Seeking' );
		Log( Name$' Begin Seeking' );
	}

	function EndState()
	{
		PlayerHarry.ClientMessage( Name$' End Seeking' );
		Log( Name$' End Seeking' );
	}

begin:
loop:
	if ( LookForTarget != None && !LookForTarget.bHidden )
	{
		Log( Name$" Sees Target, will pursue" );
		GotoState( 'Pursue' );
	}
	Sleep( 0.1 );
	goto 'loop';
}

auto state Pursue
{
begin:	// Override regular quidditch player's desire to catch the target
	SetCollision( [NewColActors] false, [NewBlockActors] false, [NewBlockPlayers] false );
	bCollideWorld = false;
loop:
	Sleep( 30 );
	goto 'loop';
}


defaultproperties
{
	DrawType=DT_None
	bHidden=true

	bCollideWorld=false
	bCollideActors=false
	bBlockActors=false
	bBlockPlayers=false

	Team=TA_Neutral

	TrackingOffsetRange_Horz=0	// Follow snitch dead-centered
	TrackingOffsetRange_Vert=0
}

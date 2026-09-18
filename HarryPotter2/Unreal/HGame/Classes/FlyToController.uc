
class FlyToController expands HiddenHPawn;

var bool bEnabled;

function PostBeginPlay()
{
	super.PostBeginPlay();

	//Set our Owner's TickParent to the ourself so we get ticked before the object we're moving
	if( Owner != none )
		Owner.TickParent = self;
}

function EnableController()
{
	bEnabled = true;
	GotoState( 'DoingTheFlyTo' );
}

function DisableController()
{
	bEnabled = false;
	GotoState( 'stateIdle' );
}

auto state DoingTheFlyTo
{
	event Tick(float dtime)
	{
		local float  f;
		local HPawn  h;
		local vector vDest;
		local bool   bGotToEnd;

		//super.Tick( dtime );

		//playerHarry.ClientMessage("**** actor:"$ self $"Flying...  Loc:"$owner.Location);

		if( !bEnabled )
			return;

		//playerHarry.ClientMessage("* * * * "$HPawn(owner).Location);

		if( HPawn(owner) != none )
		{
			//playerHarry.ClientMessage("**** b");
			h = HPawn(owner);

			if( h.fFlyToTime < h.fFlyToTimeSpan )
			{
				h.fFlyToTime += dtime;
				if( h.fFlyToTime > h.fFlyToTimeSpan )
				{
					bGotToEnd = true;
					h.fFlyToTime = h.fFlyToTimeSpan;
				}
			}

			if( h.aFlyToActor != none )
			{
				vDest = h.aFlyToActor.Location;
				if( h.bFlyToFixedToDestActor )
					vDest += h.vFlyToDestOffset;
				else
					vDest += h.vFlyToDestOffset >> h.aFlyToActor.Rotation;
			}
			else
			{
				vDest = h.vFlyToDest;
			}

			if( h.fFlyToTimeSpan == 0 )
				h.SetLocation( vDest );
			else
				h.SetLocation( h.vFlyToStart + (vDest-h.vFlyToStart) * EaseFunction(h.fFlyToTime/h.fFlyToTimeSpan, h.eFlyMoveType) );

			//Did we make it to dest?
			if( h.fFlyToTime == h.fFlyToTimeSpan )
			{
				if( bGotToEnd )
					h.OnFlyToDone();

				//Dont stop following if we're supposed to stay locked on.
				if( !h.bFlyToStayLockedToActor )
					bEnabled = false;
			}
		}
	}

}

defaultproperties
{
     bHidden=false
	DrawType=DT_None
}
class ActorChaser expands HiddenHPawn;

var()  bool   bActive;

state stateIdle
{
  Begin:
	Acceleration = vect(0,0,0);
	Velocity = vect(0,0,0);
}

auto state stateChase
{
	function Tick(float dtime)
	{
		local float f;

		super.Tick(dtime);

		Acceleration = normal(owner.Location - Location) * AccelRate;
		Acceleration += -Velocity * 0.5;
		Velocity += Acceleration * dtime;
		f = vsize( Velocity );
		if( f > AirSpeed )
			Velocity *= AirSpeed/f;
		SetLocation( Location + Velocity*dtime );
	}
}

defaultproperties
{
	bActive=true
	Physics=PHYS_None
	AirSpeed=90
	AccelRate=150

	//bHidden=false;
	//DrawType=DT_Sprite;
	//Style=STY_Normal;
}

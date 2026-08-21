//===============================================================================
//  [WiggenWell] 
//===============================================================================

class WiggenWell extends HProp;

var float fPickupFlyTime;

var bool	bTiming;
var float	fTimeout;

function Spawned()
{
	SetPhysics(PHYS_Falling);
	bTiming = false;
}

function touch (actor other)
{
	Super.Touch(other);
}

auto state wwellbottlo
{
	function BeginState()
	{
		bTiming = false;
		fTimeout = 5.0;
	}

	function tick(float deltatime)
	{
		local Rotator	NewRotation;

		NewRotation = Rotation;
		NewRotation.Yaw += (30000 * deltatime);
		NewRotation.Yaw = NewRotation.Yaw & 0xffff;

		SetRotation(NewRotation);

		if (bTiming)
		{
			fTimeout -= deltatime;
		}
	}

	function HitWall( vector HitNormal, actor Wall )
	{
		Velocity *= 0.5;
		Velocity = MirrorVectorByNormal( Velocity, HitNormal );

		bTiming = true;
		if (bTiming && fTimeout >= 0)
		{
			if (abs(Velocity.z) > 10)
			{
				playsound(sound'HPSounds.Magic_sfx.bean_bounce', , abs(Velocity.z) / 100, , , );
			}
		}
	}

	begin:
	loop:
		sleep(1);
		goto 'loop';


}

defaultproperties
{
	bProjTarget=false

	eVulnerableToSpell=SPELL_None
	bStatic=false
	CollisionRadius=13
	CollisionHeight=8
  	physics=phys_walking
	bBounce=True

	DrawType=DT_Mesh
	bPickupOnTouch=true
	PickupFlyTo=FT_HudPosition

	bBlockActors=false
	bBlockPlayers=false
	bBlockCamera=false
	nPickupIncrement=1

	soundPickup=sound'HPSounds.menu_sfx.add_item_to_HUD'

	classStatusGroup=Class'HGame.StatusGroupPotions'
	classStatusItem=Class'HGame.StatusItemWiggenWell'
}


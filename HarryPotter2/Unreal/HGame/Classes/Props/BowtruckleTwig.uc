//===============================================================================
//  [BowtruckleTwig] 
//===============================================================================

class BowTruckleTwig extends HProp;

var class<ParticleFX>	Particles;
var int					Damage;

function touch (actor other)
{

	Super.Touch(other);

	if(other == playerHarry)
	{
		playerHarry.TakeDamage( Damage, self, Location, vect(0,0,0), '');
		Spawn(Particles, , , location, rot(0, 0, 0));
		Destroy();
	}
}

auto state twigno
{
	function HitWall( vector HitNormal, actor Wall )
	{
		Spawn(Particles, , , location, rot(0, 0, 0));
		Destroy();
	}

	begin:
	loop:
		sleep(1);
		goto 'loop';
}

defaultproperties
{
	bProjTarget=false

    DrawType=DT_Mesh
	eVulnerableToSpell=SPELL_None
    bStatic=false
    CollisionRadius=5
    CollisionHeight=5
  	ambientglow=200
    bBounce=True
	Physics=PHYS_Falling

	// pickup related
	bBlockActors=false
	bBlockPlayers=false
	bBlockCamera=false
}


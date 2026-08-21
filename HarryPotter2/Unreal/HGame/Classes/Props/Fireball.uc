class Fireball extends HiddenHpawn;

var bool bTouch;
var float fLifetime;
var vector	CurrentDir;	

function postBeginPlay()
{
	setTimer(fLifetime,false);
}

function timer()
{
	Destroy();
}	

function touch (actor other)
{

	if( pawn(other) == instigator )
		return;

	if( other == playerHarry && bTouch == true)
	{
		other.TakeDamage( 5, none, vect(0,0,0), vect(0,0,0), '' );
		bTouch = false;
	}


	PlaySound(Sound'HPSounds.magic_sfx.spell_hit', SLOT_Interact,  1.0, false, 2000.0, 1);


}

function bounce( vector HitNormal)
{

	// Get out of the wall or floor
	SetLocation( OldLocation );

	// Slow it down
	Velocity *= 0.90;

	// Set the velocity to reflect the normal
	Velocity = MirrorVectorByNormal( Velocity, HitNormal );

	// Set our Direction vector
	CurrentDir = vector( Rotation );

	// Aim the Current Direction to the normal.
	CurrentDir += HitNormal;
	
	SetRotation( rotator( CurrentDir ) );

}

function bump( actor other)
{
	touch(other);
}

function HitWall(vector HitNormal, actor HitWall)
{
	bounce(HitNormal);
}


defaultproperties
{
	 drawType=DT_NONE
     attachedParticleClass(0)=Class'HPParticle.Crabfire2'
     attachedParticleClass(1)=Class'HPParticle.CrabSmoke'
     attachedParticleOffset(0)=(Z=-32)
     CollisionRadius=10
     CollisionHeight=22
     bCollideActors=True
     bCollideWorld=True
	 bTouch=True
	 fLifeTime=3.0;
}

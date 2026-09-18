class FireballLarge extends HiddenHpawn;

var bool bTouch;
var float fLifetime;
var (VisualFX)ParticleFX	fxGrenadeParticleEffect;

var float GrenadeRadius;
var float GrenadeExplosionGravity;
var float iDamage;
var float smallDamage;

//************************************

function postBeginPlay()
{
	setTimer(fLifetime,false);

	fxGrenadeParticleEffect = spawn( Class'HPParticle.Crabfire3' );
}

function ShootFireballs()
{
	local int i;
	local crabFire fireball;
	local spellFireSmall smallFire;
	local FireballGrenadeCenter centerFire;
	local int NumFireballs;
	local rotator rotate_fireball;
	local vector fireball_locn, harrys_head;
	local vector currentLoc;
	local rotator currentRot;
	local float grenadeDamage; 
	local float ratio;

	harrys_head = playerHarry.location;

	harrys_head.z += playerHarry.collisionHeight/2;

	NumFireballs = 10;

	rotate_fireball = rotator(harrys_head - location);

	rotate_fireball.roll = 0;
	rotate_fireball.pitch += (65536*10) / 4;

	if ( vSize(playerHarry.location-location) < GrenadeRadius )
	{
		// Shake the camera when the fireballs explode only if Harry gets damaged by the grenade
		playerHarry.ShakeView( 0.3, 200, 200 );

		ratio = vSize(playerHarry.location-location) / GrenadeRadius;
		grenadeDamage = iDamage - (iDamage * ratio);

		playerHarry.TakeDamage( grenadeDamage, none, vect(0,0,0), vect(0,0,0), '' );
	}

	for (i=0; i<NumFireballs; ++i)
	{
		rotate_fireball.yaw = (65536 / NumFireballs) * i + rand(10000);
		fireball_locn = location+vect(0,0,5);
		smallFire = spawn(class'spellFireSmall',owner,,fireball_locn, rotate_fireball);
		smallFire.iDamage = smallDamage;
		smallFire.GrenadeExplosionGravity = GrenadeExplosionGravity;
	}

	currentLoc = Oldlocation;
	currentRot = rotation;

	fxGrenadeParticleEffect.Shutdown();
	Destroy();

	for (i=0; i<NumFireballs*2; ++i)
	{

	}
	
	// create the center fireball
	centerFire = spawn(class'FireballGrenadeCenter',owner,,currentLoc, currentRot);
	centerFire.iDamage = iDamage;


}

function timer()
{
	ShootFireballs();
}	

function touch (actor other)
{

	if( pawn(other) == instigator )
		return;

	if( other == playerHarry && bTouch == true)
	{
		other.TakeDamage( iDamage, none, vect(0,0,0), vect(0,0,0), '' );
		SetTimer(0.2,false);
		bTouch = false;
	}


	PlaySound(Sound'HPSounds.magic_sfx.spell_hit', SLOT_Interact,  1.0, false, 2000.0, 1);


}

function bump( actor other)
{
	touch(other);
}

auto state stateBegin
{

	begin:

	

}


defaultproperties
{
	 drawType=DT_NONE
//     attachedParticleClass(0)=Class'HPParticle.Crabfire3'
//     attachedParticleClass(1)=Class'HPParticle.CrabSmoke'
//     attachedParticleOffset(0)=(Z=-32)

     CollisionRadius=10
     CollisionHeight=10
     bCollideActors=True
     bCollideWorld=True
	 bTouch=True
	 fLifeTime=2.5;
	 bAlignBottom=True
}

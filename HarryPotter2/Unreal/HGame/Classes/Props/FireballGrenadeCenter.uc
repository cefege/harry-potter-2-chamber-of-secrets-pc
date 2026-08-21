class FireballGrenadeCenter extends HiddenHpawn;

var bool bTouch;
var float fLifetime;
var (VisualFX)ParticleFX	fxGrenadeParticleEffect;

var float iDamage;

//************************************

function postBeginPlay()
{
	setTimer(fLifetime,false);

//	fxGrenadeParticleEffect = spawn( Class'HPParticle.Crabfire3' );
	fxGrenadeParticleEffect = spawn( Class'HPParticle.Crabfireball' );
}

function timer()
{
	fxGrenadeParticleEffect.ShutDown();
	Destroy();
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
     CollisionRadius=10
     CollisionHeight=10
     bCollideActors=True
     bCollideWorld=True
	 bTouch=True
	 fLifeTime=2.5;
	 bAlignBottom=True
}

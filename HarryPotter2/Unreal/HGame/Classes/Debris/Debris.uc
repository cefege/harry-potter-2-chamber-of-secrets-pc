//===============================================================================
//  [Debris] 
//===============================================================================

class Debris extends projectile;

// Vars ...
//-----------

var rotator randSpin;

var int     NumBounces;

var int     PlaySoundTimer;

var class<ParticleFX>	Particles;
var Sound				MySound;
var float				newDrawScale;
var bool				hasParticles;

var	float				MaxLiveTime;

var	float				ScaleDown;
var float				LifeBase;
var float				LifeRand;
var int					MaxParticles;
var int 				VelocityMultiplier;

// Fns ....
//-----------

function InitializeDebris()
{
	local	rotator	randRotVelocity;
	local	int		velocityMult;

	DrawScale *= 0.5 + (3.0-0.5)*FRand();

	// setup initial velocity and spin
	randSpin		= rotRand() / 4;
	randRotVelocity = rotRand ();
	velocityMult	= velocityMultiplier;
	velocity		= vector(randRotVelocity) * velocityMult;
	velocity.z	   += 2.0 * velocityMult;	

	NumBounces = 1 + Rand(2);
}

auto state isFalling
{
	function beginState ()
	{
		MaxLiveTime = 1.0;
	}

	function Tick (float deltaTime)
	{
		local ParticleFX 	p;

		SetRotation(rotation + (randSpin*deltaTime*3));
		velocity.z -= (deltaTime*1000);

		MaxLiveTime -= deltaTime;
		if(	MaxLiveTime < 0 )
		{
			if(hasParticles)
			{
				p = Spawn(Particles,,, location,rot(0,0,0));
				if(p != NONE)
				{
					p.DrawScale	*= newDrawScale;

					if(LifeBase > 0)
					{
						p.LifeTime.Base = LifeBase;
						p.LifeTime.Base*= ScaleDown;
					}
					if(LifeRand > 0)
					{
						p.Lifetime.Rand	= LifeRand;
						p.Lifetime.Rand*= ScaleDown;
					}
					if(MaxParticles > 0)
					{
						p.ParticlesMax	= MaxParticles;
						p.ParticlesMax *= ScaleDown;
					}
				}
			}

			destroy ();
		}
	}

	function HitWall( vector HitNormal, actor Wall )
	{
		local ParticleFX 	p;
		Velocity.z *= 0.20 + FRand()*0.3;
		Velocity = MirrorVectorByNormal( Velocity, HitNormal );

		PlaySound( MySound, Slot_none, [Volume]0.75, [Radius]100000, [Pitch]RandRange(0.9, 1.1) );	// ?????

		if(hasParticles)
		{
			p = Spawn(Particles,,, location,rot(0,0,0));
			if(p != NONE)
			{
				p.DrawScale	*= newDrawScale;

				if(LifeBase > 0)
				{
					p.LifeTime.Base = LifeBase;
					p.LifeTime.Base*= ScaleDown;
				}
				if(LifeRand > 0)
				{
					p.Lifetime.Rand	= LifeRand;
					p.Lifetime.Rand*= ScaleDown;
				}
				if(MaxParticles > 0)
				{
					p.ParticlesMax	= MaxParticles;
					p.ParticlesMax *= ScaleDown;
				}
			}
		}

		//if (fVelocity < 2*velocityMult)
		if( NumBounces == 0 )
			destroy ();

		NumBounces--;
	}

	function Landed( vector HitNormal )
	{
		Velocity = MirrorVectorByNormal( Velocity, HitNormal );
	}

  Begin:
}


defaultproperties
{
    DrawType=DT_Mesh
	bcollideworld=true
    bStatic=False
	Physics=PHYS_Falling
	drawscale=1
	collisionheight=1
	collisionradius=1
    bBounce=True
}

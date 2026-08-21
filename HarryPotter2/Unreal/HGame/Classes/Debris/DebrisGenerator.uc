//===============================================================================
//  [DebrisGenerator] 
//===============================================================================

class DebrisGenerator extends HPawn;

const NUM_ENTRIES	= 8;

const DEFAULT_VELOCITY	= 250;

struct DebrisParams
{
	var() Mesh 				aMesh;
	var() class<ParticleFX>	bParticles;
	var() Sound				cSound;
	var() float				drawScale;
	var() bool				hasParticles;

	var() float				LifeBase;
	var() float				LifeRand;
	var() int				MaxParticles;
	var() int 				Velocity;
};

// Vars ...
var(DebrisGenerator)	DebrisParams		HitDebris;
var(DebrisGenerator)	DebrisParams		BaseDebris[8];

var(DebrisGenerator)	bool				JustOnce;
var(DebrisGenerator)	int					NumDebris;
var(DebrisGenerator)	float				ScaleForSlow;
var(DebrisGenerator)	float				WaitingTime;

var						float				ScaleDown;
var						int					NumMeshs;

function PostBeginPlay()                                  
{                                                         
	local int i;
                                                          
	Super.PostBeginPlay();                                

	// if it is a slow machine, scale everything down
	//	ScaleDown = ScaleForSlow;
	// else
		ScaleDown = 1.0;
                                                          
	NumMeshs = 0;
	for( i = 0; i < NUM_ENTRIES; i++)
	{                                                     
		if( BaseDebris[i].aMesh == none )              
		{                                                 
			NumMeshs = i;                     
			break;                                        
		}                                                 
	}                     
	
	for( i = 0; i < NUM_ENTRIES; i++)
	{
		if(BaseDebris[i].Velocity == 0)
			BaseDebris[i].Velocity = DEFAULT_VELOCITY;
	}

	for( i = 0; i < NUM_ENTRIES; i++)
	{
		if(BaseDebris[i].drawScale == 0)
			BaseDebris[i].drawScale = 1;
	}
}

function Mesh GetRandomMesh(int index)
{
	local Mesh lMesh;
	lMesh = BaseDebris[index].aMesh;

	return lMesh;
}

function GenerateDebris()
{
	local vector   v;
	local rotator  r;

	v = Location;
	v.z += -collisionHeight + collisionRadius;
	r = rotator( vect(0,0,1) );

	Disintegrate(v, r);
}

state() TriggerOpenTimed
{
	function Trigger( actor Other, pawn EventInstigator )
	{
		GotoState( 'TriggerOpenTimed', 'Start' );
	}

Start:
	Disable( 'Trigger' );

	GenerateDebris();

	Sleep(WaitingTime);

	Enable( 'Trigger' );
}

function Disintegrate (vector start_locn, rotator dirn)
{
	local rotator	randRotVelocity;
	local int	 	i, index, nums;
	local vector 	vectGap;
	local Debris 		a;
	local ParticleFX 	p;

	nums = NumDebris * ScaleDown;

	vectGap.z	= CollisionHeight * 2 / nums;
  	vectGap		= vectGap >> dirn;

	start_locn += (vectGap / 2);

	// first biggggg boooom !!!!!!!
	if(HitDebris.bParticles != none)
	{
		p = Spawn(HitDebris.bParticles, , , start_locn, rot(0,0,0));
		if(p != none)
		{
			p.DrawScale *= HitDebris.drawScale;

			// reset life and max, if we need it
			if(HitDebris.LifeBase > 0)
			{
				p.LifeTime.Base = HitDebris.LifeBase;
				p.LifeTime.Base *= ScaleDown;
			}
			if(HitDebris.LifeRand > 0)
			{
				p.Lifetime.Rand	= HitDebris.LifeRand;
				p.Lifetime.Rand *= ScaleDown;
			}
			if(HitDebris.MaxParticles > 0)
			{
				p.ParticlesMax	= HitDebris.MaxParticles;
				p.ParticlesMax *= ScaleDown;
			}
			if(HitDebris.cSound != none)
				PlaySound(HitDebris.cSound, [Radius]100000, [Pitch]RandRange(0.9, 1.1) );
		}
	}

	// do not do anything, if there are no base meshes
	if(NumMeshs == 0)
		return;

	for (i = 0; i < nums; ++i)
	{
		index = Rand(numMeshs);

		a = spawn(class'Debris', , , start_locn, RotRand ());
		if(a != none)
		{
			a.Mesh				= GetRandomMesh(index);

			a.DrawScale		   *= BaseDebris[index].drawScale;

			a.Particles			= BaseDebris[index].bParticles;
			a.MySound 			= BaseDebris[index].cSound;
			a.newDrawScale		= BaseDebris[index].drawScale;
			a.hasParticles		= BaseDebris[index].hasParticles;
			a.LifeBase			= BaseDebris[index].LifeBase;
			a.LifeRand			= BaseDebris[index].LifeRand;
			a.MaxParticles		= BaseDebris[index].MaxParticles;
			a.VelocityMultiplier= BaseDebris[index].Velocity;
			a.ScaleDown			= ScaleDown;

			a.InitializeDebris();
		}

		start_locn += vectGap;
	}

	if(JustOnce)
		Destroy();
}

defaultproperties
{
    DrawType=DT_Mesh
    bStatic=False
	bHidden=true;

	InitialState=TriggerOpenTimed
	bprojtarget=true
	bCollideActors=true
    bCollideWorld=true
    bBlockActors=true
    bBlockPlayers=true
	bdirectional=true

	JustOnce=false
	NumDebris=20
	ScaleForSlow=0.5
	WaitingTime=5.0
}
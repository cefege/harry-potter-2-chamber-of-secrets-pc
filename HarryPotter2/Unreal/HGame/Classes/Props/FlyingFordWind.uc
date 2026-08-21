
// Class Name  : FlyingFordWind
//
// Created on  : 06/11/2002
// Authored by : Janet Weddle
// 
// Description : The wind that the flying ford will run into. Basically will send a touch to the director
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class FlyingFordWind extends HiddenHpawn;

var bool		bTouch;			// Has this object been touched
var() float		violence;		// The distance the car will move (in the direction of the wind's rotation)
var() float		triggerRadius;	// The radius of the collision cylinder that will start the particles
var() float		triggerHeight;	// The height of the collision cylinder

var FlyingFordDirector	Director;		// Current object in control of the mini-game or puzzle that Harry's playing
var FlyingFordWindTrigger windTrigger;	// The object that triggers the particles to start

var (VisualFX)ParticleFX		fxWindParticleEffect;			
var (VisualFX)class<ParticleFX>	fxWindParticleEffectClass;



// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


function postBeginPlay()
{

	Super.PostBeginPlay();

	// Find mini-game director
	foreach AllActors( class'FlyingFordDirector', Director )
		break;

	windTrigger = spawn(class'FlyingFordWindTrigger',self,,location+vec(0,0,10), rotation);
	windTrigger.SetCollisionSize(triggerRadius, triggerHeight);

}

function Tick(float DeltaTime)
{

	Super.Tick(DeltaTime);

	if ( fxWindParticleEffect != None )
	{
		fxWindParticleEffect.SetLocation( Location );
//		playerHarry.clientMessage("Placing wind here");
	}
}
 

function touch (actor other)
{
	Super.Touch(other);
	
	// When the car touches a wind zone. Send a touch but also send a call to
	// StartTurbulence(float violence, vector direction) the direction is the rotation 
	// of the wind object

	if ( other.IsA('Harry') )
	{
		if ( bTouch == false )
		{
			bTouch = true;
	
			Director.OnTouchEvent( Self, Other );
			Director.StartTurbulence(violence,vector(rotation));
		}
	}


}

function untouch(actor other)
{

	Super.UnTouch(other);

	if ( other.IsA('Harry') )
	{
		// You have left the wind
//		bTouch = false;

	
		Director.OnUnTouchEvent( Self, Other );
	}
}

function bump( actor other)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( "I have been bumped " );
	touch(other);
}

function StartWind()
{
	fxWindParticleEffect = spawn( fxWindParticleEffectClass,,,Location );
}

function StopWind()
{
//	playerHarry.clientMessage("Stopping the WIND");
	if ( fxWindParticleEffect != None )
	{
		fxWindParticleEffect.ShutDown();
		fxWindParticleEffect.Destroy();
		fxWindParticleEffect = None;
	}
		
}

defaultproperties
{
	 Tag='FlyingFordWind'
	 fxWindParticleEffectClass=class'CloudWind'
	 CollideType=CT_CYLINDER
	 drawType=DT_SPRITE
	 CollisionHeight=150
     CollisionRadius=200
     CollisionWidth=55
     bCollideActors=True
     bCollideWorld=True
	 bTouch=False
	 bHidden=True

	 violence=100
	 triggerRadius=800
	 triggerHeight=300
}

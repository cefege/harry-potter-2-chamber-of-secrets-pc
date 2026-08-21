// --------------------------------------------------------------------------------------------
//  _                                _______      _                                      
// | |                              |__   __|    (_)                                     
// | |     _   _ _ __ ___   ___  ___   | |   _ __ _  __ _  __ _  ___ _ __     _   _  ___ 
// | |    | | | | '_ ` _ \ / _ \/ __|  | |  | '__| |/ _` |/ _` |/ _ \ '__|   | | | |/ __|
// | |____| |_| | | | | | | (_) \__ \  | |  | |  | | (_| | (_| |  __/ |    _ | |_| | (__ 
// |______|\__,_|_| |_| |_|\___/|___/  |_|  |_|  |_|\__, |\__, |\___|_|   (_) \__,_|\___|
//                                                   __/ | __/ |                         
//                                                  |___/ |___/                          
// --------------------------------------------------------------------------------------------
// Class Name  : LumosTrigger
//
// Created on  : 05/14/2002
// 
// Description : The LumosTrigger 
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class LumosTrigger extends Triggers;


// --------------------------------------------------------------------------------------------
// *** Variables

var bool	bFirstEventSent;		// have we sent the first event yet?

var() bool	bNoEventWhileInRadius;	// Do not send an event while you are still in the radius (when lumos is off)
var() bool	bEventEntering;			// Do we send an event when inside the lumos radius?
var() bool	bEventLeaving;			// Do we send an event when lumos turns off?

var Harry	playerharry;			// refrence to harry

// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions

function PostBeginPlay()
{
	Super.PostBeginPlay();
	playerHarry = Harry(Level.playerHarryActor);
}

// We will need to know if its "safe" to turn lumos off before actually turning lumos off.
// The reason for this is to keep lumos on while inside of a lumos wall.
function bool IsSafeToTurnLumosOff()
{
	local HPawn hpawn;

	// Even when Harry's Lumos is OFF the Lumos Trigger is not considered "safe" to
	// turn off until Harry (and all other HPawns ) are not "inside" the Lumos trigger 
	// collision box/radius.
	
	// In this way, when you have a lumos trigger's collision box/radius encompasing a 
	// lumos wall/zone a trigger event will not be sent untill harry leaves that
	// wall/zone.
	
/*	if( bNoEventWhileInRadius )
	{
		// Make sure Harry is out of our collision radius
		if( vsize(location - playerHarry.location) <= CollisionRadius )
		{
			//DEBUG
			playerHarry.ClientMessage("*** LUMOS: Harry is still inside LumosTrigger " $self $"'s radius!!!");
			log("*** LUMOS: Harry is still inside LumosTrigger " $self $"'s radius!!!");
			return false;
		}
		else
		{
			// Make sure everyone else is out of our collision radius
			foreach AllActors( class'HPawn', hpawn )
			{
				if( !hpawn.bStatic && !hpawn.IsA('HProp') &&
					vsize(location - hpawn.location) <= CollisionRadius )
				{
					//DEBUG
					playerHarry.ClientMessage(" HPawn " $hpawn $" is still inside LumosTrigger " $self $"'s radius!!!");
					log("*** LUMOS: HPawn " $hpawn $" is still inside LumosTrigger " $self $"'s radius!!!");
					return false;
				}
			}
		}
	}
*/	
	// If we WANT an event while in radius then its automaticly safe to "turn off"
	return true;
}


// --------------------------------------------------------------------------------------------
// *** States
auto state StateLumosOff
{
	// When harry turns on/off lumos then this function will be called
	function OnLumosOn()
	{
		// Guarantee one event sent
		if( !bFirstEventSent )
			GotoState('StateWaitingToTurnOn');
	}
}

state StateWaitingToTurnOn
{
	function beginState()
	{
		playerHarry.clientmessage(" LumosTrigger " $self $" Waiting To turn on once we are within lumos range!" );
	}
	
	event Tick( float fTimeDelta )
	{
		// if we are within the lumos radius then turn on!	
		if( playerHarry.InLumosRadius( location ) )
			GotoState('StateLumosOn');
	}
	
	function OnLumosOff()	
	{ 	
		GotoState('StateLumosOff');
	}
}

state StateLumosOn
{
	function beginState()
	{
		// If we just entered the lumosRadius then send an event
		if( bEventEntering && !bFirstEventSent )
		{
			playerHarry.clientmessage("LumosTrigger " $self $" Sending ENTERING TriggerEvent!" );
			TriggerEvent( event, self, none );
			bFirstEventSent = true;
		}
		
		// Play LumosFX sound
		PlaySound( Sound'HPSounds.Magic_sfx.Lumos_hit01', SLOT_None, 125, false);
	}
	
	event Tick( float fTimeDelta )
	{
		// if we are within the lumos radius then turn on!	
		if( !playerHarry.InLumosRadius( location ) )
			OnLumosOff();
	}

	function OnLumosOff()	
	{ 
		// If we want an event when leaving
		if( bEventLeaving && !bFirstEventSent )
		{
			playerHarry.clientmessage(" LumosTrigger " $self $" Sending EXITING TriggerEvent!" );
			TriggerEvent( event, self, none );
			bFirstEventSent = true;
		}
		GotoState('StateLumosOff');
	}
}

defaultproperties
{
	// --- LumosTrigger
	bNoEventWhileInRadius=false
	
	bEventEntering=true
	bEventLeaving=false
}

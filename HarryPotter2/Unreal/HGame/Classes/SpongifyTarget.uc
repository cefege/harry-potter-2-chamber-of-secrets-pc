// --------------------------------------------------------------------------------------------
//   _____                         _  __       _______                       _                  
//  / ____|                       (_)/ _|     |__   __|                     | |                 
// | (___  _ __   ___  _ __   __ _ _| |_ _   _   | |    __ _ _ __  __ _  ___| |_     _   _  ___ 
//  \___ \| '_ \ / _ \| '_ \ / _` | |  _| | | |  | |   / _` | '__|/ _` |/ _ \ __|   | | | |/ __|
//  ____) | |_) | (_) | | | | (_| | | | | |_| |  | |  | (_| | |  | (_| |  __/ |_  _ | |_| | (__ 
// |_____/| .__/ \___/|_| |_|\__, |_|_|  \__, |  |_|   \__,_|_|   \__, |\___|\__|(_) \__,_|\___|
//        | |                 __/ |       __/ |                    __/ |                        
//        |_|                |___/       |___/                    |___/                         
// --------------------------------------------------------------------------------------------
// Class Name  : SpongifyTarget
//
// Created on  : 06/04/2002
// 
// Description : A Spongify Pad will aim for the Spongify Target as long as the 
//				 target's tag == pad's event.
//				 
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpongifyTarget extends HProp;

#exec Texture Import File=Textures\SpongyTarget.pcx Name=SpongifyTargetTexture Mips=Off Flags=2

// --------------------------------------------------------------------------------------------
// *** Variables


// --------------------------------------------------------------------------------------------
// *** Constants


// --------------------------------------------------------------------------------------------
// *** Functions


function PreBeginPlay()
{
	// Set our collision to collide actors = true, block actors or players == false
	SetCollision( false, false, false );
}

// --------------------------------------------------------------------------------------------
// *** States


// --------------------------------------------------------------------------------------------
// *** DefaultProperties

defaultproperties
{
	// -- HPawn
	bStatic=true
	bHidden=true
	DrawType=DT_Sprite
	Texture=Texture'HGame.SpongifyTargetTexture'
	Mesh=None
	
	bCollideActors=false
	bCollideWorld=false
	bBlockActors=false
	bBlockPlayers=false
	
	CollisionHeight=16
	CollisionWidth=16
	CollisionRadius=16
}

// --------------------------------------------------------------------------------------------
// SpongifyTarget.uc - End of file   
// Thanks to FluidStudios for their comment generator
// --------------------------------------------------------------------------------------------
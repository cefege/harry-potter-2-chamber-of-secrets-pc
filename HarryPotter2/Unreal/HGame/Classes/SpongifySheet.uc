// --------------------------------------------------------------------------------------------
//   _____                         _  __        _____ _               _                  
//  / ____|                       (_)/ _|      / ____| |             | |                 
// | (___  _ __   ___  _ __   __ _ _| |_ _   _| (___ | |__   ___  ___| |_     _   _  ___ 
//  \___ \| '_ \ / _ \| '_ \ / _` | |  _| | | |\___ \| '_ \ / _ \/ _ \ __|   | | | |/ __|
//  ____) | |_) | (_) | | | | (_| | | | | |_| |____) | | | |  __/  __/ |_  _ | |_| | (__ 
// |_____/| .__/ \___/|_| |_|\__, |_|_|  \__, |_____/|_| |_|\___|\___|\__|(_) \__,_|\___|
//        | |                 __/ |       __/ |                                          
//        |_|                |___/       |___/                                           
// --------------------------------------------------------------------------------------------
// Class Name  : SpongifySheet
//
// Created on  : 06/13/2002
// 
// Description : SpongifySheet is used in conjunction with the SpongifyPad, and is only used
//				 for drawing the spongify sheet. (a quad to draw the pad's texture)
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpongifySheet extends HProp;


state auto StateIdle
{
	begin:
	LoopAnim( AnimSequence );
}

//	Style=STY_Translucent

defaultproperties
{
     AnimSequence=Idle
     Style=STY_Translucent
     Mesh=SkeletalMesh'HPModels.skSpongifyRugMesh'
     AmbientGlow=254
     MultiSkins(0)=WetTexture'HPParticle.hp_fx.General.SpongifyRugWet'
     bCollideActors=False
     bCollideWorld=False
     bBlockActors=False
     bBlockPlayers=False
}

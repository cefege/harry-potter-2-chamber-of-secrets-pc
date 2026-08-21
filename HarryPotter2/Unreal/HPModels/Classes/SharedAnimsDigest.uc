//===============================================================================
//  [SharedAnimsDigest] 
//
//  Each of the animations in the file are shared between two or more models.
//  They are digested here so that they are loaded only once in the package.
//  This file gets compiled after the models that use them so that the models
//	can do their DEFAULTANIM command, which computes their mesh bounds against
//  the raw animation data (which gets deleted during the Digestion process).
//  The fact that this file's name starts with the letter 'S' insures that it
//  will compile after all the HPMesh classes).
//
//  This class is derived from Object to insure that it gets compiled in the
//  same nested level as the HPMesh abstract classes (which compiles after
//  actor-derived models).
//
//  Note that the corresponding Import commands for all these anims can't go
//  here; they must be placed in the _SharedAnims.uc file in this folder.
//
//  To make an animation shared:
//		Move the ANIM IMPORT command to the _SharedAnims.uc file;
//		Move the ANIM DIGEST command to this file;
//		Move all ANIM NOTIFY commands to this file;
//		Delete the ANIM IMPORT, DIGEST, and NOTIFY commands from all the
//				mesh classes that will share the anim;
//		Change the ANIM parameter on the MESH DEFAULTANIM command in all
//				those mesh classes to the common name for the shared anim.
//===============================================================================

class SharedAnimsDigest extends Object abstract;

// This anim is an empty animation file; used by models that don't need any animations
#exec ANIM DIGEST  ANIM=NoAnims VERBOSE

#exec ANIM DIGEST  ANIM=skbronzechestAnims VERBOSE
#exec ANIM DIGEST  ANIM=skectoblobAnims VERBOSE
#exec ANIM DIGEST  ANIM=skGenFemaleAnims VERBOSE
#exec ANIM DIGEST  ANIM=skGenMaleAnims VERBOSE
#exec ANIM DIGEST  ANIM=skGeorgeWeasleyAnims VERBOSE
#exec ANIM DIGEST  ANIM=skHarryAnims VERBOSE
#exec ANIM DIGEST  ANIM=skHarryQuidAnims VERBOSE
#exec ANIM DIGEST  ANIM=skTomRiddleAnims VERBOSE

#exec ANIM NOTIFY  ANIM=skHarryAnims SEQ=Cast TIME=0.1 FUNCTION=Cast
#exec ANIM NOTIFY  ANIM=skHarryAnims SEQ=SwordCast TIME=0.1 FUNCTION=Cast


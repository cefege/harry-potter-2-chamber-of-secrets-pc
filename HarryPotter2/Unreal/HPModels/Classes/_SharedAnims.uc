//===============================================================================
//  [_SharedAnims] 
//
//  Each of the animations in the file are shared between two or more models.
//  They are imported here so that they are defined and loaded only once.
//  This file gets compiled before the models that use them so that the anims
//  are defined in time (the '_' in the class name insures this).
//
//  This class is derived from Actor to insure that it gets compiled before
//  actor-derived models (which are compiled before object-derived models).
//
//  Note that the corresponding Digest commands for all these anims can't go
//  here; they must be placed in the SharedAnimsDigest.uc file in this folder.
//
//  To make an animation shared:
//		Move the ANIM IMPORT command to this file;
//		Move the ANIM DIGEST command to the SharedAnimsDigest.uc file;
//		Move all ANIM NOTIFY commands to the SharedAnimsDigest.uc file;
//		Delete the ANIM IMPORT, DIGEST, and NOTIFY commands from all the
//				mesh classes that will share the anim;
//		Change the ANIM parameter on the MESH DEFAULTANIM command in all
//				those mesh classes to the common name for the shared anim.
//===============================================================================

class _SharedAnims extends Actor abstract;

// This anim is an empty animation file; used by models that don't need any animations
#exec ANIM  IMPORT ANIM=NoAnims ANIMFILE=models\NoAnims.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1

#exec ANIM  IMPORT ANIM=skbronzechestAnims ANIMFILE=models\skbronzechest.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec ANIM  IMPORT ANIM=skectoblobAnims ANIMFILE=models\skectoblob.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec ANIM  IMPORT ANIM=skGenFemaleAnims ANIMFILE=models\skgenfemale.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec ANIM  IMPORT ANIM=skGenMaleAnims ANIMFILE=models\skgenmale.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec ANIM  IMPORT ANIM=skGeorgeWeasleyAnims ANIMFILE=models\skGeorgeWeasley.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec ANIM  IMPORT ANIM=skHarryAnims ANIMFILE=models\skHarryAnim.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec ANIM  IMPORT ANIM=skHarryQuidAnims ANIMFILE=models\skHarryQuid.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec ANIM  IMPORT ANIM=skTomRiddleAnims ANIMFILE=models\skTomRiddle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1

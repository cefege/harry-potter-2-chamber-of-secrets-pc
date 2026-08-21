//=============================================================================
// Pawn, the base class of all actors that can be controlled by players or AI.
// This is a built-in Unreal class and it shouldn't be modified.
//=============================================================================
class Pawn extends Actor 
	abstract
	native
	nativereplication;

#exec Texture Import File=Textures\Pawn.pcx Name=S_Pawn Mips=Off Flags=2

//-----------------------------------------------------------------------------
// Pawn variables.

// General flags.
var bool		bBehindView;    // Outside-the-player view.
var bool        bIsPlayer;      // Pawn is a player or a player-bot.
var bool		bJustLanded;	// used by eyeheight adjustment
var bool		bUpAndOut;		// used by swimming 
var bool		bIsWalking;
var const bool	bHitSlopedWall;	// used by Physics
var globalconfig bool	bNeverSwitchOnPickup;	// if true, don't automatically switch to picked up weapon
var bool		bWarping;		// Set when travelling through warpzone (so shouldn't telefrag)
var bool		bUpdatingDisplay; // to avoid infinite recursion through inventory setdisplay

//AI flags
var(Combat) bool	bCanStrafe;
var(Orders) bool	bFixedStart;
var const bool		bReducedSpeed;		//used by movement natives
var		bool		bCanJump;
var		bool 		bCanWalk;
var		bool		bCanSwim;
var		bool		bCanFly;
var		bool		bCanOpenDoors;
var		bool		bCanDoSpecial;
var		bool		bDrowning;
var const bool		bLOSflag;			// used for alternating LineOfSight traces
var 	bool 		bFromWall;
var		bool		bHunting;			// tells navigation code that pawn is hunting another pawn,
										//	so fall back to finding a path to a visible pathnode if none
										//	are reachable
var		bool		bAvoidLedges;		// don't get too close to ledges
var		bool		bStopAtLedges;		// if bAvoidLedges and bStopAtLedges, Pawn doesn't try to walk along the edge at all
var		bool		bJumpOffPawn;		
var		bool		bShootSpecial;
var		bool		bAutoActivate;
var		bool		bIsHuman;			// for games which care about whether a pawn is a human
var		bool		bIsFemale;
var		bool		bIsMultiSkinned;
var		bool		bCountJumps;
var		bool		bAdvancedTactics;	// used during movement between pathnodes
var		bool		bViewTarget;
var     bool        bNoZoneFriction;

// Ticked pawn timers
var		float		SightCounter;	//Used to keep track of when to check player visibility
var		float       PainTime;		//used for getting PainTimer() messages (for Lava, no air, etc.)
var		float		SpeechTime;	

// Physics updating time monitoring (for AI monitoring reaching destinations)
var const	float		AvgPhysicsTime;

// Additional pawn region information.
var PointRegion FootRegion;
var PointRegion HeadRegion;

// Navigation AI
var 	float		MoveTimer;
var 	Actor		MoveTarget;		// set by movement natives
var		Actor		FaceTarget;		// set by strafefacing native
var		vector	 	Destination;	// set by Movement natives
var	 	vector		Focus;			// set by Movement natives
var		float		DesiredSpeed;
var		float		MaxDesiredSpeed;
var(Combat) float	MeleeRange; // Max range for melee attack (not including collision radii)

// Player and enemy movement.
var(Movement) float      GroundSpeed;     // The maximum ground speed.
var(Movement) float      WaterSpeed;      // The maximum swimming speed.
var(Movement) float      AirSpeed;        // The maximum flying speed.
var(Movement) float		 AccelRate;		  // max acceleration rate
var(Movement) float		 JumpZ;      		// vertical acceleration w/ jump
var(Movement) float      MaxStepHeight;		// Maximum size of upward/downward step.
var(Movement) float      MaxMountHeight;	// Maximum size of auto-mounting step.
var(Movement) float      StepHeight;		// Size of upward/downward step.
var(Movement) float      StepWidth;			// Depth of upward/downward step.
var(Movement) float      AirControl;		// amount of AirControl available to the pawn

// AI basics.
var	 	float		MinHitWall;		// Minimum HitNormal dot Velocity.Normal to get a HitWall from the
									// physics
var() 	byte       	Visibility;      //How visible is the pawn? 0 = invisible. 
									// 128 = normal.  255 = highly visible.
var		float		Alertness; // -1 to 1 ->Used within specific states for varying reaction to stimuli 
var		float 		Stimulus; // Strength of stimulus - Set when stimulus happens, used in Acquisition state 
var(AI) float		SightRadius;     //Maximum seeing distance.
var(AI) float		PeripheralVision;//Cosine of limits of peripheral vision.
var(AI) float		HearingThreshold;  //Minimum noise loudness for hearing
var		vector		LastSeenPos; 		// enemy position when I last saw enemy (auto updated if EnemyNotVisible() enabled)
var		vector		LastSeeingPos;		// position where I last saw enemy (auto updated if EnemyNotVisible enabled)
var		float		LastSeenTime;
var	 	Pawn    	Enemy;

// Player info.
var travel Weapon       Weapon;        // The pawn's current weapon.
var		   Vector		WeaponLoc;  	// Weapon location updated at every frame (read only)
var		   Rotator		WeaponRot;  	// Weapon rotation updated at every frame  (read only)
var Weapon				PendingWeapon;	// Will become weapon once current weapon is put down
var travel Inventory	SelectedItem;	// currently selected inventory item

// Movement.
var rotator     	ViewRotation;  	// View rotation.
var vector			WalkBob;
var() float      	BaseEyeHeight; 	// Base eye height above collision center.
var float        	EyeHeight;     	// Current eye height, adjusted for bobbing and stairs.
var	const	vector	Floor;			// Normal of floor pawn is standing on (only used
									//	by PHYS_Spider)
var float			SplashTime;		// time of last splash

// View
var float        OrthoZoom;     // Orthogonal/map view zoom factor.
var() float      FovAngle;      // X field of view angle in degrees, usually 90.

// Player game statistics.
var int			DieCount, ItemCount, KillCount, SecretCount, Spree;

//Health
var() travel int      Health;          // Health: 100 = normal maximum

// Selection Mesh
var() string			SelectionMesh;
var() string			SpecialMesh;

// Inherent Armor (for creatures).
var() name	ReducedDamageType; //Either a damagetype name or 'All', 'AllEnvironment' (Burned, Corroded, Frozen)
var() float ReducedDamagePct;

// Inventory to drop when killed (for creatures)
var() class<inventory> DropWhenKilled;

// Zone pain
var(Movement) float		UnderWaterTime;  	//how much time pawn can go without air (in seconds)

var(AI) enum EAttitude  //important - order in decreasing importance
{
	ATTITUDE_Fear,		//will try to run away
	ATTITUDE_Hate,		// will attack enemy
	ATTITUDE_Frenzy,	//will attack anything, indiscriminately
	ATTITUDE_Threaten,	// animations, but no attack
	ATTITUDE_Ignore,
	ATTITUDE_Friendly,
	ATTITUDE_Follow 	//accepts player as leader
} AttitudeToPlayer;	//determines how creature will react on seeing player (if in human form)

var(AI) enum EIntelligence //important - order in increasing intelligence
{
	BRAINS_NONE, //only reacts to immediate stimulus
	BRAINS_REPTILE, //follows to last seen position
	BRAINS_MAMMAL, //simple navigation (limited path length)
	BRAINS_HUMAN   //complex navigation, team coordination, use environment stuff (triggers, etc.)
}	Intelligence;

var(AI) float		Skill;			// skill, scaled by game difficulty (add difficulty to this value)	
var		actor		SpecialGoal;	// used by navigation AI
var		float		SpecialPause;

// Sound and noise management
var const 	vector 		noise1spot;
var const 	float 		noise1time;
var const	pawn		noise1other;
var const	float		noise1loudness;
var const 	vector 		noise2spot;
var const 	float 		noise2time;
var const	pawn		noise2other;
var const	float		noise2loudness;
var			float		LastPainSound;

// chained pawn list
var const	pawn		nextPawn;

// Common sounds
var(Sounds)	sound	HitSound1;
var(Sounds)	sound	HitSound2;
var(Sounds)	sound	Land;
var(Sounds)	sound	Die;
var(Sounds) sound	WaterStep;

// Input buttons.
var input byte
	bZoom, bRun, bLook, bDuck, bSnapLevel,
	bStrafe, bFire, bAltFire, bFreeLook, 
	bExtra0, bExtra1, bExtra2, bExtra3;

var(Combat) float CombatStyle; // -1 to 1 = low means tends to stay off and snipe, high means tends to charge and melee
var NavigationPoint home; //set when begin play, used for retreating and attitude checks
 
var name NextState; //for queueing states
var name NextLabel; //for queueing states

var float SoundDampening;
var float DamageScaling;

var(Orders) name AlarmTag; // tag of object to go to when see player
var(Orders) name SharedAlarmTag;
var	Decoration	carriedDecoration;

var Name PlayerReStartState;

var() localized  string MenuName; //Name used for this pawn type in menus (e.g. player selection) 
var() localized  string NameArticle; //article used in conjunction with this class (e.g. "a", "an")

var() byte VoicePitch; //for speech
var() string VoiceType; //for speech
var float OldMessageTime; //to limit frequency of voice messages

// Route Cache for Navigation
var NavigationPoint RouteCache[16];

// 
var              bool    bSnapToLocIfYouDontMakeIt;
//var              name    MoveToAnimSequence;
const                     NUM_WALKTO_LOCATIONS = 5;
var              vector  vMoveToLoc[5];
var              float   fMoveToSpeed; //optional, alternate speed.
var              int     MoveToLocIdx;

var              bool    bTurnTo_FollowActor;
//var              bool    bTurnTo_FollowActorPermanent; // Keep following, even if state changes.  Added so existing cutscenes dont break.
var              actor   TurnTo_TargetActor;
var              bool    bSendQWhenDone;

var(patrol)        name  IdleAnimName;
var(patrol)        name  WalkAnimName;
var(patrol)        name  RunAnimName;
var(patrol)        name  TalkAnimName;
var(patrol)        name  RotateLeftAnimName;
var(patrol)        name  RotateRightAnimName;

var               bool   bDoingWalk;        //true when patrol or walkto/runto or any of those are using walk speed/anim instead of run speed/anim
var(Movement)     float  GroundWalkSpeed;  //use this var to save the GroundSpeed
var(Movement)     float  GroundRunSpeed;

// Replication Info
var() class<PlayerReplicationInfo> PlayerReplicationInfoClass;
var PlayerReplicationInfo PlayerReplicationInfo;

var name  CutAnimName;
var bool  bCutAnimLoop;
var float fCutAnimRate;
var float fCutAnimTween;
var name  nCutAnimBone;

//*************************************************************************************************************************
var   name  TalkStartAnimName;
var   name  TalkLoopAnimName;
var   name  TalkEndAnimName;

var   cHeadLookAnimChannel	HeadLookAnimChannel;
var   cFacialAnimChannel	FacialAnimChannel;
var   cEyeBlinkAnimChannel	EyeBlinkAnimChannel;
var() bool  bDoEyeBlinks;

//*******************************************************************************

//Ease types for FlyTo and FollowSpline.
enum enumMoveType
{
	MOVE_TYPE_LINEAR,
	MOVE_SNAP,
	MOVE_TYPE_EASE_FROM,
	MOVE_TYPE_EASE_TO,
	MOVE_TYPE_EASE_FROM_AND_TO,
};

//Spline following support
var                SplineManager  SplineManager;
var(Spline)        name         SplinePathName;
var(Spline)        name         FirstSplinePoint;
var(Spline)        name         DestSplinePoint;
var(Spline)        float        SplineSpeed;
var(Spline)        float        SplineAccel; //for ease from/ease to
var(Spline)        enumMoveType eSplineMoveType;
var                InterpolationPoint LastIPoint; //Really just for saving the reference
var                rotator      SplineRotSave;

//*******************************************************************************

replication
{
	// Variables the server should send to the client.
	reliable if( Role==ROLE_Authority )
		Weapon, PlayerReplicationInfo, Health, bCanFly;
	reliable if( bNetOwner && Role==ROLE_Authority )
		 bIsPlayer, CarriedDecoration, SelectedItem,
		 GroundSpeed, WaterSpeed, AirSpeed, AccelRate, JumpZ, AirControl,
		 bBehindView, PlayerRestartState;
	unreliable if( (bNetOwner && bIsPlayer && bNetInitial && Role==ROLE_Authority) || bDemoRecording )
		ViewRotation;
	unreliable if( bNetOwner && Role==ROLE_Authority )
         MoveTarget;
	reliable if( bDemoRecording )
		EyeHeight;

	// Functions the server calls on the client side.
	reliable if( RemoteRole==ROLE_AutonomousProxy ) 
		ClientDying, ClientReStart, ClientGameEnded, ClientSetRotation, ClientSetLocation, ClientPutDown;
	unreliable if( (!bDemoRecording || bClientDemoRecording && bClientDemoNetFunc) && Role==ROLE_Authority )
		ClientHearSound;
	reliable if ( (!bDemoRecording || (bClientDemoRecording && bClientDemoNetFunc)) && Role == ROLE_Authority )
		ClientVoiceMessage;
	reliable if ( (!bDemoRecording || (bClientDemoRecording && bClientDemoNetFunc) || (Level.NetMode==NM_Standalone && IsA('PlayerPawn'))) && Role == ROLE_Authority )
		ClientMessage, TeamMessage, ReceiveLocalizedMessage;

	// Functions the client calls on the server.
	unreliable if( Role<ROLE_Authority )
		SendVoiceMessage, NextItem, SwitchToBestWeapon, TeamBroadcast;
}

// Latent Movement.
//Note that MoveTo sets the actor's Destination, and MoveToward sets the
//actor's MoveTarget.  Actor will rotate towards destination

native(500) final latent function MoveTo( vector NewDestination, optional float speed);
native(502) final latent function MoveToward(actor NewTarget, optional float speed);
native(504) final latent function StrafeTo(vector NewDestination, vector NewFocus);
native(506) final latent function StrafeFacing(vector NewDestination, actor NewTarget);
native(508) final latent function TurnTo(vector NewFocus);
native(510) final latent function TurnToward(actor NewTarget);

// native AI functions
//LineOfSightTo() returns true if any of several points of Other is visible 
// (origin, top, bottom)
native(514) final function bool LineOfSightTo(actor Other); 
// CanSee() similar to line of sight, but also takes into account Pawn's peripheral vision
native(533) final function bool CanSee(actor Other); 
native(518) final function Actor FindPathTo(vector aPoint, optional bool bSinglePath, 
												optional bool bClearPaths);
native(517) final function Actor FindPathToward(actor anActor, optional bool bSinglePath, 

												optional bool bClearPaths);
native(553) final function NavigationPoint FindPath(actor startPoint,name DestClass);
native(525) final function NavigationPoint FindRandomDest(optional bool bClearPaths);

native(522) final function ClearPaths();
native(523) final function vector EAdjustJump();

//Reachable returns what part of direct path from Actor to aPoint is traversable
//using the current locomotion method
native(521) final function bool pointReachable(vector aPoint);
native(520) final function bool actorReachable(actor anActor);

/* PickWallAdjust()
Check if could jump up over obstruction (only if there is a knee height obstruction)
If so, start jump, and return current destination
Else, try to step around - return a destination 90 degrees right or left depending on traces
out and floor checks
*/
native(526) final function bool PickWallAdjust();
native(524) final function int FindStairRotation(float DeltaTime);
native(527) final latent function WaitForLanding();
native(540) final function actor FindBestInventoryPath(out float MinWeight, bool bPredictRespawns);

native(529) final function AddPawn();
native(530) final function RemovePawn();

// Pick best pawn target
native(531) final function pawn PickTarget(out float bestAim, out float bestDist, vector FireDir, vector projStart);
native(534) final function actor PickAnyTarget(out float bestAim, out float bestDist, vector FireDir, vector projStart);

// Force end to sleep
native function StopWaiting();

native static final function bool CheckValidSkinPackage(string SkinPack, string MeshName);

event MayFall(); //return true if allowed to fall - called by engine when pawn is about to fall
event AlterDestination(); // called when using movetoward with bAdvancedTactics true to temporarily modify destination
event Mount( vector Delta )	// Called when physics detects a mount situation.
{
}

//***********************************************************************************************************
simulated event RenderOverlays( canvas Canvas )
{
	if ( Weapon != None )
		Weapon.RenderOverlays(Canvas);
}

function String GetHumanName()
{
	if ( PlayerReplicationInfo != None )
		return PlayerReplicationInfo.PlayerName;
	return NameArticle$MenuName;
}

function ClientPutDown(Weapon Current, Weapon Next)
{
	Current.ClientPutDown(Next);
}

function SetDisplayProperties(ERenderStyle NewStyle, texture NewTexture, bool bLighting, bool bEnviroMap )
{
	Style = NewStyle;
	texture = NewTexture;
	bUnlit = bLighting;
	bMeshEnviromap = bEnviromap;
	if ( Weapon != None )
		Weapon.SetDisplayProperties(Style, Texture, bUnlit, bMeshEnviromap);

	if ( !bUpdatingDisplay && (Inventory != None) )
	{
		bUpdatingDisplay = true;
		Inventory.SetOwnerDisplay();
	}
	bUpdatingDisplay = false;
}

function SetDefaultDisplayProperties()
{
	Style = Default.Style;
	texture = Default.Texture;
	bUnlit = Default.bUnlit;
	bMeshEnviromap = Default.bMeshEnviromap;
	if ( Weapon != None )
		Weapon.SetDisplayProperties(Weapon.Default.Style, Weapon.Default.Texture, Weapon.Default.bUnlit, Weapon.Default.bMeshEnviromap);

	if ( !bUpdatingDisplay && (Inventory != None) )
	{
		bUpdatingDisplay = true;
		Inventory.SetOwnerDisplay();
	}
	bUpdatingDisplay = false;
}

//
// Client gateway functions.
//
event ClientMessage( coerce string S, optional name Type, optional bool bBeep );
event TeamMessage( PlayerReplicationInfo PRI, coerce string S, name Type, optional bool bBeep );
event ReceiveLocalizedMessage( class<LocalMessage> Message, optional int Switch, optional PlayerReplicationInfo RelatedPRI_1, optional PlayerReplicationInfo RelatedPRI_2, optional Object OptionalObject );

function BecomeViewTarget()
{
	bViewTarget = true;
}

event FellOutOfWorld()
{
	if ( Role < ROLE_Authority )
		return;
	Health = -1;
	SetPhysics(PHYS_None);
	Weapon = None;
	Died(None, 'Fell', Location);
}

function PlayRecoil(float Rate);

function SpecialFire();

function bool CheckFutureSight(float DeltaTime)
{
	return true;
}

function RestartPlayer();

//
// Broadcast a message to all players, or all on the same team.
//
function TeamBroadcast( coerce string Msg)
{
	local Pawn P;
	local bool bGlobal;

	if ( Left(Msg, 1) ~= "@" )
	{
		Msg = Right(Msg, Len(Msg)-1);
		bGlobal = true;
	}

	if ( Left(Msg, 1) ~= "." )
		Msg = "."$VoicePitch$Msg;

	if ( bGlobal || !Level.Game.bTeamGame )
	{
		if ( Level.Game.AllowsBroadcast(self, Len(Msg)) )
			for( P=Level.PawnList; P!=None; P=P.nextPawn )
				if( P.bIsPlayer  || P.IsA('MessagingSpectator') )
					P.TeamMessage( PlayerReplicationInfo, Msg, 'Say' );
		return;
	}
		
	if ( Level.Game.AllowsBroadcast(self, Len(Msg)) )
		for( P=Level.PawnList; P!=None; P=P.nextPawn )
			if( P.bIsPlayer && (P.PlayerReplicationInfo.Team == PlayerReplicationInfo.Team) )
			{
				if ( P.IsA('PlayerPawn') )
					P.TeamMessage( PlayerReplicationInfo, Msg, 'TeamSay' );
			}
}

//------------------------------------------------------------------------------
// Speech related

function SendGlobalMessage(PlayerReplicationInfo Recipient, name MessageType, byte MessageID, float Wait)
{
	SendVoiceMessage(PlayerReplicationInfo, Recipient, MessageType, MessageID, 'GLOBAL');
}


function SendTeamMessage(PlayerReplicationInfo Recipient, name MessageType, byte MessageID, float Wait)
{
	SendVoiceMessage(PlayerReplicationInfo, Recipient, MessageType, MessageID, 'TEAM');
}

function SendVoiceMessage(PlayerReplicationInfo Sender, PlayerReplicationInfo Recipient, name messagetype, byte messageID, name broadcasttype)
{
	local Pawn P;
	local bool bNoSpeak;

	if ( Level.TimeSeconds - OldMessageTime < 2.5 )
		bNoSpeak = true;
	else
		OldMessageTime = Level.TimeSeconds;

	for ( P=Level.PawnList; P!=None; P=P.NextPawn )
	{
		if ( P.IsA('PlayerPawn') )
		{  
			if ( !bNoSpeak )
			{
				if ( (broadcasttype == 'GLOBAL') || !Level.Game.bTeamGame )
					P.ClientVoiceMessage(Sender, Recipient, messagetype, messageID);
				else if ( Sender.Team == P.PlayerReplicationInfo.Team )
					P.ClientVoiceMessage(Sender, Recipient, messagetype, messageID);
			}
		}
		else if ( (P.PlayerReplicationInfo == Recipient) || ((messagetype == 'ORDER') && (Recipient == None)) )
			P.BotVoiceMessage(messagetype, messageID, self);
	}
}

function ClientVoiceMessage(PlayerReplicationInfo Sender, PlayerReplicationInfo Recipient, name messagetype, byte messageID);
function BotVoiceMessage(name messagetype, byte MessageID, Pawn Sender);

//***************************************************************
function HandleHelpMessageFrom(Pawn Other);

function FearThisSpot(Actor ASpot);

function float GetRating()
{
	return 1000;
}

function AddVelocity( vector NewVelocity)
{
	if (Physics == PHYS_Walking)
		SetPhysics(PHYS_Falling);
	if ( (Velocity.Z > 380) && (NewVelocity.Z > 0) )
		NewVelocity.Z *= 0.5;
	Velocity += NewVelocity;
}

function ClientSetLocation( vector NewLocation, rotator NewRotation )
{
	local Pawn P;

	ViewRotation      = NewRotation;
	If ( (ViewRotation.Pitch > RotationRate.Pitch) && (ViewRotation.Pitch < 65536 - RotationRate.Pitch) )
	{
		If (ViewRotation.Pitch < 32768) 
			NewRotation.Pitch = RotationRate.Pitch;
		else
			NewRotation.Pitch = 65536 - RotationRate.Pitch;
	}

	NewRotation.Roll  = 0;
	SetRotation( NewRotation );
	SetLocation( NewLocation );
}

function ClientSetRotation( rotator NewRotation )
{
	local Pawn P;

	ViewRotation      = NewRotation;
	NewRotation.Pitch = 0;
	NewRotation.Roll  = 0;
	SetRotation( NewRotation );
}

function ClientDying(name DamageType, vector HitLocation)
{
	PlayDying(DamageType, HitLocation);
	GotoState('Dying');
}

function ClientReStart()
{
	//log("client restart");
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);
	BaseEyeHeight = Default.BaseEyeHeight;
	EyeHeight = BaseEyeHeight;
	PlayWaiting();

	if ( Region.Zone.bWaterZone && (PlayerRestartState == 'PlayerWalking') )
	{
		if (HeadRegion.Zone.bWaterZone)
				PainTime = UnderWaterTime;
		setPhysics(PHYS_Swimming);
		GotoState('PlayerSwimming');
	}
	else
		GotoState(PlayerReStartState);
}

function ClientGameEnded()
{
	GotoState('GameEnded');
}

//=============================================================================
// Inventory related functions.

function float AdjustDesireFor(Inventory Inv)
{
	return 0;
}

// toss out the weapon currently held
function TossWeapon()
{
	local vector X,Y,Z;
	if ( Weapon == None )
		return;
	GetAxes(Rotation,X,Y,Z);
	Weapon.DropFrom(Location + 0.8 * CollisionRadius * X + - 0.5 * CollisionRadius * Y); 
}	

// The player/bot wants to select next item
exec function NextItem()
{
	local Inventory Inv;

	if (SelectedItem==None) {
		SelectedItem = Inventory.SelectNext();
		Return;
	}
	if (SelectedItem.Inventory!=None)
		SelectedItem = SelectedItem.Inventory.SelectNext(); 
	else
		SelectedItem = Inventory.SelectNext();

	if ( SelectedItem == None )
		SelectedItem = Inventory.SelectNext();
}

// FindInventoryType()
// returns the inventory item of the requested class
// if it exists in this pawn's inventory 

function Inventory FindInventoryType( class DesiredClass )
{
	local Inventory Inv;

	for( Inv=Inventory; Inv!=None; Inv=Inv.Inventory )   
		if ( Inv.class == DesiredClass )
			return Inv;
	return None;
} 

// Add Item to this pawn's inventory. 
// Returns true if successfully added, false if not.
function bool AddInventory( inventory NewItem )
{
	// Skip if already in the inventory.
	local inventory Inv;
	
	// The item should not have been destroyed if we get here.
	if (NewItem ==None )
		log("tried to add none inventory to "$self);

	for( Inv=Inventory; Inv!=None; Inv=Inv.Inventory )
		if( Inv == NewItem )
			return false;

	// Add to front of inventory chain.
	NewItem.SetOwner(Self);
	NewItem.Inventory = Inventory;
	Inventory = NewItem;

	return true;
}

// Remove Item from this pawn's inventory, if it exists.
// Returns true if it existed and was deleted, false if it did not exist.
function bool DeleteInventory( inventory Item )
{
	// If this item is in our inventory chain, unlink it.
	local actor Link;

	if ( Item == Weapon )
		Weapon = None;
	if ( Item == SelectedItem )
		SelectedItem = None;
	for( Link = Self; Link!=None; Link=Link.Inventory )
	{
		if( Link.Inventory == Item )
		{
			Link.Inventory = Item.Inventory;
			break;
		}
	}
	Item.SetOwner(None);
}

// Just changed to pendingWeapon
function ChangedWeapon()
{
	local Weapon OldWeapon;

	OldWeapon = Weapon;

	if (Weapon == PendingWeapon)
	{
		if ( Weapon == None )
			SwitchToBestWeapon();
		else if ( Weapon.IsInState('DownWeapon') ) 
			Weapon.BringUp();
		if ( Weapon != None )
			Weapon.SetDefaultDisplayProperties();
		Inventory.ChangedWeapon(); // tell inventory that weapon changed (in case any effect was being applied)
		PendingWeapon = None;
		return;
	}
	if ( PendingWeapon == None )
		PendingWeapon = Weapon;
	PlayWeaponSwitch(PendingWeapon);
	if ( (PendingWeapon != None) && (PendingWeapon.Mass > 20) && (carriedDecoration != None) )
		DropDecoration();
	if ( Weapon != None )
		Weapon.SetDefaultDisplayProperties();
		
	Weapon = PendingWeapon;
	Inventory.ChangedWeapon(); // tell inventory that weapon changed (in case any effect was being applied)
	if ( Weapon != None )
	{
		Weapon.RaiseUp(OldWeapon);
		if ( (Level.Game != None) && (Level.Game.Difficulty > 1) )
			MakeNoise(0.1 * Level.Game.Difficulty);		
	}
	PendingWeapon = None;
}

//==============
// Encroachment
event bool EncroachingOn( actor Other )
{
	if ( (Other.Brush != None) || (Brush(Other) != None) )
		return true;
		
	if ( (!bIsPlayer || bWarping) && (Pawn(Other) != None))
		return true;
		
	return false;
}

event EncroachedBy( actor Other )
{
	if ( Pawn(Other) != None )
		gibbedBy(Other);
		
}

function gibbedBy(actor Other)
{
	local pawn instigatedBy;

	if ( Role < ROLE_Authority )
		return;
	instigatedBy = pawn(Other);
	if (instigatedBy == None)
		instigatedBy = Other.instigator;
	health = -1000; //make sure gibs
	Died(instigatedBy, 'Gibbed', Location);
}

event PlayerTimeOut()
{
	if (Health > 0)
		Died(None, 'Suicided', Location);
}

//Base change - if new base is pawn or decoration, damage based on relative mass and old velocity
// Also, non-players will jump off pawns immediately
function JumpOffPawn()
{
	Velocity += 60 * VRand();
	Velocity.Z = 180;
	SetPhysics(PHYS_Falling);
}

function JumpOffDecoration()
{
	if(Decoration(Base).bslick)
	{
		Velocity += 60 * VRand();
		Velocity.Z = 180;
		SetPhysics(PHYS_Falling);
	}
}

//********************************************************************************************************
function UnderLift(Mover M);

//********************************************************************************************************
singular event BaseChange()
{
	local float decorMass;

	if ( (base == None) && (Physics == PHYS_None) )
	{
		SetPhysics(PHYS_Falling);
	}
	else if (Pawn(Base) != None)
	{
		//cm("***** BaseChange:"$name$" Velocity.Z:"$Velocity.z$" Mass:"$Mass$" Base.Mass:"$Base.Mass$" BAse:"$Base.name);
		if( Mass > Base.Mass )
			Base.TakeDamage( (1-Velocity.Z/400)* Mass/Base.Mass, Self,Location,0.5 * Velocity , 'stomped');

		//If self cant stand on any pawns, or the pawn we're standing on cant be stood on,
		if( bJumpOffPawn  ||  Pawn(Base).PawnCantStandOnMe() )
			JumpOffPawn();
	}
	else if ( (Decoration(Base) != None) && (Velocity.Z < -400) )
	{
		decorMass = FMax(Decoration(Base).Mass, 1);
		Base.TakeDamage((-2* Mass/decorMass * Velocity.Z/400), Self, Location, 0.5 * Velocity, 'stomped');
		
	}

	if(Decoration(Base) != None)
		JumpOffDecoration();
}

//********************************************************************************************************
function bool PawnCantStandOnMe()
{
	return true;
}

//********************************************************************************************************
event LongFall();

//=============================================================================
// Network related functions.


simulated event Destroyed()
{
	local Inventory Inv;
	local Pawn OtherPawn;

	if ( Shadow != None )
		Shadow.Destroy();
	if ( Role < ROLE_Authority )
		return;

	RemovePawn();

	for( Inv=Inventory; Inv!=None; Inv=Inv.Inventory )   
		Inv.Destroy();
	Weapon = None;
	Inventory = None;
	if ( bIsPlayer && (Level.Game != None) )
		Level.Game.logout(self);
	if ( PlayerReplicationInfo != None )
		PlayerReplicationInfo.Destroy();
	for ( OtherPawn=Level.PawnList; OtherPawn!=None; OtherPawn=OtherPawn.nextPawn )
		OtherPawn.Killed(None, self, '');

	//Bug with engine, cant destroy AnimChannels...
	//if( FacialAnimChannel != none )
	//{
	//	FacialAnimChannel.Destroy();
	//	FacialAnimChannel = none;
	//}

	Super.Destroyed();
}

//=============================================================================
// functions.

//
// native client-side functions.
//
native simulated event ClientHearSound ( 
	actor	Actor, 
	int		Id, 
	sound	S, 
	vector	SoundLocation, 
	vector	Parameters,
	bool	Disable3D,
	bool	Loop
);

//
// Called immediately before gameplay begins.
//
event PreBeginPlay()
{
	AddPawn();
	Super.PreBeginPlay();
	if ( bDeleteMe )
		return;

	// Set instigator to self.
	Instigator = Self;
	DesiredRotation = Rotation;
	SightCounter = 0.2 * FRand();  //offset randomly 
	if ( Level.Game != None )
		Skill += Level.Game.Difficulty; 
	Skill = FClamp(Skill, 0, 3);
	PreSetMovement();
	
	/*if ( DrawScale != Default.Drawscale )
	{
		SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale, Default.CollisionHeight*DrawScale/Default.DrawScale);
		Health = Health * DrawScale/Default.DrawScale;
	}*/

	if (bIsPlayer)
	{
		if (PlayerReplicationInfoClass != None)
			PlayerReplicationInfo = Spawn(PlayerReplicationInfoClass, Self,,vect(0,0,0),rot(0,0,0));
		else
			PlayerReplicationInfo = Spawn(class'PlayerReplicationInfo', Self,,vect(0,0,0),rot(0,0,0));
		InitPlayerReplicationInfo();
	}

	if (!bIsPlayer) 
	{
		if ( BaseEyeHeight == 0 )
			BaseEyeHeight = 0.8 * CollisionHeight;
		EyeHeight = BaseEyeHeight;
		if (Fatness == 0) //vary monster fatness slightly if at default
			Fatness = 120 + Rand(8) + Rand(8);
	}

	if ( menuname == "" )
		menuname = GetItemName(string(class));

	if (SelectionMesh == "")
		SelectionMesh = string(Mesh);
}

//***********************************************************************************************************************
event PostBeginPlay()
{
	Super.PostBeginPlay();
	SplashTime = 0;

	if( bDoEyeBlinks )
		CreateEyeBlinkAnimChannel();
}

//***********************************************************************************************************************
// called after PostBeginPlay on net client
simulated event PostNetBeginPlay()
{
	if ( Role != ROLE_SimulatedProxy )
		return;
	if ( bIsMultiSkinned )
	{
		if ( MultiSkins[1] == None )
		{
			if ( bIsPlayer )
				SetMultiSkin(self, "","", PlayerReplicationInfo.team);
			else
				SetMultiSkin(self, "","", 0);
		}
	}
	else if ( Skin == None )
		Skin = Default.Skin;

	if ( (PlayerReplicationInfo != None) 
		&& (PlayerReplicationInfo.Owner == None) )
		PlayerReplicationInfo.SetOwner(self);
}

//***********************************************************************************************************************
function bool CreateHeadLookAnimChannel()
{
	// Check if bone exists and create anim channel for it
	if( HeadLookAnimChannel == None )
	{
		if( BoneNumber( 'bip01 Head' ) <= 0 )
		{
			Log("Error: Couldn't create HeadLookAnimChannel for character '"$self.name$"'  'bip01 Head' doesn't exist.");
			return false;
		}

		HeadLookAnimChannel = cHeadLookAnimChannel( CreateAnimChannel(class'cHeadLookAnimChannel', AT_Replace, 'bip01 Head') );

		if( HeadLookAnimChannel == None )
		{
			Log("Error: Couldn't create HeadLookAnimChannel for character '"$self.name$"'");
			return false;
		}

		HeadLookAnimChannel.SetOwner( self );
	}

	return true;
}

//***********************************************************************************************************************
function bool EnableEyeBlinks( bool bEnable )
{
	//If no anim channel, but you want to enable, try and make it.
	if( EyeBlinkAnimChannel == none  &&  bEnable )
		CreateEyeBlinkAnimChannel();

	if( EyeBlinkAnimChannel == none )
		return false;

	EyeBlinkAnimChannel.bBlinksEnabled = bEnable;

	return true;
}

//***********************************************************************************************************************
function bool CreateEyeBlinkAnimChannel()
{
	//'eye_root'  'blink'
	if( EyeBlinkAnimChannel == none )
	{
		if( BoneNumber( 'eye_root' ) <= 0 )
		{
			Log("Error: Couldn't create EyeBlinkAnimChannel for character '"$self.name$"'  'eye_root' doesn't exist.");
			return false;
		}

		if( !HasAnim( 'eye_close' ) )
		{
			Log("Error: Couldn't create EyeBlinkAnimChannel for character '"$self.name$"'  anim 'eye_close' doesn't exist.");
			return false;
		}

		EyeBlinkAnimChannel = cEyeBlinkAnimChannel( CreateAnimChannel(class'cEyeBlinkAnimChannel', AT_Replace, 'eye_root') );

		if( EyeBlinkAnimChannel == none )
		{
			Log("Error: Couldn't create EyeBlinkAnimChannel for character '"$self.name$"'");
			return false;
		}

		EyeBlinkAnimChannel.SetOwner( self );
		EyeBlinkAnimChannel.bAnimNotReplaceable = true;
	}

	return true;
}

//***********************************************************************************************************************
/* PreSetMovement()
default for walking creature.  Re-implement in subclass
for swimming/flying capability
*/
function PreSetMovement()
{
	if (JumpZ > 0)
		bCanJump = true;
	bCanWalk = true;
	bCanSwim = false;
	bCanFly = false;
	MinHitWall = -0.6;
	if (Intelligence > BRAINS_Reptile)
		bCanOpenDoors = true;
	if (Intelligence == BRAINS_Human)
		bCanDoSpecial = true;
}

simulated function SetMesh()
{
	mesh = default.mesh;
}

//=============================================================================
// Multiskin support
static function SetMultiSkin( actor SkinActor, string SkinName, string FaceName, byte TeamNum )
{
	local Texture NewSkin;

	if(SkinName != "")
	{
		NewSkin = texture(DynamicLoadObject(SkinName, class'Texture'));
		if ( NewSkin != None )
			SkinActor.Skin = NewSkin;
	}
}

static function GetMultiSkin( Actor SkinActor, out string SkinName, out string FaceName )
{
	SkinName = String(SkinActor.Skin);
	FaceName = "";
}

static function bool SetSkinElement(Actor SkinActor, int SkinNo, string SkinName, string DefaultSkinName)
{
	local Texture NewSkin;
	local bool bProscribed;
	local string pkg, SkinItem, MeshName;
	local int i;

	
	if ( (SkinActor.Level.NetMode != NM_Standalone)
		&& (SkinActor.Level.NetMode != NM_Client)
		&& (DefaultSkinName != "") )
	{
		// make sure that an illegal skin package is not used
		// ignore if already have skins set
		if ( SkinActor.Mesh != None )
			MeshName = SkinActor.GetItemName(string(SkinActor.Mesh));
		else
			MeshName = SkinActor.GetItemName(string(SkinActor.Default.Mesh));
		SkinItem = SkinActor.GetItemName(SkinName);
		pkg = Left(SkinName, Len(SkinName) - Len(SkinItem) - 1);
		bProscribed = !CheckValidSkinPackage(pkg, MeshName);
		if ( bProscribed )
			log("Attempted to use illegal skin from package "$pkg$" for "$Meshname);
	}

	NewSkin = Texture(DynamicLoadObject(SkinName, class'Texture'));
	if ( !bProscribed && (NewSkin != None) )
	{
		SkinActor.Multiskins[SkinNo] = NewSkin;
		return True;
	}
	else
	{
		log("Failed to load "$SkinName$" so load "$DefaultSkinName);
		if(DefaultSkinName != "")
		{
			NewSkin = Texture(DynamicLoadObject(DefaultSkinName, class'Texture'));
			SkinActor.Multiskins[SkinNo] = NewSkin;
		}
		return False;
	}
}

//=============================================================================
// Replication
function InitPlayerReplicationInfo()
{
	if (PlayerReplicationInfo.PlayerName == "")
		PlayerReplicationInfo.PlayerName = class'GameInfo'.Default.DefaultPlayerName;
}
	
//=============================================================================
// Animation playing - should be implemented in subclass, 
//
// PlayWaiting, PlayRunning, and PlayGutHit, PlayMovingAttack (if used)
// and PlayDying are required to be implemented in the subclass

function PlayRunning()
{
	////log("Error - PlayRunning should be implemented in subclass of"@class);
}

function PlayWalking()
{
	PlayRunning(); 
}

function PlayWaiting()
{
	////log("Error - PlayWaiting should be implemented in subclass");
}

function PlayMovingAttack()
{
	////log("Error - PlayMovingAttack should be implemented in subclass");
	//Note - must restart attack timer when done with moving attack
	PlayRunning();
}

function PlayWaitingAmbush()
{
	PlayWaiting();
}

function TweenToFighter(float tweentime)
{
}

function TweenToRunning(float tweentime)
{
	TweenToFighter(0.1);
}

function TweenToWalking(float tweentime)
{
	TweenToRunning(tweentime);
}

function TweenToPatrolStop(float tweentime)
{
	TweenToFighter(tweentime);
}

function TweenToWaiting(float tweentime)
{
	TweenToFighter(tweentime);
}

function PlayThreatening()
{
	TweenToFighter(0.1);
}

function PlayPatrolStop()
{
	PlayWaiting();
}

function PlayTurning()
{
	TweenToFighter(0.1);
}

function PlayBigDeath(name DamageType);
function PlayHeadDeath(name DamageType);
function PlayLeftDeath(name DamageType);
function PlayRightDeath(name DamageType);
function PlayGutDeath(name DamageType);

function PlayDying(name DamageType, vector HitLoc)
{
	local vector X,Y,Z, HitVec, HitVec2D;
	local float dotp;

	if ( Velocity.Z > 250 )
	{
		PlayBigDeath(DamageType);
		return;
	}
	
	if ( DamageType == 'Decapitated' )
	{
		PlayHeadDeath(DamageType);
		return;
	}
			
	GetAxes(Rotation,X,Y,Z);
	X.Z = 0;
	HitVec = Normal(HitLoc - Location);
	HitVec2D= HitVec;
	HitVec2D.Z = 0;
	dotp = HitVec2D dot X;

	//first check for head hit
	if ( HitLoc.Z - Location.Z > 0.5 * CollisionHeight )
	{
		if (dotp > 0)
			PlayHeadDeath(DamageType);
		else
			PlayGutDeath(DamageType);
		return;
	}
	
	if (dotp > 0.71) //then hit in front
		PlayGutDeath(DamageType);
	else
	{
		dotp = HitVec dot Y;
		if (dotp > 0.0)
			PlayLeftDeath(DamageType);
		else
			PlayRightDeath(DamageType);
	}
}

function PlayGutHit(float tweentime)
{
	log("Error - play gut hit must be implemented in subclass of"@class);
}

function PlayHeadHit(float tweentime)
{
	PlayGutHit(tweentime);
}

function PlayLeftHit(float tweentime)
{
	PlayGutHit(tweentime);
}

function PlayRightHit(float tweentime)
{
	PlayGutHit(tweentime);
}

function FireWeapon();

/* TraceShot - used by instant hit weapons, and monsters 
*/
function actor TraceShot(out vector HitLocation, out vector HitNormal, vector EndTrace, vector StartTrace)
{
	local vector realHit;
	local actor Other;
	Other = Trace(HitLocation,HitNormal,EndTrace,StartTrace,True);
	if ( Pawn(Other) != None )
	{
		realHit = HitLocation;
		if ( !Pawn(Other).AdjustHitLocation(HitLocation, EndTrace - StartTrace) )
			Other = Pawn(Other).TraceShot(HitLocation,HitNormal,EndTrace,realHit);
	}
	return Other;
}

/* Adjust hit location - adjusts the hit location in for pawns, and returns
true if it was really a hit, and false if not (for ducking, etc.)
*/
simulated function bool AdjustHitLocation(out vector HitLocation, vector TraceDir)
{
	local float adjZ, maxZ;

	TraceDir = Normal(TraceDir);
	HitLocation = HitLocation + 0.4 * CollisionRadius * TraceDir;

	if ( (GetAnimGroup(AnimSequence) == 'Ducking') && (AnimFrame > -0.03) )
	{
		maxZ = Location.Z + 0.25 * CollisionHeight;
		if ( HitLocation.Z > maxZ )
		{
			if ( TraceDir.Z >= 0 )
				return false;
			adjZ = (maxZ - HitLocation.Z)/TraceDir.Z;
			HitLocation.Z = maxZ;
			HitLocation.X = HitLocation.X + TraceDir.X * adjZ;
			HitLocation.Y = HitLocation.Y + TraceDir.Y * adjZ;
			if ( VSize(HitLocation - Location) > CollisionRadius )	
				return false;
		}
	}
	return true;
}
			
function PlayTakeHit(float tweentime, vector HitLoc, int damage)
{
	local vector X,Y,Z, HitVec, HitVec2D;
	local float dotp;
	
	GetAxes(Rotation,X,Y,Z);
	X.Z = 0;
	HitVec = Normal(HitLoc - Location);
	HitVec2D= HitVec;
	HitVec2D.Z = 0;
	dotp = HitVec2D dot X;

	//first check for head hit
	if ( HitLoc.Z - Location.Z > 0.5 * CollisionHeight )
	{
		if (dotp > 0)
			PlayHeadHit(tweentime);
		else
			PlayGutHit(tweentime);
		return;
	}
	
	if (dotp > 0.71) //then hit in front
		PlayGutHit( tweentime);
	else if (dotp < -0.71) // then hit in back
		PlayHeadHit(tweentime);
	else
	{
		dotp = HitVec dot Y;
		if (dotp > 0.0)
			PlayLeftHit(tweentime);
		else
			PlayRightHit(tweentime);
	}
}

function PlayVictoryDance()
{
	TweenToFighter(0.1);
}

function PlayOutOfWater()
{
	TweenToFalling();
}

function PlayDive();
function TweenToFalling();
function PlayInAir();
function PlayDuck();
function PlayCrawling();

function PlayLanded(float impactVel)
{
	local float landVol;
	//default - do nothing (keep playing existing animation)
	landVol = impactVel/JumpZ;
	landVol = 0.005 * Mass * landVol * landVol;
	PlaySound(Land, SLOT_Interact, FMin(20, landVol));
}

function PlayFiring();
function PlayWeaponSwitch(Weapon NewWeapon);
function TweenToSwimming(float tweentime);

//-----------------------------------------------------------------------------
// Sound functions
function PlayTakeHitSound(int Damage, name damageType, int Mult)
{
	if ( Level.TimeSeconds - LastPainSound < 0.25 )
		return;

	if (HitSound1 == None)return;
	LastPainSound = Level.TimeSeconds;
	if (FRand() < 0.5)
		PlaySound(HitSound1, SLOT_Pain, FMax(Mult * TransientSoundVolume, Mult * 2.0));
	else
		PlaySound(HitSound2, SLOT_Pain, FMax(Mult * TransientSoundVolume, Mult * 2.0));
}

function Gasp();

function DropDecoration()
{
	if (CarriedDecoration != None)
	{
		CarriedDecoration.bWasCarried = true;
		CarriedDecoration.SetBase(None);
		CarriedDecoration.SetPhysics(PHYS_Falling);
		CarriedDecoration.Velocity = Velocity + 10 * VRand();
		CarriedDecoration.Instigator = self;
		CarriedDecoration = None;
	}
}

function GrabDecoration()
{
	local vector lookDir, HitLocation, HitNormal, T1, T2, extent;
	local actor HitActor;

	if ( carriedDecoration == None )
	{
		//first trace to find it
		lookDir = vector(Rotation);
		lookDir.Z = 0;
		T1 = Location + BaseEyeHeight * vect(0,0,1) + lookDir * 0.8 * CollisionRadius;
		T2 = T1 + lookDir * 1.2 * CollisionRadius;
		HitActor = Trace(HitLocation, HitNormal, T2, T1, true);
		if ( HitActor == None )
		{
			T1 = T2 - (BaseEyeHeight + CollisionHeight - 2) * vect(0,0,1);
			HitActor = Trace(HitLocation, HitNormal, T1, T2, true);
		}
		else if ( HitActor == Level )
		{
			T2 = HitLocation - lookDir;
			T1 = T2 - (BaseEyeHeight + CollisionHeight - 2) * vect(0,0,1);
			HitActor = Trace(HitLocation, HitNormal, T1, T2, true);
		}	
		if ( (HitActor == None) || (HitActor == Level) )
		{
			extent.X = CollisionRadius;
			extent.Y = CollisionRadius;
			extent.Z = CollisionHeight;
			HitActor = Trace(HitLocation, HitNormal, Location + lookDir * 1.2 * CollisionRadius, Location, true, extent);
		}

		if ( Mover(HitActor) != None )
		{
			if ( Mover(HitActor).bUseTriggered )
				HitActor.Trigger( self, self );
		}		
		else if ( (Decoration(HitActor) != None)  && ((weapon == None) || (weapon.Mass < 20)) )
		{
			CarriedDecoration = Decoration(HitActor);
			if ( !CarriedDecoration.bPushable || (CarriedDecoration.Mass > 40) 
				|| (CarriedDecoration.StandingCount > 0) )
			{
				CarriedDecoration = None;
				return;
			}
			lookDir.Z = 0;				
			if ( CarriedDecoration.SetLocation(Location + (0.5 * CollisionRadius + CarriedDecoration.CollisionRadius) * lookDir) )
			{
				CarriedDecoration.SetPhysics(PHYS_None);
				CarriedDecoration.SetBase(self);
			}
			else
				CarriedDecoration = None;
		}
	}
}
	
function StopFiring();

function ShakeView( float shaketime, float RollMag, float vertmag);

function TakeFallingDamage()
{
	if (Velocity.Z < -1.4 * JumpZ)
	{
		MakeNoise(-0.5 * Velocity.Z/(FMax(JumpZ, 150.0)));
		if (Velocity.Z <= -750 - JumpZ)
		{
			if ( (Velocity.Z < -1650 - JumpZ) && (ReducedDamageType != 'All') )
				TakeDamage(1000, None, Location, vect(0,0,0), 'Fell');
			else if ( Role == ROLE_Authority )
				TakeDamage(-0.15 * (Velocity.Z + 700 + JumpZ), None, Location, vect(0,0,0), 'Fell');
			ShakeView(0.175 - 0.00007 * Velocity.Z, -0.85 * Velocity.Z, -0.002 * Velocity.Z);
		}
	}
	else if ( Velocity.Z > 0.5 * Default.JumpZ )
		MakeNoise(0.35);				
}

/* AdjustAim()
ScriptedPawn version does adjustment for non-controlled pawns. 
PlayerPawn version does the adjustment for player aiming help.
Only adjusts aiming at pawns
allows more error in Z direction (full as defined by AutoAim - only half that difference for XY)
*/

function rotator AdjustAim(float projSpeed, vector projStart, int aimerror, bool bLeadTarget, bool bWarnTarget)
{
	return ViewRotation;
}

function rotator AdjustToss(float projSpeed, vector projStart, int aimerror, bool bLeadTarget, bool bWarnTarget)
{
	return ViewRotation;
}

function WarnTarget(Pawn shooter, float projSpeed, vector FireDir)
{
	// AI controlled creatures may duck
	// if not falling, and projectile time is long enough
	// often pick opposite to current direction (relative to shooter axis)
}

function SetMovementPhysics()
{
	//implemented in sub-class
}

function PlayHit(float Damage, vector HitLocation, name damageType, vector Momentum)
{
}

function PlayDeathHit(float Damage, vector HitLocation, name damageType, vector Momentum)
{
}

function TakeDamage( int Damage, Pawn instigatedBy, Vector hitlocation, 
						Vector momentum, name damageType)
{
	local int actualDamage;
	local bool bAlreadyDead;

	if ( Role < ROLE_Authority )
	{
		log(self$" client damage type "$damageType$" by "$instigatedBy);
		return;
	}

	//log(self@"take damage in state"@GetStateName());	
	bAlreadyDead = (Health <= 0);

	if (Physics == PHYS_None)
		SetMovementPhysics();
	if (Physics == PHYS_Walking)
		momentum.Z = FMax(momentum.Z, 0.4 * VSize(momentum));
	if ( instigatedBy == self )
		momentum *= 0.6;
	momentum = momentum/Mass;

	actualDamage = Level.Game.ReduceDamage(Damage, DamageType, self, instigatedBy);
	if ( bIsPlayer )
	{
		if (ReducedDamageType == 'All') //God mode
			actualDamage = 0;
		else if (Inventory != None) //then check if carrying armor
			actualDamage = Inventory.ReduceDamage(actualDamage, DamageType, HitLocation);
		else
			actualDamage = Damage;
	}
	else if ( (InstigatedBy != None) &&
				(InstigatedBy.IsA(Class.Name) || self.IsA(InstigatedBy.Class.Name)) )
		ActualDamage = ActualDamage * FMin(1 - ReducedDamagePct, 0.35); 
	else if ( (ReducedDamageType == 'All') || 
		((ReducedDamageType != '') && (ReducedDamageType == damageType)) )
		actualDamage = float(actualDamage) * (1 - ReducedDamagePct);
	
	if ( Level.Game.DamageMutator != None )
		Level.Game.DamageMutator.MutatorTakeDamage( ActualDamage, Self, InstigatedBy, HitLocation, Momentum, DamageType );

	AddVelocity( momentum ); 
	Health -= actualDamage;
	if (CarriedDecoration != None)
		DropDecoration();
	if ( HitLocation == vect(0,0,0) )
		HitLocation = Location;
	if (Health > 0)
	{
		if ( (instigatedBy != None) && (instigatedBy != Self) )
			damageAttitudeTo(instigatedBy);
		PlayHit(actualDamage, hitLocation, damageType, Momentum);
	}
	else if ( !bAlreadyDead )
	{
		//log(self$" died");
		NextState = '';
		PlayDeathHit(actualDamage, hitLocation, damageType, Momentum);
		if ( actualDamage > mass )
			Health = -1 * actualDamage;
		if ( (instigatedBy != None) && (instigatedBy != Self) )
			damageAttitudeTo(instigatedBy);
		Died(instigatedBy, damageType, HitLocation);
	}
	else
	{
		//Warn(self$" took regular damage "$damagetype$" from "$instigator$" while already dead");
		// SpawnGibbedCarcass();
		if ( bIsPlayer )
		{
			HidePlayer();
			GotoState('Dying');
		}
		else
			Destroy();
	}
	MakeNoise(1.0); 
}

function Died(pawn Killer, name damageType, vector HitLocation)
{
	local pawn OtherPawn;
	local actor A;

	// mutator hook to prevent deaths
	// WARNING - don't prevent bot suicides - they suicide when really needed
	if ( Level.Game.BaseMutator.PreventDeath(self, Killer, damageType, HitLocation) )
	{
		Health = max(Health, 1); //mutator should set this higher
		return;
	}
	if ( bDeleteMe )
		return; //already destroyed
	Health = Min(0, Health);
	for ( OtherPawn=Level.PawnList; OtherPawn!=None; OtherPawn=OtherPawn.nextPawn )
		OtherPawn.Killed(Killer, self, damageType);
	if ( CarriedDecoration != None )
		DropDecoration();
	level.game.Killed(Killer, self, damageType);
	//log(class$" dying");
	if( Event != '' )
		foreach AllActors( class 'Actor', A, Event )
			A.Trigger( Self, Killer );
	Level.Game.DiscardInventory(self);
	Velocity.Z *= 1.3;
	if ( Gibbed(damageType) )
	{
		SpawnGibbedCarcass();
		if ( bIsPlayer )
			HidePlayer();
		else
			Destroy();
	}
	PlayDying(DamageType, HitLocation);
	if ( Level.Game.bGameEnded )
		return;
	if ( RemoteRole == ROLE_AutonomousProxy )
		ClientDying(DamageType, HitLocation);
	GotoState('Dying');
}

function bool Gibbed(name damageType)
{
	return false;
}

function Carcass SpawnCarcass()
{
	log(self$" should never call base spawncarcass");
	return None;
}

function SpawnGibbedCarcass()
{
}
	
function HidePlayer()
{
	SetCollision(false, false, false);
	TweenToFighter(0.01);
	bHidden = true;
}

event HearNoise( float Loudness, Actor NoiseMaker);
event SeePlayer( actor Seen );
event UpdateEyeHeight( float DeltaTime );
event UpdateTactics(float DeltaTime); // for advanced tactics
event EnemyNotVisible();

function Killed(pawn Killer, pawn Other, name damageType)
{
	if ( Enemy == Other )
		Enemy = None;
}

//Typically implemented in subclass
function string KillMessage( name damageType, pawn Other )
{
	local string message;

	message = Level.Game.CreatureKillMessage(damageType, Other);
	return (Other.PlayerReplicationInfo.PlayerName$message$namearticle$menuname);
}

function damageAttitudeTo(pawn Other);

function Falling()
	{
		//SetPhysics(PHYS_Falling); //Note - physics changes type to PHYS_Falling by default
		//log(class$" Falling");
		PlayInAir();
	}

// Pawn interface called while PHYS_Walking and PHYS_Swimming to update the pawn with 
// the latest information about the walk surface -- added by Legend on 1/31/1999
event WalkTexture( texture Texture, vector StepLocation, vector StepNormal );

event Landed(vector HitNormal)
{
	SetMovementPhysics();
	if ( !IsAnimating() )
		PlayLanded(Velocity.Z);
	if (Velocity.Z < -1.4 * JumpZ)
		MakeNoise(-0.5 * Velocity.Z/(FMax(JumpZ, 150.0)));
	bJustLanded = true;
}

event FootZoneChange(ZoneInfo newFootZone)
{
	local actor HitActor;
	local vector HitNormal, HitLocation;
	local float splashSize;
	local actor splash;
	
	if ( Level.NetMode == NM_Client )
		return;
	if ( Level.TimeSeconds - SplashTime > 0.25 ) 
	{
		SplashTime = Level.TimeSeconds;
		if (Physics == PHYS_Falling)
			MakeNoise(1.0);
		else
			MakeNoise(0.3);
		if ( FootRegion.Zone.bWaterZone )
		{
			if ( !newFootZone.bWaterZone && (Role==ROLE_Authority) )
			{
				if ( FootRegion.Zone.ExitSound != None )
					PlaySound(FootRegion.Zone.ExitSound, SLOT_Interact, 1); 
				if ( FootRegion.Zone.ExitActor != None )
					Spawn(FootRegion.Zone.ExitActor,,,Location - CollisionHeight * vect(0,0,1));
			}
		}
		else if ( newFootZone.bWaterZone && (Role==ROLE_Authority) )
		{
			splashSize = FClamp(0.000025 * Mass * (300 - 0.5 * FMax(-500, Velocity.Z)), 1.0, 4.0 );
			if ( newFootZone.EntrySound != None )
			{
				HitActor = Trace(HitLocation, HitNormal, 
						Location - (CollisionHeight + 40) * vect(0,0,0.8), Location - CollisionHeight * vect(0,0,0.8), false);
				if ( HitActor == None )
					PlaySound(newFootZone.EntrySound, SLOT_Misc, 2 * splashSize);
				else 
					PlaySound(WaterStep, SLOT_Misc, 1.5 + 0.5 * splashSize);
			}
			if( newFootZone.EntryActor != None )
			{
				splash = Spawn(newFootZone.EntryActor,,,Location - CollisionHeight * vect(0,0,1));
				if ( splash != None )
					splash.DrawScale = splashSize;
			}
			//log("Feet entering water");
		}
	}
	
	if (FootRegion.Zone.bPainZone)
	{
		if ( !newFootZone.bPainZone && !HeadRegion.Zone.bWaterZone )
			PainTime = -1.0;
	}
	else if (newFootZone.bPainZone)
		PainTime = 0.01;
}
	
event HeadZoneChange(ZoneInfo newHeadZone)
{
	if ( Level.NetMode == NM_Client )
		return;
	if (HeadRegion.Zone.bWaterZone)
	{
		if (!newHeadZone.bWaterZone)
		{
			if ( bIsPlayer && (PainTime > 0) && (PainTime < 8) )
				Gasp();
			if ( Inventory != None )
				Inventory.ReduceDamage(0, 'Breathe', Location); //inform inventory of zone change
			bDrowning = false;
			if ( !FootRegion.Zone.bPainZone )
				PainTime = -1.0;
		}
	}
	else
	{
		if (newHeadZone.bWaterZone)
		{
			if ( !FootRegion.Zone.bPainZone )
				PainTime = UnderWaterTime;
			if ( Inventory != None )
				Inventory.ReduceDamage(0, 'Drowned', Location); //inform inventory of zone change
			//log("Can't breathe");
		}
	}
}

event SpeechTimer();

//Pain timer just expired.
//Check what zone I'm in (and which parts are)
//based on that cause damage, and reset PainTime
	
event PainTimer()
{
	local float depth;
	local name  DamageType;

	//log("Pain Timer");
	if ( (Health < 0) || (Level.NetMode == NM_Client) )
		return;
		
	if ( FootRegion.Zone.bPainZone )
	{
		depth = 0.4;
		if (Region.Zone.bPainZone)
			depth += 0.4;
		if (HeadRegion.Zone.bPainZone)
			depth += 0.2;

		if (FootRegion.Zone.DamagePerSec > 0)
		{
			if ( IsA('PlayerPawn') )
				Level.Game.SpecialDamageString = FootRegion.Zone.DamageString;

			//For harry, if no damage type set, set DamageType to 'ZonePain' so the game knows whats going on.  This is so the designers dont have to go
			// through and set all the pits to some value.
			if( FootRegion.Zone.DamageType == '' )
				DamageType = 'ZonePain';
			else
				DamageType = FootRegion.Zone.DamageType;

			TakeDamage(int(float(FootRegion.Zone.DamagePerSec) * depth), None, Location, vect(0,0,0), DamageType); 
		}
		else if ( Health < Default.Health )
			Health = Min(Default.Health, Health - depth * FootRegion.Zone.DamagePerSec);

		if (Health > 0)
			PainTime = 1.0;
	}
	else if ( HeadRegion.Zone.bWaterZone )
	{
		TakeDamage(5, None, Location + CollisionHeight * vect(0,0,0.5), vect(0,0,0), 'Drowned'); 
		if ( Health > 0 )
			PainTime = 2.0;
	}
}		

function bool CheckWaterJump(out vector WallNormal)
{
	local actor HitActor;
	local vector HitLocation, HitNormal, checkpoint, start, checkNorm, Extent;

	if (CarriedDecoration != None)
		return false;
	checkpoint = vector(Rotation);
	checkpoint.Z = 0.0;
	checkNorm = Normal(checkpoint);
	checkPoint = Location + CollisionRadius * checkNorm;
	Extent = CollisionRadius * vect(1,1,0);
	Extent.Z = CollisionHeight;
	HitActor = Trace(HitLocation, HitNormal, checkpoint, Location, true, Extent);
	if ( (HitActor != None) && (Pawn(HitActor) == None) )
	{
		WallNormal = -1 * HitNormal;
		start = Location;
		start.Z += 1.1 * MaxStepHeight;
		checkPoint = start + 2 * CollisionRadius * checkNorm;
		HitActor = Trace(HitLocation, HitNormal, checkpoint, start, true);
		if (HitActor == None)
			return true;
	}

	return false;
}

exec function bool SwitchToBestWeapon()
{
	local float rating;
	local int usealt;

	if ( Inventory == None )
		return false;

	PendingWeapon = Inventory.RecommendWeapon(rating, usealt);
	if ( PendingWeapon == Weapon )
		PendingWeapon = None;
	if ( PendingWeapon == None )
		return false;

	if ( Weapon == None )
		ChangedWeapon();
	if ( Weapon != PendingWeapon )
		Weapon.PutDown();

	return (usealt > 0);
}

//*************************************************************************************************
// Head-look support
//*************************************************************************************************
//	MakeHeadLookAtDirectionOfRotator	-- Point head in the relative direction indicated by a rotator
//	MakeHeadLookAtDirectionOfVector		-- Point head in the relative direction indicated by a vector
//	MakeHeadLookAtPosition				-- Point head in the absolute direction of the indicated point in space
//	MakeHeadLookAtActor					-- Point head in the absolute direction of the indicated actor
//	MakeHeadWatchPosition				-- Continually look at a position
//	MakeHeadWatchTarget					-- Continually look at a target, with an optional offset
//	StopHeadLook						-- Turn off explicit head pointing
//*************************************************************************************************

function MakeHeadLookInDirectionOfRotator( rotator NewLookRot )
{
	// Make head look in the direction indicated by the given rotator, relative to self
	if( HeadLookAnimChannel == None )
		CreateHeadLookAnimChannel();

	if( HeadLookAnimChannel != None )
		HeadLookAnimChannel.LookInDirectionOfRotator( NewLookRot );
}

function MakeHeadLookInDirectionOfVector( vector NewLookDir )
{
	// Make head look in the direction indicated by the given vector, relative to self
	if( HeadLookAnimChannel == None )
		CreateHeadLookAnimChannel();

	if( HeadLookAnimChannel != None )
		HeadLookAnimChannel.LookInDirectionOfVector( NewLookDir );
}

function MakeHeadLookAtPosition( vector NewLookPosition )
{
	// Make head look in the direction of the indicated point in space
	if( HeadLookAnimChannel == None )
		CreateHeadLookAnimChannel();

	if( HeadLookAnimChannel != None )
		HeadLookAnimChannel.LookAtPosition( NewLookPosition );
}

function MakeHeadLookAtActor( actor NewLookTarget, optional vector Offset )
{
	// Make head look in the direction of the indicated actor in space, with an optional offset
	if( HeadLookAnimChannel == None )
		CreateHeadLookAnimChannel();

	if( HeadLookAnimChannel != None )
		HeadLookAnimChannel.LookAtActor( NewLookTarget, Offset );
}

function MakeHeadWatchPosition( vector NewWatchPosition )
{
	// Make head continually look at the indicated point in space
	if( HeadLookAnimChannel == None )
		CreateHeadLookAnimChannel();

	if( HeadLookAnimChannel != None )
		HeadLookAnimChannel.WatchPosition( NewWatchPosition );
}

function MakeHeadWatchActor( actor NewWatchTarget, optional vector Offset )
{
	// Make head continually look at the indicated actor in space, with an optional offset
	if( HeadLookAnimChannel == None )
		CreateHeadLookAnimChannel();

	if( HeadLookAnimChannel != None )
		HeadLookAnimChannel.WatchActor( NewWatchTarget, Offset );
}

function StopHeadLook()
{
	// Stop making head look at anything
	if( HeadLookAnimChannel != None )
		HeadLookAnimChannel.StopLooking();
}

//*************************************************************************************************
function PlayFacialAnim(name AnimName, bool bDefaultAnim, float TweenTime, float HoldTime)
{
	if( FacialAnimChannel == none )
	{
		FacialAnimChannel = cFacialAnimChannel( CreateAnimChannel(class'cFacialAnimChannel', AT_Replace, 'brow_root') );

		if( FacialAnimChannel == none )
			return;

		FacialAnimChannel.SetOwner( self );
		FacialAnimChannel.bAnimNotReplaceable = true;
	}

	//Tweek any special anim names
	switch( AnimName )
	{
		case 'normal':  case 'rest':  case 'calm':
			AnimName = 'brow_rest';	  break;
		case 'mad':  case 'upset':  case 'angry':  case 'annoyed':  case 'scolding':  case 'mean':
		    AnimName = 'brow_1';      break;
		case 'sad':  case 'worried':  case 'scared':
			AnimName = 'brow_2';      break;
		case 'whisper':  case 'firm':
			AnimName = 'brow_3';      break;
		case 'happy':  case 'excited':  case 'suprised':  case 'surprised':
			AnimName = 'brow_4';      break;
		case 'smug':  case 'sarcastic':  case 'question':
			AnimName = 'brow_5';      break;
	}

	FacialAnimChannel.PlayFacialAnim( AnimName, bDefaultAnim, TweenTime, HoldTime );
}

//*************************************************************************************************

State Dying
{
ignores SeePlayer, EnemyNotVisible, HearNoise, KilledBy, Trigger, Bump, HitWall, HeadZoneChange, FootZoneChange, ZoneChange, Falling, WarnTarget, Died, LongFall, PainTimer;

	function TakeDamage( int Damage, Pawn instigatedBy, Vector hitlocation, 
							Vector momentum, name damageType)
	{
		if ( bDeleteMe )
			return;
		Health = Health - Damage;
		Momentum = Momentum/Mass;
		AddVelocity( momentum ); 
		if ( !bHidden && Gibbed(damageType) )
		{
			bHidden = true;
			SpawnGibbedCarcass();
			if ( bIsPlayer )
				HidePlayer();
			else
				Destroy();
		}
	}

	function Timer()
	{
		if ( !bHidden )
		{
			bHidden = true;
			SpawnCarcass();
			if ( bIsPlayer )
				HidePlayer();
			else
				Destroy();
		}
	}

	event Landed(vector HitNormal)
	{
		SetPhysics(PHYS_None);
	}

	function BeginState()
	{
		SetTimer(0.3, false);
	}
}

state GameEnded
{
ignores SeePlayer, HearNoise, KilledBy, Bump, HitWall, HeadZoneChange, FootZoneChange, ZoneChange, Falling, TakeDamage, WarnTarget, Died;

	function BeginState()
	{
		SetPhysics(PHYS_None);
		HidePlayer();
	}
}

//***********************************************************************************************************
//When these actions are done, they call the generic method OnEvent( 'ActionDone' );
function DoMoveTo(vector v0, optional bool in_bSnapToLocIfYouDontMakeIt, optional float speed
                , optional vector v1, optional vector v2, optional vector v3, optional vector v4)
{
	GotoState( 'stateMovingToLoc' );
	//vMoveToLoc = v;
	vMoveToLoc[0] = v0;
	vMoveToLoc[1] = v1;
	vMoveToLoc[2] = v2;
	vMoveToLoc[3] = v3;
	vMoveToLoc[4] = v4;
	fMoveToSpeed = speed;

	MoveToLocIdx = 0;

	bSnapToLocIfYouDontMakeIt = in_bSnapToLocIfYouDontMakeIt;
}


//***********************************************************************************************************
function DoWalkTo(vector v0, optional bool in_bSnapToLocIfYouDontMakeIt, optional float speed
                , optional vector v1, optional vector v2, optional vector v3, optional vector v4)
{
	bDoingWalk = true;
	DoMoveTo( v0, in_bSnapToLocIfYouDontMakeIt, speed, v1, v2, v3, v4 );
}


//***********************************************************************************************************
function DoRunTo(vector v0, optional bool in_bSnapToLocIfYouDontMakeIt, optional float speed
                , optional vector v1, optional vector v2, optional vector v3, optional vector v4)
{
	bDoingWalk = false;
	DoMoveTo( v0, in_bSnapToLocIfYouDontMakeIt, speed, v1, v2, v3, v4 );
}

//***********************************************************************************************************
function DoTurnTo( actor a, optional bool bDoSnap, optional bool bFollowActor, optional bool bFollowActorPermanent, optional bool in_bSendQWhenDone )
{
	local rotator TempRotator;

	//Snap our character now, if need be
	if( bDoSnap )
	{
		TempRotator = Rotation;
		TempRotator.Yaw = rotator(a.Location - Location).Yaw;
		SetRotation( TempRotator ); //just do it now.
		DesiredRotation = TempRotator;
	}

	TurnTo_TargetActor = a;
	bTurnTo_FollowActor = bFollowActor;
	//bTurnTo_FollowActorPermanent = bFollowActorPermanent;
	bSendQWhenDone = in_bSendQWhenDone;

	if( bFollowActorPermanent )
		MakeTurnToPermanentController();
	else
		GotoState( 'stateTurningTo' );
}

//***********************************************************************************************************
function MakeTurnToPermanentController()
{
	local TurnToController   a;

	a = spawn(class'TurnToController' );
	a.Init( self );
}

//***********************************************************************************************************
function DestroyTurnToPermanentController()
{
	local TurnToController   a;

	ForEach AllActors(class'TurnToController', a)
		if( a.owner == self )
			a.Destroy();
}

//***********************************************************************************************************
function OnEvent(name EventName)
{
	if( EventName == 'ActionDone' )
	{
	}
}

state stateMovingToLoc
{
	function EndState()
	{
		if( IsA('harry') )
			level.playerHarryActor.ClientMessage("**** Harry Left state stateMovingToLoc");

		//Try and restore groundSpeed back to running.  This potentially could fail if someone sets bDoingWalk to true
		// before this EndState func is called.
		if( bDoingWalk )
		{
			RestoreNormalMovementSpeed();

			//if( GroundWalkSpeed != 0 )
			//	GroundSpeed = GroundWalkSpeed;
			bDoingWalk = false;
		}

		bNoZoneFriction = false;
	}

  Begin:

	bNoZoneFriction = true;

	MovingToInit();
  Loop:
	MoveTo( vMoveToLoc[MoveToLocIdx] );
	DesiredRotation.Yaw = Rotation.Yaw;
	if( bSnapToLocIfYouDontMakeIt  &&  VSize2d(Location-vMoveToLoc[MoveToLocIdx]) > 16 )
		SetLocation2( vMoveToLoc[MoveToLocIdx] );

	MoveToLocIdx++;
	if(   MoveToLocIdx < NUM_WALKTO_LOCATIONS
	   && vMoveToLoc[MoveToLocIdx] != vect(0,0,0)
	  )
		goto 'Loop';
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	//if( Physics
	AirSpeed = 0;

	LoopAnim( IdleAnimName, 1.0, 1.0 );

	if( bDoingWalk )
		RestoreNormalMovementSpeed();
	bDoingWalk = false;

	OnEvent( 'ActionDone' );
	DoCutCueNotify();
}

//**********************************************************************************************
function PlayIdleAnim()
{
	LoopAnim( IdleAnimName, ,0.75 );
}

//***************************************************************
//private function
function MovingToInit()
{
	local float AnimScale;
	local name  AnimName;

	AnimScale = 1;

	if( bDoingWalk )
	{
		//Check alternate move speed
		if( fMoveToSpeed != 0 )
		{
			GroundSpeed = fMoveToSpeed;

			if( GroundWalkSpeed != 0 )
				AnimScale = fMoveToSpeed / GroundWalkSpeed;
		}
		else
		if( GroundWalkSpeed != 0 )
			GroundSpeed = GroundWalkSpeed;

		AnimName = WalkAnimName;
	}
	else
	{
		if( fMoveToSpeed != 0 )
		{
			GroundSpeed = fMoveToSpeed;
			
			if( GroundRunSpeed != 0 )
				AnimScale = fMoveToSpeed / GroundRunSpeed;
		}
		else
		if( GroundRunSpeed != 0 )
		{
			//Forget that, the defaults are now set in Pawn's defaultproperties
			//If we have a ground run speed, save it off
			//if( GroundWalkSpeed == 0 )
			//	GroundWalkSpeed = GroundSpeed;

			GroundSpeed = GroundRunSpeed;
		}

		AnimName = RunAnimName;
	}

	AirSpeed = GroundSpeed;

	LoopAnim( AnimName, AnimScale, 1.0 );
}

//***********************************************************************************************************
//PlayerPawn overrides this cause their normal movement speed is run
function RestoreNormalMovementSpeed()
{
	//if( GroundWalkSpeed != 0 )
	//	GroundSpeed = GroundWalkSpeed;
	if( GroundRunSpeed != 0 )
		GroundSpeed = GroundRunSpeed;
}

//***********************************************************************************************************
state stateTurningTo
{
  Begin:
	//LoopAnim( MoveToAnimSequence, 1.0, 0.2 );
	MoveTarget = none;
	//if( Physics == PHYS_Flying )
	//{
	//	Velocity = vect(0,0,0);
	//	Acceleration = vect(0,0,0);
	//}
	TurnToward( TurnTo_TargetActor );
	//DesiredRotation.Yaw = Rotation.Yaw;
	if( bTurnTo_FollowActor )
		Goto 'Begin';

	if( bSendQWhenDone )
		DoCutCueNotify();
	
	OnEvent( 'ActionDone' );

	//LoopAnim( IdleAnimName, 1.0, 0.2 );
}

//***********************************************************************************************************
function DoCutCueNotify()
{
	if( CutNotifyActor != None  &&  sCutNotifyCue != "" )
		CutNotifyActor.CutCue( sCutNotifyCue );
	//else
	//	playerHarry.ClientMessage("Couldn't do the DoCutCueNotify.  Cue:"$sCutNotifyCue);
}

//*******************************************************************************************************
function DestroyControllers()
{
	if( SplineManager != none )
	{
		TickParent = none;
		SplineManager.SetOwner( none );
		SplineManager.Destroy();
		SplineManager = none;
	}
}

//********************************************************************************************************************************************
//********************************************************************************************************************************************
//********************************************************************************************************************************************
//********************************************************************************************************************************************

//*******************************************************************************************************
//All params are optional.  If PathTagName is '', it will use SplinePathName.  If speed is 0, it will use each InterpolationPoint's speed.
// if StartPointName is '', it will start at the first InterpolationPoint.  If EndPointName is '', it will follow for ever.
//function FollowSplinePath(  optional name  PathTagName
//                          , optional float speed
//                          , optional float accel
//						  , optional name  StartPointName
//						  , optional name  EndPointName
//						  , optional bool  bSnapToStartLoc
//						  , optional enumMoveType MoveType
//						 )
//{
//	GotoState( 'patrolFollowSpline', 'Begin_ParmsSet' );
//	FollowSplinePath( PathTagName, speed, accel, StartPointName, EndPointName, bSnapToStartLoc, MoveType );
//}

//* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * 
//if speed is zero, use the speeds on each InterpolationPoint in the spline.
function FollowSplinePath(  optional name  PathTagName
	                      , optional float speed
						  , optional float accel
						  , optional name  StartPointName
						  , optional name  EndPointName
						  , optional bool  bSnapToStartLoc
						  , optional enumMoveType MoveType
						  , optional bool  bDontAlignAlongPath
                          , optional float time   //optional time, overrides speed param.  Calcs a new speed based on length/time
						 )
{
	local InterpolationPoint	ip;   //actual start point.
	local InterpolationPoint    StartPoint;

	//Destroy/Inactivate controllers, like FlyTo, spline, patrol, bla bla.
	DestroyControllers();

	if( PathTagName != '' )
		SplinePathName = PathTagName;
	if( SplinePathName == '' )
	{
		cm("***** ERROR: SplinePathName not set");
		return;
	}

	// Put pawn at first point on its path
	foreach AllActors( class'InterpolationPoint', ip, SplinePathName )
	{
		if( StartPointName != '' )
		{
			if( ip.Name == StartPointName )
				StartPoint = ip.Next;
		}
		else
		{
			if( ip.Position == 0 )
				StartPoint = ip.Next;

			StartPointName = ip.Name;  //Annoying, cause StartPoint isnt' the actual start point.
		}

		if( StartPoint != none )
			break;
	}

	if( StartPoint == none )
	{
		cm("***** ERROR: Couldn't find a spline start point.  SplinePathName:"$SplinePathName$" StartPointName:"$StartPointName);
		return;
	}

	//If you have supplied time, try and calculate a speed.  The path also needs to be finite.
	if( time > 0 )
	{
		if( EndPointName == '' )
		{
			cm("***** SPLINE ERROR: time is set, but no spline dest point is specified.  Speed can't be calculated.");
			return;
		}

		speed = CalcSplineSpeedFromTime( time, ip, EndPointName );  //use up, the actual first spline point.
	}

	//Not going to bother making ease to/from work if you are trying to use the IPoint speed setting.
	if( speed == 0 )
		MoveType = MOVE_TYPE_LINEAR;

	if( accel == 0 )
		SplineAccel = 3000;  //default
	else
		SplineAccel = accel;

	if( MoveType != MOVE_TYPE_LINEAR )
		bSnapToStartLoc = true;

	LoopAnim( WalkAnimName, 1.0, 0.75 );

	if( StartPoint != none )
	{
		eSplineMoveType = MoveType;

		SplineSpeed = speed;
		IPSpeed = 0;//SplineSpeed;  //Start off at zero, tick will then set.

		//SetLocation( ip.Location );
		//SetRotation( i.Rotation );
		//SetCollision( true, false, false );
		bCollideWorld = false;
		bInterpolating = true;
		SetPhysics( PHYS_Interpolating );  //This needs to be PHYS_Interpolating instead of PHYS_None, cause if the HPawn lands on a new base, SetBase will set phys to PHYS_Walking.

		//Something's wrong here, like the other Destroy() call didnt' work, even though it was called for sure.  Call Destroy here again, and make a new one.
		if( SplineManager != none )
			SplineManager.Destroy();
		//if( SplineManager == none )

		SplineManager = Spawn( class'SplineManager', self );

		SplineManager.Init( StartPoint, 1.0, bSnapToStartLoc );

		//Dont forget to set FirstSplinePoint and DestSplinePoint.
		FirstSplinePoint = StartPointName;
		DestSplinePoint = EndPointName;

		// Hack?  Maybe.  Probably need a function called GotoStateIdleIfYourInAStateThatIsMovingTheObjectAround
		if( IsInState( 'patrol' ) )
			GotoState( 'stateIdle' );
	}

	//Do an extra loop here, if you want to turn off all the bDontAlignAlongPath tags.  Should fix engine so the actor can do it...
	//if( bDontAlignAlongPath )
	//	foreach AllActors( class'InterpolationPoint', ip, SplinePathName )
	//		{ ip.bFaceMoveDirection = false;       ip.bDontAlterRotation = true; }

	if( bDontAlignAlongPath )
		bInterpolating_IgnoreRot = true;

	if( ip == None )
		Log( Name$" couldn't find path "$SplinePathName );
	else
		Log( Name$" Started interpolation with "$SplineManager.Name );
}

//**********************************************************************
function float CalcSplineSpeedFromTime( float time, InterpolationPoint StartPoint, name EndPointName )
{
	local InterpolationPoint   ip;
	local float                len;

	if( StartPoint.name == EndPointName )
	{
		cm("***** SPLINE ERROR: End point is same as start point");
		return 0;
	}

	ip = StartPoint.Next;

	do
	{
		Log("********** PathDist:"$ip.PathDist);

		len += ip.PathDist;

		if( ip.name == EndPointName )
			break;
		if( ip.name == StartPoint.name )
		{
			cm("***** SPLINE ERROR: Path looped around.  EndPoint "$EndPointName$" couldn't be found");
			return 0;
		}

		ip = ip.next;
	}until(false);

	return len/time;
}

//**********************************************************************
//Called from the spline manager
function SplineTick(float dtime)
{
	local float  CurrentDist;// = PhysAlpha * Dest.PathDist;
	local float  EaseToFromTime; //hard coded for now
	local float  Accel;
	local float  d;

  //This all may be too much for Tick...

	CurrentDist = SplineManager.PhysAlpha * SplineManager.Dest.PathDist;
	EaseToFromTime = 0.75;  //Time you want the ease to take
	EaseToFromTime *= 0.5;  //Because this is ass backwards
	//Accel = 30;

	//quick safety check
	if( SplineAccel == 0 )
		SplineAccel = 200;

	//At what distance should we be at full speed
	d = SplineSpeed * SplineSpeed / SplineAccel;

	//Adjust speed for easing from
	if(   (eSplineMoveType == MOVE_TYPE_EASE_FROM  ||  eSplineMoveType == MOVE_TYPE_EASE_FROM_AND_TO) //If you're supposed to do an ease from
	   && SplineManager.Dest.Prev.Name == FirstSplinePoint //If where you're currently moving from is your start point.
	   && SplineManager.PhysAlpha < 0.5
	  )
	{
		IPSpeed += SplineAccel * dtime;
	}
	else//adjust speed for easing to
	if(   (eSplineMoveType == MOVE_TYPE_EASE_TO  ||  eSplineMoveType == MOVE_TYPE_EASE_FROM_AND_TO) //If you're supposed to do an ease to
	   && SplineManager.Dest.Name == DestSplinePoint //If where your currently moving to is your dest
	   && (CurrentDist > SplineManager.Dest.PathDist - d) //and you're in the "slow down" region.
	  )
	{
		IPSpeed = sqrt( fMax(SplineManager.Dest.PathDist - CurrentDist, 0) * SplineAccel );
	}
	else
	{
		IPSpeed = SplineSpeed;
	}
	//playerHarry.ClientMessage("SplineAccel:"$SplineAccel$" SplineSpeed:"$SplineSpeed$" IPSpeed:"$IPSpeed$" phys:"$Physics$"SM:"$SplineManager);
	//Log("**************** "$self.name$" IPSpeed:"$IPSpeed$" currentrot.yaw:"$rotation.yaw);

	//Simple clamp
	if( IPSpeed > SplineSpeed )
		IPSpeed = SplineSpeed;
	//else
	//if( IPSpeed < 1 ) // Cant do this!!!
	//	IPSpeed = 1;

	//playerHarry.ClientMessage("****** "$name$" splinetick IPSpeed:"$IPSpeed$" SplineManager.PhysAlpha:"$SplineManager.PhysAlpha$" SplineManager.Dest:"$SplineManager.Dest);
	
}

//******************************************************************
state() patrolFollowSpline
{
	function bool ShouldPlayIdleOnRelease() { return false; }

	//* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * 
	function EndState()
	{
		if( SplineManager != none )
			SplineManager.Destroy();
		SplineManager = none;
	}

	//* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * 
  Begin:
	FollowSplinePath( SplinePathName, SplineSpeed, 0, FirstSplinePoint, DestSplinePoint );
  Begin_ParmsSet:
}

//***********************************************************************************
//Called when this pawn is following a spline and it gets to a interpolation point.
//Return true if you want InterpolateEnd() in InterpolationPoint to continue processing the pawn.
function bool PawnAtInterpolationPoint( InterpolationPoint IPoint, InterpolationManager IManager )
{

	if( DestSplinePoint != ''  &&  DestSplinePoint == IPoint.Name )
	{
		if( IManager == SplineManager )
			SplineManager = none;
		//Log("*********** 1 "$name$" " $ Location $ " " $ rotation);
		IManager.SetOwner( none );
		IManager.Destroy();  //Huh, seems like sometimes this doesn't work.  How about most the friggin' time...
		//Log("*********** 2 "$name$" " $ Location $ " " $ rotation);
		OnEvent('ActionDone');
		DoCutCueNotify();
		TickParent = none; //You dont really have to do this, cause DestroyActor will clean this up, but fut the wuk.
		return false;
	}

	if( IPoint.PatrolSound != none )
		PlaySound( IPoint.PatrolSound );

	if( IPoint.EventToSend != '' )
		TriggerEvent( IPoint.EventToSend, self, none );

	if( IPoint.bDestroyPawn )
	{
		IManager.SetOwner( none );
		IManager.Destroy();
		Destroy();
		SplineManager = none;
		return false;
	}

	if( IPoint.PauseTime > 0 )
	{
		LastIPoint = IPoint;
		Velocity = vect(0,0,0);
		bInterpolating = false;
		GotoState( 'stateSplinePause' );
		return false;
	}

	if( IPoint.PatrolAnim != ''  &&  AnimSequence != IPoint.PatrolAnim )
		LoopAnim( IPoint.PatrolAnim, , 0.75 );

	return true;
}

//**********************************************************************************
state stateSplinePause
{
  Begin:
	if( LastIPoint.bUseLookDir )
	{
		SplineRotSave = Rotation;
		SetPhysics( PHYS_Rotating );  //Have to do this, otherwise TurnTo never returns
		TurnTo( Location + vector(LastIPoint.Rotation) );
	}

	if( LastIPoint.PauseAnim != '' )
		LoopAnim( LastIPoint.PauseAnim, , LastIPoint.tweenTime );
	else
	if( IdleAnimName != '' )
		LoopAnim( IdleAnimName, , LastIPoint.tweenTime );
	
	Sleep( LastIPoint.PauseTime );

	if( LastIPoint.PatrolAnim != '' )
		LoopAnim( LastIPoint.PatrolAnim, , LastIPoint.tweenTime );
	else
	if( WalkAnimName != '' )
		LoopAnim( WalkAnimName, , LastIPoint.tweenTime );
	else
	if( IdleAnimName != '' )
		LoopAnim( IdleAnimName, , LastIPoint.tweenTime );

	if( LastIPoint.bUseLookDir )
		TurnTo( Location + vector(SplineRotSave) );

	FollowSplinePath( [speed]SplineSpeed, [StartPointName]LastIPoint.Name );
}

//*********************************************************************************************************************
//*********************************************************************************************************************
//*********************************************************************************************************************
//*********************************************************************************************************************
//*********************************************************************************************************************

//***********************************************************************************************************
//command is like "MoveTo Target Arg1 Arg2 ....."
//cue is the cue to send when the action is complete. Save this somewhere for when the command is finished.
//bFastFlag is for the fastforward stuff. Its not hooked up to anything yet tho.
function bool CutCommand(string command, optional string cue, optional bool bFastFlag)
{
	local string  sActualCommand;

	//playerHarry.ClientMessage(self $ " Got a Command:"$command);
	sActualCommand = ParseDelimitedString( command, " ", 1, false );

	if( sActualCommand ~= "WalkTo" )
	{
		return CutCommand_WalkTo(command, cue, bFastFlag, false/*walk*/);
	}
	else
	if( sActualCommand ~= "SetIdle" )
	{
		IdleAnimName = name(ParseDelimitedString( command, " ", 2, false ));
		CutCue(cue);
		return true;
	}
	else
	if( sActualCommand ~= "Animate" )
	{
		return CutCommand_Animate(command,cue,bFastFlag);
	}
	else 
	if( sActualCommand ~= "RunTo" )
	{
		return CutCommand_WalkTo(command, cue, bFastFlag, true/*run*/);
	}
	else 
	if(   sActualCommand ~= "Face"
	   || sActualCommand ~= "TurnTo"
	  )
	{
		return CutCommand_TurnTo( command, cue, bFastFlag );
	}
	else
	if( sActualCommand ~= "StopTurnToFollowing" )
	{
		DestroyTurnToPermanentController();
	}
	else 
	if( sActualCommand ~= "SetRotateRate" )
	{
		RotationRate.yaw = float( ParseDelimitedString( command, " ", 2, false ) )/360 * 65536;
		return true;
	}
	else 
	if( sActualCommand ~= "AttachActorToBone" )
	{
		return CutCommand_AttachActorToBone( command, cue, bFastFlag );
	}
	else 
	if( sActualCommand ~= "UnAttachActor" )
	{
		return CutCommand_UnAttachActor( command, cue, bFastFlag );
	}
	else
	if( sActualCommand ~= "Talk" )
	{
		return CutCommand_Talk(command,cue,bFastFlag);
	}
	else
	if( sActualCommand ~= "PlayFacialAnim" )
	{
		return CutCommand_PlayFacialAnim( command, cue, bFastFlag );
	}
	else
	if( sActualCommand ~= "FollowSpline" )
	{
		return CutCommand_FollowSpline( command, cue, bFastFlag );
	}

	return  super.CutCommand(command, cue, bFastFlag);
}

//************************************************************************************************************
//PlayFacialAnim          will play the DefaultAnim
//PlayFacialAnim Brow_2   the name of the anim to play
// options:  default      will stay as the default expression for this character, after other anims have been done
// options:  HoldTime=n   will hold then go back to default
// options:  TweenTime=n  passes through as the tween time.  default is 0.2
//  normal mad sad whisper happy smug
function bool CutCommand_PlayFacialAnim(string command, optional string cue, optional bool bFastFlag)
{
	local actor        a;
	local string       sString;
	local int          i;
	local name         AnimName;
	local bool         bDefaultAnim;
	//local bool         bBackToNormalWhenDone;
	local float        HoldTime, TweenTime;

	i = 2;

	AnimName = name( ParseDelimitedString( command, " ", i, false ) );
	if( AnimName == 'default' )
		AnimName = '';
	i++;  //skip this parameter...

	if( bFastFlag )
	{
		//Do what needs to get done NOW, then call the cut cue...
		CutCue( cue );
		return true;
	}

	for( i=i; i < 20; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
		else
		if( Left(sString, len("TweenTime=")) ~= "TweenTime=" )
		{
			TweenTime = float( Mid(sString, len("TweenTime=")) );
		}
		else
		if( Left(sString, len("HoldTime=")) ~= "HoldTime=" )
		{
			HoldTime = float( Mid(sString, len("HoldTime=")) );
		}
		else
		if( sString ~= "default" )
		{
			bDefaultAnim = true;
		}
	}

	//Do your stuff...
	PlayFacialAnim(AnimName, bDefaultAnim, TweenTime, HoldTime);
	
	CutCue( cue );

	//And set the cut notify cue
	//sCutNotifyCue = cue;

	return true;
}


//************************************************************************************************************************
function bool CutCommand_Talk(string command, optional string cue, optional bool bFastFlag)
{
local string sString;
local name animName;
local string sAnimName;
local int i;

	if(bFastFlag)
		{
		CutCue(cue);
		return(true);
		}

		//Find and set the animation parameters.	
	for( i=0; i < 20; i++ )
		{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
		else
		if( Left(sString, len("STARTANIM=")) ~= "STARTANIM=" )
			{
				sAnimName=Mid(sString, len("STARTANIM="));
				animName=StringToAnimName(sAnimName);
				if(animName!='')
					TalkStartAnimName=animName;	
			}
		else
		if( Left(sString, len("LOOPANIM=")) ~= "LOOPANIM=" )
			{
				sAnimName=Mid(sString, len("LOOPANIM="));
				animName=StringToAnimName(sAnimName);
				if(animName!='')
					TalkLoopAnimName=animName;	
			}
		if( Left(sString, len("ENDANIM=")) ~= "ENDANIM=" )
			{
				sAnimName=Mid(sString, len("ENDANIM="));
				animName=StringToAnimName(sAnimName);
				if(animName!='')
					TalkEndAnimName=animName;	
			}
		}

		//Start the regular say command.
	if( CutCommand_Say(command,cue,bFastFlag) )
		{
			//succesful so start talk anim.
		GotoState('DoingTalk');
		return true;
		}
	else
		return false;	//say errored somehow.

}

//*************************************************************************************************************************

state DoingTalk
{
	function CutCue(string cue)
	{
		if(TalkEndAnimName!='')
			PlayAnim(TalkEndAnimName, , 0.75, [Type]AT_Combine);
		super.CutCue(cue);	//pass the cue back to the cutscene.
	}

	function AnimEnd()
	{
		if( AnimSequence == TalkEndAnimName )
			PlayIdleAnim();
	}

  begin:

	if(TalkStartAnimName!='')
	{
		PlayAnim(TalkStartAnimName, ,0.75, [Type]AT_Combine);
		finishAnim();
	}

	if(TalkLoopAnimName!='')
		LoopAnim(TalkLoopAnimName, ,0.75, [Type]AT_Combine);
	else
		PlayIdleAnim();
}
//**********************************************************************************************************
//WalkTo CutName [cutname2] [cutname3]...[cutname15] [speed=rate]
function bool CutCommand_WalkTo(string command, optional string cue, optional bool bFastFlag, optional bool bRun)
{
	local string  sCutName;
	local actor   a;
	local int     i;
	local int     count;
	local float   speed;
	local vector  v[5];

	i = 2;

	do
	{
		sCutName = ParseDelimitedString( command, " ", i, false );
		//break out if there's an '=' in the string, or if it's empty
		if( instr(sCutName,"=") != -1  ||  sCutName=="" )
			break;
		ForEach AllActors( class'actor', a )
			if( a.CutName ~= sCutName )
				break;
		if( a == none )
		{
			CutErrorString = "No actor with CutName '" $ sCutName $ "'";
			CutNotifyActor.CutCue( cue );
			return false;
		}
		if( count >= NUM_WALKTO_LOCATIONS )
		{
			CutErrorString = "Too many runto/walkto locations.  Just use another runto/walkto line...";
			CutNotifyActor.CutCue( cue );
			return false;
		}
		v[count] = a.Location;
		count++;
		i++;
	}

	if( count == 0 )
	{
		CutErrorString = "No runto/walkto locations defined.";
		CutNotifyActor.CutCue( cue );
		return false;
	}

	for( i=i; i < 20; i++ )
	{
		sCutName = ParseDelimitedString( command, " ", i, false );

		if( sCutName == "" )
			break;

		if( Left(sCutName, 6) ~= "speed=" )
			speed = float( Mid(sCutName, 6) );
	}

	if( bFastFlag )
	{
		SetLocation2( v[count-1] );
		CutNotifyActor.CutCue( cue );
		return true;
	}

	if( a != none )
	{
		if( bRun )
			DoRunTo( v[0], true, speed, v[1],v[2],v[3],v[4] );
		else
			DoWalkTo(v[0], true, speed, v[1],v[2],v[3],v[4] );

		sCutNotifyCue = cue;
	}

	return true;
}

//**********************************************************************************************************
//Face/TurnTo  CutName  [Follow]  [rate=DegreesPerSecond] [FollowPermanent]
// FollowPermanent will keep following even if the state changes.  This option is added so that existing "follow"
// commands dont break existing cutscenes.  You use StopTurnToFollowing to turn it off.
function bool CutCommand_TurnTo(string command, optional string cue, optional bool bFastFlag)
{
	local string  sString;
	local bool    bDoSnap;
	local bool    bDoFollow;
	local bool    bDoFollowPermanent;
	local rotator TempRotator;
	local string  sActualCommand;
	local string  sCutName;
	local actor   a;
	local int     i;
	local bool    bSendQWhenDone;

	//playerHarry.ClientMessage(self $ " Got a Command:"$command);
	sActualCommand = ParseDelimitedString( command, " ", 1, false );

	if( sActualCommand ~= "Face" )
		bDoSnap = true; //do a snap

	sCutName = ParseDelimitedString( command, " ", 2, false );

	ForEach AllActors( class'actor', a )
		if( a.CutName ~= sCutName )
			break;

	if( a == none )
	{
		CutErrorString = "No Actor with CutName '" $ sCutName $ "'";
		CutNotifyActor.CutCue( cue );
		return false;
	}

	if( bFastFlag )
	{
		TempRotator = Rotation;
		TempRotator.Yaw = rotator(a.Location - Location).Yaw;
		SetRotation( TempRotator ); //just do it now.
		DesiredRotation = TempRotator;
		CutNotifyActor.CutCue( cue );
		return true;
	}

	for( i = 3; i < 20; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;

		if( sString ~= "Follow" )
		{
			bDoFollow = true;
		}
		if( sString ~= "FollowPermanent" )
		{
			bDoFollow = true;
			bDoFollowPermanent = true;
		}
		else
		if( Left(sString, 5) ~= "rate=" )
		{
			RotationRate.yaw = float( Mid(sString, 5) )/360 * 65536;
		}
	}

	//If we are doing a turnto with NO snap, and NO follow, we'll want to call the cue later, otherwise we'll call it now.
	if( !bDoSnap  &&  !bDoFollow )
		bSendQWhenDone = true;

	//If we're supposed to follow, or we DONT snap, use the turn to state, otherwise do it now.
	if( bDoFollow  ||  !bDoSnap)
	{
		DoTurnTo( a, bDoSnap, bDoFollow, bDoFollowPermanent, bSendQWhenDone );
	}
	else
	{
		TempRotator = Rotation;
		TempRotator.Yaw = rotator(a.Location - Location).Yaw;
		SetRotation( TempRotator ); //just do it now.
		DesiredRotation = TempRotator;
	}

	if( bSendQWhenDone )
		sCutNotifyCue = cue;
	else
		CutNotifyActor.CutCue( cue );

	return true;
}

//*************************************************************************************************************************
function bool CutCommand_Animate(string command, optional string cue, optional bool bFastFlag)
{
local string sString;
local name animName;
local string sAnimName;
local int i;

	if(bFastFlag)
		{
		CutCue(cue);
		return(true);
		}


	sAnimName = ParseDelimitedString( command, " ", 2, false );
	animName=StringToAnimName(sAnimName);

	if(animName!='')
		CutAnimName=animName;	

		//assume no loop
	bCutAnimLoop=false;
	fCutAnimRate = 1;
	fCutAnimTween = 1.0;
	nCutAnimBone = '';

		//Find and set the animation parameters.	
	for( i=0; i < 20; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
		else
		if( Left(sString, len("LOOP")) ~= "LOOP" )
			bCutAnimLoop=true;
		else
		if( Left(sString, len("rate=")) ~= "rate=" )
			fCutAnimRate = float( Mid(sString, len("rate=")) );
		else
		if( Left(sString, len("tween=")) ~= "tween=" )
			fCutAnimTween = float( Mid(sString, len("tween=")) );
		else
		if( Left(sString, len("bone=")) ~= "bone=" )
			nCutAnimBone = name( Mid(sString, len("bone=")) );
	}

	if(animName!='')
	{
		// for looping anims, need to do it now, and return.  Going to the DoingCutAnimate state isn't nessesary, and it interrupts stuff like TurnTo and walkto.
		if( bCutAnimLoop )
		{
			if( nCutAnimBone == '' )
				LoopAnim( CutAnimName, fCutAnimRate, fCutAnimTween );
			else
				LoopAnim( CutAnimName, fCutAnimRate, fCutAnimTween, , AT_Combine, nCutAnimBone );

			CutCue(cue);
			return true;
		}
		else //not looping
		{
			//If we have a sub bone anim, we also have to do it now.
			if( nCutAnimBone != '' )
			{
				PlayAnim( CutAnimName, fCutAnimRate, fCutAnimTween, AT_Combine, nCutAnimBone );
				CutCue(cue);
				return true;
			}
		}
			
		//If we make it here, we have to use DointCutAnimate so that the appropriate cues can be dealt with.
		sCutNotifyCue=cue;
		GotoState('DoingCutAnimate');
		return true;
	}
	else
	{
		CutCue(cue);
		CutErrorString="Cant find Animation named:"$sAnimName;
		return false;	//errored somehow.
	}

}

//*************************************************************************************************************************
// AttachActorToBone ActorCutMarkName BoneName
function bool CutCommand_AttachActorToBone(string command, optional string cue, optional bool bFastFlag)
{
	local actor        a;
	local name         BoneName;
	local string       sString;
	local int          i;

	i = 2;

	//This is if you need to look for an actor
	if( true )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		ForEach AllActors( class'actor', a )
			if( a.CutName ~= sString )
				break;
		if( a == none )
		{
			CutErrorString = "No Actor with CutName '" $ sString $ "'";
			CutCue( cue );
			return false;
		}
		i++;  //skip this parameter...


		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
		{
			CutErrorString = "No Bone name specified";
			CutCue( cue );
			return false;
		}
		BoneName = name(sString);
		i++;  //skip this parameter...
	}

	//Do your stuff...
	a.SetOwner( self );
	a.AttachToOwner( BoneName );
	
	// make sure there is no collision now that we are carrying this object
	a.SetCollision( false, false, false );


	CutCue( cue );
	return true;
}

//**** **** **** **** **** **** **** **** **** **** **** **** **** **** **** **** **** **** **** **** **** **** **** 
// UnAttachActor ActorCutMarkName
function bool CutCommand_UnAttachActor(string command, optional string cue, optional bool bFastFlag)
{
	local actor        a;
	local string       sString;
	local int          i;

	i = 2;

	//This is if you need to look for an actor
	if( true )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		ForEach AllActors( class'actor', a )
			if( a.CutName ~= sString )
				break;
		if( a == none )
		{
			CutErrorString = "No Actor with CutName '" $ sString $ "'";
			CutCue( cue );
			return false;
		}
		i++;  //skip this parameter...
	}

	//Do your stuff...
	a.SetPhysics(PHYS_Falling);
	a.AnimBone		= 0;		// <- detach ourself from any holdingBone
	//Only mess with bCollideActors
	a.SetCollision( bCollideActors );
	a.bCollideWorld  = true;

	//Toss it away
	a.Velocity = (normal((a.Location - Location) * vect(1,1,0)) + vect(0,0,1.5)) * 75;
	
	// clear our holding actor var
	a.SetOwner( None );


	CutCue( cue );
	return true;
}

//*************************************************************************************************************************
state DoingCutAnimate
{
  begin:
	if(CutAnimName!='')
	{
		if(!bCutAnimLoop)
		{
			if( nCutAnimBone == '' )
				PlayAnim( CutAnimName, fCutAnimRate, fCutAnimTween );
			else
				PlayAnim( CutAnimName, fCutAnimRate, fCutAnimTween, AT_Combine, nCutAnimBone );

			finishAnim();
			CutCue(sCutNotifyCue);
		}
		else
		{
			CutCue(sCutNotifyCue);	//finish right away.

			if( nCutAnimBone == '' )
				LoopAnim( CutAnimName, fCutAnimRate, fCutAnimTween );
			else
				LoopAnim( CutAnimName, fCutAnimRate, fCutAnimTween, , AT_Combine, nCutAnimBone );
		}

	}

	PlayIdleAnim();
}

//*************************************************************************************************************************
// FollowSpline SplineTag start=StartNodeCutName dest=DestNodeCutName speed=x accel=x [EaseFrom/EaseTo/EaseBetween] time=t
//  time param overrides the speed param
function bool CutCommand_FollowSpline(string command, optional string cue, optional bool bFastFlag)
{
	local actor              a;
	local string             sString;
	local int                i;
	local name               SplineTag;
	local name               StartActorName;
	local name               DestActorName;
	local float              speed;
	local float              time;
	local float              accel;
	local InterpolationPoint ip;
	local enumMoveType       MoveType;
	local bool               bAlignAlongSpline;

	MoveType = MOVE_TYPE_LINEAR;
	i = 2;

	SplineTag = name(ParseDelimitedString( command, " ", 2, false ));
	i++;  //skip the parameter...

	for( i=i; i < 20; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
		else
		if( Left(sString, Len("start=")) ~= "start=" )
		{
			sString = Mid(sString, Len("start="));
			ForEach AllActors( class'actor', a )
				if( a.CutName ~= sString )
					break;

			if( a == none )
			{
				CutErrorString = "No Actor with CutName '" $ sString $ "'";
				CutNotifyActor.CutCue( cue );
				return false;
			}

			StartActorName = a.Name;
		}
		else
		if( Left(sString, 5) ~= "dest=" )
		{
			sString = Mid(sString, 5);
			ForEach AllActors( class'actor', a )
				if( a.CutName ~= sString )
					break;

			if( a == none )
			{
				CutErrorString = "No Actor with CutName '" $ sString $ "'";
				CutNotifyActor.CutCue( cue );
				return false;
			}

			DestActorName = a.Name;
		}
		else
		if( Left(sString, Len("speed=")) ~= "speed=" )
		{
			speed = float(Mid(sString, Len("speed=")));
		}
		else
		if( Left(sString, Len("time=")) ~= "time=" )
		{
			time = float(Mid(sString, Len("time=")));
		}
		else
		if( Left(sString, Len("accel=")) ~= "accel=" )
		{
			accel = float(Mid(sString, Len("accel=")));
		}
		else
		if( sString ~= "EaseFrom" )
			MoveType = MOVE_TYPE_EASE_FROM;
		else
		if( sString ~= "EaseTo" )
			MoveType = MOVE_TYPE_EASE_TO;
		else
		if( sString ~= "EaseBetween" )
			MoveType = MOVE_TYPE_EASE_FROM_AND_TO;
		else
		if( sString ~= "Align" )
			bAlignAlongSpline = true;
		else
		{
			cm( "***** ERROR:"$self$":FollowSpline option '"$sString$"' not recognised.  Ignoring.");
		}
	}

	//Error check
	if( DestActorName == ''  &&  time != 0 )
	{
		cm( "  *** WARNING:"$self$":FollowSpline: Cant have time=n with no dest point.  Use Speed=n instead");
		time = 0;
	}		

	if( bFastFlag )
	{
		if( DestActorName != '' )
			ForEach AllActors(class'InterpolationPoint', ip)  //who cares if we do another AllActors...
				if( ip.Name == DestActorName )
					{ SetLocation( ip.Location );    SetRotation( rotator(ip.StartControlPoint) );     CutNotifyActor.CutCue( cue );     break; }
	}
	else
	{
		//Do your stuff...
		FollowSplinePath(   SplineTag
						  , speed
	        			  , accel
						  , StartActorName
						  , DestActorName
						  , false
						  , MoveType
		                  , !bAlignAlongSpline  //do or dont align along spline path
		                  , time
						);
	}

	if( DestActorName != '' )
		sCutNotifyCue = cue;
	else
		CutNotifyActor.CutCue( cue );

	return true;
}

//**********************************************************************************************************
defaultproperties
{
	 NameArticle=" a "
     OrthoZoom=+40000.000000
     FovAngle=+00090.000000
     Health=100
     Visibility=128
     SightRadius=+02500.000000
     HearingThreshold=+00001.000000
     AttitudeToPlayer=ATTITUDE_Hate
     Intelligence=BRAINS_MAMMAL
     MaxDesiredSpeed=+00001.000000
     GroundSpeed=+00320.000000
	GroundRunSpeed=210
	GroundWalkSpeed=80
     WaterSpeed=+00200.000000
     AccelRate=+00500.000000
     JumpZ=+00325.000000
     MaxStepHeight=+00025.000000
	 StepHeight=+16.000000
	 StepWidth=+32.000000
     noise1time=-00010.000000
     noise2time=-00010.000000
     AvgPhysicsTime=+00000.100000
     SoundDampening=+00001.000000
     DamageScaling=+00001.000000
     bDirectional=True
     bCanTeleport=True
     bIsKillGoal=True
	 bStasis=false
	 bIsPawn=True
     SoundRadius=9
	 SoundVolume=240
	 TransientSoundVolume=+00002.000000
     bCollideActors=True
     bCollideWorld=True
     bBlockActors=True
     bBlockPlayers=True
     bProjTarget=True
     bRotateToDesired=True
     RotationRate=(Pitch=4096,Yaw=50000,Roll=3072)
	 Texture=S_Pawn
     RemoteRole=ROLE_SimulatedProxy
	 AnimSequence=Fighter
     NetPriority=+00002.000000
	 PlayerRestartState=PlayerWalking
	 PlayerReplicationInfoClass=Class'Engine.PlayerReplicationInfo'
	 AirControl=+0.05

	IdleAnimName="idle"
	WalkAnimName="walk"
	RunAnimName="run"
	TalkAnimName="talk"
	RotateLeftAnimName="idle";
	RotateRightAnimName="idle";

}

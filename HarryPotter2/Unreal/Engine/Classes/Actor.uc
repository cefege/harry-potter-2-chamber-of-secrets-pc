//=============================================================================
// Actor: The base class of all actors.
// This is a built-in Unreal class and it shouldn't be modified.
//=============================================================================
class Actor extends Object
	abstract
	native
	nativereplication;

// Imported data (during full rebuild).
#exec Texture Import File=Textures\S_Actor.pcx Name=S_Actor Mips=Off Flags=2

// Flags.
var(Advanced) const bool  bStatic;       // Does not move or change over time.
var(Advanced) bool        bHidden;       // Is hidden during gameplay.
var(Advanced) const bool  bNoDelete;     // Cannot be deleted during play.
var bool				  bAnimFinished; // Unlooped animation sequence has finished.
var(Display) bool		  bAnimLoop;     // Whether animation is looping.
var(Display) bool         bAnimMove;     // Movement-driven animation.
var bool				  bAnimNotify;   // Whether a notify is applied to the current sequence.
var bool				  bAnimByOwner;	 // Animation dictated by owner.
var bool                  bAnimTransient;// Whether this channel exists only for duration of animation.
var	bool				  bAnimNotReplaceable;	// Won't be replaced by upstream subsequent call with AT_Replace
var const bool            bDeleteMe;     // About to be deleted.
var transient const bool  bAssimilated;  // Actor dynamics are assimilated in world geometry.
var transient const bool  bTicked;       // Actor has been updated.
var transient bool        bLightChanged; // Recalculate this light's lighting now.
var bool                  bDynamicLight; // Temporarily treat this as a dynamic light.
var bool                  bTimerLoop;    // Timer loops (else is one-shot).

// Other flags.
var(Advanced) bool		bLabel;
var(Advanced) bool        bCanTeleport;  // This actor can be teleported.
var(Advanced) bool        bHiddenEdGroup;// Is hidden by the group brower.
var(Advanced) bool		bOwnerNoSee;	 // Everything but the owner can see this actor.
var(Advanced) bool      bOnlyOwnerSee;   // Only owner can see this actor.
var Const     bool		bIsMover;		 // Is a mover.
var(Advanced) bool		bAlwaysRelevant; // Always relevant for network.
var Const	  bool		bAlwaysTick;     // Update even when players-only.
var(Advanced) bool        bHighDetail;	 // Only show up on high-detail.
var(Advanced) bool		  bStasis;		 // In StandAlone games, turn off if not in a recently rendered zone turned off if  bCanStasis  and physics = PHYS_None or PHYS_Rotating.
var(Advanced) bool		  bForceStasis;	 // Force stasis when not recently rendered, even if physics not none or rotating.
var const	  bool		  bIsPawn;		 // True only for pawns.
var(Advanced) const bool  bNetTemporary; // Tear-off simulation in network play.
var(Advanced) const bool  bNetOptional;  // Actor should only be replicated if bandwidth available.
var			  bool		  bReplicateInstigator;	// Replicate instigator to client (used by bNetTemporary projectiles).
var			  bool		  bTrailerSameRotation;	// If PHYS_Trailer and true, have same rotation as owner.
var			  bool		  bTrailerPrePivot;	// If PHYS_Trailer and true, offset from owner by PrePivot.
var			  bool		  bClientAnim;
var			  bool		  bSimFall;			// dumb proxy should simulate fall

var				bool		bCanMoveInSpecialPause;		// Can the object move when we are in special
														// pause mode
var				bool		bInSpecialPause;
// Priority Parameters
// Actor's current physics mode.
var(Movement) const enum EPhysics
{
	PHYS_None,
	PHYS_Walking,
	PHYS_Falling,
	PHYS_Swimming,
	PHYS_Flying,
	PHYS_Rotating,
	PHYS_Projectile,
	PHYS_Rolling,
	PHYS_Interpolating,
	PHYS_MovingBrush,
	PHYS_Spider,
	PHYS_Trailer
} Physics;

// Net variables.
enum ENetRole
{
	ROLE_None,              // No role at all.
	ROLE_DumbProxy,			// Dumb proxy of this actor.
	ROLE_SimulatedProxy,	// Locally simulated proxy of this actor.
	ROLE_AutonomousProxy,	// Locally autonomous proxy of this actor.
	ROLE_Authority,			// Authoritative control over the actor.
};
var ENetRole Role;
var(Networking) ENetRole RemoteRole;
var const transient int NetTag;

// Owner.
var         const Actor   Owner;      // Owner actor.  Engine will call cpp Owner->Tick before doing it's own tick, but not if Owner->TickParent == self.
                                      //  Also, engine will call TickParent->Tick before Owner->Tick.
var               Actor   TickParent; // Normally none, but if !none, like Owner, Engine will call cpp TickParent->Tick.  This takes priority over owner->Tick.
                                      //  For example: You have a Obj1 being moved around by a FlyToController, the FlyToController is moving Obj1 to Obj2 which is moving,
									  //  and Obj3 is attached to obj1 using PhysTrailer.
									  //  Another example: The camera will have it's owner be the CamTarget, and have it's TickParent be a FlyToController.  The FlyToController might
									  //  follow Obj1 which is attached to a bone of Obj2 which is also bing moved by a FlyToController.
									  // Note that you can still get a screen flash if a CutScene object Moves the CamTarget and Camera AFTER they've all been
									  //  updated.  You still need to reorient the camera towards the CamTarget in this case (or make the Camera's TickParent be the CutScene, but then
									  //  you have problems with the TickParent member of camera changing back and forth between say FlyToControllers and the Cutscene actor. That's messy.)
var               Actor   TickParent2;// Just another TickParent variable.  Most likely is only used for the camera.  When the camera is captured, he sets TickParent2 to the CutScene.
                                      //  The camera needs his Owner to point to the CamTarget, his TickParent to point to a FlyToController, and his TickParent2 to point to a CutScene
									  //  object(or something similar) which might move the camera, CamTarget, and other objects that the Camera/CamTarget is supposed to be looking at.
                                      // If we need a third TickParent var, then things are just getting too complex.  Would then want to implement some sort of priority index system
									  // where CutScenes have the highest priority, controllers are next, pawns next, CamTarget next, Camera last.  Something like that...
var(Object) name   InitialState;
var(Object) name   Group;
var(Object) string ExcludeGameStates; // Used to exclude ourselves from GameStates that are within this string, by default it is none

// Execution and timer variables.
var float                 TimerRate;     // Timer event, 0=no timer.
var const float           TimerCounter;	 // Counts up until it reaches TimerRate.
var(Advanced) float		  LifeSpan;      // How old the object lives before dying, 0=forever.

// Animation variables.
var(Display) name         AnimSequence;  // Animation sequence we're playing.
var(Display) float        AnimFrame;     // Current animation frame, 0.0 to 1.0.
var(Display) float        AnimRate;      // Animation rate in frames per second, 0=none, negative=velocity scaled.
var          float        TweenRate;     // Tween-into rate.
var          float        TweenAlpha;    // Fraction of current anim, 0.0 to 1.0.
var          byte         AnimBone;      // Bone range affected.
var transient Animation   SkelAnim;
var	array<Actor>		  AuxAnims;		 // Auxilliary anim channels.

var(Display) float		  LODBias;



//-----------------------------------------------------------------------------
// Structures.

// Identifies a unique convex volume in the world.
struct PointRegion
{
	var zoneinfo Zone;       // Zone.
	var int      iLeaf;      // Bsp leaf.
	var byte     ZoneNumber; // Zone number.
};

//-----------------------------------------------------------------------------
// Major actor properties.

// Scriptable.
var       const LevelInfo Level;         // Level this actor is on.
var transient const Level XLevel;        // Level object.
var(Events) name		  Tag;			 // Actor's tag name.
var(Events) name          Event;         // The event this actor causes.
var Actor                 Target;        // Actor we're aiming at (other uses as well).
var Pawn                  Instigator;    // Pawn responsible for damage.
var(Sound) sound        AmbientSound;    // Ambient sound effect.
var Inventory             Inventory;     // Inventory chain.
var const Actor           Base;          // Moving brush actor we're standing on.
var const PointRegion     Region;        // Region this actor is in.
var(Movement)	name	  AttachTag;

// @PAB this is the only place which is shared by all objects that can be 
// acted upon by spells.

var(Spells) enum ESpellType
{
	SPELL_None,
	SPELL_Alohomora,
	SPELL_Incendio,
	SPELL_LocomotorWibbly,
	SPELL_Lumos,
	SPELL_Nox,
	SPELL_PetrificusTotalus,
	SPELL_WingardiumLeviosa,
	SPELL_Verdimillious,
	SPELL_Vermillious,
	SPELL_Flintifores,
	SPELL_Reparo,
	SPELL_MucorAdNauseum,
	SPELL_Flipendo,
	SPELL_Ectomatic,
	SPELL_Avifores,
	SPELL_FireCracker,
	SPELL_Transfiguration,
	SPELL_WingSustain,
	
	SPELL_Diffindo,
	SPELL_Skurge,
	SPELL_Spongify,
	SPELL_Rictusempra,
	SPELL_Ecto,
	SPELL_Fire,
	
	SPELL_DuelRictusempra,
	SPELL_DuelMimblewimble,
	SPELL_DuelExpelliarmus,

} eVulnerableToSpell;


//-----------------------------------------------------------------------------
// Game State

var(GameState) bool	bPersistent;			// If the actor is persistant it will be saved/loaded seperatly when you save/load your game
var const bool		bInCurrentGameState;	// Set by Engine: ULevelBase::ScreenActorsByGameState()


//-----------------------------------------------------------------------------
// Targeting.

var(Targeting) float  SizeModifier;
var(Targeting) vector CentreOffset;
var(Targeting) float  GestureDistance;
var(Targeting) bool	  bGestureFaceHorizOnly;	// If true the spell gesture will only be horizontal (rotate about the z axis)

//-----------------------------------------------------------------------------
// Internal.
var const byte            StandingCount; // Count of actors standing on this actor.
var const byte            MiscNumber;    // Internal use.
var const byte            LatentByte;    // Internal latent function use.
var const int             LatentInt;     // Internal latent function use.
var const float           LatentFloat;   // Internal latent function use.
var const actor           LatentActor;   // Internal latent function use.
var const actor           Touching[4];   // List of touching actors.
var const actor           Deleted;       // Next actor in just-deleted chain.

// Internal tags.
var const transient int CollisionTag, LightingTag, OtherTag, ExtraTag, SpecialTag;

// The actor's position and rotation.
var(Movement) const vector Location;     // Actor's location; use Move to set.
var(Movement) const rotator Rotation;    // Rotation.
var       const vector    OldLocation;   // Actor's old location one tick ago.
var       const vector    ColLocation;   // Actor's old location one move ago.
var(Movement) vector      Velocity;      // Velocity.
var(Movement) float		  IPSpeed;		 // For interpolation only ==> desired velocity size if 0 than DesiredSpeed from IP keypoint is used
var       vector          Acceleration;  // Acceleration.
var(Filter) float		  OddsOfAppearing; // 0-1 - chance actor will appear in relevant game modes.
  
//Editing flags
var(Advanced) bool        bHiddenEd;     // Is hidden during editing.
var(Advanced) bool        bDirectional;  // Actor shows direction arrow during editing.
var const bool            bSelected;     // Selected in UnrealEd.
var const bool            bMemorized;    // Remembered in UnrealEd.
var const bool            bHighlighted;  // Highlighted in UnrealEd.
var bool                  bEdLocked;     // Locked in editor (no movement or rotation).
var(Advanced) bool        bEdShouldSnap; // Snap to grid in editor.
var transient bool        bEdSnap;       // Should snap to grid in UnrealEd.
var transient const bool  bTempEditor;   // Internal UnrealEd.

// What kind of gameplay scenarios to appear in.
var(Filter) bool          bDifficulty0;  // Appear in difficulty 0.
var(Filter) bool          bDifficulty1;  // Appear in difficulty 1.
var(Filter) bool          bDifficulty2;  // Appear in difficulty 2.
var(Filter) bool          bDifficulty3;  // Appear in difficulty 3.
var(Filter) bool          bSinglePlayer; // Appear in single player.
var(Filter) bool          bNet;          // Appear in regular network play.
var(Filter) bool          bNetSpecial;   // Appear in special network play mode.
var(Filter) bool          bRelevantInSoftwareRenderer;   // Appear when software rendering.

// set to prevent re-initializing of actors spawned during level startup
var	bool				  bScriptInitialized;

// Editor support.
var Actor				  HitActor;		// Actor to return instead of this one, if hit.

//-----------------------------------------------------------------------------
// Display properties.

// Drawing effect.
var(Display) enum EDrawType
{
	DT_None,
	DT_Sprite,
	DT_Mesh,
	DT_Brush,
	DT_RopeSprite,
	DT_VerticalSprite,
	DT_Terraform,
	DT_SpriteAnimOnce,
	DT_Particles,

} DrawType;

// Style for rendering sprites, meshes, particles, decals.
var(Display) enum ERenderStyle
{
	STY_None,
	STY_Normal,
	STY_Masked,
	STY_Translucent,
	STY_Modulated,
} Style;

// Other display properties.
var(Display) texture    Sprite;			 // Sprite texture if DrawType=DT_Sprite.
var(Display) texture    Texture;		 // Misc texture.
var(Display) texture    Skin;            // Special skin or enviro map texture.
var(Display) mesh       Mesh;            // Mesh if DrawType=DT_Mesh.
var const export model  Brush;           // Brush if DrawType=DT_Brush.
var(Display) float      DrawScale;		 // Scaling factor, 1.0=normal size.
var			 vector		PrePivot;		 // Offset from box center for drawing.
var			 float		SavedPrePivotZ;	 // helps to step up / down stairs
var(Display) float      ScaleGlow,		 // Multiplies lighting.
						SpecularGlow;
var(Display)  float     VisibilityRadius;// Actor is drawn if viewer is within its visibility
var(Display)  float     VisibilityHeight;// cylinder.  Zero=infinite visibility.
var(Display) byte       AmbientGlow;     // Ambient brightness, or 255=pulsing.
var(Display) byte       SpecularWidth;   // Width of highlight: 255 = diffuse.
var(Display) byte       Fatness;         // Fatness (mesh distortion).
var(Display) byte       Wideness;        // Sort the same, only different (XY-only scaling).
var(Display) float		SpriteProjForward;// Distance forward to draw sprite from actual location.
var(Display) float		Opacity;		 // Opacity - 0.0 = totally transparent, 1.0 = totally opaque.

// Display.
var(Display)  bool      bUnlit;          // Lights don't affect actor.
var(Display)  bool      bNoSmooth;       // Don't smooth actor's texture.
var(Display)  bool      bRandomFrame;    // Particles use a random texture from among the default texture and the multiskins textures
var(Display)  bool      bMeshEnviroMap;  // Environment-map the mesh.
var(Display)  bool      bMeshCurvy;      // Curvy mesh.
var(Display)  bool		bFilterByVolume; // Filter this sprite by its Visibility volume.
var(Display)  bool		bTwoSidedBias;	 // Bias 2-sided textures forward in software sorting.

// Not yet implemented.
var(Display) bool       bShadowCast;     // Casts shadows.

// Advanced.
var			  bool		bHurtEntry;	     // keep HurtRadius from being reentrant
var(Advanced) bool		bGameRelevant;	 // Always relevant for game
var			  bool		bCarriedItem;	 // being carried, and not responsible for displaying self, so don't replicated location and rotation
var			  bool		bForcePhysicsUpdate; // force a physics update for simulated pawns
var(Advanced) bool        bIsSecretGoal; // This actor counts in the "secret" total.
var(Advanced) bool        bIsKillGoal;   // This actor counts in the "death" toll.
var(Advanced) bool        bIsItemGoal;   // This actor counts in the "item" count.
var(Advanced) bool		  bCollideWhenPlacing; // This actor collides with the world when placing.
var(Advanced) bool		  bTravel;       // Actor is capable of travelling among servers.
var(Advanced) bool		  bMovable;      // Actor is capable of travelling among servers.


// Multiple skin support.
var(Display) texture MultiSkins[8];

//-----------------------------------------------------------------------------
// Sound.

// Ambient sound.
var(Sound) byte         SoundRadius;	 // Radius of ambient sound.
var(Sound) byte         SoundVolume;	 // Volume of amient sound.
var(Sound) byte         SoundPitch;	     // Sound pitch shift, 64.0=none.

// Sound occlusion
enum ESoundOcclusion
{
	OCCLUSION_Default,
	OCCLUSION_None,
	OCCLUSION_BSP,
	OCCLUSION_StaticMeshes,
};

var(Sound) ESoundOcclusion SoundOcclusion;		// Sound occlusion approach.

// Regular sounds.
var(Sound) float TransientSoundVolume;
var(Sound) float TransientSoundRadius;
var(Sound) float TransientSoundPitch;

// Sound slots for actors.
enum ESoundSlot
{
	SLOT_None,
	SLOT_Misc,
	SLOT_Pain,
	SLOT_Interact,
	SLOT_Ambient,
	SLOT_Talk,
	SLOT_Interface,
};

// Music transitions.
enum EMusicTransition
{
	MTRAN_None,
	MTRAN_Instant,
	MTRAN_Segue,
	MTRAN_Fade,
	MTRAN_FastFade,
	MTRAN_SlowFade,
};

//-----------------------------------------------------------------------------
// Collision.

// Collision size.
var(Collision) const float CollisionRadius; // Radius of collision cylinder, or X of collision box.
var(Collision) const float CollisionWidth;  // Y of collision box.
var(Collision) const float CollisionHeight; // Half-height of cylinder, or Z of collision box.

// Collision flags. 
var(Collision) enum ECollideType
// Determines primitive to use for collision.
// Some actors (e.g. brushes) force a certain primitive type.
// Must add new types at end of enumeration to preserve backward compatibilty with older map files
{
	CT_AlignedCylinder,				// Default: World-aligned (vertical) cylinder,
									// using CollisionRadius/Height, CollisionWidth as vertical offset.
	CT_OrientedCylinder,			// Oriented according to actor rotation.
	CT_Box,							// Oriented centered box, using CollisionRadius/Width/Height.
	CT_Shape,						// Non-centered oriented box, derived from mesh/brush primitive.
	CT_AlignedOvalCylinder,			// World-aligned extruded ellipse, using CollisionWidth for X axis radius, CollisionRadius for Y axis
	CT_OrientedOvalCylinder,		// As above but oriented according to actor rotation.
} CollideType;
var(Collision) const bool bCollideActors;   // Collides with other actors.
var(Collision) bool       bCollideWorld;    // Collides with the world.
var(Collision) const bool bBlockActors;	    // Blocks other nonplayer actors.
var(Collision) const bool bBlockPlayers;    // Blocks other player actors.
var(Collision) bool       bProjTarget;      // Projectiles should potentially target this actor.
var(Collision) bool       bDoNotBlockProjectiles;// Do Not Blocks projectiles
var(Collision) bool       bAlignBottom;		// Align bottoms of object visual and collision bounds
											// (only if bCollideWorld and not CT_Shape). Otherwise, aligns centers.
var(Collision) bool       bAlignBottomAlways;	// Unconditionally align visual and collision bounds at their bottoms.

// Camera Collision 
var(Collision) bool		  bBlockCamera;		// Blocks camera ( added in HP2 )


//-----------------------------------------------------------------------------
// Lighting.

// Light modulation.
var(Lighting) enum ELightType
{
	LT_None,
	LT_Steady,
	LT_Pulse,
	LT_Blink,
	LT_Flicker,
	LT_Strobe,
	LT_BackdropLight,
	LT_SubtlePulse,
	LT_TexturePaletteOnce,
	LT_TexturePaletteLoop
} LightType;

// Spatial light effect to use.
var(Lighting) enum ELightEffect
{
	LE_None,
	LE_TorchWaver,
	LE_FireWaver,
	LE_WateryShimmer,
	LE_Searchlight,
	LE_SlowWave,
	LE_FastWave,
	LE_CloudCast,
	LE_StaticSpot,
	LE_Shock,
	LE_Disco,
	LE_Warp,
	LE_Spotlight,
	LE_NonIncidence,
	LE_Shell,
	LE_OmniBumpMap,
	LE_Interference,
	LE_Cylinder,
	LE_Rotor,
	LE_Unused
} LightEffect;

// Lighting info.
var(LightColor) byte
	LightBrightness,
	LightHue,
	LightSaturation;

// Light properties.
var(Lighting) byte
	LightRadius,
	LightRadiusInner,
	LightPeriod,
	LightPhase,
	LightCone,
	VolumeBrightness,
	VolumeRadius,
	VolumeFog;

// Lighting.
var(Lighting) bool	     bSpecialLit;	 // Only affects special-lit surfaces.
var(Lighting) bool	     bActorShadows;  // Light casts actor shadows.
var(Lighting) bool	     bCorona;        // Light uses Skin as a corona.
var(Lighting) bool	     bLensFlare;     // Whether to use zone lens flare.
var(LightColor) bool	 bDarkLight;     // Whether lighting is subtractive.
var(Lighting) enum ELightSource
{
	LD_Point,							// Regular point light.
	LD_Plane,							// Parallel light.
	LD_Ambient							// All directions equally.
} LightSource;



//-----------------------------------------------------------------------------
// Physics.

// Options.
var(Movement) bool        bBounce;           // Bounces when hits ground fast. FT:Also note, velocity in PHYS_Falling and PHYS_Projectile is handled
                                             //  differently when this is false verses true.  Search for bBounce and check it out.  Also note, HitWall
                                             //  wont get called from PHYS_Falling unless this is true.  Just doesn't seem right...
var(Movement) bool		  bFixedRotationDir; // Fixed direction of rotation. FT:This actually means that it will keep rotating by itself using the current rate in RotationRate.
var(Movement) bool		  bRotateToDesired;  // Rotate to DesiredRotation.
var           bool        bInterpolating;    // Performing interpolating.
var(Movement) bool        bInterpolating_IgnoreRot; //Does no rotation math while interpolating
var			  const bool  bJustTeleported;   // Used by engine physics - not valid for scripts.

// Dodge move direction.
var enum EDodgeDir
{
	DODGE_None,
	DODGE_Left,
	DODGE_Right,
	DODGE_Forward,
	DODGE_Back,
	DODGE_Active,
	DODGE_Done
} DodgeDir;

// Physics properties.
var(Movement) float       Mass;            // Mass of this actor.
var(Movement) float       Buoyancy;        // Water buoyancy.
var(Movement) rotator	  RotationRate;    // Change in rotation per second.
var(Movement) rotator     DesiredRotation; // Physics will rotate pawn to this if bRotateToDesired.
var           float       PhysAlpha;       // Interpolating position, 0.0-1.0.
var           float       PhysRate;        // Interpolation rate per second.
var			  Actor		  PendingTouch;		// Actor touched during move which wants to add an effect after the movement completes 
//-----------------------------------------------------------------------------
// Animation.

// Animation control.
var          float        AnimLast;        // Last frame.
var          float        AnimMinRate;     // Minimum rate for velocity-scaled animation.
var			 plane		  SimAnim;		   // replicated to simulated proxies.

//-----------------------------------------------------------------------------
// Networking.

// Network control.
var(Networking) float NetPriority; // Higher priorities means update it more frequently.
var(Networking) float NetUpdateFrequency; // How many seconds between net updates.

// Symmetric network flags, valid during replication only.
var const bool bNetInitial;       // Initial network update.
var const bool bNetOwner;         // Player owns this actor.
var const bool bNetRelevant;      // Actor is currently relevant. Only valid server side, only when replicating variables.
var const bool bNetSee;           // Player sees it in network play.
var const bool bNetHear;          // Player hears it in network play.
var const bool bNetFeel;          // Player collides with/feels it in network play.
var const bool bSimulatedPawn;	  // True if Pawn and simulated proxy.
var const bool bDemoRecording;	  // True we are currently demo recording
var const bool bClientDemoRecording;// True we are currently recording a client-side demo
var const bool bClientDemoNetFunc;// True if we're client-side demo recording and this call originated from the remote.

//-----------------------------------------------------------------------------
// Enums.

// Travelling from server to server.
enum ETravelType
{
	TRAVEL_Absolute,	// Absolute URL.
	TRAVEL_Partial,		// Partial (carry name, reset server).
	TRAVEL_Relative,	// Relative URL.
};

// Input system states.
enum EInputAction
{
	IST_None,    // Not performing special input processing.
	IST_Press,   // Handling a keypress or button press.
	IST_Hold,    // Handling holding a key or button.
	IST_Release, // Handling a key or button release.
	IST_Axis,    // Handling analog axis movement.
};

enum EAnimType
{
	AT_Replace,		// Replace all existing anims (on specified bone set) with this.
	AT_Combine,		// Combine with existing anims.
};

// Input keys.
enum EInputKey
{
/*00*/	IK_None			,IK_LeftMouse	,IK_RightMouse	,IK_Cancel		,
/*04*/	IK_MiddleMouse	,IK_Unknown05	,IK_Unknown06	,IK_Unknown07	,
/*08*/	IK_Backspace	,IK_Tab         ,IK_Unknown0A	,IK_Unknown0B	,
/*0C*/	IK_Unknown0C	,IK_Enter	    ,IK_Unknown0E	,IK_Unknown0F	,
/*10*/	IK_Shift		,IK_Ctrl	    ,IK_Alt			,IK_Pause       ,
/*14*/	IK_CapsLock		,IK_Unknown15	,IK_Unknown16	,IK_Unknown17	,
/*18*/	IK_Unknown18	,IK_Unknown19	,IK_Unknown1A	,IK_Escape		,
/*1C*/	IK_Unknown1C	,IK_Unknown1D	,IK_Unknown1E	,IK_Unknown1F	,
/*20*/	IK_Space		,IK_PageUp      ,IK_PageDown    ,IK_End         ,
/*24*/	IK_Home			,IK_Left        ,IK_Up          ,IK_Right       ,
/*28*/	IK_Down			,IK_Select      ,IK_Print       ,IK_Execute     ,
/*2C*/	IK_PrintScrn	,IK_Insert      ,IK_Delete      ,IK_Help		,
/*30*/	IK_0			,IK_1			,IK_2			,IK_3			,
/*34*/	IK_4			,IK_5			,IK_6			,IK_7			,
/*38*/	IK_8			,IK_9			,IK_Unknown3A	,IK_Unknown3B	,
/*3C*/	IK_Unknown3C	,IK_Unknown3D	,IK_Unknown3E	,IK_Unknown3F	,
/*40*/	IK_Unknown40	,IK_A			,IK_B			,IK_C			,
/*44*/	IK_D			,IK_E			,IK_F			,IK_G			,
/*48*/	IK_H			,IK_I			,IK_J			,IK_K			,
/*4C*/	IK_L			,IK_M			,IK_N			,IK_O			,
/*50*/	IK_P			,IK_Q			,IK_R			,IK_S			,
/*54*/	IK_T			,IK_U			,IK_V			,IK_W			,
/*58*/	IK_X			,IK_Y			,IK_Z			,IK_Unknown5B	,
/*5C*/	IK_Unknown5C	,IK_Unknown5D	,IK_Unknown5E	,IK_Unknown5F	,
/*60*/	IK_NumPad0		,IK_NumPad1     ,IK_NumPad2     ,IK_NumPad3     ,
/*64*/	IK_NumPad4		,IK_NumPad5     ,IK_NumPad6     ,IK_NumPad7     ,
/*68*/	IK_NumPad8		,IK_NumPad9     ,IK_GreyStar    ,IK_GreyPlus    ,
/*6C*/	IK_Separator	,IK_GreyMinus	,IK_NumPadPeriod,IK_GreySlash   ,
/*70*/	IK_F1			,IK_F2          ,IK_F3          ,IK_F4          ,
/*74*/	IK_F5			,IK_F6          ,IK_F7          ,IK_F8          ,
/*78*/	IK_F9           ,IK_F10         ,IK_F11         ,IK_F12         ,
/*7C*/	IK_F13			,IK_F14         ,IK_F15         ,IK_F16         ,
/*80*/	IK_F17			,IK_F18         ,IK_F19         ,IK_F20         ,
/*84*/	IK_F21			,IK_F22         ,IK_F23         ,IK_F24         ,
/*88*/	IK_Unknown88	,IK_Unknown89	,IK_Unknown8A	,IK_Unknown8B	,
/*8C*/	IK_Unknown8C	,IK_Unknown8D	,IK_Unknown8E	,IK_Unknown8F	,
/*90*/	IK_NumLock		,IK_ScrollLock  ,IK_Unknown92	,IK_Unknown93	,
/*94*/	IK_Unknown94	,IK_Unknown95	,IK_Unknown96	,IK_Unknown97	,
/*98*/	IK_Unknown98	,IK_Unknown99	,IK_Unknown9A	,IK_Unknown9B	,
/*9C*/	IK_Unknown9C	,IK_Unknown9D	,IK_Unknown9E	,IK_Unknown9F	,
/*A0*/	IK_LShift		,IK_RShift      ,IK_LControl    ,IK_RControl    ,
/*A4*/	IK_UnknownA4	,IK_UnknownA5	,IK_UnknownA6	,IK_UnknownA7	,
/*A8*/	IK_UnknownA8	,IK_UnknownA9	,IK_UnknownAA	,IK_UnknownAB	,
/*AC*/	IK_UnknownAC	,IK_UnknownAD	,IK_UnknownAE	,IK_UnknownAF	,
/*B0*/	IK_UnknownB0	,IK_UnknownB1	,IK_UnknownB2	,IK_UnknownB3	,
/*B4*/	IK_UnknownB4	,IK_UnknownB5	,IK_UnknownB6	,IK_UnknownB7	,
/*B8*/	IK_UnknownB8	,IK_UnknownB9	,IK_Semicolon	,IK_Equals		,
/*BC*/	IK_Comma		,IK_Minus		,IK_Period		,IK_Slash		,
/*C0*/	IK_Tilde		,IK_UnknownC1	,IK_UnknownC2	,IK_UnknownC3	,
/*C4*/	IK_UnknownC4	,IK_UnknownC5	,IK_UnknownC6	,IK_UnknownC7	,
/*C8*/	IK_Joy1	        ,IK_Joy2	    ,IK_Joy3	    ,IK_Joy4	    ,
/*CC*/	IK_Joy5	        ,IK_Joy6	    ,IK_Joy7	    ,IK_Joy8	    ,
/*D0*/	IK_Joy9	        ,IK_Joy10	    ,IK_Joy11	    ,IK_Joy12		,
/*D4*/	IK_Joy13		,IK_Joy14	    ,IK_Joy15	    ,IK_Joy16	    ,
/*D8*/	IK_UnknownD8	,IK_UnknownD9	,IK_UnknownDA	,IK_LeftBracket	,
/*DC*/	IK_Backslash	,IK_RightBracket,IK_SingleQuote	,IK_UnknownDF	,
/*E0*/  IK_JoyX			,IK_JoyY		,IK_JoyZ		,IK_JoyR		,
/*E4*/	IK_MouseX		,IK_MouseY		,IK_MouseZ		,IK_MouseW		,
/*E8*/	IK_JoyU			,IK_JoyV		,IK_UnknownEA	,IK_UnknownEB	,
/*EC*/	IK_MouseWheelUp ,IK_MouseWheelDown,IK_Unknown10E,UK_Unknown10F  ,
/*F0*/	IK_JoyPovUp     ,IK_JoyPovDown	,IK_JoyPovLeft	,IK_JoyPovRight	,
/*F4*/	IK_UnknownF4	,IK_UnknownF5	,IK_Attn		,IK_CrSel		,
/*F8*/	IK_ExSel		,IK_ErEof		,IK_Play		,IK_Zoom		,
/*FC*/	IK_NoName		,IK_PA1			,IK_OEMClear
};

enum ESoundParam
{
	SOUND_Volume,
	SOUND_Radius,
	SOUND_Pitch
};



var(Display) class<RenderIterator> RenderIteratorClass;	// class to instantiate as the actor's RenderInterface
var transient RenderIterator RenderInterface;		// abstract iterator initialized in the Rendering engine



// shadow decal
var Decal Shadow;



//*************************************************************************************************************
//*************************************************************************************************************
var (CutScene) string CutName;	//Used to id actor for the cutscenes.
var actor CutNotifyActor;		//actor (script most likely) to notify when a CutCommand is finished.
var string CutErrorString;		//if CutCommand returns false. This contains text saying for reason why.
var string  sCutNotifyCue;		//storage for the cue until ready to send.
var HUD playerHarryHud;


	//used by cutscenes.
function name StringToAnimName(string animName)
{
	if(HasAnim(name(animName)))
		{
		return name(animName);
		}
	else
		return '';
}

//fDisplayDuration is the length of time in seconds to display the text. If it is equal to 0 duration will be calculated

function float DeliverLocalizedDialog(string dlgId,bool bPlaySound,float fDisplayDuration,optional string intFileName)
{
local sound  dlgSound;
local string dlgString;
local float  sndLen;
local int    i;

	if(intFileName=="")
		dlgString=Localize( "all", dlgId,"HPdialog" );
	else
		dlgString=Localize( "all", dlgId,intFileName );

	if(bPlaySound)
		dlgSound = Sound( DynamicLoadObject("AllDialog."$dlgId, class'Sound') );
	
	if(dlgSound!=None)
		{
		sndLen=GetSoundDuration(dlgSound);
		PlaySound(dlgSound, , , , 10000, , true);
		}
	else
		{
			//make up a duration if no sound
		sndLen=(Len(dlgString)*0.01)+3.0;
		}

		//override duration?
	if(fDisplayDuration>0)
		sndLen=fDisplayDuration;

	dlgString = HandleFacialExpression( dlgString, sndLen );

	level.playerHarryActor.MyHud.SetSubtitleText(dlgString, sndLen);
	return(sndLen);

}

function bool CutCommand_Say(string command, optional string cue, optional bool bFastFlag)
{
local string  sSayText;
local string  sSayTextID;
local sound dlgSound;
local float sndLen;
local TimedCue tcue;
local bool    bIndefiniteText;
local string  sString;
local int     i;

//level.playerHarryActor.clientmessage("****Doing SAY:"$cue $" fast:"$bFastFlag);
	if(bFastFlag)
		{
		CutNotifyActor.CutCue(cue);
		return true;
		}

	sSayTextID = ParseDelimitedString( command, " ", 2, false );
	
	sSayText=Localize( "all", sSayTextID,"HPdialog" );

	dlgSound = Sound( DynamicLoadObject("AllDialog."$sSayTextID, class'Sound') );
	
	if(dlgSound!=None)
		{
		sndLen=GetSoundDuration(dlgSound);
		PlaySound(dlgSound, , , , 100000, , true);
		}
	else
		{
			//make up a duration if no sound
		sndLen=(Len(sSayText)*0.01)+3.0;
		}

		//Find and set the animation parameters.	
	for( i=0; i < 20; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
		else
        if( Left(sString, len("IndefiniteText")) ~= "IndefiniteText" )
			bIndefiniteText=true;
    }

	sSayText = HandleFacialExpression( sSayText, sndLen );
	
		//create a TimedCue to cue object after sndLen seconds.
	tcue=spawn(class 'TimedCue');
	tcue.CutNotifyActor=Self;		//Tell me when done. This is auto passed back to the CutNotifyActor if any.
									//Or it can be used by the talk to find out when the talk is finished.
	tcue.SetupTimer(sndLen+0.5,cue); //little extra time for slop

    if (bIndefiniteText)
	    level.playerHarryActor.MyHud.SetSubtitleText(sSayText, 0);
    else
        level.playerHarryActor.MyHud.SetSubtitleText(sSayText, sndLen);

	return(true);
}

//***********************************************************************************************************
//Looks for a <mad> at the beginning of the string, if so, strips it out, plays the facial anim, and returns
// the string without that prefix.
function string HandleFacialExpression( string dlgString, int sndLen, optional bool bDONTdoFacialAnim )
{
	local int i;

	if(   left(dlgString,1)  == "["
	   //&& mid(dlgString,1,1) != "?"
	  )
	{
		i = instr(dlgString, "]");
		if( i != -1 )
		{
			if( !bDONTdoFacialAnim  &&  Pawn(self) != none )
				Pawn(self).PlayFacialAnim( name( mid(dlgString, 1, i-1) ), false, 0, sndLen );

			dlgString = mid(dlgString, i+1);
		}
	}

	return dlgString;
}

//***********************************************************************************************************
//Keeps trying with increasing z until it's successfull
function bool SetLocation2( vector v )
{
	local int i;

	for( i = 0; i <= 50; i+=2.5 )
		if( SetLocation( v + vec(0,0,i) ) )
			return true;
	return false;
}

//************************************************************************************************************
function actor FancySpawn
(
	class<actor>      type,
	optional actor	  owner,
	optional name     SpawnTag,
	optional vector   SpawnLocation,
	optional rotator  SpawnRotation,
	optional bool     bZOnly
)
{
	local float r;
	local actor a;
	local vector v;

	v = SpawnLocation;
	if( v == vect(0,0,0) )
		v = Location;

	r = 0;

	for( r = 0; r < 70; r+=2.5 )
	{
		if( !bZOnly )
		{
			a = spawn( type, owner, SpawnTag, v + vec( r,0,0), SpawnRotation );
			if( a != none )
				return a;
			a = spawn( type, owner, SpawnTag, v + vec(-r,0,0), SpawnRotation );
			if( a != none )
				return a;
			a = spawn( type, owner, SpawnTag, v + vec(0, r,0), SpawnRotation );
			if( a != none )
				return a;
			a = spawn( type, owner, SpawnTag, v + vec(0,-r,0), SpawnRotation );
			if( a != none )
				return a;
			a = spawn( type, owner, SpawnTag, v + vec( 0,0,-r), SpawnRotation );
			if( a != none )
				return a;
		}
		a = spawn( type, owner, SpawnTag, v + vec( 0,0,r), SpawnRotation );
		if( a != none )
			return a;

		Log("*******FancySpawn r = "$r);
	}

	return none;
}
//***********************************************************************************************************
function bool CutQuestion(string question)
{
	CutErrorString=self $" Doesn't know the answer to the question:"$question;
	return false;
}

//***********************************************************************************************************
function bool CutCommand(string command,optional string cue,optional bool bFastFlag)
{
local string sActualCommand;
local string targetName;
local actor tempActor;
local sound tempSound;

	sActualCommand = ParseDelimitedString( command, " ", 1, false );
	if( sActualCommand ~= "Say" )
	{
		return CutCommand_Say(command,cue,bFastFlag);
	}
	else
	if( sActualCommand ~= "Teleport" )
	{
		targetName = ParseDelimitedString( command, " ", 2, false );  
		ForEach AllActors( class'actor', tempActor )
			if( tempActor.CutName ~= targetName )
				break;

		if(tempActor!=None)
			{
			SetLocation2(tempActor.location);
				//immediate return cue.
			CutCue( cue );
			return true;
			}
		else
			{
			CutErrorString="Can't find Teleport location:"$targetName;
			CutCue( cue );
			return(false);
			}
	}
	else 
	if( sActualCommand ~= "SetAmbientSound" )
	{
		targetName = ParseDelimitedString( command, " ", 2, false );  

		tempSound = Sound( DynamicLoadObject(targetName, class'Sound') );
		if(tempSound==None)
		{
			CutErrorString="Can't SetAmbientSound. Can't find sound:"$targetName;
			CutCue( cue );
			return(false);
		}
		else
		{
			ambientSound=tempSound;
			CutCue( cue );
			return(true);
		}
	}
	else 
	if( sActualCommand ~= "Destroy" )
	{
		Destroy();
		return(true);
	}

	CutErrorString=self $"->CutCommand not supported:" $command;
	CutCue( cue );
	return(false);
}

function CutCue(string cue)
{
	if(CutNotifyActor!=None && CutNotifyActor!=Self)
		CutNotifyActor.CutCue(cue);
	return;
}

function string ParseDelimitedString(string Text, string Delimiter, int Count, optional bool bToEndOfLine)
{
	local string Result;
	local int Found, i;
	local string s;

	Result = "";	
	Found = 1;
	
	for(i=0;i<Len(Text);i++)
	{
		s = Mid(Text, i, 1);
		if(InStr(Delimiter, s) != -1)
		{
			if(Found == Count)
			{
				if(bToEndOfLine)
					return Result$Mid(Text, i);
				else
					return Result;
			}

			Found++;			
		}
		else
		{
			if(Found >= Count)
				Result = Result $ s;
		}
	}
	
	return Result;
}



//*************************************************************************************************************
//*************************************************************************************************************


//-----------------------------------------------------------------------------
// natives.

// Execute a console command in the context of the current level and game engine.
native function string ConsoleCommand( string Command );


//-----------------------------------------------------------------------------
// Network replication.

replication
{
	// Relationships.
	unreliable if( Role==ROLE_Authority )
		Owner, Role, RemoteRole;
	unreliable if( bNetOwner && Role==ROLE_Authority )
		bNetOwner, Inventory;
	unreliable if( bReplicateInstigator && (RemoteRole>=ROLE_SimulatedProxy) && (Role==ROLE_Authority) )
		Instigator;

	// Ambient sound.
	unreliable if( (Role==ROLE_Authority) && (!bNetOwner || !bClientAnim) )
		AmbientSound;
	unreliable if( AmbientSound!=None && Role==ROLE_Authority  && (!bNetOwner || !bClientAnim) )
		SoundRadius, SoundVolume, SoundPitch;
	unreliable if( bDemoRecording )
		DemoPlaySound;

	// Collision.
	unreliable if( Role==ROLE_Authority )
		bCollideActors, bCollideWorld;
	unreliable if( (bCollideActors || bCollideWorld) && Role==ROLE_Authority )
		bProjTarget, bBlockActors, bBlockPlayers, CollisionRadius, CollisionWidth, CollisionHeight, CollideType;

	// Location.
	unreliable if( !bCarriedItem && (bNetInitial || bSimulatedPawn || RemoteRole<ROLE_SimulatedProxy) && Role==ROLE_Authority )
		Location;
	unreliable if( !bCarriedItem && (DrawType==DT_Mesh || DrawType==DT_Brush) && (bNetInitial || bSimulatedPawn || RemoteRole<ROLE_SimulatedProxy) && Role==ROLE_Authority )
		Rotation;
	unreliable if( RemoteRole==ROLE_SimulatedProxy )
		Base;

	// Velocity.
	unreliable if( bSimFall || ((RemoteRole==ROLE_SimulatedProxy && (bNetInitial || bSimulatedPawn)) || bIsMover) )
		Velocity;

	// Physics.
	unreliable if( bSimFall || (RemoteRole==ROLE_SimulatedProxy && bNetInitial && !bSimulatedPawn) )
		Physics, Acceleration, bBounce;
	unreliable if( RemoteRole==ROLE_SimulatedProxy && Physics==PHYS_Rotating && bNetInitial )
		bFixedRotationDir, bRotateToDesired, RotationRate, DesiredRotation;

	// Animation. 
	unreliable if( DrawType==DT_Mesh && ((RemoteRole<=ROLE_SimulatedProxy && (!bNetOwner || !bClientAnim)) || bDemoRecording) )
		AnimSequence, SimAnim, AnimMinRate, bAnimNotify;

	// Rendering.
	unreliable if( Role==ROLE_Authority )
		bHidden, bOnlyOwnerSee;
	unreliable if( Role==ROLE_Authority )
		Texture, DrawScale, DrawType, Style;
	unreliable if( DrawType==DT_Sprite && !bHidden && (!bOnlyOwnerSee || bNetOwner) && Role==ROLE_Authority)
		Sprite;
	unreliable if( DrawType==DT_Mesh && Role==ROLE_Authority )
		Mesh, PrePivot, bMeshEnviroMap, Skin, MultiSkins, Fatness, Wideness, AmbientGlow, ScaleGlow, SpecularGlow, SpecularWidth, bUnlit;
	unreliable if( DrawType==DT_Brush && Role==ROLE_Authority )
		Brush;

	// Lighting.
	unreliable if( Role==ROLE_Authority )
		LightType;
	unreliable if( LightType!=LT_None && Role==ROLE_Authority )
		LightEffect, LightBrightness, LightHue, LightSaturation, bDarkLight,
		LightRadius, LightRadiusInner, LightPeriod, LightPhase,
		VolumeBrightness, VolumeRadius,
		bSpecialLit;

	// Messages
	reliable if( Role<ROLE_Authority )
		BroadcastMessage, BroadcastLocalizedMessage;
}

//=============================================================================
// Actor error handling.

// Handle an error and kill this one actor.
native(233) final function Error( coerce string S );

//=============================================================================
// General functions.

// Latent functions.
native(256) final latent function Sleep( float Seconds );

// Collision.
native(262) final function SetCollision( optional bool NewColActors, optional bool NewBlockActors, optional bool NewBlockPlayers );
native(283) final function bool SetCollisionSize( float NewRadius, float NewHeight, optional float NewWidth );
native(286) final function BoundingBox GetWorldCollisionBox( optional bool bVisual );
native(274) final function vector GetRenderExtent();

// Movement.
native(266) final function bool Move( vector Delta );
native(267) final function bool SetLocation( vector NewLocation );
native(299) final function bool SetRotation( rotator NewRotation );
native(3969) final function bool MoveSmooth( vector Delta );
native(3971) final function AutonomousPhysics(float DeltaSeconds);

// Relations.
native(298) final function SetBase( actor NewBase );
native(272) final function SetOwner( actor NewOwner );


native(3972) final function bool SaveGameExists();

//=============================================================================
// Animation.

// Animation functions.
native(259) final function PlayAnim( name Sequence, optional float Rate, optional float TweenTime, optional EAnimType Type, optional name RootBone );
native(260) final function LoopAnim( name Sequence, optional float Rate, optional float TweenTime, optional float MinRate, optional EAnimType Type, optional name RootBone );
native(294) final function TweenAnim( name Sequence, float Time );
native(282) final function bool IsAnimating( optional name RootBone );
native(293) final function name GetAnimGroup( name Sequence );
native(261) final latent function FinishAnim( optional name RootBone );
native(263) final function bool HasAnim( name Sequence );
native(265) final function Actor CreateAnimChannel( class NewClass, EAnimType Type, name RootBone, optional bool bTransient, optional bool bNotReplaceable );
native(268) final function int BoneNumber( name Bone );
native(269) final function name BoneName ( int Bone );
native(257) final function vector BonePos( name Bone );
native(328) final function Rotator BoneRot( name Bone );

// Animation notifications.
event AnimEnd();

// Skeletal animation linkup.
native final function LinkSkelAnim( Animation Anim );

//=========================================================================
// Physics.

// Physics control.
native(301) final latent function FinishInterpolation();
native(3970) final function SetPhysics( EPhysics newPhysics );

function AttachToOwner( optional name OwnerBone )
{
	setPhysics(PHYS_Trailer);
	AnimBone = Owner.BoneNumber(OwnerBone)+1;
}

//=========================================================================
// Music

native final function int PlayMusic( string Song, float FadeInTime );
native final function StopMusic( int SongHandle, float FadeOutTime );
native final function StopAllMusic( float FadeOutTime );


//=============================================================================
// Engine notification functions.

//
// Major notifications.
//
event Spawned();
event Destroyed();
event Expired();
event GainedChild( Actor Other );
event LostChild( Actor Other );
event Tick( float DeltaTime );

//
// Triggers.
//
event Trigger( Actor Other, Pawn EventInstigator );
event UnTrigger( Actor Other, Pawn EventInstigator );
event BeginEvent();
event EndEvent();

//
// Physics & world interaction.
//
event Timer();
event HitWall( vector HitNormal, actor HitWall );
event Falling();
event Landed( vector HitNormal );
event ZoneChange( ZoneInfo NewZone );
event Touch( Actor Other );
event PostTouch( Actor Other ); // called for PendingTouch actor after physics completes
event UnTouch( Actor Other );
event Bump( Actor Other );
event BaseChange();
event Attach( Actor Other );
event Detach( Actor Other );
event KillCredit( Actor Other );
event Actor SpecialHandling(Pawn Other);
event bool EncroachingOn( actor Other );
event EncroachedBy( actor Other );
//event InterpolateEnd( actor Other );
event FinishedInterpolation(InterpolationPoint Other);

event EndedRotation();

event FellOutOfWorld()
{
	SetPhysics(PHYS_None);
	Destroy();
}	

//
// Damage and kills.
//
event KilledBy( pawn EventInstigator );
event TakeDamage( int Damage, Pawn EventInstigator, vector HitLocation, vector Momentum, name DamageType);
event targeted();

//
// Trace a line and see what it collides with first.
// Takes this actor's collision properties into account.
// Returns first hit actor, Level if hit level, or None if hit nothing.
//
native(277) final function Actor Trace
(
	out vector      HitLocation,
	out vector      HitNormal,
	vector          TraceEnd,
	optional vector TraceStart,
	optional bool   bTraceActors,
	optional vector Extent
);

// returns true if did not hit world geometry
native(548) final function bool FastTrace
(
	vector          TraceEnd,
	optional vector TraceStart
);

native(285) final function Texture TraceTexture
(
	vector TraceEnd,
	vector TraceStart,
	out int Flags,
	optional bool bTraceDecals
	//P_GET_VECTOR_REF(ScrollDir);	
);



//
// Spawn an actor. Returns an actor of the specified class, not
// of class Actor (this is hardcoded in the compiler). Returns None
// if the actor could not be spawned (either the actor wouldn't fit in
// the specified location, or the actor list is full).
// Defaults to spawning at the spawner's location.
//
native(278) final function actor Spawn
(
	class<actor>      SpawnClass,
	optional actor	  SpawnOwner,
	optional name     SpawnTag,
	optional vector   SpawnLocation,
	optional rotator  SpawnRotation
);

//
// Destroy this actor. Returns true if destroyed, false if indestructable.
// Destruction is latent. It occurs at the end of the tick.
//
native(279) final function bool Destroy();

//=============================================================================
// Timing.

// Causes Timer() events every NewTimerRate seconds.
native(280) final function SetTimer( float NewTimerRate, bool bLoop );

//=============================================================================
// Sound functions.

// Default values for optional parameters for sound fns :

// Sound = None,
// slot = misc, noOveride=false
// volume = TRansientSoundVolume, radius=TransientSoundRadius, 
// Pitch = 1.0

// Play a sound effect.
//---------------------

native(264) final function PlaySound
(
	sound				Sound,
	optional ESoundSlot Slot,
	optional float		Volume,
	optional bool		bNoOverride,
	optional float		Radius,
	optional float		Pitch,
	optional bool		Disable3D,
	optional bool		Loop
);

// play a sound effect, but don't propagate to a remote owner
// (he is playing the sound clientside
native simulated final function PlayOwnedSound
(
	sound				Sound,
	optional ESoundSlot Slot,
	optional float		Volume,
	optional bool		bNoOverride,
	optional float		Radius,
	optional float		Pitch,
	optional bool		Disable3D,
	optional bool		Loop
);

native simulated event DemoPlaySound
(
	sound				Sound,
	optional ESoundSlot Slot,
	optional float		Volume,
	optional bool		bNoOverride,
	optional float		Radius,
	optional float		Pitch,
	optional bool		Disable3D,
	optional bool		Loop
);

// If sound = None, ModifySound and StopSound perform on the sound found in the Slot.

native(567) final function bool ModifySound
(
				ESoundParam	parameter,
				float		value,

	optional	sound		Sound,
	optional	ESoundSlot	Slot
); // returns false if sound is no longer playing

native(568) final function StopSound
(
	optional sound		Sound,
	optional ESoundSlot Slot,
	optional float		FadeOutTime
);

// Get a sound duration.
native final function float GetSoundDuration( sound Sound );

//=============================================================================
// AI functions.

//
// Inform other creatures that you've made a noise
// they might hear (they are sent a HearNoise message)
// Senders of MakeNoise should have an instigator if they are not pawns.
//
native(512) final function MakeNoise( float Loudness );

//
// PlayerCanSeeMe returns true if some player has a line of sight to 
// actor's location.
//
native(532) final function bool PlayerCanSeeMe();

//=============================================================================
// Regular engine functions.

// Teleportation.
event bool PreTeleport( Teleporter InTeleporter );
event PostTeleport( Teleporter OutTeleporter );

// Level state.
event BeginPlay();

//========================================================================
// Disk access.

// Find files.
native(539) final function string GetMapName( string NameEnding, string MapName, int Dir );
native(545) final function GetNextSkin( string Prefix, string CurrentSkin, int Dir, out string SkinName, out string SkinDesc );
native(547) final function string GetURLMap();
native final function string GetNextInt( string ClassName, int Num );
native final function GetNextIntDesc( string ClassName, int Num, out string Entry, out string Description );
native final function bool GetCacheEntry( int Num, out string GUID, out string Filename );
native final function bool MoveCacheEntry( string GUID, optional string NewFilename );  


//=============================================================================
// Iterator functions.

// Iterator functions for dealing with sets of actors.
native(304) final iterator function AllActors     ( class<actor> BaseClass, out actor Actor, optional name MatchTag );
native(305) final iterator function ChildActors   ( class<actor> BaseClass, out actor Actor );
native(306) final iterator function BasedActors   ( class<actor> BaseClass, out actor Actor );
native(307) final iterator function TouchingActors( class<actor> BaseClass, out actor Actor );
native(309) final iterator function TraceActors   ( class<actor> BaseClass, out actor Actor, out vector HitLoc, out vector HitNorm, vector End, optional vector Start, optional vector Extent );
native(310) final iterator function RadiusActors  ( class<actor> BaseClass, out actor Actor, float Radius, optional vector Loc );
native(311) final iterator function VisibleActors ( class<actor> BaseClass, out actor Actor, optional float Radius, optional vector Loc );
native(312) final iterator function VisibleCollidingActors ( class<actor> BaseClass, out actor Actor, optional float Radius, optional vector Loc, optional bool bIgnoreHidden );

//=============================================================================
// Color operators
native(549) static final operator(20)  color -     ( color A, color B );
native(550) static final operator(16) color *     ( float A, color B );
native(551) static final operator(20) color +     ( color A, color B );
native(552) static final operator(16) color *     ( color A, float B );

//=============================================================================
// Scripted Actor functions.

// draw on canvas before flash and fog are applied (used for drawing weapons)
event RenderOverlays( canvas Canvas );

//
// Called immediately before gameplay begins.
//
event PreBeginPlay()
{
	if (IsSoftwareRendering() && !bRelevantInSoftwareRenderer)
		Destroy();

	// Handle autodestruction if desired.
	if( !bGameRelevant && (Level.NetMode != NM_Client) && !Level.Game.IsRelevant(Self) )
		Destroy();
}

//
// Broadcast a message to all players.
//
event BroadcastMessage( coerce string Msg, optional bool bBeep, optional name Type )
{
	local Pawn P;

	if (Type == '')
		Type = 'Event';

	if ( Level.Game.AllowsBroadcast(self, Len(Msg)) )
		for( P=Level.PawnList; P!=None; P=P.nextPawn )
			if( P.bIsPlayer || P.IsA('MessagingSpectator') )
			{
				if ( (Level.Game != None) && (Level.Game.MessageMutator != None) )
				{
					if ( Level.Game.MessageMutator.MutatorBroadcastMessage(Self, P, Msg, bBeep, Type) )
						P.ClientMessage( Msg, Type, bBeep );
				} else
					P.ClientMessage( Msg, Type, bBeep );
			}
}

//
// Broadcast a localized message to all players.
// Most message deal with 0 to 2 related PRIs.
// The LocalMessage class defines how the PRI's and optional actor are used.
//
event BroadcastLocalizedMessage( class<LocalMessage> Message, optional int Switch, optional PlayerReplicationInfo RelatedPRI_1, optional PlayerReplicationInfo RelatedPRI_2, optional Object OptionalObject )
{
	local Pawn P;

	for ( P=Level.PawnList; P != None; P=P.nextPawn )
		if ( P.bIsPlayer || P.IsA('MessagingSpectator') )
		{
			if ( (Level.Game != None) && (Level.Game.MessageMutator != None) )
			{
				if ( Level.Game.MessageMutator.MutatorBroadcastLocalizedMessage(Self, P, Message, Switch, RelatedPRI_1, RelatedPRI_2, OptionalObject) )
					P.ReceiveLocalizedMessage( Message, Switch, RelatedPRI_1, RelatedPRI_2, OptionalObject );
			} else
				P.ReceiveLocalizedMessage( Message, Switch, RelatedPRI_1, RelatedPRI_2, OptionalObject );
		}
}

//
// Called immediately after gameplay begins.
//
event PostBeginPlay();

//
// Called after PostBeginPlay.
//
simulated event SetInitialState()
{
	bScriptInitialized = true;
	if( InitialState!='' )
		GotoState( InitialState );
	else
		GotoState( 'Auto' );
}

// called after PostBeginPlay on net client
simulated event PostNetBeginPlay();

//
// Hurt actors within the radius.
//
final function HurtRadius( float DamageAmount, float DamageRadius, name DamageName, float Momentum, vector HitLocation )
{
	local actor Victims;
	local float damageScale, dist;
	local vector dir;
	
	if( bHurtEntry )
		return;

	bHurtEntry = true;
	foreach VisibleCollidingActors( class 'Actor', Victims, DamageRadius, HitLocation )
	{
		if( Victims != self )
		{
			if( HurtRadiusCallBack( Victims ) )
			{
				dir = Victims.Location - HitLocation;
				dist = FMax(1,VSize(dir));
				dir = dir/dist; 
				damageScale = 1 - FMax(0,(dist - Victims.CollisionRadius)/DamageRadius);
				Victims.TakeDamage
				(
					damageScale * DamageAmount,
					Instigator, 
					Victims.Location - 0.5 * (Victims.CollisionHeight + Victims.CollisionRadius) * dir,
					(damageScale * Momentum * dir),
					DamageName
				);
			}
		} 
	}
	bHurtEntry = false;
}

//
// Called from HurtRadius for each actor hurt.
//
function bool HurtRadiusCallBack( Actor a )
{
	//Return true if you want HurtRadius to hurt this actor
	return true;
}

//
// Called when carried onto a new level, before AcceptInventory.
//
event TravelPreAccept();

//
// Called when carried into a new level, after AcceptInventory.
//
event TravelPostAccept();

//
// Called when a scripted texture needs rendering
//
event RenderTexture(ScriptedTexture Tex);

//
// Called by PlayerPawn when this actor becomes its ViewTarget.
//
function BecomeViewTarget();

//
// Returns the string representation of the name of an object without the package
// prefixes.
//
function String GetItemName( string FullName )
{
	local int pos;

	pos = InStr(FullName, ".");
	While ( pos != -1 )
	{
		FullName = Right(FullName, Len(FullName) - pos - 1);
		pos = InStr(FullName, ".");
	}

	return FullName;
}

/* 
Trigger an event
*/
function TriggerEvent( Name EventName, Actor Other, Pawn EventInstigator )
{
	local Actor A;

	if ( (EventName == '') || (EventName == 'None') )
		return;

	ForEach AllActors( class 'Actor', A, EventName )
		A.Trigger(Other, EventInstigator);
}

/*
Untrigger an event
*/
function UntriggerEvent( Name EventName, Actor Other, Pawn EventInstigator )
{
	local Actor A;

	if ( (EventName == '') || (EventName == 'None') )
		return;

	ForEach AllActors( class 'Actor', A, EventName )
		A.Untrigger(Other, EventInstigator);
}

/*
Disable actor using a trigger
*/
function TriggerDisable()
{
	// This function should be overwritten in specific actors. 
	return;
}

/* StartInterpolation()
when this function is called, the actor will start moving along an interpolation path
beginning at Dest
*/	
simulated function StartInterpolation( InterpolationPoint Dest, float Rate, bool bJumpToStart )
{
	local InterpolationManager IM;

	GotoState('');
	SetCollision(True,false,false);
	bCollideWorld = False;
	bInterpolating = true;
	SetPhysics(PHYS_None);
	IM = Spawn(class'InterpolationManager', self);
	IM.Init(Dest,Rate, bJumpToStart);
}


//
// Returns the human readable string representation of an object.
//

function String GetHumanName()
{
	return GetItemName(string(class));
}

// Set the display properties of an actor.  By setting them through this function, it allows
// the actor to modify other components (such as a Pawn's weapon) or to adjust the result
// based on other factors (such as a Pawn's other inventory wanting to affect the result)
function SetDisplayProperties(ERenderStyle NewStyle, texture NewTexture, bool bLighting, bool bEnviroMap )
{
	Style = NewStyle;
	texture = NewTexture;
	bUnlit = bLighting;
	bMeshEnviromap = bEnviromap;
}

function SetDefaultDisplayProperties()
{
	Style = Default.Style;
	texture = Default.Texture;
	bUnlit = Default.bUnlit;
	bMeshEnviromap = Default.bMeshEnviromap;
}


native(322) final function Texture CreateTextureFromScreenShot (Viewport vport);
native(321) final function Texture CreateTextureFromBMP (string name, string filename);

// Sto: The loadObject call doesn't work yet,
// but have left in case I ever get working as would be useful
// Wrote the specific load/save gameInfo fns instead for now
native(323) final function bool SaveObjectAsFile (string dir, Object object);
native(324) final function bool LoadObjectAsFile (string dir, Object object);

native(325) final function bool SaveGameSaveInfo (string dir, Object object);
native(326) final function bool LoadGameSaveInfo (string dir, Object object);

native(327) final function bool IsOSVer2kOrXP	 ();
native(329) final function bool IsSoftwareRendering();


// When harry turns on/off lumos then these functions will be called
function OnLumosOn()
{
	// left blank on purpose
}
function OnLumosOff()	
{ 
	// left blank on purpose
}


// --- GameState
// OnResolveGameState is called when the actor should resolve itself 
// it may be IN or OUT of the current gamestate. (look at bInCurrentGameState to find out)
event OnResolveGameState();


// --- Math Utility Functions
function vector ComputeTrajectoryByTime( vector vPosStart, vector vPosEnd, float fTimeEnd, optional float AlternateZAccel )
{	
	local vector Velocity;
	local float  ZAccel;

	if( AlternateZAccel != 0 )
		ZAccel = AlternateZAccel;
	else
		ZAccel = Region.Zone.ZoneGravity.Z;
	
	// Error checking
	fTimeEnd = FMax( 0.0001f, fTimeEnd );
	
	// *** Compute Vx using the X position formula
	//
	// Xe = Xs + (Vx * Te)
	// 
	// --- Rework the formula to solve for Vx.
	// Vx = ( Xe - Xs ) / Te
	
	Velocity = ( vPosEnd - vPosStart ) / fTimeEnd;
	
	// *** Compute Vy using the Y position formula
	//
	// Ye = Ys + (Vy * Te) + (0.5f * a * Te^2 )
	//
	// --- Rework the formula to solve for Vy.
	// Vy = ( (Ye - Ys) - (0.5f * g * Te^2) ) / Te
	
	Velocity.z = ((vPosEnd.z - vPosStart.z) - (0.5f * ZAccel * (fTimeEnd*fTimeEnd)) ) / fTimeEnd;
	
	// return the inital velocity required to hit our target at t = fTimeEnd
	return Velocity;
}

function cm( string s )
{
	level.playerHarryActor.ClientMessage( s );
}

defaultproperties
{
	bDifficulty0=True
	bDifficulty1=True
	bDifficulty2=True
	bDifficulty3=True
	bSinglePlayer=True
	bNet=True
	bNetSpecial=True
	bRelevantInSoftwareRenderer=True
	OddsOfAppearing=+00001.000000
	DrawType=DT_Sprite
	Texture=S_Actor
	DrawScale=+00001.000000
	bMeshCurvy=false
	bTwoSidedBias=true
	SoundRadius=32
	SoundVolume=128
	SoundPitch=64
	TransientSoundVolume=+00001.000000
	TransientSoundPitch=64
	CollisionRadius=+00022.000000
	CollisionHeight=+00022.000000
	bAlignBottom=true
	bJustTeleported=True
	Mass=+00100.000000
	Role=ROLE_Authority
	RemoteRole=ROLE_DumbProxy
	NetPriority=+00001.000000
	ScaleGlow=1.0
	SpecularGlow=1.0
	SpecularWidth=170
	Fatness=128
	Wideness=128
	Opacity=1.0
	Style=STY_Normal
	bMovable=True
	bHighDetail=False
	VolumeFog=0
	InitialState=None
	NetUpdateFrequency=100
	LODBias=1.0
	SpriteProjForward=32.0
	eVulnerableToSpell=SPELL_None
	bCanMoveInSpecialPause=false
	bInSpecialPause=false;
	SizeModifier=1.0
	CentreOffset=(x=0,y=0,z=0)

	// --- GameState
	bInCurrentGameState=true
	bPersistent=false

	bGestureFaceHorizOnly=true

	bDoNotBlockProjectiles=false
	bBlockCamera=false
}
